
use std::collections::HashMap;

use rusty_http2_codec::{
    data_payload, decode_frame_header, decode_settings_payload_checked, encode_data, encode_goaway,
    encode_headers, encode_ping_ack, encode_settings_ack, encode_settings_frame,
    header_block_fragment, take_frame, window_update_increment, FrameType, Setting,
    CONNECTION_PREFACE, DEFAULT_MAX_FRAME_SIZE, ERR_ENHANCE_YOUR_CALM, ERR_FLOW_CONTROL_ERROR,
    ERR_FRAME_SIZE_ERROR, ERR_PROTOCOL_ERROR, ERR_STREAM_CLOSED, FLAG_ACK, FLAG_END_HEADERS,
    FLAG_END_STREAM, SETTINGS_ENABLE_PUSH, SETTINGS_HEADER_TABLE_SIZE,
    SETTINGS_INITIAL_WINDOW_SIZE, SETTINGS_MAX_CONCURRENT_STREAMS, SETTINGS_MAX_FRAME_SIZE,
    SETTINGS_MAX_HEADER_LIST_SIZE,
};
use rusty_http2_hpack::{
    decode_header_block_limited, encode_header_block, DecodeError, DecodeLimits, DynamicTable,
};

const SERVER_MAX_CONCURRENT_STREAMS: usize = 100;
const SERVER_INITIAL_WINDOW_SIZE: u32 = 1 << 20;
const SERVER_HEADER_TABLE_SIZE: usize = 4_096;
const SERVER_MAX_HEADER_LIST_SIZE: usize = 65_536;
const SERVER_MAX_HEADER_BLOCK_BYTES: usize = 65_536;
const SERVER_MAX_CONTINUATIONS_PER_BLOCK: usize = 32;
const SERVER_MAX_RST_STREAMS: usize = 100;
const SERVER_MAX_STREAM_LIFETIME: usize = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2Request {
    pub stream_id: u32,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Http2Request {

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
    pub fn method(&self) -> Option<&str> {
        self.header(":method")
    }
    pub fn path(&self) -> Option<&str> {
        self.header(":path")
    }
}

#[derive(Default)]
struct Stream {
    header_block: Vec<u8>,
    continuation_count: usize,
    headers: Vec<(String, String)>,
    headers_done: bool,
    body: Vec<u8>,
    recv_window: i64,
    end_stream: bool,
    emitted: bool,
}

#[derive(Debug, Default)]
pub struct Http2Feed {
    pub outbound: Vec<u8>,
    pub requests: Vec<Http2Request>,
}

struct SendState {
    pending: Vec<u8>,
    window: i64,
}

pub struct Http2Connection {
    rbuf: Vec<u8>,
    preface_seen: bool,
    settings_sent: bool,
    goaway_sent: bool,
    streams: HashMap<u32, Stream>,
    last_stream_id: u32,
    active_header_stream: Option<u32>,
    rst_stream_count: usize,
    stream_lifetime_count: usize,
    inbound_max_frame_size: u32,
    inbound_header_list_size: usize,
    inbound_header_table_size: usize,
    inbound_hpack: DynamicTable,
    conn_recv_window: i64,

    conn_send_window: i64,
    peer_initial_window: i64,
    peer_max_frame_size: u32,
    send: HashMap<u32, SendState>,
}

impl Default for Http2Connection {
    fn default() -> Self {
        Self::new()
    }
}

impl Http2Connection {
    pub fn new() -> Self {
        Http2Connection {
            rbuf: Vec::new(),
            preface_seen: false,
            settings_sent: false,
            goaway_sent: false,
            streams: HashMap::new(),
            last_stream_id: 0,
            active_header_stream: None,
            rst_stream_count: 0,
            stream_lifetime_count: 0,
            inbound_max_frame_size: DEFAULT_MAX_FRAME_SIZE,
            inbound_header_list_size: SERVER_MAX_HEADER_LIST_SIZE,
            inbound_header_table_size: SERVER_HEADER_TABLE_SIZE,
            inbound_hpack: DynamicTable::new(SERVER_HEADER_TABLE_SIZE),
            conn_recv_window: SERVER_INITIAL_WINDOW_SIZE as i64,
            conn_send_window: 65_535,
            peer_initial_window: 65_535,
            peer_max_frame_size: DEFAULT_MAX_FRAME_SIZE,
            send: HashMap::new(),
        }
    }

    fn server_settings() -> Vec<u8> {
        encode_settings_frame(&[
            Setting {
                id: SETTINGS_MAX_CONCURRENT_STREAMS,
                value: SERVER_MAX_CONCURRENT_STREAMS as u32,
            },
            Setting {
                id: SETTINGS_INITIAL_WINDOW_SIZE,
                value: SERVER_INITIAL_WINDOW_SIZE,
            },
            Setting {
                id: SETTINGS_HEADER_TABLE_SIZE,
                value: SERVER_HEADER_TABLE_SIZE as u32,
            },
            Setting {
                id: SETTINGS_MAX_HEADER_LIST_SIZE,
                value: SERVER_MAX_HEADER_LIST_SIZE as u32,
            },
        ])
    }

    pub fn feed(&mut self, inbound: &[u8]) -> Result<Http2Feed, String> {
        self.rbuf.extend_from_slice(inbound);
        let mut feed = Http2Feed::default();

        if !self.preface_seen {
            if self.rbuf.len() < CONNECTION_PREFACE.len() {
                return Ok(feed);
            }
            if &self.rbuf[..CONNECTION_PREFACE.len()] != CONNECTION_PREFACE {
                return Err("http2: invalid connection preface".into());
            }
            self.rbuf.drain(0..CONNECTION_PREFACE.len());
            self.preface_seen = true;
            feed.outbound.extend_from_slice(&Self::server_settings());
            self.settings_sent = true;
        }

        while let Some((frame, used)) = take_frame(&self.rbuf) {
            self.rbuf.drain(0..used);
            if frame.header.length > self.inbound_max_frame_size {
                self.goaway_code(&mut feed, ERR_FRAME_SIZE_ERROR, "inbound frame too large")?;
                break;
            }
            self.handle_frame(frame, &mut feed)?;
        }
        if let Some(header) = decode_frame_header(&self.rbuf) {
            if header.length > self.inbound_max_frame_size {
                self.goaway_code(
                    &mut feed,
                    ERR_FRAME_SIZE_ERROR,
                    "partial inbound frame too large",
                )?;
                self.rbuf.clear();
            }
        }
        Ok(feed)
    }

    fn handle_frame(
        &mut self,
        frame: rusty_http2_codec::Frame,
        feed: &mut Http2Feed,
    ) -> Result<(), String> {
        let sid = frame.header.stream_id;
        match frame.header.frame_type {
            FrameType::Settings => {
                if sid != 0 {
                    return self.goaway(feed, "SETTINGS on stream");
                }
                if frame.header.flags & FLAG_ACK != 0 && !frame.payload.is_empty() {
                    return self.goaway_code(
                        feed,
                        ERR_FRAME_SIZE_ERROR,
                        "SETTINGS ACK with payload",
                    );
                }

                if frame.header.flags & FLAG_ACK == 0 {
                    let Some(settings) = decode_settings_payload_checked(&frame.payload) else {
                        return self.goaway_code(feed, ERR_FRAME_SIZE_ERROR, "bad SETTINGS length");
                    };
                    for s in settings {
                        match s.id {
                            id if id == SETTINGS_ENABLE_PUSH && s.value > 1 => {
                                return self.goaway(feed, "bad SETTINGS_ENABLE_PUSH");
                            }
                            id if id == SETTINGS_MAX_FRAME_SIZE => {
                                if !(16_384..=16_777_215).contains(&s.value) {
                                    return self.goaway(feed, "bad SETTINGS_MAX_FRAME_SIZE");
                                }
                                self.peer_max_frame_size = s.value;
                            }
                            id if id == SETTINGS_INITIAL_WINDOW_SIZE => {
                                if s.value > 0x7FFF_FFFF {
                                    return self.goaway_code(
                                        feed,
                                        ERR_FLOW_CONTROL_ERROR,
                                        "bad SETTINGS_INITIAL_WINDOW_SIZE",
                                    );
                                }

                                let delta = s.value as i64 - self.peer_initial_window;
                                self.peer_initial_window = s.value as i64;
                                for ss in self.send.values_mut() {
                                    ss.window += delta;
                                }
                            }
                            _ => {}
                        }
                    }
                    feed.outbound.extend_from_slice(&encode_settings_ack());
                    self.flush_all(&mut feed.outbound);
                }
            }
            FrameType::Ping => {
                if sid != 0 || frame.payload.len() != 8 {
                    return self.goaway_code(feed, ERR_FRAME_SIZE_ERROR, "bad PING frame");
                }
                if frame.header.flags & FLAG_ACK == 0 && frame.payload.len() == 8 {
                    let mut opaque = [0u8; 8];
                    opaque.copy_from_slice(&frame.payload);
                    feed.outbound.extend_from_slice(&encode_ping_ack(&opaque));
                }
            }
            FrameType::WindowUpdate => {
                if let Some(inc) = window_update_increment(&frame.payload) {
                    if inc == 0 {
                        return self.goaway(feed, "WINDOW_UPDATE zero increment");
                    }
                    if sid == 0 {
                        if self.conn_send_window + inc as i64 > 0x7FFF_FFFF {
                            return self.goaway_code(
                                feed,
                                ERR_FLOW_CONTROL_ERROR,
                                "connection send window overflow",
                            );
                        }
                        self.conn_send_window += inc as i64;
                    } else if let Some(ss) = self.send.get_mut(&sid) {
                        if ss.window + inc as i64 > 0x7FFF_FFFF {
                            return self.goaway_code(
                                feed,
                                ERR_FLOW_CONTROL_ERROR,
                                "stream send window overflow",
                            );
                        }
                        ss.window += inc as i64;
                    }

                    self.flush_all(&mut feed.outbound);
                } else {
                    return self.goaway_code(
                        feed,
                        ERR_FRAME_SIZE_ERROR,
                        "bad WINDOW_UPDATE length",
                    );
                }
            }
            FrameType::RstStream => {
                if sid == 0 || frame.payload.len() != 4 {
                    return self.goaway_code(feed, ERR_FRAME_SIZE_ERROR, "bad RST_STREAM");
                }
                self.rst_stream_count += 1;
                if self.rst_stream_count > SERVER_MAX_RST_STREAMS {
                    return self.goaway_code(feed, ERR_ENHANCE_YOUR_CALM, "too many RST_STREAM");
                }
                self.streams.remove(&sid);
                if self.active_header_stream == Some(sid) {
                    self.active_header_stream = None;
                }
            }
            FrameType::GoAway => {   }
            FrameType::Headers => {
                if sid == 0 {
                    return self.goaway(feed, "HEADERS on stream 0");
                }
                if sid % 2 == 0 || sid <= self.last_stream_id {
                    return self.goaway(feed, "invalid client stream id");
                }
                if self.active_header_stream.is_some() {
                    return self.goaway(feed, "HEADERS before END_HEADERS");
                }
                if self.streams.len() >= SERVER_MAX_CONCURRENT_STREAMS {
                    return self.goaway_code(feed, ERR_ENHANCE_YOUR_CALM, "too many streams");
                }
                self.stream_lifetime_count += 1;
                if self.stream_lifetime_count > SERVER_MAX_STREAM_LIFETIME {
                    return self.goaway_code(feed, ERR_ENHANCE_YOUR_CALM, "too many streams total");
                }
                let fragment = header_block_fragment(&frame).ok_or("http2: bad HEADERS padding")?;
                if fragment.len() > SERVER_MAX_HEADER_BLOCK_BYTES {
                    return self.goaway_code(feed, ERR_ENHANCE_YOUR_CALM, "header block too large");
                }
                let end_stream = frame.header.flags & FLAG_END_STREAM != 0;
                let end_headers = frame.header.flags & FLAG_END_HEADERS != 0;
                self.last_stream_id = sid;
                let st = self.streams.entry(sid).or_insert_with(|| Stream {
                    recv_window: SERVER_INITIAL_WINDOW_SIZE as i64,
                    ..Stream::default()
                });
                st.header_block.extend_from_slice(&fragment);
                if end_stream {
                    st.end_stream = true;
                }
                if end_headers {
                    self.finish_headers(sid)?;
                    self.active_header_stream = None;
                } else {
                    self.active_header_stream = Some(sid);
                }
                self.try_emit(sid, feed);
            }
            FrameType::Continuation => {
                if self.active_header_stream != Some(sid) {
                    return self.goaway(feed, "CONTINUATION outside header block");
                }
                let st = self
                    .streams
                    .get_mut(&sid)
                    .ok_or("http2: CONTINUATION without stream")?;
                st.continuation_count += 1;
                if st.continuation_count > SERVER_MAX_CONTINUATIONS_PER_BLOCK {
                    return self.goaway_code(feed, ERR_ENHANCE_YOUR_CALM, "too many CONTINUATION");
                }
                if st.header_block.len() + frame.payload.len() > SERVER_MAX_HEADER_BLOCK_BYTES {
                    return self.goaway_code(feed, ERR_ENHANCE_YOUR_CALM, "header block too large");
                }
                st.header_block.extend_from_slice(&frame.payload);
                if frame.header.flags & FLAG_END_HEADERS != 0 {
                    self.finish_headers(sid)?;
                    self.active_header_stream = None;
                    self.try_emit(sid, feed);
                }
            }
            FrameType::Data => {
                if sid == 0 {
                    return self.goaway(feed, "DATA on stream 0");
                }
                if !self.streams.contains_key(&sid) {
                    return self.goaway_code(feed, ERR_STREAM_CLOSED, "DATA on closed stream");
                }
                let body = data_payload(&frame).ok_or("http2: bad DATA padding")?;
                if self.conn_recv_window - (frame.header.length as i64) < 0 {
                    return self.goaway_code(
                        feed,
                        ERR_FLOW_CONTROL_ERROR,
                        "connection receive window exceeded",
                    );
                }
                self.conn_recv_window -= frame.header.length as i64;
                let st = self.streams.get_mut(&sid).ok_or("http2: no DATA stream")?;
                if st.recv_window - (frame.header.length as i64) < 0 {
                    return self.goaway_code(
                        feed,
                        ERR_FLOW_CONTROL_ERROR,
                        "stream receive window exceeded",
                    );
                }
                st.recv_window -= frame.header.length as i64;
                st.body.extend_from_slice(&body);
                if frame.header.flags & FLAG_END_STREAM != 0 {
                    st.end_stream = true;
                }
                self.try_emit(sid, feed);
            }
            FrameType::Priority | FrameType::PushPromise | FrameType::Unknown(_) => {}
        }
        Ok(())
    }

    fn finish_headers(&mut self, sid: u32) -> Result<(), String> {
        let block = {
            let st = self.streams.get_mut(&sid).ok_or("http2: no stream")?;
            std::mem::take(&mut st.header_block)
        };

        let headers = decode_header_block_limited(
            &block,
            &mut self.inbound_hpack,
            DecodeLimits {
                max_header_list_size: self.inbound_header_list_size,
                max_dynamic_table_size: self.inbound_header_table_size,
            },
        )
        .map_err(|e| match e {
            DecodeError::HeaderListTooLarge => "http2: HPACK header list too large".to_string(),
            DecodeError::DynamicTableSizeUpdateTooLarge => {
                "http2: HPACK dynamic table size update too large".to_string()
            }
            DecodeError::Malformed => "http2: HPACK decode failed".to_string(),
        })?;
        let st = self.streams.get_mut(&sid).ok_or("http2: no stream")?;
        st.headers = headers;
        st.headers_done = true;
        st.continuation_count = 0;
        Ok(())
    }

    fn try_emit(&mut self, sid: u32, feed: &mut Http2Feed) {
        let ready = self
            .streams
            .get(&sid)
            .map(|s| s.headers_done && s.end_stream && !s.emitted)
            .unwrap_or(false);
        if !ready {
            return;
        }
        let st = self.streams.get_mut(&sid).unwrap();
        st.emitted = true;
        feed.requests.push(Http2Request {
            stream_id: sid,
            headers: st.headers.clone(),
            body: std::mem::take(&mut st.body),
        });
        self.streams.remove(&sid);
    }

    fn goaway(&mut self, feed: &mut Http2Feed, _why: &str) -> Result<(), String> {
        self.goaway_code(feed, ERR_PROTOCOL_ERROR, _why)
    }

    fn goaway_code(&mut self, feed: &mut Http2Feed, code: u32, _why: &str) -> Result<(), String> {
        if !self.goaway_sent {
            feed.outbound
                .extend_from_slice(&encode_goaway(self.last_stream_id, code, b""));
            self.goaway_sent = true;
        }
        Ok(())
    }

    pub fn respond(
        &mut self,
        stream_id: u32,
        status: u16,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Vec<u8> {
        let mut header_list = Vec::with_capacity(1 + headers.len());
        header_list.push((":status".to_string(), status.to_string()));
        header_list.extend_from_slice(headers);
        let block = encode_header_block(&header_list);
        let no_body = body.is_empty();
        let mut out = encode_headers(stream_id, &block, no_body, true);
        self.streams.remove(&stream_id);
        if no_body {
            return out;
        }
        self.send.insert(
            stream_id,
            SendState {
                pending: body.to_vec(),
                window: self.peer_initial_window,
            },
        );
        self.flush_stream(stream_id, &mut out);
        out
    }

    fn flush_stream(&mut self, sid: u32, out: &mut Vec<u8>) {
        let max_frame = self.peer_max_frame_size as i64;
        loop {
            let ss = match self.send.get_mut(&sid) {
                Some(s) => s,
                None => return,
            };
            if ss.pending.is_empty() {
                self.send.remove(&sid);
                return;
            }
            let allowed = self
                .conn_send_window
                .min(ss.window)
                .min(max_frame)
                .min(ss.pending.len() as i64);
            if allowed <= 0 {
                return;
            }
            let chunk: Vec<u8> = ss.pending.drain(0..allowed as usize).collect();
            let end = ss.pending.is_empty();
            out.extend_from_slice(&encode_data(sid, end, &chunk));
            self.conn_send_window -= allowed;
            ss.window -= allowed;
            if end {
                self.send.remove(&sid);
                return;
            }
        }
    }

    fn flush_all(&mut self, out: &mut Vec<u8>) {
        let sids: Vec<u32> = self.send.keys().copied().collect();
        for sid in sids {
            self.flush_stream(sid, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_http2_codec::{
        decode_goaway, encode_frame, encode_settings_frame, take_frame, FrameType,
        ERR_FLOW_CONTROL_ERROR, ERR_FRAME_SIZE_ERROR, ERR_PROTOCOL_ERROR, FLAG_END_HEADERS,
        FLAG_END_STREAM, SETTINGS_ENABLE_PUSH, SETTINGS_INITIAL_WINDOW_SIZE,
        SETTINGS_MAX_FRAME_SIZE,
    };
    use rusty_http2_hpack::{
        decode_header_block, encode_header_block, encode_integer, encode_string, DynamicTable,
    };

    fn client_headers(stream_id: u32, headers: &[(&str, &str)], end_stream: bool) -> Vec<u8> {
        let list: Vec<(String, String)> = headers
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect();
        let block = encode_header_block(&list);
        let mut flags = FLAG_END_HEADERS;
        if end_stream {
            flags |= FLAG_END_STREAM;
        }
        encode_frame(FrameType::Headers, flags, stream_id, &block)
    }

    fn goaway_code(bytes: &[u8]) -> Option<u32> {
        let mut p = bytes;
        while let Some((f, n)) = take_frame(p) {
            if f.header.frame_type == FrameType::GoAway {
                return decode_goaway(&f.payload).map(|g| g.error_code);
            }
            p = &p[n..];
        }
        None
    }

    fn indexed_header_block_bomb() -> Vec<u8> {
        let mut block = vec![0x40];
        block.extend_from_slice(&encode_string(b"x-big"));
        block.extend_from_slice(&encode_string(&vec![b'a'; 2000]));
        for _ in 0..40 {
            block.extend_from_slice(&encode_integer(62, 7, 0x80));
        }
        block
    }

    #[test]
    fn brings_up_connection_and_extracts_get_request() {
        let mut conn = Http2Connection::new();

        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_settings_frame(&[]));
        let req = client_headers(
            1,
            &[
                (":method", "GET"),
                (":path", "/hello"),
                (":scheme", "https"),
                (":authority", "x"),
            ],
            true,
        );
        inbound.extend_from_slice(&req);

        let f0 = conn.feed(&inbound[..10]).unwrap();
        assert!(f0.requests.is_empty());

        let f1 = conn.feed(&inbound[10..]).unwrap();

        let mut p = &f1.outbound[..];
        let (s1, n1) = take_frame(p).unwrap();
        assert_eq!(s1.header.frame_type, FrameType::Settings);
        assert_eq!(s1.header.flags, 0);
        p = &p[n1..];
        let (s2, _) = take_frame(p).unwrap();
        assert_eq!(s2.header.frame_type, FrameType::Settings);
        assert_eq!(s2.header.flags, FLAG_ACK);

        assert_eq!(f1.requests.len(), 1);
        let r = &f1.requests[0];
        assert_eq!(r.stream_id, 1);
        assert_eq!(r.method(), Some("GET"));
        assert_eq!(r.path(), Some("/hello"));
        assert!(r.body.is_empty());
    }

    #[test]
    fn extracts_post_with_body_then_responds() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&client_headers(
            1,
            &[(":method", "POST"), (":path", "/up")],
            false,
        ));
        inbound.extend_from_slice(&encode_frame(
            FrameType::Data,
            FLAG_END_STREAM,
            1,
            b"payload",
        ));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(feed.requests.len(), 1);
        assert_eq!(feed.requests[0].method(), Some("POST"));
        assert_eq!(feed.requests[0].body, b"payload");

        let wire = conn.respond(
            1,
            200,
            &[("content-type".to_string(), "text/plain".to_string())],
            b"hi",
        );
        let (hf, n) = take_frame(&wire).unwrap();
        assert_eq!(hf.header.frame_type, FrameType::Headers);
        assert_eq!(hf.header.stream_id, 1);
        let mut dt = DynamicTable::new(4096);
        let decoded = decode_header_block(
            &rusty_http2_codec::header_block_fragment(&hf).unwrap(),
            &mut dt,
        )
        .unwrap();
        assert_eq!(decoded[0], (":status".to_string(), "200".to_string()));
        let (df, _) = take_frame(&wire[n..]).unwrap();
        assert_eq!(df.header.frame_type, FrameType::Data);
        assert_eq!(df.header.flags & FLAG_END_STREAM, FLAG_END_STREAM);
        assert_eq!(rusty_http2_codec::data_payload(&df).unwrap(), b"hi");
    }

    #[test]
    fn large_body_respects_frame_size_and_flow_control() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&client_headers(
            1,
            &[(":method", "GET"), (":path", "/big")],
            true,
        ));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(feed.requests.len(), 1);

        let body: Vec<u8> = (0..100_000u32).map(|i| i as u8).collect();
        let first = conn.respond(1, 200, &[], &body);

        let mut got = Vec::new();
        let mut p = &first[..];
        let mut last_end = false;
        while let Some((f, n)) = take_frame(p) {
            if f.header.frame_type == FrameType::Data {
                assert!(f.payload.len() <= 16_384, "DATA exceeds MAX_FRAME_SIZE");
                got.extend_from_slice(&rusty_http2_codec::data_payload(&f).unwrap());
                last_end = f.header.flags & FLAG_END_STREAM != 0;
            }
            p = &p[n..];
        }

        assert_eq!(got.len(), 65_535);
        assert!(!last_end);

        let mut wu = rusty_http2_codec::encode_window_update(0, 200_000);
        wu.extend_from_slice(&rusty_http2_codec::encode_window_update(1, 200_000));
        let more = conn.feed(&wu).unwrap();
        let mut p = &more.outbound[..];
        while let Some((f, n)) = take_frame(p) {
            if f.header.frame_type == FrameType::Data {
                assert!(f.payload.len() <= 16_384);
                got.extend_from_slice(&rusty_http2_codec::data_payload(&f).unwrap());
                last_end = f.header.flags & FLAG_END_STREAM != 0;
            }
            p = &p[n..];
        }
        assert_eq!(got, body);
        assert!(last_end);
    }

    #[test]
    fn ping_is_acked() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        let opaque = [9u8, 8, 7, 6, 5, 4, 3, 2];
        inbound.extend_from_slice(&rusty_http2_codec::encode_ping(&opaque));
        let feed = conn.feed(&inbound).unwrap();

        let mut p = &feed.outbound[..];
        let mut saw_ping_ack = false;
        while let Some((f, n)) = take_frame(p) {
            if f.header.frame_type == FrameType::Ping && f.header.flags & FLAG_ACK != 0 {
                assert_eq!(f.payload, opaque);
                saw_ping_ack = true;
            }
            p = &p[n..];
        }
        assert!(saw_ping_ack);
    }

    #[test]
    fn hpack_header_list_bomb_is_rejected() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_frame(
            FrameType::Headers,
            FLAG_END_HEADERS | FLAG_END_STREAM,
            1,
            &indexed_header_block_bomb(),
        ));
        assert!(conn
            .feed(&inbound)
            .unwrap_err()
            .contains("header list too large"));
    }

    #[test]
    fn hpack_dynamic_table_size_update_above_advertised_limit_is_rejected() {
        let mut conn = Http2Connection::new();
        let mut block = encode_integer(8192, 5, 0x20);
        block.push(0x82);
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_frame(
            FrameType::Headers,
            FLAG_END_HEADERS | FLAG_END_STREAM,
            1,
            &block,
        ));
        assert!(conn
            .feed(&inbound)
            .unwrap_err()
            .contains("dynamic table size update too large"));
    }

    #[test]
    fn continuation_flood_gets_enhance_your_calm_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_frame(FrameType::Headers, 0, 1, &[0x82]));
        for _ in 0..=SERVER_MAX_CONTINUATIONS_PER_BLOCK {
            inbound.extend_from_slice(&encode_frame(FrameType::Continuation, 0, 1, &[]));
        }
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_ENHANCE_YOUR_CALM));
    }

    #[test]
    fn concurrent_stream_flood_gets_enhance_your_calm_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        for i in 0..=SERVER_MAX_CONCURRENT_STREAMS {
            inbound.extend_from_slice(&client_headers(
                (i as u32 * 2) + 1,
                &[(":method", "GET")],
                false,
            ));
        }
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_ENHANCE_YOUR_CALM));
    }

    #[test]
    fn rapid_reset_flood_gets_enhance_your_calm_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        for i in 0..=SERVER_MAX_RST_STREAMS {
            inbound.extend_from_slice(&rusty_http2_codec::encode_rst_stream(
                (i as u32 * 2) + 1,
                rusty_http2_codec::ERR_CANCEL,
            ));
        }
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_ENHANCE_YOUR_CALM));
    }

    #[test]
    fn inbound_frame_larger_than_advertised_max_gets_frame_size_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_frame(
            FrameType::Data,
            0,
            1,
            &vec![0u8; DEFAULT_MAX_FRAME_SIZE as usize + 1],
        ));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_FRAME_SIZE_ERROR));
    }

    #[test]
    fn partial_inbound_frame_larger_than_advertised_max_gets_frame_size_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        let frame = encode_frame(
            FrameType::Data,
            0,
            1,
            &vec![0u8; DEFAULT_MAX_FRAME_SIZE as usize + 1],
        );
        inbound.extend_from_slice(&frame[..9]);
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_FRAME_SIZE_ERROR));
    }

    #[test]
    fn malformed_settings_get_frame_or_protocol_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_frame(FrameType::Settings, 0, 0, &[0, 1, 0, 0, 0]));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_FRAME_SIZE_ERROR));

        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_settings_frame(&[Setting {
            id: SETTINGS_ENABLE_PUSH,
            value: 2,
        }]));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_PROTOCOL_ERROR));

        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_settings_frame(&[Setting {
            id: SETTINGS_MAX_FRAME_SIZE,
            value: 16_383,
        }]));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_PROTOCOL_ERROR));

        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&encode_settings_frame(&[Setting {
            id: SETTINGS_INITIAL_WINDOW_SIZE,
            value: 0x8000_0000,
        }]));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_FLOW_CONTROL_ERROR));
    }

    #[test]
    fn window_update_zero_and_overflow_are_rejected() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&rusty_http2_codec::encode_window_update(0, 0));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_PROTOCOL_ERROR));

        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&rusty_http2_codec::encode_window_update(0, 0x7FFF_0000));
        inbound.extend_from_slice(&rusty_http2_codec::encode_window_update(0, 0x7FFF_0000));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_FLOW_CONTROL_ERROR));
    }

    #[test]
    fn inbound_data_exceeding_receive_window_gets_flow_control_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&client_headers(
            1,
            &[(":method", "POST"), (":path", "/up")],
            false,
        ));
        for _ in 0..=64 {
            inbound.extend_from_slice(&encode_frame(FrameType::Data, 0, 1, &vec![b'a'; 16_384]));
        }
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(goaway_code(&feed.outbound), Some(ERR_FLOW_CONTROL_ERROR));
    }

    #[test]
    fn data_on_closed_stream_gets_stream_closed_goaway() {
        let mut conn = Http2Connection::new();
        let mut inbound = CONNECTION_PREFACE.to_vec();
        inbound.extend_from_slice(&client_headers(
            1,
            &[(":method", "GET"), (":path", "/done")],
            true,
        ));
        let feed = conn.feed(&inbound).unwrap();
        assert_eq!(feed.requests.len(), 1);

        let feed = conn
            .feed(&encode_frame(FrameType::Data, FLAG_END_STREAM, 1, b"late"))
            .unwrap();
        assert_eq!(
            goaway_code(&feed.outbound),
            Some(rusty_http2_codec::ERR_STREAM_CLOSED)
        );
    }
}
