use anyhow::Result;
use cid::Cid;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::{reader::CarReaderSend, varint::read_varint_async};

use super::error::Error;

/// Maximum size that is used for single node.
pub(crate) const MAX_ALLOC: usize = 4 * 1024 * 1024;

pub(crate) async fn ld_read<R>(mut reader: R, buf: &mut Vec<u8>) -> Result<Option<&[u8]>, Error>
where
    R: AsyncRead + CarReaderSend + Unpin,
{
    let length: usize = match read_varint_async(&mut reader).await {
        Ok(len) => len,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Ok(None);
            }
            return Err(Error::Parsing(e.to_string()));
        }
    };

    if length > MAX_ALLOC {
        return Err(Error::LdReadTooLarge(length));
    }
    if length > buf.len() {
        buf.resize(length, 0);
    }

    reader
        .read_exact(&mut buf[..length])
        .await
        .map_err(|e| Error::Parsing(e.to_string()))?;

    Ok(Some(&buf[..length]))
}

pub(crate) async fn read_node<R>(
    buf_reader: &mut R,
    buf: &mut Vec<u8>,
) -> Result<Option<(Cid, Vec<u8>)>, Error>
where
    R: AsyncRead + CarReaderSend + Unpin,
{
    if let Some(buf) = ld_read(buf_reader, buf).await? {
        let mut cursor = std::io::Cursor::new(buf);
        let c = Cid::read_bytes(&mut cursor)?;
        let pos = cursor.position() as usize;

        return Ok(Some((c, buf[pos..].to_vec())));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use integer_encoding::VarIntAsyncWriter;
    use tokio::io::{AsyncWrite, AsyncWriteExt};

    use super::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    async fn ld_write<'a, W>(writer: &mut W, bytes: &[u8]) -> Result<(), Error>
    where
        W: AsyncWrite + Send + Unpin,
    {
        writer.write_varint_async(bytes.len()).await?;
        writer.write_all(bytes).await?;
        writer.flush().await?;
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn ld_read_write_good() {
        let mut buffer = Vec::<u8>::new();
        ld_write(&mut buffer, b"test bytes").await.unwrap();
        let reader = std::io::Cursor::new(buffer);

        let mut buffer = vec![1u8; 1024];
        let read = ld_read(reader, &mut buffer).await.unwrap().unwrap();
        assert_eq!(read, b"test bytes");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn ld_read_write_fail() {
        let mut buffer = Vec::<u8>::new();
        let size = MAX_ALLOC + 1;
        ld_write(&mut buffer, &vec![2u8; size]).await.unwrap();
        let reader = std::io::Cursor::new(buffer);

        let mut buffer = vec![1u8; 1024];
        let read = ld_read(reader, &mut buffer).await;
        assert!(matches!(read, Err(Error::LdReadTooLarge(_))));
    }
}
