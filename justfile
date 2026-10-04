set positional-arguments

xtask := "cargo run --quiet --locked --manifest-path xtask/Cargo.toml --"

# Show available development and deployment commands.
default:
    @just --list

# Generate config/dev/{hagency,palpo,pasion}.toml without overwriting files.
init-dev *args:
    {{ xtask }} init-config --dev "$@"

# Generate config/docker/ and a private .env with a random database password.
init-docker *args:
    {{ xtask }} init-config "$@"

# Start only the PostgreSQL service, containing the three component databases.
db-up:
    docker compose up -d postgres

# Install the WASM target and build/copy Pasion frontend resources.
prepare-pasion *args:
    rustup target add wasm32-unknown-unknown
    {{ xtask }} prepare-pasion "$@"

# Watch source/configuration and gracefully replace successful server builds.
dev *args:
    {{ xtask }} dev "$@"

# Run the server once; accepts --config and bootstrap options.
run *args:
    cargo run --locked --bin hagency-server -- "$@"

# Validate the host and referenced component configurations without starting.
check-config config="config/dev/hagency.toml":
    cargo run --locked --bin hagency-server -- --config "$1" --check-config

# Build the server image and bundled Pasion assets.
docker-build:
    docker compose build server

# Start the configured Compose deployment (PostgreSQL plus one Rust server).
docker-up:
    docker compose up -d

# Stop Compose services while preserving data volumes.
docker-down:
    docker compose down

# Check formatting and compile/test the independent Rust development tools.
check-tools:
    cargo fmt --manifest-path xtask/Cargo.toml -- --check
    cargo clippy --locked --manifest-path xtask/Cargo.toml --all-targets -- -D warnings
    cargo test --locked --manifest-path xtask/Cargo.toml
