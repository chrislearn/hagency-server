#!/usr/bin/env python3
"""Generate private Hagency, Palpo and Pasion config files without starting services."""
import argparse, json, pathlib, secrets, os
from urllib.parse import quote
root = pathlib.Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--dev', action='store_true')
p.add_argument('--origin', default='http://127.0.0.1:8088')
p.add_argument('--server-name', default='localhost:8088')
p.add_argument('--output-dir', type=pathlib.Path)
a = p.parse_args()
output = (a.output_dir or root / 'config' / ('dev' if a.dev else 'docker')).resolve()
if output.exists(): raise SystemExit(f'Configuration directory already exists: {output}')
env = root / '.env'
if a.dev:
    password = os.environ.get('HAGENCY_DB_PASSWORD')
    if password is None and env.exists():
        for line in env.read_text().splitlines():
            if line.startswith('HAGENCY_DB_PASSWORD='):
                password = line.partition('=')[2].strip().strip('"').strip("'")
    password = password or 'hagency_dev'
else:
    if env.exists(): raise SystemExit('An .env file already exists; preserve it and configure Compose manually.')
    password = secrets.token_urlsafe(32)

configs = {}
for component in ['hagency', 'palpo', 'pasion']:
    text = (root / 'config/examples' / (component + '.toml')).read_text()
    text = text.replace('"http://127.0.0.1:8088"', json.dumps(a.origin)).replace('"localhost:8088"', json.dumps(a.server_name))
    endpoint = '@127.0.0.1:55438/' if a.dev else '@postgres:5432/'
    text = text.replace('hagency:hagency_dev@127.0.0.1:55438/', 'hagency:' + quote(password, safe='') + endpoint)
    if component == 'hagency':
        text = text.replace('data_dir = "../../data"', 'data_dir = ' + json.dumps(str(root/'data') if a.dev else '/app/data'))
        if a.dev: text = text.replace('# public_dir = "../../public"', 'public_dir = ' + json.dumps(str(root/'public')))
        else: text = text.replace('listen = "127.0.0.1:8088"', 'listen = "0.0.0.0:8088"')
    elif component == 'palpo':
        text = text.replace('root = "../../data/media"', 'root = ' + json.dumps(str(root/'data/media') if a.dev else '/app/data/media'))
    else:
        text = text.replace('resources_dir = "../../resources/pasion"', 'resources_dir = ' + json.dumps(str(root/'resources/pasion') if a.dev else '/app/resources/pasion'))
    configs[component] = text

output.parent.mkdir(parents=True, exist_ok=True)
output.mkdir(mode=0o700)
for component, text in configs.items():
    fd = os.open(output/(component+'.toml'), os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, 'w') as file: file.write(text)
if not a.dev:
    fd = os.open(env, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    with os.fdopen(fd, 'w') as file: file.write('HAGENCY_DB_PASSWORD=' + password + '\n')
print('Created component configurations in', output)
