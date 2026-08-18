
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl Opcode {
    pub fn from_u8(b: u8) -> Option<Opcode> {
        use Opcode::*;
        Some(match b & 0x0F {
            0x0 => Continuation,
            0x1 => Text,
            0x2 => Binary,
            0x8 => Close,
            0x9 => Ping,
            0xA => Pong,
            _ => return None,
        })
    }
    pub fn is_control(&self) -> bool {
        matches!(self, Opcode::Close | Opcode::Ping | Opcode::Pong)
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub fin: bool,
    pub opcode: Opcode,
    pub payload: Vec<u8>,

    pub mask: Option<[u8; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub opcode: Opcode,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameLimits {
    pub max_frame_payload: usize,
    pub max_message_payload: usize,
}

impl FrameLimits {
    pub const fn new(max_frame_payload: usize, max_message_payload: usize) -> Self {
        Self {
            max_frame_payload,
            max_message_payload,
        }
    }
}

impl Default for FrameLimits {
    fn default() -> Self {
        Self {
            max_frame_payload: 16 * 1024 * 1024,
            max_message_payload: 64 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub enum WsError {
    UnexpectedEnd,
    InvalidOpcode(u8),
    ControlTooLong,
    ControlFragmented,
    ReservedBitsSet,
    PayloadTooLong,
    MessageTooLong,
    UnexpectedContinuation,
    FragmentAlreadyStarted,
    InvalidTextUtf8,
    InvalidClosePayload,
    InvalidCloseCode(u16),
    InvalidHandshakeKey,
    UnsupportedWebSocketVersion,
    OriginRejected,
    UnmaskedClientFrame,
    MaskedServerFrame,
    Crypto(String),
}

impl std::fmt::Display for WsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WsError::UnexpectedEnd => write!(f, "unexpected end of frame"),
            WsError::InvalidOpcode(b) => write!(f, "invalid opcode 0x{:x}", b),
            WsError::ControlTooLong => write!(f, "control frame payload >125 bytes"),
            WsError::ControlFragmented => write!(f, "control frame must not be fragmented (FIN=0)"),
            WsError::ReservedBitsSet => write!(f, "reserved bits set (extensions unsupported)"),
            WsError::PayloadTooLong => write!(f, "frame payload too long"),
            WsError::MessageTooLong => write!(f, "message payload too long"),
            WsError::UnexpectedContinuation => write!(f, "unexpected continuation frame"),
            WsError::FragmentAlreadyStarted => {
                write!(f, "data frame while fragmented message is open")
            }
            WsError::InvalidTextUtf8 => write!(f, "text message is not valid UTF-8"),
            WsError::InvalidClosePayload => write!(f, "invalid close frame payload"),
            WsError::InvalidCloseCode(code) => write!(f, "invalid close code {}", code),
            WsError::InvalidHandshakeKey => write!(f, "invalid Sec-WebSocket-Key"),
            WsError::UnsupportedWebSocketVersion => write!(f, "unsupported WebSocket version"),
            WsError::OriginRejected => write!(f, "WebSocket origin rejected"),
            WsError::UnmaskedClientFrame => write!(f, "client frame must be masked"),
            WsError::MaskedServerFrame => write!(f, "server frame must not be masked"),
            WsError::Crypto(s) => write!(f, "crypto: {}", s),
        }
    }
}

impl std::error::Error for WsError {}

pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, WsError> {
    if frame.opcode.is_control() {
        if frame.payload.len() > 125 {
            return Err(WsError::ControlTooLong);
        }
        if !frame.fin {
            return Err(WsError::ControlFragmented);
        }
    }
    let mut out = Vec::with_capacity(2 + frame.payload.len());
    let b0 = if frame.fin { 0x80 } else { 0x00 } | (frame.opcode as u8);
    out.push(b0);
    let mask_bit: u8 = if frame.mask.is_some() { 0x80 } else { 0x00 };
    let len = frame.payload.len();
    if len <= 125 {
        out.push(mask_bit | (len as u8));
    } else if len <= 0xFFFF {
        out.push(mask_bit | 126);
        out.push((len >> 8) as u8);
        out.push((len & 0xFF) as u8);
    } else if len <= 0x7FFF_FFFF_FFFF_FFFF {
        out.push(mask_bit | 127);
        for shift in (0..8).rev() {
            out.push(((len >> (8 * shift)) & 0xFF) as u8);
        }
    } else {
        return Err(WsError::PayloadTooLong);
    }
    if let Some(mask) = frame.mask {
        out.extend_from_slice(&mask);
        for (i, b) in frame.payload.iter().enumerate() {
            out.push(b ^ mask[i % 4]);
        }
    } else {
        out.extend_from_slice(&frame.payload);
    }
    Ok(out)
}

pub fn encode_client_frame(frame: &Frame) -> Result<Vec<u8>, WsError> {
    if frame.mask.is_none() {
        return Err(WsError::UnmaskedClientFrame);
    }
    encode_frame(frame)
}

pub fn encode_server_frame(frame: &Frame) -> Result<Vec<u8>, WsError> {
    if frame.mask.is_some() {
        return Err(WsError::MaskedServerFrame);
    }
    encode_frame(frame)
}

pub fn decode_frame(buf: &[u8]) -> Result<(Frame, usize), WsError> {
    decode_frame_with_limits(
        buf,
        FrameLimits {
            max_frame_payload: usize::MAX,
            max_message_payload: usize::MAX,
        },
    )
}

pub fn decode_frame_with_limits(
    buf: &[u8],
    limits: FrameLimits,
) -> Result<(Frame, usize), WsError> {
    if buf.len() < 2 {
        return Err(WsError::UnexpectedEnd);
    }
    let b0 = buf[0];
    let fin = (b0 & 0x80) != 0;
    if (b0 & 0x70) != 0 {
        return Err(WsError::ReservedBitsSet);
    }
    let opcode = Opcode::from_u8(b0 & 0x0F).ok_or(WsError::InvalidOpcode(b0 & 0x0F))?;
    if opcode.is_control() && !fin {
        return Err(WsError::ControlFragmented);
    }
    let b1 = buf[1];
    let masked = (b1 & 0x80) != 0;
    let len7 = b1 & 0x7F;
    let mut pos = 2;
    let payload_len: usize = match len7 {
        0..=125 => len7 as usize,
        126 => {
            if buf.len() < pos + 2 {
                return Err(WsError::UnexpectedEnd);
            }
            let l = ((buf[pos] as usize) << 8) | (buf[pos + 1] as usize);
            pos += 2;
            l
        }
        127 => {
            if buf.len() < pos + 8 {
                return Err(WsError::UnexpectedEnd);
            }
            let mut l: u64 = 0;
            for i in 0..8 {
                l = (l << 8) | (buf[pos + i] as u64);
            }
            pos += 8;
            if l > 0x7FFF_FFFF_FFFF_FFFF {
                return Err(WsError::PayloadTooLong);
            }
            l as usize
        }
        _ => unreachable!(),
    };
    if payload_len > limits.max_frame_payload {
        return Err(WsError::PayloadTooLong);
    }
    if opcode.is_control() && payload_len > 125 {
        return Err(WsError::ControlTooLong);
    }
    let mask: Option<[u8; 4]> = if masked {
        if buf.len() < pos + 4 {
            return Err(WsError::UnexpectedEnd);
        }
        let m = [buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]];
        pos += 4;
        Some(m)
    } else {
        None
    };
    if buf.len() < pos + payload_len {
        return Err(WsError::UnexpectedEnd);
    }
    let mut payload = buf[pos..pos + payload_len].to_vec();
    if let Some(m) = mask {
        for (i, b) in payload.iter_mut().enumerate() {
            *b ^= m[i % 4];
        }
    }
    Ok((
        Frame {
            fin,
            opcode,
            payload,
            mask,
        },
        pos + payload_len,
    ))
}

pub fn decode_server_frame(buf: &[u8]) -> Result<(Frame, usize), WsError> {
    decode_server_frame_with_limits(buf, FrameLimits::default())
}

pub fn decode_server_frame_with_limits(
    buf: &[u8],
    limits: FrameLimits,
) -> Result<(Frame, usize), WsError> {
    let (frame, used) = decode_frame_with_limits(buf, limits)?;
    if frame.mask.is_none() {
        return Err(WsError::UnmaskedClientFrame);
    }
    Ok((frame, used))
}

pub fn decode_client_frame(buf: &[u8]) -> Result<(Frame, usize), WsError> {
    decode_client_frame_with_limits(buf, FrameLimits::default())
}

pub fn decode_client_frame_with_limits(
    buf: &[u8],
    limits: FrameLimits,
) -> Result<(Frame, usize), WsError> {
    let (frame, used) = decode_frame_with_limits(buf, limits)?;
    if frame.mask.is_some() {
        return Err(WsError::MaskedServerFrame);
    }
    Ok((frame, used))
}

#[derive(Debug, Clone)]
pub struct MessageReassembler {
    limits: FrameLimits,
    current_opcode: Option<Opcode>,
    payload: Vec<u8>,
}

impl MessageReassembler {
    pub fn new(limits: FrameLimits) -> Self {
        Self {
            limits,
            current_opcode: None,
            payload: Vec::new(),
        }
    }

    pub fn push_frame(&mut self, frame: Frame) -> Result<Option<Message>, WsError> {
        if frame.opcode.is_control() {
            return Ok(Some(Message {
                opcode: frame.opcode,
                payload: frame.payload,
            }));
        }

        match frame.opcode {
            Opcode::Text | Opcode::Binary => {
                if self.current_opcode.is_some() {
                    return Err(WsError::FragmentAlreadyStarted);
                }
                if frame.payload.len() > self.limits.max_message_payload {
                    return Err(WsError::MessageTooLong);
                }
                if frame.fin {
                    validate_message_payload(frame.opcode, &frame.payload)?;
                    return Ok(Some(Message {
                        opcode: frame.opcode,
                        payload: frame.payload,
                    }));
                }
                self.current_opcode = Some(frame.opcode);
                self.payload = frame.payload;
                Ok(None)
            }
            Opcode::Continuation => {
                let opcode = self.current_opcode.ok_or(WsError::UnexpectedContinuation)?;
                let next_len = self
                    .payload
                    .len()
                    .checked_add(frame.payload.len())
                    .ok_or(WsError::MessageTooLong)?;
                if next_len > self.limits.max_message_payload {
                    self.current_opcode = None;
                    self.payload.clear();
                    return Err(WsError::MessageTooLong);
                }
                self.payload.extend_from_slice(&frame.payload);
                if frame.fin {
                    let payload = std::mem::take(&mut self.payload);
                    self.current_opcode = None;
                    validate_message_payload(opcode, &payload)?;
                    Ok(Some(Message { opcode, payload }))
                } else {
                    Ok(None)
                }
            }
            Opcode::Close | Opcode::Ping | Opcode::Pong => unreachable!(),
        }
    }
}

pub fn validate_message_payload(opcode: Opcode, payload: &[u8]) -> Result<(), WsError> {
    if opcode == Opcode::Text && std::str::from_utf8(payload).is_err() {
        return Err(WsError::InvalidTextUtf8);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct CloseFrame {
    pub code: Option<u16>,
    pub reason: String,
}

pub fn encode_close(code: Option<u16>, reason: &str) -> Vec<u8> {
    let mut p = Vec::new();
    if let Some(c) = code {
        p.push((c >> 8) as u8);
        p.push((c & 0xFF) as u8);
        p.extend_from_slice(reason.as_bytes());
    }
    p
}

pub fn decode_close(payload: &[u8]) -> Result<CloseFrame, WsError> {
    if payload.is_empty() {
        return Ok(CloseFrame {
            code: None,
            reason: String::new(),
        });
    }
    if payload.len() < 2 {
        return Err(WsError::InvalidClosePayload);
    }
    let code = ((payload[0] as u16) << 8) | (payload[1] as u16);
    validate_close_code(code)?;
    let reason = std::str::from_utf8(&payload[2..])
        .map_err(|_| WsError::InvalidClosePayload)?
        .to_string();
    Ok(CloseFrame {
        code: Some(code),
        reason,
    })
}

pub fn validate_close_code(code: u16) -> Result<(), WsError> {
    let valid = match code {
        1000..=1003 | 1007..=1014 => true,
        3000..=4999 => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(WsError::InvalidCloseCode(code))
    }
}

const ACCEPT_MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub fn generate_key() -> Result<String, WsError> {
    let mut bytes = [0u8; 16];
    rusty_web_crypto::get_random_values(&mut bytes)
        .map_err(|e| WsError::Crypto(format!("RNG: {}", e)))?;
    Ok(base64_encode(&bytes))
}

pub fn derive_accept(client_key: &str) -> String {
    let concat = format!("{}{}", client_key, ACCEPT_MAGIC);
    let hash = rusty_web_crypto::digest_sha1(concat.as_bytes());
    base64_encode(&hash)
}

pub fn verify_accept(client_key: &str, server_accept: &str) -> bool {
    derive_accept(client_key) == server_accept
}

#[derive(Debug, Clone, Copy)]
pub struct HandshakePolicy<'a> {
    pub allowed_origins: &'a [&'a str],
}

impl<'a> HandshakePolicy<'a> {
    pub const fn allow_any() -> Self {
        Self {
            allowed_origins: &[],
        }
    }
}

pub fn validate_server_handshake_request(
    sec_websocket_key: &str,
    sec_websocket_version: &str,
    origin: Option<&str>,
    policy: HandshakePolicy<'_>,
) -> Result<(), WsError> {
    if sec_websocket_version.trim() != "13" {
        return Err(WsError::UnsupportedWebSocketVersion);
    }
    validate_client_key(sec_websocket_key)?;
    if !policy.allowed_origins.is_empty() {
        let origin = origin.ok_or(WsError::OriginRejected)?;
        if !policy
            .allowed_origins
            .iter()
            .any(|allowed| *allowed == origin)
        {
            return Err(WsError::OriginRejected);
        }
    }
    Ok(())
}

fn validate_client_key(key: &str) -> Result<(), WsError> {
    let decoded = base64_decode(key).ok_or(WsError::InvalidHandshakeKey)?;
    if decoded.len() == 16 {
        Ok(())
    } else {
        Err(WsError::InvalidHandshakeKey)
    }
}

fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= input.len() {
        let a = input[i] as u32;
        let b = input[i + 1] as u32;
        let c = input[i + 2] as u32;
        let v = (a << 16) | (b << 8) | c;
        out.push(ALPHABET[((v >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((v >> 12) & 0x3F) as usize] as char);
        out.push(ALPHABET[((v >> 6) & 0x3F) as usize] as char);
        out.push(ALPHABET[(v & 0x3F) as usize] as char);
        i += 3;
    }
    let rem = input.len() - i;
    if rem == 1 {
        let v = (input[i] as u32) << 16;
        out.push(ALPHABET[((v >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((v >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let v = ((input[i] as u32) << 16) | ((input[i + 1] as u32) << 8);
        out.push(ALPHABET[((v >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((v >> 12) & 0x3F) as usize] as char);
        out.push(ALPHABET[((v >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let mut vals = [0u8; 4];
        let mut pad = 0usize;
        for (i, b) in chunk.iter().copied().enumerate() {
            vals[i] = match b {
                b'A'..=b'Z' => b - b'A',
                b'a'..=b'z' => b - b'a' + 26,
                b'0'..=b'9' => b - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' if i >= 2 => {
                    pad += 1;
                    0
                }
                _ => return None,
            };
        }
        if pad > 0 && chunk[4 - pad..].iter().any(|b| *b != b'=') {
            return None;
        }
        let n = ((vals[0] as u32) << 18)
            | ((vals[1] as u32) << 12)
            | ((vals[2] as u32) << 6)
            | (vals[3] as u32);
        out.push(((n >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((n >> 8) & 0xFF) as u8);
        }
        if pad == 0 {
            out.push((n & 0xFF) as u8);
        }
    }
    Some(out)
}
