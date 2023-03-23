//! This module has been adapted from the [integer_encoding] crate. The
//! constructs here are mostly unchanged, except that the `Send` bound on the
//! async reader has been made optional in the case that we are compiling for
//! Wasm and deploying to a browser.

use std::{
    io::{Error as IoError, ErrorKind as IoErrorKind},
    mem::size_of,
};

use integer_encoding::VarInt;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::reader::CarReaderSend;

pub(crate) trait VarIntMaxSize {
    fn varint_max_size() -> usize;
}

impl<VI: VarInt> VarIntMaxSize for VI {
    fn varint_max_size() -> usize {
        (size_of::<VI>() * 8 + 7) / 7
    }
}

pub const MSB: u8 = 0b1000_0000;

/// VarIntProcessor encapsulates the logic for decoding a VarInt byte-by-byte.
#[derive(Default)]
pub struct VarIntProcessor {
    buf: [u8; 10],
    maxsize: usize,
    i: usize,
}

impl VarIntProcessor {
    fn new<VI: VarIntMaxSize>() -> VarIntProcessor {
        VarIntProcessor {
            maxsize: VI::varint_max_size(),
            ..VarIntProcessor::default()
        }
    }
    fn push(&mut self, b: u8) -> Result<(), IoError> {
        if self.i >= self.maxsize {
            return Err(IoError::new(
                IoErrorKind::InvalidData,
                "Unterminated varint",
            ));
        }
        self.buf[self.i] = b;
        self.i += 1;
        Ok(())
    }
    fn finished(&self) -> bool {
        self.i > 0 && (self.buf[self.i - 1] & MSB == 0)
    }
    fn decode<VI: VarInt>(&self) -> Option<VI> {
        Some(VI::decode_var(&self.buf[0..self.i])?.0)
    }
}

pub async fn read_varint_async<V, R>(reader: &mut R) -> Result<V, std::io::Error>
where
    V: VarInt,
    R: AsyncRead + CarReaderSend + Unpin,
{
    let mut read_buffer = [0 as u8; 1];
    let mut p = VarIntProcessor::new::<V>();

    while !p.finished() {
        let read = reader.read(&mut read_buffer).await?;

        // EOF
        if read == 0 && p.i == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "Reached EOF",
            ));
        }
        if read == 0 {
            break;
        }

        p.push(read_buffer[0])?;
    }

    p.decode()
        .ok_or_else(|| IoError::new(IoErrorKind::UnexpectedEof, "Reached EOF"))
}
