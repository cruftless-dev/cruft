
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread::JoinHandle;
use std::time::Duration;

pub const MAX_HANDLES: usize = 4096;
pub const MAX_STREAM_HANDLES: usize = 2048;

enum Handle {
    Listener(ListenerHandle),
    Stream(Arc<TcpStream>),
    AsyncListener(AsyncListener),
    Udp(std::net::UdpSocket),
}

struct ListenerHandle {
    socket: Arc<TcpListener>,
    accept_timeout: Option<Duration>,
}

struct AsyncListener {

    rx: Arc<Mutex<Receiver<AsyncEvent>>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    local_addr: String,
}

#[derive(Debug, Clone)]
pub enum AsyncEvent {

    Connection { stream_id: u64, peer: String },

    Readable { stream_id: u64 },

    Closed,

    Error(String),
}

struct Registry {
    next_id: u64,
    handles: HashMap<u64, Handle>,
}

fn registry() -> &'static Mutex<Registry> {
    static REG: OnceLock<Mutex<Registry>> = OnceLock::new();
    REG.get_or_init(|| {
        Mutex::new(Registry {
            next_id: 1,
            handles: HashMap::new(),
        })
    })
}

fn lock_registry() -> MutexGuard<'static, Registry> {
    registry()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn lock_rx(rx: &Mutex<Receiver<AsyncEvent>>) -> MutexGuard<'_, Receiver<AsyncEvent>> {
    rx.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn put(h: Handle) -> Result<u64, SocketError> {
    let mut r = lock_registry();
    if r.handles.len() >= MAX_HANDLES {
        return Err(SocketError::ResourceLimit("handle registry full".into()));
    }
    if matches!(h, Handle::Stream(_)) {
        let streams = r
            .handles
            .values()
            .filter(|h| matches!(h, Handle::Stream(_)))
            .count();
        if streams >= MAX_STREAM_HANDLES {
            return Err(SocketError::ResourceLimit(
                "stream handle cap reached".into(),
            ));
        }
    }
    let id = r.next_id;
    r.next_id += 1;
    r.handles.insert(id, h);
    Ok(id)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketError {
    Bind(String),
    Connect(String),
    Accept(String),
    Read(String),
    Write(String),
    ResourceLimit(String),
    UnsupportedPlatform,
    NotFound,
    WrongKind,
}

impl std::fmt::Display for SocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketError::Bind(s) => write!(f, "sockets: bind failed: {}", s),
            SocketError::Connect(s) => write!(f, "sockets: connect failed: {}", s),
            SocketError::Accept(s) => write!(f, "sockets: accept failed: {}", s),
            SocketError::Read(s) => write!(f, "sockets: read failed: {}", s),
            SocketError::Write(s) => write!(f, "sockets: write failed: {}", s),
            SocketError::ResourceLimit(s) => write!(f, "sockets: resource limit: {}", s),
            SocketError::UnsupportedPlatform => write!(f, "sockets: unsupported platform"),
            SocketError::NotFound => write!(f, "sockets: handle not in registry"),
            SocketError::WrongKind => write!(f, "sockets: handle is wrong kind"),
        }
    }
}

pub fn listener_bind(addr: &str) -> Result<(u64, String), SocketError> {
    let listener = TcpListener::bind(addr).map_err(|e| SocketError::Bind(e.to_string()))?;
    let local = listener
        .local_addr()
        .map_err(|e| SocketError::Bind(e.to_string()))?;
    let id = put(Handle::Listener(ListenerHandle {
        socket: Arc::new(listener),
        accept_timeout: None,
    }))?;
    Ok((id, local.to_string()))
}

pub fn listener_accept(id: u64) -> Result<(u64, String), SocketError> {
    let (listener, timeout) = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Listener(l) => (l.socket.clone(), l.accept_timeout),
            _ => return Err(SocketError::WrongKind),
        }
    };
    let (stream, peer) = accept_with_timeout(&listener, timeout)?;
    let stream_id = put(Handle::Stream(Arc::new(stream)))?;
    Ok((stream_id, peer.to_string()))
}

fn accept_with_timeout(
    listener: &TcpListener,
    timeout: Option<Duration>,
) -> Result<(TcpStream, SocketAddr), SocketError> {
    let Some(timeout) = timeout else {
        return listener
            .accept()
            .map_err(|e| SocketError::Accept(e.to_string()));
    };
    listener
        .set_nonblocking(true)
        .map_err(|e| SocketError::Accept(e.to_string()))?;
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok(pair) => {
                let _ = listener.set_nonblocking(false);
                return Ok(pair);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    let _ = listener.set_nonblocking(false);
                    return Err(SocketError::Accept("timed out".into()));
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => {
                let _ = listener.set_nonblocking(false);
                return Err(SocketError::Accept(e.to_string()));
            }
        }
    }
}

pub fn listener_set_accept_timeout(id: u64, ms: u64) -> Result<(), SocketError> {
    let mut r = lock_registry();
    match r.handles.get_mut(&id).ok_or(SocketError::NotFound)? {
        Handle::Listener(l) => {
            l.accept_timeout = Some(Duration::from_millis(ms));
            Ok(())
        }
        _ => Err(SocketError::WrongKind),
    }
}

pub fn stream_connect(addr: &str) -> Result<u64, SocketError> {
    let stream = TcpStream::connect(addr).map_err(|e| SocketError::Connect(e.to_string()))?;
    put(Handle::Stream(Arc::new(stream)))
}

pub fn stream_connect_timeout(addr: &str, timeout_ms: u64) -> Result<u64, SocketError> {
    use std::net::ToSocketAddrs;

    let sa: SocketAddr = addr
        .to_socket_addrs()
        .map_err(|e| SocketError::Connect(e.to_string()))?
        .next()
        .ok_or_else(|| SocketError::Connect("no addresses resolved".into()))?;
    let stream = TcpStream::connect_timeout(&sa, Duration::from_millis(timeout_ms))
        .map_err(|e| SocketError::Connect(e.to_string()))?;
    put(Handle::Stream(Arc::new(stream)))
}

pub fn stream_read(id: u64, max: usize) -> Result<Vec<u8>, SocketError> {
    let stream = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Stream(s) => s.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    let mut buf = vec![0u8; max];
    let mut stream_ref = &*stream;
    let n = stream_ref
        .read(&mut buf)
        .map_err(|e| SocketError::Read(e.to_string()))?;
    buf.truncate(n);
    Ok(buf)
}

pub fn stream_write(id: u64, data: &[u8]) -> Result<usize, SocketError> {
    let stream = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Stream(s) => s.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    let mut stream_ref = &*stream;
    let n = stream_ref
        .write(data)
        .map_err(|e| SocketError::Write(e.to_string()))?;
    Ok(n)
}

pub fn stream_write_all(id: u64, data: &[u8]) -> Result<(), SocketError> {
    let stream = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Stream(s) => s.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    let mut stream_ref = &*stream;
    stream_ref
        .write_all(data)
        .map_err(|e| SocketError::Write(e.to_string()))?;
    Ok(())
}

pub fn stream_peer_addr(id: u64) -> Result<String, SocketError> {
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::Stream(s) => s
            .peer_addr()
            .map(|a| a.to_string())
            .map_err(|e| SocketError::Read(e.to_string())),
        _ => Err(SocketError::WrongKind),
    }
}

pub fn stream_local_addr(id: u64) -> Result<String, SocketError> {
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::Stream(s) => s
            .local_addr()
            .map(|a| a.to_string())
            .map_err(|e| SocketError::Read(e.to_string())),
        _ => Err(SocketError::WrongKind),
    }
}

pub fn stream_set_nonblocking(id: u64, on: bool) -> Result<(), SocketError> {
    let stream = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Stream(s) => s.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    stream
        .set_nonblocking(on)
        .map_err(|e| SocketError::Read(e.to_string()))
}

pub fn stream_try_read(id: u64, max: usize) -> Result<Option<Vec<u8>>, SocketError> {
    let stream = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Stream(s) => s.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    let mut buf = vec![0u8; max];
    let mut stream_ref = &*stream;
    match stream_ref.read(&mut buf) {
        Ok(n) => {
            buf.truncate(n);
            Ok(Some(buf))
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(SocketError::Read(e.to_string())),
    }
}

#[cfg(unix)]
pub fn stream_raw_fd(id: u64) -> Result<std::os::unix::io::RawFd, SocketError> {
    use std::os::unix::io::AsRawFd;
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::Stream(s) => Ok(s.as_raw_fd()),
        _ => Err(SocketError::WrongKind),
    }
}

#[cfg(not(unix))]
pub fn stream_raw_fd(_id: u64) -> Result<i32, SocketError> {
    Err(SocketError::UnsupportedPlatform)
}

#[cfg(unix)]
pub fn listener_raw_fd(id: u64) -> Result<std::os::unix::io::RawFd, SocketError> {
    use std::os::unix::io::AsRawFd;
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::Listener(l) => Ok(l.socket.as_raw_fd()),
        _ => Err(SocketError::WrongKind),
    }
}

#[cfg(not(unix))]
pub fn listener_raw_fd(_id: u64) -> Result<i32, SocketError> {
    Err(SocketError::UnsupportedPlatform)
}

pub fn listener_set_nonblocking(id: u64, on: bool) -> Result<(), SocketError> {
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::Listener(l) => l
            .socket
            .set_nonblocking(on)
            .map_err(|e| SocketError::Bind(e.to_string())),
        _ => Err(SocketError::WrongKind),
    }
}

pub fn listener_try_accept(id: u64) -> Result<Option<(u64, String)>, SocketError> {
    let listener = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Listener(l) => l.socket.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    match listener.accept() {
        Ok((stream, peer)) => {

            let _ = stream.set_nonblocking(false);
            let stream_id = put(Handle::Stream(Arc::new(stream)))?;
            Ok(Some((stream_id, peer.to_string())))
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(SocketError::Accept(e.to_string())),
    }
}

pub fn handle_close(id: u64) -> Result<(), SocketError> {
    let mut r = lock_registry();
    r.handles.remove(&id).ok_or(SocketError::NotFound)?;
    Ok(())
}

pub fn udp_bind(addr: &str) -> Result<(u64, String), SocketError> {
    use std::net::ToSocketAddrs;
    let resolved: std::net::SocketAddr = addr
        .to_socket_addrs()
        .map_err(|e| SocketError::Bind(e.to_string()))?
        .next()
        .ok_or_else(|| SocketError::Bind("no address".into()))?;
    let sock = std::net::UdpSocket::bind(resolved).map_err(|e| SocketError::Bind(e.to_string()))?;
    let local = sock
        .local_addr()
        .map_err(|e| SocketError::Bind(e.to_string()))?
        .to_string();
    sock.set_nonblocking(true)
        .map_err(|e| SocketError::Bind(e.to_string()))?;
    Ok((put(Handle::Udp(sock))?, local))
}

pub fn udp_send_to(id: u64, data: &[u8], addr: &str) -> Result<usize, SocketError> {
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::Udp(s) => s
            .send_to(data, addr)
            .map_err(|e| SocketError::Write(e.to_string())),
        _ => Err(SocketError::WrongKind),
    }
}

pub fn udp_try_recv(id: u64, max: usize) -> Result<Option<(Vec<u8>, String)>, SocketError> {
    let sock = {
        let r = lock_registry();
        match r.handles.get(&id).ok_or(SocketError::NotFound)? {
            Handle::Udp(s) => s
                .try_clone()
                .map_err(|e| SocketError::Read(e.to_string()))?,
            _ => return Err(SocketError::WrongKind),
        }
    };
    let mut buf = vec![0u8; max];
    match sock.recv_from(&mut buf) {
        Ok((n, from)) => {
            buf.truncate(n);
            Ok(Some((buf, from.to_string())))
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(e) => Err(SocketError::Read(e.to_string())),
    }
}

pub fn udp_local_addr(id: u64) -> Result<String, SocketError> {
    let r = lock_registry();
    match r.handles.get(&id).ok_or(SocketError::NotFound)? {
        Handle::Udp(s) => s
            .local_addr()
            .map(|a| a.to_string())
            .map_err(|e| SocketError::Read(e.to_string())),
        _ => Err(SocketError::WrongKind),
    }
}

pub fn stream_shutdown_write(id: u64) -> Result<(), SocketError> {
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::Stream(s) => s
            .shutdown(std::net::Shutdown::Write)
            .map_err(|e| SocketError::Read(e.to_string())),
        _ => Err(SocketError::WrongKind),
    }
}

pub fn handle_kind(id: u64) -> Result<&'static str, SocketError> {
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    Ok(match h {
        Handle::Listener(_) => "listener",
        Handle::Stream(_) => "stream",
        Handle::AsyncListener(_) => "async-listener",
        Handle::Udp(_) => "udp",
    })
}

pub fn listener_bind_async(addr: &str) -> Result<(u64, String), SocketError> {
    let listener = TcpListener::bind(addr).map_err(|e| SocketError::Bind(e.to_string()))?;
    let local = listener
        .local_addr()
        .map_err(|e| SocketError::Bind(e.to_string()))?;
    let local_str = local.to_string();
    listener
        .set_nonblocking(true)
        .map_err(|e| SocketError::Bind(e.to_string()))?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let (tx, rx): (Sender<AsyncEvent>, Receiver<AsyncEvent>) = mpsc::channel();
    let thread_shutdown = shutdown.clone();
    let thread = std::thread::spawn(move || {
        accept_loop(listener, tx, thread_shutdown);
    });
    let async_listener = AsyncListener {
        rx: Arc::new(Mutex::new(rx)),
        shutdown,
        thread: Some(thread),
        local_addr: local_str.clone(),
    };
    let id = put(Handle::AsyncListener(async_listener))?;
    Ok((id, local_str))
}

fn accept_loop(listener: TcpListener, tx: Sender<AsyncEvent>, shutdown: Arc<AtomicBool>) {

    while !shutdown.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, peer)) => {

                if let Err(e) = stream.set_nonblocking(false) {
                    let _ = tx.send(AsyncEvent::Error(format!("set_blocking: {}", e)));
                    continue;
                }
                match put(Handle::Stream(Arc::new(stream))) {
                    Ok(stream_id) => {
                        if tx
                            .send(AsyncEvent::Connection {
                                stream_id,
                                peer: peer.to_string(),
                            })
                            .is_err()
                        {

                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AsyncEvent::Error(format!("accept resource limit: {e}")));
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => {
                let _ = tx.send(AsyncEvent::Error(format!("accept: {}", e)));
                break;
            }
        }
    }
    let _ = tx.send(AsyncEvent::Closed);
}

pub fn listener_poll(id: u64, max_wait_ms: u64) -> Result<Option<AsyncEvent>, SocketError> {

    let rx = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::AsyncListener(al) => al.rx.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    let guard = lock_rx(&rx);
    match guard.recv_timeout(Duration::from_millis(max_wait_ms)) {
        Ok(ev) => Ok(Some(ev)),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => Ok(Some(AsyncEvent::Closed)),
    }
}

pub fn listener_stop_async(id: u64) -> Result<(), SocketError> {

    let mut r = lock_registry();
    let h = r.handles.remove(&id).ok_or(SocketError::NotFound)?;
    drop(r);
    match h {
        Handle::AsyncListener(mut al) => {
            al.shutdown.store(true, Ordering::Release);
            if let Some(t) = al.thread.take() {
                let _ = t.join();
            }
            Ok(())
        }
        _ => Err(SocketError::WrongKind),
    }
}

pub fn async_listener_addr(id: u64) -> Result<String, SocketError> {
    let r = lock_registry();
    let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
    match h {
        Handle::AsyncListener(al) => Ok(al.local_addr.clone()),
        _ => Err(SocketError::WrongKind),
    }
}

type ReadyEntry = (Arc<TcpStream>, std::time::Instant);

struct ReadinessReactor {
    rx: Arc<Mutex<Receiver<AsyncEvent>>>,
    registered: Arc<Mutex<HashMap<u64, ReadyEntry>>>,
}

fn readiness_reactor() -> &'static ReadinessReactor {
    static REACTOR: OnceLock<ReadinessReactor> = OnceLock::new();
    REACTOR.get_or_init(|| {
        let (tx, rx): (Sender<AsyncEvent>, Receiver<AsyncEvent>) = mpsc::channel();
        let registered: Arc<Mutex<HashMap<u64, ReadyEntry>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let t_registered = registered.clone();

        std::thread::spawn(move || {
            readiness_loop(t_registered, tx);
        });
        ReadinessReactor {
            rx: Arc::new(Mutex::new(rx)),
            registered,
        }
    })
}

fn readiness_loop(
    registered: Arc<Mutex<HashMap<u64, ReadyEntry>>>,
    tx: Sender<AsyncEvent>,
) {
    let mut probe = [0u8; 1];
    loop {

        let snapshot: Vec<(u64, Arc<TcpStream>, std::time::Instant)> = {
            let map = registered.lock().unwrap_or_else(|p| p.into_inner());
            map.iter().map(|(id, (s, d))| (*id, s.clone(), *d)).collect()
        };
        if snapshot.is_empty() {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        }
        let now = std::time::Instant::now();
        let mut ready: Vec<u64> = Vec::new();
        for (id, stream, deadline) in &snapshot {

            if now >= *deadline {
                ready.push(*id);
                continue;
            }

            match stream.peek(&mut probe) {

                Ok(_) => ready.push(*id),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}

                Err(_) => ready.push(*id),
            }
        }
        if !ready.is_empty() {
            let mut map = registered.lock().unwrap_or_else(|p| p.into_inner());
            for id in ready {

                if map.remove(&id).is_some() && tx.send(AsyncEvent::Readable { stream_id: id }).is_err() {
                    return;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub fn stream_register_readable(id: u64, keep_alive_timeout_ms: u64) -> Result<(), SocketError> {
    let stream = {
        let r = lock_registry();
        let h = r.handles.get(&id).ok_or(SocketError::NotFound)?;
        match h {
            Handle::Stream(s) => s.clone(),
            _ => return Err(SocketError::WrongKind),
        }
    };
    stream
        .set_nonblocking(true)
        .map_err(|e| SocketError::Read(e.to_string()))?;
    let deadline = std::time::Instant::now() + Duration::from_millis(keep_alive_timeout_ms);
    let reactor = readiness_reactor();
    let mut map = reactor
        .registered
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.insert(id, (stream, deadline));
    Ok(())
}

pub fn stream_deregister_readable(id: u64) -> Result<(), SocketError> {
    let reactor = readiness_reactor();
    let mut map = reactor
        .registered
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.remove(&id);
    Ok(())
}

pub fn readiness_poll(max_wait_ms: u64) -> Result<Option<AsyncEvent>, SocketError> {
    let reactor = readiness_reactor();
    let guard = lock_rx(&reactor.rx);

    if max_wait_ms == 0 {
        return match guard.try_recv() {
            Ok(ev) => Ok(Some(ev)),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Ok(None),
        };
    }
    match guard.recv_timeout(Duration::from_millis(max_wait_ms)) {
        Ok(ev) => Ok(Some(ev)),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => Ok(None),
    }
}

pub fn readiness_pending() -> bool {
    let reactor = readiness_reactor();
    let map = reactor
        .registered
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    !map.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    static REGISTRY_SERIAL: Mutex<()> = Mutex::new(());

    fn tcp_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).unwrap();
        let (server, _) = listener.accept().unwrap();
        (client, server)
    }

    #[test]
    fn stream_registry_cap_rejects_unbounded_growth() {
        let _serial = REGISTRY_SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let (client, server) = tcp_pair();
        let shared = Arc::new(client);
        let mut ids = Vec::new();
        for _ in 0..MAX_STREAM_HANDLES {
            ids.push(put(Handle::Stream(shared.clone())).unwrap());
        }
        let err = put(Handle::Stream(Arc::new(server))).unwrap_err();
        assert!(matches!(err, SocketError::ResourceLimit(_)));
        for id in ids {
            let _ = handle_close(id);
        }
    }

    #[test]
    fn readiness_reactor_events_bytes_and_auto_deregisters() {
        let _serial = REGISTRY_SERIAL.lock().unwrap_or_else(|p| p.into_inner());

        let (client, server) = tcp_pair();
        let server_id = put(Handle::Stream(Arc::new(server))).unwrap();

        stream_register_readable(server_id, 5000).unwrap();
        assert!(readiness_pending(), "stream should be registered");

        assert!(
            readiness_poll(30).unwrap().is_none(),
            "no Readable before the peer writes"
        );

        (&client).write_all(b"PING").unwrap();

        let mut got = None;
        for _ in 0..50 {
            if let Some(ev) = readiness_poll(20).unwrap() {
                got = Some(ev);
                break;
            }
        }
        match got {
            Some(AsyncEvent::Readable { stream_id }) => assert_eq!(stream_id, server_id),
            other => panic!("expected Readable, got {other:?}"),
        }

        assert!(!readiness_pending(), "stream auto-deregistered after firing");

        let chunk = stream_try_read(server_id, 8).unwrap().unwrap();
        assert_eq!(&chunk, b"PING");
        let _ = handle_close(server_id);

        let (client2, server2) = tcp_pair();
        let id2 = put(Handle::Stream(Arc::new(server2))).unwrap();
        stream_register_readable(id2, 5000).unwrap();
        stream_deregister_readable(id2).unwrap();
        assert!(!readiness_pending());
        (&client2).write_all(b"X").unwrap();
        assert!(
            readiness_poll(30).unwrap().is_none(),
            "deregistered stream emits no event"
        );
        let _ = handle_close(id2);

        let (_client3, server3) = tcp_pair();
        let id3 = put(Handle::Stream(Arc::new(server3))).unwrap();
        stream_register_readable(id3, 40).unwrap();
        let mut timed_out = false;
        for _ in 0..50 {
            if let Some(AsyncEvent::Readable { stream_id }) = readiness_poll(20).unwrap() {
                assert_eq!(stream_id, id3);
                timed_out = true;
                break;
            }
        }
        assert!(timed_out, "idle keep-alive timeout should fire Readable");
        assert!(!readiness_pending(), "timed-out stream auto-deregistered");
        let _ = handle_close(id3);
    }

    #[test]
    fn poisoned_registry_lock_is_recovered() {
        let _ = std::panic::catch_unwind(|| {
            let _guard = registry().lock().unwrap();
            panic!("poison sockets registry for recovery witness");
        });

        let (id, addr) = listener_bind("127.0.0.1:0").unwrap();
        assert!(addr.starts_with("127.0.0.1:"));
        handle_close(id).unwrap();
    }
}
