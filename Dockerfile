# syntax=docker/dockerfile:1
FROM rust:1.98-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends libpq-dev cmake clang && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY public ./public
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/build/target \
    cargo build --release --locked --jobs 1 --bin hagency-server \
    && cp target/release/hagency-server /build/hagency-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libpq5 ca-certificates gosu && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home hagency && mkdir -p /app/data && chown hagency:hagency /app/data
WORKDIR /app
COPY --from=build /build/hagency-server /usr/local/bin/hagency-server
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
EXPOSE 8088
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["--config", "/app/config.toml"]
