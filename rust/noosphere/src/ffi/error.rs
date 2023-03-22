use safer_ffi::prelude::*;

use crate::error::NoosphereError;

impl From<NoosphereError> for repr_c::Box<NsError> {
    fn from(error: NoosphereError) -> Self {
        Box::new(NsError { inner: error }).into()
    }
}

/// A trait to help with late initialization of otherwise uninitialized
/// out error values.
pub trait TryOrInitialize<E>: Sized
where
    E: From<Self::InnerError>,
{
    type InnerError;

    /// Invoke the given closure, returning Some(T) on an Ok result or else
    /// None. In an Err condition, the late initialization method will be
    /// invoked with the lazily created value to initialize with.
    fn try_or_initialize<T>(
        self,
        closure: impl FnOnce() -> Result<T, Self::InnerError>,
    ) -> Option<T> {
        match closure() {
            Ok(value) => Some(value),
            Err(error) => {
                self.late_initialize(E::from(error));
                None
            }
        }
    }

    fn late_initialize(self, error: E);
}

impl TryOrInitialize<repr_c::Box<NsError>> for Option<Out<'_, repr_c::Box<NsError>>> {
    type InnerError = NoosphereError;

    fn late_initialize(self, error: repr_c::Box<NsError>) {
        if let Some(out_error) = self {
            out_error.write(error);
        }
    }
}

#[derive_ReprC(rename = "ns_error")]
#[repr(opaque)]
/// @class ns_error_t
/// An opaque struct representing an error.
pub struct NsError {
    inner: NoosphereError,
}

#[ffi_export]
/// @memberof ns_error_t
/// Deallocate an ns_error_t.
pub fn ns_error_free(error: repr_c::Box<NsError>) {
    drop(error)
}

#[ffi_export]
/// @memberof ns_error_t
/// Returns an owned string describing the error in greater detail.
///
/// Caller is responsible for deallocating returned string via ns_string_free.
pub fn ns_error_string(error: &NsError) -> char_p::Box {
    error
        .inner
        .to_string()
        .try_into()
        .unwrap_or_else(|_| char_p::new("Unknown"))
}
