use async_trait::async_trait;
use std::str::FromStr;
use std::{pin::Pin, rc::Rc};
use tokio_stream::StreamExt;
use url::Url;

use anyhow::{anyhow, Result};
use js_sys::Function;
use js_sys::Promise;
use noosphere_fs::{SphereFile as SphereFileImpl, SphereFs};
use tokio::io::{AsyncRead, AsyncReadExt};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use noosphere_into::transformer::SubtextToHtmlTransformer;
use noosphere_into::transformer::{Link, SphereFsTranscluder};
use noosphere_into::transformer::{Resolver, Transformer};

use crate::platform::{PlatformKeyMaterial, PlatformStorage};

/// A `SphereFile` contains metadata about a file, and accessors that enable
/// the user to defer reading the file's content until such time as it is
/// needed.
#[wasm_bindgen]
pub struct SphereFile {
    #[wasm_bindgen(skip)]
    pub inner: SphereFileImpl<Pin<Box<dyn AsyncRead>>>,

    #[wasm_bindgen(skip)]
    pub fs: SphereFs<PlatformStorage, PlatformKeyMaterial>,
}

#[wasm_bindgen]
impl SphereFile {
    #[wasm_bindgen(js_name = "sphereIdentity")]
    pub fn sphere_identity(&self) -> String {
        self.inner.sphere_identity.to_string()
    }

    #[wasm_bindgen(js_name = "sphereVersion")]
    pub fn sphere_revision(&self) -> String {
        self.inner.sphere_revision.to_string()
    }

    #[wasm_bindgen(js_name = "intoText")]
    /// Asynchronously read the contents of the file, interpreting it as a
    /// UTF-8 encoded string.
    pub async fn into_text(mut self) -> Result<String, String> {
        let mut contents = String::new();

        self.inner
            .contents
            .read_to_string(&mut contents)
            .await
            .map_err(|error| format!("{:?}", error))?;

        Ok(contents)
    }

    #[wasm_bindgen(js_name = "intoHtml")]
    pub async fn into_html(self, resolver: Function) -> Result<String, String> {
        let resolver = JavaScriptResolver::new(resolver);
        let transcluder = SphereFsTranscluder::new(&self.fs);
        let transformer = SubtextToHtmlTransformer::new(resolver, transcluder);

        Ok(transformer.transform_file(self.inner).collect().await)
    }
}

#[derive(Clone)]
pub struct JavaScriptResolver {
    callback: Rc<Function>,
}

impl JavaScriptResolver {
    pub fn new(callback: Function) -> Self {
        JavaScriptResolver {
            callback: Rc::new(callback),
        }
    }
}

#[async_trait(?Send)]
impl Resolver for JavaScriptResolver {
    async fn resolve(&self, link: &Link) -> Result<url::Url> {
        let (link_string, link_kind) = match &link {
            Link::Hyperlink(url) => (url.to_string(), "hyperlink"),
            Link::Slashlink(slashlink) => (slashlink.to_string(), "slashlink"),
        };
        let this = JsValue::null();
        let arg1 = JsValue::from(link_string);
        let arg2 = JsValue::from(link_kind);

        let result = Promise::resolve(
            &self
                .callback
                .call2(&this, &arg1, &arg2)
                .map_err(|error| anyhow!("{:?}", error))?,
        );

        let resolver_resolves = JsFuture::from(result);
        let resolved_value = resolver_resolves
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        if let Some(string) = resolved_value.as_string() {
            Url::from_str(&string)
                .map_err(|error| anyhow!("Could not parse returned value as URL: {:?}", error))
        } else {
            Err(anyhow!("Expected a string, but got {:?}", resolved_value))
        }
    }
}
