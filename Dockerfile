FROM rust:1-buster as builder

WORKDIR /app
COPY . .

RUN cargo build --release --bin server

FROM debian:stable-slim
WORKDIR /app
COPY --from=builder /app/target/release/server /app
CMD ["/app/server"]