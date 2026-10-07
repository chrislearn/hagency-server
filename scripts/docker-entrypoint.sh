#!/bin/sh
set -eu
# Read private host-mounted component configs as root, then drop privileges.
# Relative component references keep working in the protected runtime copy.
if [ "$#" -ge 2 ] && [ "$1" = "--config" ] && [ "$2" = "/app/config/hagency.toml" ]; then
    # This generated runtime copy is ephemeral; removed host files must not
    # survive a same-container restart as stale configuration.
    rm -rf /run/hagency/config
    install -d -m 700 -o hagency -g hagency /run/hagency/config
    # Never carry host symlinks into a privileged runtime configuration copy.
    if find /app/config -type l -print -quit | read -r ignored; then
        echo "Configuration directory contains a symbolic link" >&2
        exit 1
    fi
    cp -R /app/config/. /run/hagency/config/
    chown -R hagency:hagency /run/hagency/config
    find /run/hagency/config -type d -exec chmod 700 {} +
    find /run/hagency/config -type f -exec chmod 600 {} +
    if [ -f /app/bootstrap-password ]; then
        install -m 600 -o hagency -g hagency /app/bootstrap-password /run/hagency/bootstrap-password
    fi
    shift 2
    set -- --config /run/hagency/config/hagency.toml "$@"
fi
exec gosu hagency:hagency hagency-server "$@"
