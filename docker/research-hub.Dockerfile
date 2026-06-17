FROM rust:1.85-slim AS build

WORKDIR /build/research_hub_mcp

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY research_hub_mcp/Cargo.toml research_hub_mcp/Cargo.lock ./
COPY research_hub_mcp/build.rs ./
COPY research_hub_mcp/src ./src
COPY research_hub_mcp/benches ./benches

RUN cargo build --release --bin rust-research-mcp

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /build/research_hub_mcp/target/release/rust-research-mcp /usr/local/bin/rust-research-mcp

EXPOSE 8091

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=5 \
  CMD curl -fsS http://127.0.0.1:8091/health || exit 1

CMD ["rust-research-mcp", "http", "--host", "0.0.0.0", "--port", "8091"]
