use std::sync::Arc;

use crate::{platform::PlatformStorage, wasm::SphereFile};
use js_sys::{Array, Function};
use noosphere_sphere::{
    HasMutableSphereContext, SphereContentRead, SphereContentWrite, SphereContext, SphereCursor,
    SphereWalker,
};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
/// A `SphereFs` is a view over the data of a `Sphere` that enables the user to
/// think of sphere content in terms of file system semantics. You can think of
/// the file system as having a non-hierarchical namespace, where each name
/// points to a particular file. You can read or write to any name using strings
/// or raw bytes.
pub struct SphereFs {
    #[wasm_bindgen(skip)]
    pub inner: SphereCursor<Arc<Mutex<SphereContext<PlatformStorage>>>, PlatformStorage>,
}

#[wasm_bindgen]
impl SphereFs {
    #[wasm_bindgen]
    /// Read a `SphereFile` from the sphere given a name in the namespace
    pub async fn read(&self, slug: String) -> Result<Option<SphereFile>, String> {
        let file = self
            .inner
            .read(&slug)
            .await
            .map_err(|error| format!("{:?}", error))?;

        Ok(file.map(|file| SphereFile {
            inner: file.boxed(),
            cursor: self.inner.clone(),
        }))
    }

    #[wasm_bindgen(js_name = "writeString")]
    /// Write content to a name in the sphere's namespace as a UTF-8 encoded
    /// string. Note that JavaScript strings are UTF-16, so in some cases the
    /// conversion to UTF-8 may be lossy. See [these string conversion
    /// notes](https://rustwasm.github.io/docs/wasm-bindgen/reference/types/str.html#utf-16-vs-utf-8)
    /// for specific details on how the conversion is performed and what to look
    /// out for. If the conversion leads to undesired loss, you may consider
    /// using the `write` method instead, which allows the caller to control the
    /// conversion and save the file as raw bytes.
    pub async fn write_string(
        &mut self,
        slug: String,
        content_type: String,
        value: String,
        additional_headers: Option<Array>,
    ) -> Result<String, String> {
        self.write(
            slug,
            content_type,
            value.as_bytes().to_vec(),
            additional_headers,
        )
        .await
    }

    #[wasm_bindgen]
    /// Write content to the sphere's namespace as raw bytes
    pub async fn write(
        &mut self,
        slug: String,
        content_type: String,
        value: Vec<u8>,
        additional_headers: Option<Array>,
    ) -> Result<String, String> {
        let additional_headers = self.convert_headers_representation(additional_headers);

        let version = self
            .inner
            .write(&slug, &content_type, value.as_slice(), additional_headers)
            .await
            .map_err(|error| format!("{:?}", error))?;

        Ok(version.to_string())
    }

    #[wasm_bindgen]
    /// Save the current state of the sphere. Note that `save` is not invoked
    /// automatically; in order to persist any number of writes to a given
    /// sphere, you must invoke `save` after you are done writing. The returned
    /// string is a base64-encoded [CID](https://cid.ipfs.io/) that refers to
    /// the version of the sphere after it has been saved.
    pub async fn save(&mut self, additional_headers: Option<Array>) -> Result<String, String> {
        let additional_headers = self.convert_headers_representation(additional_headers);

        self.inner
            .save(additional_headers)
            .await
            .map(|cid| cid.to_string())
            .map_err(|error| format!("{:?}", error))
    }

    #[wasm_bindgen]
    /// Stream the contents of a sphere. The `SphereFs` will invoke the given
    /// callback once for each name in the sphere's namespace. The callback will
    /// be passed a string name as the first argument, and the corresponding
    /// `SphereFile` as the second argument. The returned `Promise` will resolve
    /// after the callback has been invoked once for each entry in the sphere's
    /// namespace.
    pub async fn stream(&self, callback: Function) -> Result<(), String> {
        let stream = SphereWalker::from(&self.inner).into_content_stream();
        let this = JsValue::null();

        tokio::pin!(stream);

        while let Some(Ok((slug, file))) = stream.next().await {
            let file = file.boxed();
            let file = SphereFile {
                inner: file,
                cursor: self.inner.clone(),
            };
            let slug = JsValue::from(slug);

            let file = JsValue::from(file);

            callback
                .call2(&this, &slug, &file)
                .map_err(|error| format!("{:?}", error))?;
        }

        Ok(())
    }

    fn convert_headers_representation(
        &self,
        additional_headers: Option<Array>,
    ) -> Option<Vec<(String, String)>> {
        if let Some(headers) = additional_headers {
            let mut additional_headers = Vec::new();

            for value in headers.iter() {
                if Array::is_array(&value) {
                    let entry = Array::from(&value);

                    match (entry.get(0).as_string(), entry.get(1).as_string()) {
                        (Some(key), Some(value)) => additional_headers.push((key, value)),
                        _ => continue,
                    }
                }
            }

            Some(additional_headers)
        } else {
            None
        }
    }
}
