use safer_ffi::prelude::*;

ReprC! {
    #[ReprC::opaque]
    pub struct NsHeaders {
        inner: Vec<(String, String)>
    }
}

impl NsHeaders {
    pub fn inner(&self) -> &Vec<(String, String)> {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut Vec<(String, String)> {
        &mut self.inner
    }
}

#[ffi_export]
/// Create a [NsHeaders] buffer for the purpose of building up a set of headers
/// intended to be added to a memo before it is written to a sphere
pub fn ns_headers_create() -> repr_c::Box<NsHeaders> {
    repr_c::Box::new(NsHeaders { inner: Vec::new() })
}

#[ffi_export]
/// Add a name/value pair to an [NsHeaders] buffer
pub fn ns_headers_add(headers: &mut NsHeaders, name: char_p::Ref<'_>, value: char_p::Ref<'_>) {
    headers.inner.push((name.to_string(), value.to_string()))
}

#[ffi_export]
/// De-allocate an [NsHeaders] buffer
pub fn ns_headers_free(headers: repr_c::Box<NsHeaders>) {
    drop(headers)
}
