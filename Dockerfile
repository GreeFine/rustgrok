FROM rust:1-buster as builder

WORKDIR /app

# Create fake file to pre-load dependencies
COPY Cargo.lock Cargo.toml /app/
RUN mkdir -p ./src/bin && \
    touch ./src/lib.rs ./src/bin/client.rs ./src/bin/server.rs && \
    cargo build --lib && \
    rm ./src/lib.rs ./src/bin/client.rs ./src/bin/server.rs

COPY . .
RUN cargo build --bin server --features ingress,deployed --release

FROM debian:stable-slim AS runtime
WORKDIR /app

RUN apt update && apt install -y ca-certificates && rm -rf /var/lib/apt/lists/* 
COPY --from=builder /app/target/release/server /app/server
ENTRYPOINT ["/app/server"]
