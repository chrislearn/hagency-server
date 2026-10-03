#!/bin/sh
set -eu
# Host configuration remains mode 0600. Copy it as root, then drop privileges.
# Generated production configuration uses absolute /app/data paths.
if [ "$#" -ge 2 ] && [ "$1" = "--config" ] && [ "$2" = "/app/config.toml" ]; then
    install -d -m 700 -o hagency -g hagency /run/hagency
    install -m 600 -o hagency -g hagency /app/config.toml /run/hagency/config.toml
    if [ -f /app/bootstrap-password ]; then
        install -m 600 -o hagency -g hagency /app/bootstrap-password /run/hagency/bootstrap-password
    fi
    shift 2
    set -- --config /run/hagency/config.toml "$@"
fi
exec gosu hagency:hagency hagency-server "$@"
