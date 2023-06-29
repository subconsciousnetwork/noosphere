use crate::error::NoosphereError;
use safer_ffi::prelude::*;

impl From<NoosphereError> for repr_c::Box<NsError> {
    fn from(error: NoosphereError) -> Self {
        Box::new(NsError { inner: error }).into()
    }
}

impl From<std::io::Error> for NoosphereError {
    fn from(value: std::io::Error) -> Self {
        NoosphereError::Other(value.into())
    }
}

const NOOSPHERE_ERROR_OTHER: u32 = 1;
const NOOSPHERE_ERROR_NETWORK_OFFLINE: u32 = 2;
const NOOSPHERE_ERROR_NO_CREDENTIALS: u32 = 3;
const NOOSPHERE_ERROR_MISSING_CONFIGURATION: u32 = 4;

#[ffi_export]
#[derive_ReprC(rename = "ns_error_code")]
#[repr(u32)]
/// Constant values for error codes from ns_error_t.
pub enum NsErrorCode {
    Other = NOOSPHERE_ERROR_OTHER,
    NetworkOffline = NOOSPHERE_ERROR_NETWORK_OFFLINE,
    NoCredentials = NOOSPHERE_ERROR_NO_CREDENTIALS,
    MissingConfiguration = NOOSPHERE_ERROR_MISSING_CONFIGURATION,
}

impl From<u32> for NsErrorCode {
    fn from(code: u32) -> Self {
        match code {
            NOOSPHERE_ERROR_OTHER => NsErrorCode::Other,
            NOOSPHERE_ERROR_NETWORK_OFFLINE => NsErrorCode::NetworkOffline,
            NOOSPHERE_ERROR_NO_CREDENTIALS => NsErrorCode::NoCredentials,
            NOOSPHERE_ERROR_MISSING_CONFIGURATION => NsErrorCode::MissingConfiguration,
            _ => NsErrorCode::Other,
        }
    }
}

impl From<&NoosphereError> for NsErrorCode {
    fn from(error: &NoosphereError) -> Self {
        match error {
            NoosphereError::Other(_) => NsErrorCode::Other,
            NoosphereError::NetworkOffline => NsErrorCode::NetworkOffline,
            NoosphereError::NoCredentials => NsErrorCode::NoCredentials,
            NoosphereError::MissingConfiguration(_) => NsErrorCode::MissingConfiguration,
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
pub fn ns_error_message_get(error: &NsError) -> char_p::Box {
    error
        .inner
        .to_string()
        .try_into()
        .unwrap_or_else(|_| char_p::new("Unknown"))
}

#[ffi_export]
/// @memberof ns_error_t
/// Returns an error code that identifies the error.
pub fn ns_error_code_get(error: &NsError) -> u32 {
    NsErrorCode::from(&error.inner) as u32
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
