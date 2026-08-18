
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

enum Cmd {
    Register {
        id: u64,
        deadline: Instant,
        interrupt: Arc<AtomicBool>,
    },
    Unregister {
        id: u64,
    },
    Shutdown,
}

pub struct Watchdog {
    tx: mpsc::Sender<Cmd>,
    handle: Option<JoinHandle<()>>,
}

impl Watchdog {

    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let handle = std::thread::Builder::new()
            .name("cruft-availability-watchdog".into())
            .spawn(move || watchdog_loop(rx))
            .expect("spawn watchdog thread");
        Self {
            tx,
            handle: Some(handle),
        }
    }

    pub fn register(&self, id: u64, deadline: Instant, interrupt: Arc<AtomicBool>) {
        let _ = self.tx.send(Cmd::Register {
            id,
            deadline,
            interrupt,
        });
    }

    pub fn unregister(&self, id: u64) {
        let _ = self.tx.send(Cmd::Unregister { id });
    }

    pub fn shutdown(mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {

        let _ = self.tx.send(Cmd::Shutdown);
    }
}

fn watchdog_loop(rx: mpsc::Receiver<Cmd>) {

    let mut heap: BinaryHeap<Reverse<(Instant, u64)>> = BinaryHeap::new();
    let mut live: HashMap<u64, (Instant, Arc<AtomicBool>)> = HashMap::new();

    loop {

        let now = Instant::now();
        while let Some(Reverse((deadline, id))) = heap.peek().copied() {
            if deadline > now {
                break;
            }
            heap.pop();

            if let Some((live_deadline, interrupt)) = live.get(&id) {
                if *live_deadline == deadline {
                    interrupt.store(true, Ordering::Release);
                    live.remove(&id);
                }
            }
        }

        let cmd = match heap.peek().copied() {
            Some(Reverse((next_deadline, _))) => {
                let dur = next_deadline.saturating_duration_since(Instant::now());
                match rx.recv_timeout(dur) {
                    Ok(c) => c,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match rx.recv() {
                Ok(c) => c,
                Err(_) => return,
            },
        };

        match cmd {
            Cmd::Register {
                id,
                deadline,
                interrupt,
            } => {
                live.insert(id, (deadline, interrupt));
                heap.push(Reverse((deadline, id)));
            }
            Cmd::Unregister { id } => {
                live.remove(&id);

            }
            Cmd::Shutdown => return,
        }
    }
}

#[cfg(test)]
mod watchdog_tests {
    use super::*;
    use std::time::Duration;

    fn flag() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }
    fn is_set(f: &Arc<AtomicBool>) -> bool {
        f.load(Ordering::Acquire)
    }

    #[test]
    fn deadline_fires_within_grace() {
        let wd = Watchdog::spawn();
        let f = flag();
        wd.register(
            1,
            Instant::now() + Duration::from_millis(40),
            Arc::clone(&f),
        );

        std::thread::sleep(Duration::from_millis(10));
        assert!(!is_set(&f), "not fired before its deadline");

        std::thread::sleep(Duration::from_millis(120));
        assert!(is_set(&f), "fired after the deadline (within grace)");
        wd.shutdown();
    }

    #[test]
    fn multi_compartment_min_heap_fires_in_order_peer_untouched() {
        let wd = Watchdog::spawn();
        let early = flag();
        let late = flag();
        wd.register(
            1,
            Instant::now() + Duration::from_millis(40),
            Arc::clone(&early),
        );
        wd.register(
            2,
            Instant::now() + Duration::from_millis(400),
            Arc::clone(&late),
        );

        std::thread::sleep(Duration::from_millis(140));
        assert!(is_set(&early), "early deadline fired");
        assert!(!is_set(&late), "later-deadline peer untouched");
        wd.shutdown();
    }

    #[test]
    fn unregister_before_expiry_does_not_fire() {
        let wd = Watchdog::spawn();
        let f = flag();
        wd.register(
            1,
            Instant::now() + Duration::from_millis(80),
            Arc::clone(&f),
        );
        wd.unregister(1);
        std::thread::sleep(Duration::from_millis(160));
        assert!(!is_set(&f), "unregistered deadline must not fire");
        wd.shutdown();
    }

    #[test]
    fn idle_watchdog_blocks_then_serves_a_late_registration() {

        let wd = Watchdog::spawn();
        std::thread::sleep(Duration::from_millis(20));
        let f = flag();
        wd.register(
            7,
            Instant::now() + Duration::from_millis(30),
            Arc::clone(&f),
        );
        std::thread::sleep(Duration::from_millis(120));
        assert!(is_set(&f), "idle watchdog served a late registration");
        wd.shutdown();
    }
}
