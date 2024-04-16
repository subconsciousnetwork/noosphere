FROM docker.io/rust:1.75 as builder

WORKDIR /noosphere

COPY "./Cargo.toml" "./Cargo.lock" .
COPY ./rust ./rust

RUN apt-get update && apt-get install -y libssl-dev protobuf-compiler cmake jq binaryen
RUN cargo build --bin orb-ns --release

FROM docker.io/ubuntu:latest

RUN useradd -ms /bin/bash dhtuser
USER dhtuser 
WORKDIR /home/dhtuser

ENV RUST_LOG="info,orb_ns=debug"
ENV NOOSPHERE_LOG="academic"
ENV KEY_NAME="ephemeral"
ENV P2P_ADDRESS="0.0.0.0:6666"
ENV API_ADDRESS="0.0.0.0:6667"
ENV IPFS_URL="http://127.0.0.1:5001"

COPY --from=builder /noosphere/target/release/orb-ns /usr/bin/orb-ns
COPY --chown=dhtuser:dhtuser ./images/orb-ns/start.sh /home/dhtuser/start.sh

RUN chmod +x /home/dhtuser/start.sh

ENTRYPOINT ["/home/dhtuser/start.sh"]
