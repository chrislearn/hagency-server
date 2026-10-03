#!/usr/bin/env python3
"""Prepare a development config or a Compose deployment without starting it."""
import argparse, json, pathlib, secrets, os
from urllib.parse import quote
root = pathlib.Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--dev', action='store_true')
p.add_argument('--origin', default='http://127.0.0.1:8088')
p.add_argument('--server-name', default='localhost:8088')
a = p.parse_args()
output = root / ('config.dev.toml' if a.dev else 'config.docker.toml')
if output.exists(): raise SystemExit(f'Config already exists: {output}')
if a.dev:
    database_password = os.environ.get('HAGENCY_DB_PASSWORD')
    if database_password is None and (root / '.env').exists():
        for line in (root / '.env').read_text().splitlines():
            if line.startswith('HAGENCY_DB_PASSWORD='):
                database_password = line.partition('=')[2].strip().strip('"').strip("'")
    config = (root / 'config.example.toml').read_text().replace('# public_dir = "public"', 'public_dir = "public"')
    config = config.replace('hagency:hagency_dev@', 'hagency:' + quote(database_password or 'hagency_dev', safe='') + '@')
else:
    env = root / '.env'
    if env.exists(): raise SystemExit('An .env file already exists; preserve it and configure Compose manually.')
    password = secrets.token_urlsafe(32)
    config = f'''listen = "0.0.0.0:8088"
public_origin = {json.dumps(a.origin)}
data_dir = "/app/data"
database_url = "postgres://hagency:{password}@postgres:5432/hagency"
[matrix]
server_name = {json.dumps(a.server_name)}
allow_registration = false
[matrix.db]
url = "postgres://hagency:{password}@postgres:5432/palpo"
pool_size = 10
[matrix.well_known]
client = {json.dumps(a.origin)}
server = {json.dumps(a.server_name)}
[matrix.storage]
backend = "fs"
root = "/app/data/media"
'''
    config += f'''\n[pasion]\ndatabase_url = "postgres://hagency:{password}@postgres:5432/pasion"\nresources_dir = "/app/resources/pasion"\ndelegate_matrix_auth = false\n'''
    fd = os.open(env, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd,'w') as file: file.write('HAGENCY_DB_PASSWORD=' + password + '\n')
fd = os.open(output, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
with os.fdopen(fd,'w') as file: file.write(config)
print('Created', output)
