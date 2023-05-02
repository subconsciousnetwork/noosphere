use std::fmt::Display;

pub const API_VERSION: &str = "v0alpha1";

pub enum Route {
    NetworkInfo,
    GetPeerId,

    GetPeers,
    AddPeers,

    Listen,
    StopListening,
    Address,

    GetRecord,
    PostRecord,

    Bootstrap,
}

impl Display for Route {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fragment = match self {
            Route::NetworkInfo => "network_info",
            Route::GetPeerId => "peer_id",

            Route::GetPeers => "peers",
            Route::AddPeers => "peers/*addr",

            Route::Listen => "listen/*addr",
            Route::StopListening => "listen",
            Route::Address => "addresses",

            Route::GetRecord => "records/:identity",
            Route::PostRecord => "records",

            Route::Bootstrap => "bootstrap",
        };

        write!(f, "/api/{API_VERSION}/{fragment}")
    }
}
