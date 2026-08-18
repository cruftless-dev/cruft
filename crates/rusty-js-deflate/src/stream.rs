use std::io::{self, Write};

use crate::{
    compressed_deflate, compressed_gzip_deflate, compressed_zlib_deflate, gunzip_with_limit,
    inflate_with_limit, zlib_inflate_with_limit,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompressionFormat {
    Gzip,
    Zlib,
    Raw,
}

impl CompressionFormat {
    pub fn from_algorithm(algo: &str) -> Option<Self> {
        match algo {
            "gzip" => Some(Self::Gzip),
            "deflate" => Some(Self::Zlib),
            "deflate-raw" => Some(Self::Raw),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct StreamCodec {
    format: CompressionFormat,
    decode: bool,
    input: Vec<u8>,
    finished: bool,
}

impl StreamCodec {
    pub fn new(format: CompressionFormat, decode: bool) -> Self {
        Self {
            format,
            decode,
            input: Vec::new(),
            finished: false,
        }
    }

    pub fn push(&mut self, bytes: &[u8]) -> io::Result<Vec<u8>> {
        if self.finished {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "compression stream already finished",
            ));
        }
        self.input.extend_from_slice(bytes);
        Ok(Vec::new())
    }

    pub fn finish(mut self) -> io::Result<Vec<u8>> {
        self.finish_inner(None)
    }

    pub fn finish_with_limit(mut self, max_output: usize) -> io::Result<Vec<u8>> {
        self.finish_inner(Some(max_output))
    }

    fn finish_inner(&mut self, max_output: Option<usize>) -> io::Result<Vec<u8>> {
        self.finished = true;
        if self.decode {
            match self.format {
                CompressionFormat::Gzip => {
                    gunzip_with_limit(&self.input, max_output.unwrap_or(crate::MAX_OUTPUT))
                }
                CompressionFormat::Zlib => {
                    zlib_inflate_with_limit(&self.input, max_output.unwrap_or(crate::MAX_OUTPUT))
                }
                CompressionFormat::Raw => {
                    inflate_with_limit(&self.input, max_output.unwrap_or(crate::MAX_OUTPUT))
                }
            }
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
        } else {
            Ok(match self.format {
                CompressionFormat::Gzip => compressed_gzip_deflate(&self.input),
                CompressionFormat::Zlib => compressed_zlib_deflate(&self.input),
                CompressionFormat::Raw => compressed_deflate(&self.input),
            })
        }
    }
}

impl Write for StreamCodec {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.push(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(format: CompressionFormat) {
        let mut enc = StreamCodec::new(format, false);
        assert!(enc.push(b"alpha ").unwrap().is_empty());
        assert!(enc.push(b"beta beta beta ").unwrap().is_empty());
        let compressed = enc.finish().unwrap();
        assert!(!compressed.is_empty());

        let mid = compressed.len() / 2;
        let mut dec = StreamCodec::new(format, true);
        assert!(dec.push(&compressed[..mid]).unwrap().is_empty());
        assert!(dec.push(&compressed[mid..]).unwrap().is_empty());
        let decoded = dec.finish().unwrap();
        assert_eq!(decoded, b"alpha beta beta beta ");
    }

    #[test]
    fn stream_adapter_round_trips_all_formats() {
        round_trip(CompressionFormat::Gzip);
        round_trip(CompressionFormat::Zlib);
        round_trip(CompressionFormat::Raw);
    }

    #[test]
    fn stream_finish_with_limit_rejects_decode_growth() {
        let encoded = crate::compressed_deflate(b"hello");
        let mut dec = StreamCodec::new(CompressionFormat::Raw, true);
        dec.push(&encoded).unwrap();
        let err = dec.finish_with_limit(4).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
