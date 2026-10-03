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

# Build the Dioxus WASM app with the same pinned Pasion source as the backend.
FROM rust:1.98-trixie AS pasion-assets
ARG TARGETARCH
ARG PASION_REV=b8333e1ee60c6362847d0f1890461b7af7547037
RUN apt-get update && apt-get install -y --no-install-recommends curl python3 nodejs && rm -rf /var/lib/apt/lists/*
RUN rustup target add wasm32-unknown-unknown
RUN case "$TARGETARCH" in arm64) dx_arch=aarch64 ;; amd64) dx_arch=x86_64 ;; *) exit 1 ;; esac \
    && curl -fsSL "https://github.com/DioxusLabs/dioxus/releases/download/v0.7.5/dx-$dx_arch-unknown-linux-gnu.tar.gz" -o /tmp/dx.tar.gz \
    && curl -fsSL "https://github.com/DioxusLabs/dioxus/releases/download/v0.7.5/dx-$dx_arch-unknown-linux-gnu.sha256" -o /tmp/dx.sha256 \
    && sed -n '1s/  .*/  \/tmp\/dx.tar.gz/p' /tmp/dx.sha256 | sha256sum -c - \
    && tar -xzf /tmp/dx.tar.gz -C /usr/local/bin && rm /tmp/dx.tar.gz /tmp/dx.sha256
RUN git init /pasion && git -C /pasion remote add origin https://github.com/meldry-com/pasion.git \
    && git -C /pasion fetch --depth 1 origin "$PASION_REV" && git -C /pasion checkout --detach FETCH_HEAD
WORKDIR /build
COPY scripts/prepare-pasion.py /build/scripts/prepare-pasion.py
RUN --mount=type=cache,id=hagency-pasion-wasm-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=hagency-pasion-wasm-target,target=/pasion/target \
    python3 scripts/prepare-pasion.py --source /pasion --output /pasion-resources

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends libpq5 ca-certificates gosu && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home hagency && mkdir -p /app/data && chown hagency:hagency /app/data
WORKDIR /app
COPY --from=build /build/hagency-server /usr/local/bin/hagency-server
COPY --from=pasion-assets /pasion-resources /app/resources/pasion
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
EXPOSE 8088
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
CMD ["--config", "/app/config.toml"]
