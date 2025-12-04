use crate::{Error, Result};
use lz4_flex::frame::{FrameDecoder, FrameEncoder};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use zstd::stream::{Decoder as ZstdDecoder, Encoder as ZstdEncoder};

const MAX_DECOMPRESSED_SIZE: usize = 1 * 1024 * 1024;

/// Compression primitives for Milestone F / P14 (payload compression support).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CompressionCodec {
    None,
    Lz4,
    Zstd,
}

impl CompressionCodec {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "none" => Some(Self::None),
            "lz4" => Some(Self::Lz4),
            "zstd" => Some(Self::Zstd),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionCodec::None => "none",
            CompressionCodec::Lz4 => "lz4",
            CompressionCodec::Zstd => "zstd",
        }
    }
}

pub fn compress(payload: &[u8], codec: CompressionCodec) -> Result<Vec<u8>> {
    match codec {
        CompressionCodec::None => Ok(payload.to_vec()),
        CompressionCodec::Lz4 => compress_lz4(payload),
        CompressionCodec::Zstd => compress_zstd(payload),
    }
}

pub fn decompress(payload: &[u8], codec: CompressionCodec) -> Result<Vec<u8>> {
    match codec {
        CompressionCodec::None => Ok(payload.to_vec()),
        CompressionCodec::Lz4 => decompress_lz4(payload),
        CompressionCodec::Zstd => decompress_zstd(payload),
    }
}

fn compress_lz4(payload: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = FrameEncoder::new(Vec::new());
    encoder
        .write_all(payload)
        .map_err(|err| Error::Parse(format!("lz4 compression failed: {err}")))?;
    encoder
        .finish()
        .map_err(|err| Error::Parse(format!("lz4 compression finish failed: {err}")))
}

fn compress_zstd(payload: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZstdEncoder::new(Vec::new(), 0)
        .map_err(|err| Error::Parse(format!("zstd init failed: {err}")))?;
    encoder
        .write_all(payload)
        .map_err(|err| Error::Parse(format!("zstd compression failed: {err}")))?;
    encoder
        .finish()
        .map_err(|err| Error::Parse(format!("zstd compression finish failed: {err}")))
}

fn decompress_lz4(payload: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = FrameDecoder::new(payload);
    read_to_vec_limited(&mut decoder)
}

fn decompress_zstd(payload: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZstdDecoder::new(payload)
        .map_err(|err| Error::Parse(format!("zstd decoder init failed: {err}")))?;
    read_to_vec_limited(&mut decoder)
}

fn read_to_vec_limited(reader: &mut impl Read) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(1024);
    let mut buffer = [0u8; 4096];

    loop {
        let bytes = reader
            .read(&mut buffer)
            .map_err(|err| Error::Parse(format!("decompression failed: {err}")))?;
        if bytes == 0 {
            break;
        }
        let remaining = MAX_DECOMPRESSED_SIZE - output.len();
        if bytes > remaining {
            return Err(Error::Parse(format!(
                "decompressed payload exceeded {} bytes limit",
                MAX_DECOMPRESSED_SIZE
            )));
        }
        output.extend_from_slice(&buffer[..bytes]);
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_codec_roundtrips() {
        let buf = b"hello world";
        let c = CompressionCodec::None;
        let compressed = compress(buf, c).unwrap();
        assert_eq!(compressed, buf);
        let decompressed = decompress(&compressed, c).unwrap();
        assert_eq!(decompressed, buf);
    }

    #[test]
    fn lz4_roundtrips() {
        let payload = b"the quick brown fox";
        let compressed = compress(payload, CompressionCodec::Lz4).unwrap();
        let decompressed = decompress(&compressed, CompressionCodec::Lz4).unwrap();
        assert_eq!(decompressed, payload);
    }

    #[test]
    fn zstd_roundtrips() {
        let payload = b"do not go gentle";
        let compressed = compress(payload, CompressionCodec::Zstd).unwrap();
        let decompressed = decompress(&compressed, CompressionCodec::Zstd).unwrap();
        assert_eq!(decompressed, payload);
    }

    #[test]
    fn decompress_limits_size() {
        let payload = vec![0u8; MAX_DECOMPRESSED_SIZE + 1];
        let compressed = compress(&payload, CompressionCodec::Zstd).unwrap();
        let err = decompress(&compressed, CompressionCodec::Zstd).unwrap_err();
        match err {
            Error::Parse(msg) => assert!(msg.contains("exceeded")),
            _ => panic!("expected parse error for oversize payload"),
        }
    }
}
