FROM rust:1.65 as builder

RUN mkdir -p /workspace/rust
WORKDIR /workspace

COPY "./Cargo.toml" "./Cargo.lock" .
COPY ./rust ./rust

RUN apt-get update && apt-get install -y libssl-dev protobuf-compiler cmake jq binaryen
RUN cargo build --release

FROM ubuntu:latest

RUN mkdir -p /root/.noosphere
RUN mkdir -p /root/sphere

VOLUME ["/root/.noosphere", "/root/sphere"]
EXPOSE 4433

COPY --from=builder /workspace/rust/target/release/orb /usr/bin/orb
COPY ./images/orb/start.sh /start.sh

RUN chmod +x /start.sh

ENTRYPOINT ["/start.sh"]