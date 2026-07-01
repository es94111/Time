# 多階段建置（research.md R8）：builder 使用官方 Rust slim 映像編譯 release 二進位，
# runtime 使用精簡的 debian slim，僅複製編譯後二進位與必要憑證，縮小映像體積與攻擊面。

FROM rust:1-slim AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock* ./
COPY migrations ./migrations
COPY src ./src
RUN cargo build --release

FROM debian:stable-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/server /app/server
COPY migrations /app/migrations
EXPOSE 8080
ENTRYPOINT ["/app/server"]
