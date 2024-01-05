//! This crate contains substantially all of the implementation of the Noosphere Gateway
//! and provides it as a re-usable library. It is the same implementation of the gateway
//! that is used by the Noosphere CLI.

#![cfg(not(target_arch = "wasm32"))]
#![warn(missing_docs)]

#[macro_use]
extern crate tracing;

mod error;
mod extractors;
mod gateway;
mod gateway_manager;
mod handlers;
mod single_tenant;
mod try_or_reset;
mod worker;

pub use gateway::*;
pub use gateway_manager::*;
pub use single_tenant::*;
/*
#[cfg(test)]
mod test {
    #[test]
    fn test_ucan() {
        use std::str::FromStr;
        let ucan_str = "eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJhdWQiOiJkaWQ6a2V5Ono2TWtzYTJuWUR1ZHFjTnFzZGRoUEdlRkFKamQ5V3VkamhwMnR2eTNFdjlwckd5dyIsImNhcCI6eyJzcGhlcmU6ZGlkOmtleTp6Nk1ram05VWlEem8ya0JjZXk3N2RoaWk3dXc2emhqU3RmSGhOdWpCQlJDeVI4dDciOnsic3BoZXJlL2ZldGNoIjpbe31dfX0sImV4cCI6MTcwNDQwMTAzMiwiaXNzIjoiZGlkOmtleTp6Nk1rZktteHF4SzNiRWRpVHg0aGdMTGVqdVpVc3R1WGJWcFZhcmp6aVVtRjJiMWkiLCJubmMiOiJfQXRWUW1tOThRQVdlLVBNWVRDUk1WYVpzMnNibU5iNEphVWcyVERlLWZjIiwicHJmIjpbImJhZmtyNGlkbjNtNjJ1Nno3bWFmcTdocWl6Y3h2dnBqbzdreXJlamRvdnllc2VzNHM1azJmdmlud2o0Il0sInVjdiI6IjAuMTAuMC1jYW5hcnkifQ.U4zTZigj5fYqJmWJjEveBKRR_R2IuHhv8dyNHFMTkPJ00z2r_Qnn9d8qJrT-TsU3VUz6M-CLTqxipa6jJ4URDw";
        let token = ucan::Ucan::from_str(ucan_str).unwrap();
        println!("TOKEN {:#?}", token);
        panic!("panic");
    }

    #[tokio::test]
    async fn test_axum() -> anyhow::Result<()> {
        let app = axum::Router::new().route(
            "/",
            axum::routing::get(|| async {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                "Hello, World!"
            }),
        );

        // run our app with hyper, listening globally on port 3000
        let listener = std::net::TcpListener::bind("0.0.0.0:3000")?;
        let handle = tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(app.into_make_service())
                .await
                .unwrap()
        });
        let client = reqwest::Client::new();
        let response = client
            .get(url::Url::parse("http://0.0.0.0:3000").unwrap())
            .send()
            .await
            .unwrap();
        println!("RESPONSE {}", response.status());
        handle.abort();
        Ok(())
    }
}
*/
