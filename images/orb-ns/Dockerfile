FROM docker.io/rust:1.75 as builder

WORKDIR /noosphere

COPY "./Cargo.toml" "./Cargo.lock" .
COPY ./rust ./rust

RUN apt-get update && apt-get install -y libssl-dev protobuf-compiler cmake jq binaryen
RUN cargo build --release

FROM docker.io/ubuntu:latest

RUN useradd -ms /bin/bash dhtuser
USER dhtuser 
WORKDIR /home/dhtuser

ENV RUST_LOG="info,tower_http,noosphere,noosphere_ns,noosphere_ipfs,noosphere_storage,noosphere_core,noosphere_storage,orb_ns=debug"

COPY --from=builder /noosphere/target/release/orb-ns /usr/bin/orb-ns
COPY --chown=dhtuser:dhtuser ./images/orb-ns/start.sh /home/dhtuser/start.sh

RUN chmod +x /home/dhtuser/start.sh

ENTRYPOINT ["/home/dhtuser/start.sh"]
