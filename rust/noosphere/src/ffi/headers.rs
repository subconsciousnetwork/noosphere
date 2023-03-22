use safer_ffi::prelude::*;

#[derive_ReprC(rename = "ns_headers")]
#[repr(opaque)]
/// @class ns_headers_t
/// An opaque struct representing name/value headers.
///
/// Headers are used in ns_sphere_file_t to assign metadata, like content type.
pub struct NsHeaders {
    inner: Vec<(String, String)>,
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
/// @memberof ns_headers_t
/// Allocate and initialize a ns_headers_t instance with no values.
///
/// Used for the purpose of building up a set of headers
/// intended to be added to a memo before it is written to a sphere
pub fn ns_headers_create() -> repr_c::Box<NsHeaders> {
    Box::new(NsHeaders { inner: Vec::new() }).into()
}

#[ffi_export]
/// @memberof ns_headers_t
/// Add a name/value pair to a ns_headers_t instance.
pub fn ns_headers_add(headers: &mut NsHeaders, name: char_p::Ref<'_>, value: char_p::Ref<'_>) {
    headers.inner.push((name.to_string(), value.to_string()))
}

#[ffi_export]
/// @memberof ns_headers_t
/// Deallocate a ns_headers_t instance.
pub fn ns_headers_free(headers: repr_c::Box<NsHeaders>) {
    drop(headers)
}
