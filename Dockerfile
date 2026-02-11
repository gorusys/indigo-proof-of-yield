# Reproducible build and run for indigo-proof-of-yield
FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./
COPY crates ./crates

RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/indigo-poy /usr/local/bin/indigo-poy

ENV INDIGO_POY_CACHE=/data/cache
ENV INDIGO_POY_REPORTS=/reports
VOLUME ["/data", "/reports"]

ENTRYPOINT ["/usr/local/bin/indigo-poy"]
CMD ["--help"]
