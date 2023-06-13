//! These constants represent the metadata keys used when a [SphereContext] is
//! is initialized. Since these represent somewhat free-form key/values in the
//! storage layer, we are make a best effort to document them here.

#[cfg(doc)]
use crate::SphereContext;

#[cfg(doc)]
use noosphere_core::data::Did;

#[cfg(doc)]
use cid::Cid;

#[cfg(doc)]
use url::Url;

/// A key that corresponds to the sphere's identity, which is represented by a
/// [Did] when it is set.
pub const IDENTITY: &str = "identity";

/// A name that corresponds to the locally available key. This name is
/// represented as a string, and should match a credential ID for a key in
/// whatever the supported platform key storage is.
pub const USER_KEY_NAME: &str = "user_key_name";

/// The [Cid] of a UCAN JWT that authorizes the configured user key to access
/// the sphere.
pub const AUTHORIZATION: &str = "authorization";

/// The base [Url] of a Noosphere Gateway API that will allow this sphere to
/// sync with it.
pub const GATEWAY_URL: &str = "gateway_url";

/// The counterpart sphere [Did] that either tracks or is tracked by this
/// sphere.
pub const COUNTERPART: &str = "counterpart";
