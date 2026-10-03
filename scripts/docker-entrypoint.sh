#!/bin/sh
set -eu
# Read private host-mounted component configs as root, then drop privileges.
# Relative component references keep working in the protected runtime copy.
if [ "$#" -ge 2 ] && [ "$1" = "--config" ] && [ "$2" = "/app/config/hagency.toml" ]; then
    install -d -m 700 -o hagency -g hagency /run/hagency/config
    cp -R /app/config/. /run/hagency/config/
    chown -R hagency:hagency /run/hagency/config
    chmod -R u=rwX,go= /run/hagency/config
    if [ -f /app/bootstrap-password ]; then
        install -m 600 -o hagency -g hagency /app/bootstrap-password /run/hagency/bootstrap-password
    fi
    shift 2
    set -- --config /run/hagency/config/hagency.toml "$@"
fi
exec gosu hagency:hagency hagency-server "$@"
