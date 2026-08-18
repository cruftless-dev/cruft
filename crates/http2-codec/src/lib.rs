
pub const CONNECTION_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

pub const DEFAULT_MAX_FRAME_SIZE: u32 = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Data,
    Headers,
    Priority,
    RstStream,
    Settings,
    PushPromise,
    Ping,
    GoAway,
    WindowUpdate,
    Continuation,
    Unknown(u8),
}

impl FrameType {
    pub fn from_u8(b: u8) -> Self {
        match b {
            0x00 => FrameType::Data,
            0x01 => FrameType::Headers,
            0x02 => FrameType::Priority,
            0x03 => FrameType::RstStream,
            0x04 => FrameType::Settings,
            0x05 => FrameType::PushPromise,
            0x06 => FrameType::Ping,
            0x07 => FrameType::GoAway,
            0x08 => FrameType::WindowUpdate,
            0x09 => FrameType::Continuation,
            other => FrameType::Unknown(other),
        }
    }
    pub fn to_u8(self) -> u8 {
        match self {
            FrameType::Data => 0x00,
            FrameType::Headers => 0x01,
            FrameType::Priority => 0x02,
            FrameType::RstStream => 0x03,
            FrameType::Settings => 0x04,
            FrameType::PushPromise => 0x05,
            FrameType::Ping => 0x06,
            FrameType::GoAway => 0x07,
            FrameType::WindowUpdate => 0x08,
            FrameType::Continuation => 0x09,
            FrameType::Unknown(b) => b,
        }
    }
}

pub const FLAG_END_STREAM: u8 = 0x01;
pub const FLAG_ACK: u8 = 0x01;
pub const FLAG_END_HEADERS: u8 = 0x04;
pub const FLAG_PADDED: u8 = 0x08;
pub const FLAG_PRIORITY: u8 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub length: u32,
    pub frame_type: FrameType,
    pub flags: u8,
    pub stream_id: u32,
}

pub fn encode_frame_header(h: &FrameHeader) -> [u8; 9] {
    let len = h.length & 0x00FF_FFFF;
    let sid = h.stream_id & 0x7FFF_FFFF;
    [
        (len >> 16) as u8,
        (len >> 8) as u8,
        len as u8,
        h.frame_type.to_u8(),
        h.flags,
        (sid >> 24) as u8,
        (sid >> 16) as u8,
        (sid >> 8) as u8,
        sid as u8,
    ]
}

pub fn decode_frame_header(b: &[u8]) -> Option<FrameHeader> {
    if b.len() < 9 {
        return None;
    }
    let length = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
    let frame_type = FrameType::from_u8(b[3]);
    let flags = b[4];
    let stream_id =
        (((b[5] as u32) << 24) | ((b[6] as u32) << 16) | ((b[7] as u32) << 8) | (b[8] as u32))
            & 0x7FFF_FFFF;
    Some(FrameHeader {
        length,
        frame_type,
        flags,
        stream_id,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub header: FrameHeader,
    pub payload: Vec<u8>,
}

pub fn encode_frame(frame_type: FrameType, flags: u8, stream_id: u32, payload: &[u8]) -> Vec<u8> {
    let header = FrameHeader {
        length: payload.len() as u32,
        frame_type,
        flags,
        stream_id,
    };
    let mut out = Vec::with_capacity(9 + payload.len());
    out.extend_from_slice(&encode_frame_header(&header));
    out.extend_from_slice(payload);
    out
}

pub fn take_frame(buf: &[u8]) -> Option<(Frame, usize)> {
    let header = decode_frame_header(buf)?;
    let total = 9 + header.length as usize;
    if buf.len() < total {
        return None;
    }
    Some((
        Frame {
            header,
            payload: buf[9..total].to_vec(),
        },
        total,
    ))
}

pub const SETTINGS_HEADER_TABLE_SIZE: u16 = 0x01;
pub const SETTINGS_ENABLE_PUSH: u16 = 0x02;
pub const SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 0x03;
pub const SETTINGS_INITIAL_WINDOW_SIZE: u16 = 0x04;
pub const SETTINGS_MAX_FRAME_SIZE: u16 = 0x05;
pub const SETTINGS_MAX_HEADER_LIST_SIZE: u16 = 0x06;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting {
    pub id: u16,
    pub value: u32,
}

pub fn encode_settings_payload(settings: &[Setting]) -> Vec<u8> {
    let mut out = Vec::with_capacity(settings.len() * 6);
    for s in settings {
        out.extend_from_slice(&s.id.to_be_bytes());
        out.extend_from_slice(&s.value.to_be_bytes());
    }
    out
}

pub fn decode_settings_payload_checked(payload: &[u8]) -> Option<Vec<Setting>> {
    if payload.len() % 6 != 0 {
        return None;
    }
    Some(decode_settings_payload(payload))
}

pub fn decode_settings_payload(payload: &[u8]) -> Vec<Setting> {
    payload
        .chunks_exact(6)
        .map(|c| Setting {
            id: u16::from_be_bytes([c[0], c[1]]),
            value: u32::from_be_bytes([c[2], c[3], c[4], c[5]]),
        })
        .collect()
}

pub fn encode_settings_frame(settings: &[Setting]) -> Vec<u8> {
    encode_frame(
        FrameType::Settings,
        0,
        0,
        &encode_settings_payload(settings),
    )
}

pub fn encode_settings_ack() -> Vec<u8> {
    encode_frame(FrameType::Settings, FLAG_ACK, 0, &[])
}

pub const ERR_NO_ERROR: u32 = 0x0;
pub const ERR_PROTOCOL_ERROR: u32 = 0x1;
pub const ERR_INTERNAL_ERROR: u32 = 0x2;
pub const ERR_FLOW_CONTROL_ERROR: u32 = 0x3;
pub const ERR_SETTINGS_TIMEOUT: u32 = 0x4;
pub const ERR_STREAM_CLOSED: u32 = 0x5;
pub const ERR_FRAME_SIZE_ERROR: u32 = 0x6;
pub const ERR_REFUSED_STREAM: u32 = 0x7;
pub const ERR_CANCEL: u32 = 0x8;
pub const ERR_COMPRESSION_ERROR: u32 = 0x9;
pub const ERR_CONNECT_ERROR: u32 = 0xa;
pub const ERR_ENHANCE_YOUR_CALM: u32 = 0xb;
pub const ERR_INADEQUATE_SECURITY: u32 = 0xc;
pub const ERR_HTTP_1_1_REQUIRED: u32 = 0xd;

pub fn encode_window_update(stream_id: u32, increment: u32) -> Vec<u8> {
    encode_frame(
        FrameType::WindowUpdate,
        0,
        stream_id,
        &(increment & 0x7FFF_FFFF).to_be_bytes(),
    )
}

pub fn window_update_increment(payload: &[u8]) -> Option<u32> {
    if payload.len() != 4 {
        return None;
    }
    Some(u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]) & 0x7FFF_FFFF)
}

pub fn encode_ping(opaque: &[u8; 8]) -> Vec<u8> {
    encode_frame(FrameType::Ping, 0, 0, opaque)
}
pub fn encode_ping_ack(opaque: &[u8; 8]) -> Vec<u8> {
    encode_frame(FrameType::Ping, FLAG_ACK, 0, opaque)
}

pub fn encode_rst_stream(stream_id: u32, error_code: u32) -> Vec<u8> {
    encode_frame(
        FrameType::RstStream,
        0,
        stream_id,
        &error_code.to_be_bytes(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoAway {
    pub last_stream_id: u32,
    pub error_code: u32,
    pub debug_data: Vec<u8>,
}

pub fn encode_goaway(last_stream_id: u32, error_code: u32, debug_data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + debug_data.len());
    payload.extend_from_slice(&(last_stream_id & 0x7FFF_FFFF).to_be_bytes());
    payload.extend_from_slice(&error_code.to_be_bytes());
    payload.extend_from_slice(debug_data);
    encode_frame(FrameType::GoAway, 0, 0, &payload)
}

pub fn decode_goaway(payload: &[u8]) -> Option<GoAway> {
    if payload.len() < 8 {
        return None;
    }
    Some(GoAway {
        last_stream_id: u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]])
            & 0x7FFF_FFFF,
        error_code: u32::from_be_bytes([payload[4], payload[5], payload[6], payload[7]]),
        debug_data: payload[8..].to_vec(),
    })
}

pub fn encode_data(stream_id: u32, end_stream: bool, data: &[u8]) -> Vec<u8> {
    let flags = if end_stream { FLAG_END_STREAM } else { 0 };
    encode_frame(FrameType::Data, flags, stream_id, data)
}

pub fn data_payload(frame: &Frame) -> Option<Vec<u8>> {
    strip_padding(frame.header.flags & FLAG_PADDED != 0, &frame.payload)
}

pub fn encode_headers(
    stream_id: u32,
    header_block: &[u8],
    end_stream: bool,
    end_headers: bool,
) -> Vec<u8> {
    let mut flags = 0u8;
    if end_stream {
        flags |= FLAG_END_STREAM;
    }
    if end_headers {
        flags |= FLAG_END_HEADERS;
    }
    encode_frame(FrameType::Headers, flags, stream_id, header_block)
}

pub fn header_block_fragment(frame: &Frame) -> Option<Vec<u8>> {
    let mut p = &frame.payload[..];
    let padded = frame.header.flags & FLAG_PADDED != 0;
    let has_priority = frame.header.flags & FLAG_PRIORITY != 0;
    let pad_len = if padded {
        let pl = *p.first()? as usize;
        p = &p[1..];
        pl
    } else {
        0
    };
    if has_priority {
        if p.len() < 5 {
            return None;
        }
        p = &p[5..];
    }
    if p.len() < pad_len {
        return None;
    }
    Some(p[..p.len() - pad_len].to_vec())
}

fn strip_padding(padded: bool, payload: &[u8]) -> Option<Vec<u8>> {
    if !padded {
        return Some(payload.to_vec());
    }
    let pad_len = *payload.first()? as usize;
    let body = &payload[1..];
    if body.len() < pad_len {
        return None;
    }
    Some(body[..body.len() - pad_len].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_header_round_trips() {
        let h = FrameHeader {
            length: 0x0123_45,
            frame_type: FrameType::Headers,
            flags: FLAG_END_HEADERS | FLAG_END_STREAM,
            stream_id: 0x7FFF_FFFF,
        };
        let bytes = encode_frame_header(&h);
        assert_eq!(decode_frame_header(&bytes), Some(h));

        let mut with_r = bytes;
        with_r[5] |= 0x80;
        assert_eq!(decode_frame_header(&with_r).unwrap().stream_id, 0x7FFF_FFFF);
    }

    #[test]
    fn encode_and_take_frame() {
        let payload = b"\x00\x01\x02\x03";
        let wire = encode_frame(FrameType::Data, FLAG_END_STREAM, 5, payload);
        assert_eq!(wire.len(), 9 + payload.len());

        assert!(take_frame(&wire[..9 + 2]).is_none());
        let (frame, used) = take_frame(&wire).unwrap();
        assert_eq!(used, wire.len());
        assert_eq!(frame.header.frame_type, FrameType::Data);
        assert_eq!(frame.header.flags, FLAG_END_STREAM);
        assert_eq!(frame.header.stream_id, 5);
        assert_eq!(frame.payload, payload);

        let mut two = encode_settings_frame(&[]);
        two.extend_from_slice(&wire);
        let (f1, n1) = take_frame(&two).unwrap();
        assert_eq!(f1.header.frame_type, FrameType::Settings);
        let (f2, _) = take_frame(&two[n1..]).unwrap();
        assert_eq!(f2.header.frame_type, FrameType::Data);
    }

    #[test]
    fn settings_round_trip_and_ack() {
        let settings = [
            Setting {
                id: SETTINGS_MAX_CONCURRENT_STREAMS,
                value: 100,
            },
            Setting {
                id: SETTINGS_INITIAL_WINDOW_SIZE,
                value: 65_535,
            },
            Setting {
                id: SETTINGS_ENABLE_PUSH,
                value: 0,
            },
        ];
        let frame = encode_settings_frame(&settings);
        let (f, _) = take_frame(&frame).unwrap();
        assert_eq!(f.header.frame_type, FrameType::Settings);
        assert_eq!(f.header.stream_id, 0);
        assert_eq!(f.header.flags, 0);
        assert_eq!(
            decode_settings_payload_checked(&f.payload),
            Some(settings.to_vec())
        );
        assert_eq!(decode_settings_payload(&f.payload), settings.to_vec());

        let ack = encode_settings_ack();
        let (a, _) = take_frame(&ack).unwrap();
        assert_eq!(a.header.frame_type, FrameType::Settings);
        assert_eq!(a.header.flags, FLAG_ACK);
        assert!(a.payload.is_empty());
    }

    #[test]
    fn settings_payload_checked_rejects_non_multiple_of_six() {
        assert_eq!(decode_settings_payload_checked(&[0, 1, 0, 0, 0]), None);
    }

    #[test]
    fn preface_is_the_rfc_string() {
        assert_eq!(CONNECTION_PREFACE, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n");
        assert_eq!(CONNECTION_PREFACE.len(), 24);
    }

    #[test]
    fn control_frames_round_trip() {

        let wu = encode_window_update(3, 65_535);
        let (f, _) = take_frame(&wu).unwrap();
        assert_eq!(f.header.frame_type, FrameType::WindowUpdate);
        assert_eq!(f.header.stream_id, 3);
        assert_eq!(window_update_increment(&f.payload), Some(65_535));

        let opaque = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let (pf, _) = take_frame(&encode_ping(&opaque)).unwrap();
        assert_eq!(pf.header.frame_type, FrameType::Ping);
        assert_eq!(pf.header.flags, 0);
        assert_eq!(pf.payload, opaque);
        let (af, _) = take_frame(&encode_ping_ack(&opaque)).unwrap();
        assert_eq!(af.header.flags, FLAG_ACK);

        let (rf, _) = take_frame(&encode_rst_stream(7, ERR_CANCEL)).unwrap();
        assert_eq!(rf.header.frame_type, FrameType::RstStream);
        assert_eq!(rf.header.stream_id, 7);
        assert_eq!(
            u32::from_be_bytes([rf.payload[0], rf.payload[1], rf.payload[2], rf.payload[3]]),
            ERR_CANCEL
        );

        let (gf, _) = take_frame(&encode_goaway(11, ERR_NO_ERROR, b"bye")).unwrap();
        assert_eq!(gf.header.frame_type, FrameType::GoAway);
        assert_eq!(gf.header.stream_id, 0);
        let g = decode_goaway(&gf.payload).unwrap();
        assert_eq!(g.last_stream_id, 11);
        assert_eq!(g.error_code, ERR_NO_ERROR);
        assert_eq!(g.debug_data, b"bye");
    }

    #[test]
    fn data_and_headers_carriers() {

        let (df, _) = take_frame(&encode_data(1, true, b"hello")).unwrap();
        assert_eq!(df.header.frame_type, FrameType::Data);
        assert_eq!(df.header.flags, FLAG_END_STREAM);
        assert_eq!(data_payload(&df).unwrap(), b"hello");

        let mut padded_payload = vec![2u8];
        padded_payload.extend_from_slice(b"data");
        padded_payload.extend_from_slice(&[0, 0]);
        let padded = encode_frame(FrameType::Data, FLAG_PADDED, 1, &padded_payload);
        let (pf, _) = take_frame(&padded).unwrap();
        assert_eq!(data_payload(&pf).unwrap(), b"data");

        let hb = b"\x88";
        let (hf, _) = take_frame(&encode_headers(1, hb, true, true)).unwrap();
        assert_eq!(hf.header.frame_type, FrameType::Headers);
        assert_eq!(hf.header.flags, FLAG_END_HEADERS | FLAG_END_STREAM);
        assert_eq!(header_block_fragment(&hf).unwrap(), hb);

        let mut p = vec![1u8];
        p.extend_from_slice(&[0, 0, 0, 0, 0]);
        p.extend_from_slice(hb);
        p.push(0);
        let frame = encode_frame(FrameType::Headers, FLAG_PADDED | FLAG_PRIORITY, 1, &p);
        let (hpf, _) = take_frame(&frame).unwrap();
        assert_eq!(header_block_fragment(&hpf).unwrap(), hb);
    }
}
