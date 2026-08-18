
use std::io::{Read, Write};
use std::net::{TcpStream, UdpSocket};
use std::time::Duration;

pub const T_A: u16 = 1;
pub const T_NS: u16 = 2;
pub const T_CNAME: u16 = 5;
pub const T_SOA: u16 = 6;
pub const T_PTR: u16 = 12;
pub const T_MX: u16 = 15;
pub const T_TXT: u16 = 16;
pub const T_AAAA: u16 = 28;
pub const T_SRV: u16 = 33;

pub fn system_resolver() -> String {
    std::fs::read_to_string("/etc/resolv.conf")
        .ok()
        .and_then(|c| {
            c.lines()
                .find_map(|l| l.strip_prefix("nameserver").map(|s| s.trim().to_string()))
        })
        .unwrap_or_else(|| "127.0.0.53".to_string())
}

fn encode_name(name: &str, out: &mut Vec<u8>) {
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            continue;
        }
        out.push(label.len() as u8);
        out.extend_from_slice(label.as_bytes());
    }
    out.push(0);
}

fn build_query(name: &str, qtype: u16) -> Vec<u8> {
    let mut p = Vec::with_capacity(64);
    p.extend_from_slice(&[0x13, 0x37]);
    p.extend_from_slice(&[0x01, 0x00]);
    p.extend_from_slice(&[0x00, 0x01]);
    p.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    encode_name(name, &mut p);
    p.extend_from_slice(&qtype.to_be_bytes());
    p.extend_from_slice(&[0x00, 0x01]);
    p
}

fn read_name(msg: &[u8], mut pos: usize) -> (String, usize) {
    let mut labels = Vec::new();
    let mut jumped = false;
    let mut next = pos;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 128 || pos >= msg.len() {
            break;
        }
        let len = msg[pos] as usize;
        if len & 0xC0 == 0xC0 {
            let ptr = (((msg[pos] as usize) & 0x3F) << 8) | msg[pos + 1] as usize;
            if !jumped {
                next = pos + 2;
            }
            jumped = true;
            pos = ptr;
            continue;
        }
        if len == 0 {
            if !jumped {
                next = pos + 1;
            }
            break;
        }
        pos += 1;
        if pos + len > msg.len() {
            break;
        }
        labels.push(String::from_utf8_lossy(&msg[pos..pos + len]).into_owned());
        pos += len;
    }
    (labels.join("."), next)
}

fn u16be(b: &[u8], i: usize) -> u16 {
    ((b[i] as u16) << 8) | b[i + 1] as u16
}

pub enum Rr {
    A(String),
    Aaaa(String),
    Name(String),
    Mx {
        priority: u16,
        exchange: String,
    },
    Txt(Vec<String>),
    Srv {
        priority: u16,
        weight: u16,
        port: u16,
        name: String,
    },
    Soa {
        nsname: String,
        hostmaster: String,
        serial: u32,
        refresh: u32,
        retry: u32,
        expire: u32,
        minttl: u32,
    },
}

pub fn query(server: &str, name: &str, qtype: u16) -> Result<Vec<Rr>, String> {
    let packet = build_query(name, qtype);
    let sock = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    sock.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    sock.send_to(&packet, (server, 53))
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 4096];
    let (n, _) = sock.recv_from(&mut buf).map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut {
            "ETIMEOUT".to_string()
        } else {
            e.to_string()
        }
    })?;
    let msg = &buf[..n];
    if msg.len() >= 4 && (msg[2] & 0x02) != 0 {
        return query_tcp(server, &packet, qtype);
    }
    parse_response(msg, qtype)
}

fn query_tcp(server: &str, packet: &[u8], qtype: u16) -> Result<Vec<Rr>, String> {
    let mut stream = TcpStream::connect((server, 53)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let len = u16::try_from(packet.len())
        .map_err(|_| "DNS query packet too large".to_string())?
        .to_be_bytes();
    stream.write_all(&len).map_err(|e| e.to_string())?;
    stream.write_all(packet).map_err(|e| e.to_string())?;
    let mut hdr = [0u8; 2];
    stream.read_exact(&mut hdr).map_err(|e| e.to_string())?;
    let n = u16::from_be_bytes(hdr) as usize;
    let mut msg = vec![0u8; n];
    stream.read_exact(&mut msg).map_err(|e| e.to_string())?;
    parse_response(&msg, qtype)
}

fn parse_response(msg: &[u8], qtype: u16) -> Result<Vec<Rr>, String> {
    if msg.len() < 12 {
        return Err("short DNS response".into());
    }
    let rcode = msg[3] & 0x0F;
    match rcode {
        0 => {}
        2 => return Err("ESERVFAIL".into()),
        3 => return Err("ENOTFOUND".into()),
        4 => return Err("ENOTIMP".into()),
        5 => return Err("EREFUSED".into()),
        _ => return Err("ESERVFAIL".into()),
    }
    let qd = u16be(msg, 4);
    let an = u16be(msg, 6);
    if an == 0 {
        return Err("ENODATA".into());
    }
    let mut pos = 12;
    for _ in 0..qd {
        let (_n, p) = read_name(msg, pos);
        pos = p + 4;
    }
    let mut out = Vec::new();
    for _ in 0..an {
        let (_name, p) = read_name(msg, pos);
        pos = p;
        if pos + 10 > msg.len() {
            break;
        }
        let rtype = u16be(msg, pos);
        let rdlen = u16be(msg, pos + 8) as usize;
        let rdata = pos + 10;
        pos = rdata + rdlen;
        if pos > msg.len() {
            break;
        }
        if rtype != qtype {
            continue;
        }
        match rtype {
            T_A if rdlen == 4 => out.push(Rr::A(format!(
                "{}.{}.{}.{}",
                msg[rdata],
                msg[rdata + 1],
                msg[rdata + 2],
                msg[rdata + 3]
            ))),
            T_AAAA if rdlen == 16 => {
                let seg: Vec<String> = (0..8)
                    .map(|i| format!("{:x}", u16be(msg, rdata + i * 2)))
                    .collect();
                out.push(Rr::Aaaa(compress_v6(&seg)));
            }
            T_CNAME | T_NS | T_PTR => out.push(Rr::Name(read_name(msg, rdata).0)),
            T_MX => out.push(Rr::Mx {
                priority: u16be(msg, rdata),
                exchange: read_name(msg, rdata + 2).0,
            }),
            T_TXT => {
                let mut strs = Vec::new();
                let mut i = rdata;
                while i < rdata + rdlen {
                    let l = msg[i] as usize;
                    i += 1;
                    strs.push(
                        String::from_utf8_lossy(&msg[i..(i + l).min(msg.len())]).into_owned(),
                    );
                    i += l;
                }
                out.push(Rr::Txt(strs));
            }
            T_SRV => out.push(Rr::Srv {
                priority: u16be(msg, rdata),
                weight: u16be(msg, rdata + 2),
                port: u16be(msg, rdata + 4),
                name: read_name(msg, rdata + 6).0,
            }),
            T_SOA => {
                let (nsname, p1) = read_name(msg, rdata);
                let (hostmaster, p2) = read_name(msg, p1);
                let u32at = |i: usize| {
                    ((msg[i] as u32) << 24)
                        | ((msg[i + 1] as u32) << 16)
                        | ((msg[i + 2] as u32) << 8)
                        | msg[i + 3] as u32
                };
                out.push(Rr::Soa {
                    nsname,
                    hostmaster,
                    serial: u32at(p2),
                    refresh: u32at(p2 + 4),
                    retry: u32at(p2 + 8),
                    expire: u32at(p2 + 12),
                    minttl: u32at(p2 + 16),
                });
            }
            _ => {}
        }
    }
    Ok(out)
}

fn compress_v6(seg: &[String]) -> String {

    let joined = seg.join(":");

    let zeros: Vec<&str> = seg.iter().map(|s| s.as_str()).collect();
    let (mut best_start, mut best_len, mut cur_start, mut cur_len) =
        (0usize, 0usize, 0usize, 0usize);
    for (i, z) in zeros.iter().enumerate() {
        if *z == "0" {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
            if cur_len > best_len {
                best_len = cur_len;
                best_start = cur_start;
            }
        } else {
            cur_len = 0;
        }
    }
    if best_len < 2 {
        return joined;
    }
    let head = zeros[..best_start].join(":");
    let tail = zeros[best_start + best_len..].join(":");
    format!("{head}::{tail}")
}
