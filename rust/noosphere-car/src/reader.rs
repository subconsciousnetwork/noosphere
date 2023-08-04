use cid::Cid;
use futures::Stream;
use tokio::io::AsyncRead;

use crate::{
    error::Error,
    header::CarHeader,
    util::{ld_read, read_node},
};

#[cfg(not(target_arch = "wasm32"))]
pub trait CarReaderSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<S> CarReaderSend for S where S: Send {}

#[cfg(target_arch = "wasm32")]
pub trait CarReaderSend {}

#[cfg(target_arch = "wasm32")]
impl<S> CarReaderSend for S {}

/// Reads CAR files that are in a BufReader
#[derive(Debug)]
#[deprecated(note = "`noosphere-car` is deprecated. Please use `iroh-car`.")]
pub struct CarReader<R> {
    reader: R,
    header: CarHeader,
    buffer: Vec<u8>,
}

impl<R> CarReader<R>
where
    R: AsyncRead + CarReaderSend + Unpin,
{
    /// Creates a new CarReader and parses the CarHeader
    pub async fn new(mut reader: R) -> Result<Self, Error> {
        let mut buffer = Vec::new();

        match ld_read(&mut reader, &mut buffer).await? {
            Some(buf) => {
                let header = CarHeader::decode(buf)?;

                Ok(CarReader {
                    reader,
                    header,
                    buffer,
                })
            }
            None => Err(Error::Parsing(
                "failed to parse uvarint for header".to_string(),
            )),
        }
    }

    /// Returns the header of this car file.
    pub fn header(&self) -> &CarHeader {
        &self.header
    }

    /// Returns the next IPLD Block in the buffer
    pub async fn next_block(&mut self) -> Result<Option<(Cid, Vec<u8>)>, Error> {
        read_node(&mut self.reader, &mut self.buffer).await
    }

    pub fn stream(self) -> impl Stream<Item = Result<(Cid, Vec<u8>), Error>> {
        futures::stream::try_unfold(self, |mut this| async move {
            let maybe_block = read_node(&mut this.reader, &mut this.buffer).await?;
            Ok(maybe_block.map(|b| (b, this)))
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{header::CarHeaderV1, writer::CarWriter};
    use cid::{
        multihash::{Code, MultihashDigest},
        Cid,
    };
    use futures::TryStreamExt;
    use libipld_cbor::DagCborCodec;

    use super::*;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn car_write_read() {
        let digest_test = Code::Blake3_256.digest(b"test");
        let cid_test = Cid::new_v1(DagCborCodec.into(), digest_test);

        let digest_foo = Code::Blake3_256.digest(b"foo");
        let cid_foo = Cid::new_v1(DagCborCodec.into(), digest_foo);

        let header = CarHeader::V1(CarHeaderV1::from(vec![cid_foo]));

        let mut buffer = Vec::new();
        let mut writer = CarWriter::new(header, &mut buffer);
        writer.write(cid_test, b"test").await.unwrap();
        writer.write(cid_foo, b"foo").await.unwrap();
        writer.finish().await.unwrap();

        let reader = Cursor::new(&buffer);
        let car_reader = CarReader::new(reader).await.unwrap();
        let files: Vec<_> = car_reader.stream().try_collect().await.unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, cid_test);
        assert_eq!(files[0].1, b"test");
        assert_eq!(files[1].0, cid_foo);
        assert_eq!(files[1].1, b"foo");
    }
}
