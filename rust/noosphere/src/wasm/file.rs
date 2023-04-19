use async_trait::async_trait;
use noosphere_core::data::Header;
use noosphere_into::{
    file_to_html_stream, HtmlOutput, ResolvedLink, Resolver, SphereContentTranscluder, Transform,
};
use std::{pin::Pin, rc::Rc, sync::Arc};
use subtext::Slashlink;
use tokio_stream::StreamExt;

use anyhow::{anyhow, Result};
use js_sys::{Function, Promise, Uint8Array};
use noosphere_sphere::{
    AsyncFileBody, HasSphereContext, SphereContext, SphereCursor, SphereFile as SphereFileImpl,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    sync::Mutex,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::platform::{PlatformKeyMaterial, PlatformStorage};

/// A `SphereFile` contains metadata about a file, and accessors that enable
/// the user to defer reading the file's content until such time as it is
/// needed.
#[wasm_bindgen]
pub struct SphereFile {
    #[wasm_bindgen(skip)]
    pub inner: SphereFileImpl<Pin<Box<dyn AsyncFileBody>>>,

    #[wasm_bindgen(skip)]
    pub cursor: SphereCursor<
        Arc<Mutex<SphereContext<PlatformKeyMaterial, PlatformStorage>>>,
        PlatformKeyMaterial,
        PlatformStorage,
    >,
}

#[wasm_bindgen]
impl SphereFile {
    #[wasm_bindgen(js_name = "sphereIdentity")]
    /// Get the DID of the sphere that this file was read from
    pub fn sphere_identity(&self) -> String {
        self.inner.sphere_identity.to_string()
    }

    #[wasm_bindgen(js_name = "sphereVersion")]
    /// Get the version of the sphere that this file was read from as a base32
    /// [CID](https://docs.ipfs.tech/concepts/content-addressing/#identifier-formats)
    /// string
    pub fn sphere_version(&self) -> String {
        self.inner.sphere_version.to_string()
    }

    #[wasm_bindgen(js_name = "memoVersion")]
    /// Get the version of the memo that wraps this file's contents as a base32
    /// [CID](https://docs.ipfs.tech/concepts/content-addressing/#identifier-formats)
    /// string
    pub fn memo_version(&self) -> String {
        self.inner.memo_version.to_string()
    }

    #[wasm_bindgen(js_name = "contentType")]
    /// Get the MIME that is specified in the 'Content-Type' header of the
    /// memo that wraps this file's contents, if one is specified
    pub fn content_type(&self) -> Option<String> {
        self.inner
            .memo
            .get_first_header(&Header::ContentType.to_string())
    }

    #[wasm_bindgen(js_name = "getFirstHeader")]
    /// Get the first header in the memo that wraps this file's content that
    /// matches the given name
    pub fn get_first_header(&self, name: String) -> Option<String> {
        self.inner.memo.get_first_header(&name)
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
    /// Consume this SphereFile and return its contents formatted as HTML. A
    /// resolver function must be provided in order to convert slashlinks to
    /// hypertext links according to the caller's use case.
    ///
    /// Note that after this method is called, the SphereFile will be freed and
    /// is no longer usable.
    pub async fn into_html(self, resolver: Function) -> Result<String, String> {
        let transform = JavaScriptTransform::new(resolver, self.cursor.clone());
        Ok(
            file_to_html_stream(self.inner, HtmlOutput::Fragment, transform)
                .collect()
                .await,
        )
    }

    #[wasm_bindgen(js_name = "intoBytes")]
    /// Consume this SphereFile and return its contents as a raw byte array.
    ///
    /// Note that after this method is called, the SphereFile will be freed and
    /// is no longer usable.
    pub async fn into_bytes(mut self) -> Result<Uint8Array, String> {
        let mut bytes = Vec::new();

        self.inner
            .contents
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| format!("{:?}", error))?;

        let js_array = Uint8Array::new(&bytes.len().into());
        js_array.copy_from(&bytes);

        Ok(js_array)
    }
}

/// A [JavaScriptTransform] is a [Transform] implementation that is suitable
/// for converting sphere content using JavaScript and DOM-aware APIs.
#[derive(Clone)]
pub struct JavaScriptTransform<R: HasSphereContext<PlatformKeyMaterial, PlatformStorage>> {
    resolver: JavaScriptResolver,
    transcluder: SphereContentTranscluder<R, PlatformKeyMaterial, PlatformStorage>,
}

impl<R> JavaScriptTransform<R>
where
    R: HasSphereContext<PlatformKeyMaterial, PlatformStorage>,
{
    pub fn new(callback: Function, context: R) -> Self {
        JavaScriptTransform {
            resolver: JavaScriptResolver::new(callback),
            transcluder: SphereContentTranscluder::new(context),
        }
    }
}

impl<R> Transform for JavaScriptTransform<R>
where
    R: HasSphereContext<PlatformKeyMaterial, PlatformStorage>,
{
    type Resolver = JavaScriptResolver;
    type Transcluder = SphereContentTranscluder<R, PlatformKeyMaterial, PlatformStorage>;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }

    fn transcluder(&self) -> &Self::Transcluder {
        &self.transcluder
    }
}

#[derive(Clone)]
/// A [JavaScriptResolver] is a [Resolver] that can resolve slashlinks using
/// a function provided by a JavaScript caller.
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
    async fn resolve(&self, link: &Slashlink) -> Result<ResolvedLink> {
        let this = JsValue::null();
        // TODO(#185): We should try to send a parsed slashlink so that the JS
        // doesn't need its own slashlink parsers
        let arg1 = JsValue::from(link.to_string());

        let result = Promise::resolve(
            &self
                .callback
                .call1(&this, &arg1)
                .map_err(|error| anyhow!("{:?}", error))?,
        );

        let resolver_resolves = JsFuture::from(result);
        let resolved_value = resolver_resolves
            .await
            .map_err(|error| anyhow!("{:?}", error))?;

        if let Some(href) = resolved_value.as_string() {
            Ok(ResolvedLink::Slashlink {
                link: link.clone(),
                href,
            })
        } else {
            Err(anyhow!("Expected a string, but got {:?}", resolved_value))
        }
    }
}
