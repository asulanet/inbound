FROM rust:1.97-slim AS build
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --bin inbound-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /build/target/release/inbound-server /usr/local/bin/inbound-server
WORKDIR /data
ENV PORT=8080 EVERY_SECONDS=8 STORAGE_DIR=/data/storage
EXPOSE 8080
CMD ["inbound-server"]
