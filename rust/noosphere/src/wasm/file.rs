use std::pin::Pin;

use noosphere_fs::SphereFile as SphereFileImpl;
use tokio::io::{AsyncRead, AsyncReadExt};
use wasm_bindgen::prelude::*;

/// A `SphereFile` contains metadata about a file, and accessors that enable
/// the user to defer reading the file's content until such time as it is
/// needed.
#[wasm_bindgen]
pub struct SphereFile {
    #[wasm_bindgen(skip)]
    pub inner: SphereFileImpl<Pin<Box<dyn AsyncRead>>>,
}

#[wasm_bindgen]
impl SphereFile {
    #[wasm_bindgen]
    /// Asynchronously read the contents of the file, interpreting it as a
    /// UTF-8 encoded string.
    pub async fn text(&mut self) -> Result<String, String> {
        let mut contents = String::new();

        self.inner
            .contents
            .read_to_string(&mut contents)
            .await
            .map_err(|error| format!("{:?}", error))?;

        Ok(contents)
    }
}
