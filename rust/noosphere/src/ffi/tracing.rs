use noosphere_core::tracing::{initialize_tracing, NoosphereLog};
use safer_ffi::prelude::*;

// NOTE: You may be wondering why we are using `u32` here instead of a more
// compact representation. In case you are:
//
// When Swift bridges to a C enum, it defaults to [`UInt32`][1] representation
// for the variants [citation][2].
//
// Strictly speaking, it is possible to configure the bit width of the
// representation that will be used via a specialty [`NS_ENUM`][3] macro.
// However, this macro is built-for-purpose by Apple and Safer FFI has no
// knowledge of it. So, using it would require specialized header generation
// behavior for the Apple case, and that creates a lot of complexity and room
// for error on our part.
//
// TODO(#345): Add support for `NS_ENUM` and use a more compact representation
// for enums in the future.
//
// [1]: https://developer.apple.com/documentation/swift/uint
// [2]: https://natecook.com/blog/2014/07/c-style-typedef-enums-in-swift/#bridged-enums
// [3]: https://nshipster.com/ns_enum-ns_options/

const NOOSPHERE_LOG_SILENT: u32 = 0;
const NOOSPHERE_LOG_BASIC: u32 = 1;
const NOOSPHERE_LOG_CHATTY: u32 = 2;
const NOOSPHERE_LOG_INFORMED: u32 = 3;
const NOOSPHERE_LOG_ACADEMIC: u32 = 4;
const NOOSPHERE_LOG_TIRESOME: u32 = 5;
const NOOSPHERE_LOG_DEAFENING: u32 = 6;

#[derive_ReprC(rename = "ns_noosphere_log")]
#[repr(u32)]
#[ffi_export]
/// Configuration presets for Noosphere log behavior. Only intended for use with
/// ns_tracing_initialize.
pub enum NsNoosphereLog {
    /// Equivalent to minimal format / `OFF` filter
    Silent = NOOSPHERE_LOG_SILENT,
    /// Equivalent to minimal format / `INFO` filter
    Basic = NOOSPHERE_LOG_BASIC,
    /// Equivalent to minimal format / `DEBUG` filter
    Chatty = NOOSPHERE_LOG_CHATTY,
    /// Equivalent to verbose format / `DEBUG` filter
    Informed = NOOSPHERE_LOG_INFORMED,
    /// Equivalent to pretty format / `DEBUG` filter
    Academic = NOOSPHERE_LOG_ACADEMIC,
    /// Equivalent to verbose format / `TRACE` filter
    Tiresome = NOOSPHERE_LOG_TIRESOME,
    /// Equivalent to pretty format / `TRACE` filter
    Deafening = NOOSPHERE_LOG_DEAFENING,
}

impl From<NsNoosphereLog> for NoosphereLog {
    fn from(log: NsNoosphereLog) -> Self {
        match log {
            NsNoosphereLog::Silent => NoosphereLog::Silent,
            NsNoosphereLog::Basic => NoosphereLog::Basic,
            NsNoosphereLog::Chatty => NoosphereLog::Chatty,
            NsNoosphereLog::Informed => NoosphereLog::Informed,
            NsNoosphereLog::Academic => NoosphereLog::Academic,
            NsNoosphereLog::Tiresome => NoosphereLog::Tiresome,
            NsNoosphereLog::Deafening => NoosphereLog::Deafening,
        }
    }
}

impl From<u32> for NsNoosphereLog {
    fn from(num: u32) -> Self {
        match num {
            NOOSPHERE_LOG_SILENT => NsNoosphereLog::Silent,
            NOOSPHERE_LOG_BASIC => NsNoosphereLog::Basic,
            NOOSPHERE_LOG_CHATTY => NsNoosphereLog::Chatty,
            NOOSPHERE_LOG_INFORMED => NsNoosphereLog::Informed,
            NOOSPHERE_LOG_ACADEMIC => NsNoosphereLog::Academic,
            NOOSPHERE_LOG_TIRESOME => NsNoosphereLog::Tiresome,
            _ => NsNoosphereLog::Deafening,
        }
    }
}

#[ffi_export]
/// Initialize log output for Noosphere-related code
pub fn ns_tracing_initialize(configuration: u32) {
    initialize_tracing(Some(NsNoosphereLog::from(configuration).into()));
}
