#!/usr/bin/env python3
"""Build/copy Pasion resources without starting any servers.

Uses the pinned Git source by default; --source uses a local development tree.
"""
import argparse, pathlib, subprocess, shutil, json, os, platform, hashlib, io, tarfile
from urllib.request import urlopen
from urllib.parse import parse_qs, urlsplit
root = pathlib.Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--source', type=pathlib.Path)
p.add_argument('--output', type=pathlib.Path, default=root / 'resources/pasion')
p.add_argument('--skip-frontend', action='store_true')
a = p.parse_args()
if a.source:
    source = a.source.resolve()
else:
    meta = json.loads(subprocess.check_output(['cargo','metadata','--format-version','1','--no-deps'], cwd=root, text=True))
    package = next(p for p in meta['packages'] if p['name'] == 'hagency-server')
    dep = next(d for d in package['dependencies'] if d['name'] == 'pasion-backend')
    if dep.get('path'):
        source = pathlib.Path(dep['path']).parents[1]
    else:
        parsed = urlsplit(dep['source'].removeprefix('git+'))
        revision = parse_qs(parsed.query)['rev'][0]
        repository = parsed._replace(query='',fragment='').geturl()
        source = root / '.run/pasion-source'
        source.parent.mkdir(exist_ok=True)
        if not source.exists(): subprocess.run(['git','clone',repository,str(source)], check=True)
        subprocess.run(['git','-C',str(source),'fetch','origin',revision], check=True)
        subprocess.run(['git','-C',str(source),'checkout','--detach',revision], check=True)
output = a.output.resolve()
output.mkdir(parents=True,exist_ok=True)
for name, path in [('templates','templates'),('translations','translations'),('cedar','policies/cedar')]:
    shutil.copytree(source/path,output/name,dirs_exist_ok=True)
def dioxus_cli():
    existing = shutil.which('dx')
    if existing and subprocess.check_output([existing,'--version'],text=True).startswith('dioxus 0.7.5 '):
        return existing
    cached = root/'.run/tools/dioxus-0.7.5/dx'
    if not cached.exists():
        arch = {'arm64':'aarch64','aarch64':'aarch64','x86_64':'x86_64','AMD64':'x86_64'}.get(platform.machine())
        target = {'Darwin':'apple-darwin','Linux':'unknown-linux-gnu'}.get(platform.system())
        if not arch or not target:
            raise SystemExit('Install dx 0.7.5 on this platform before building Pasion.')
        artifact = f'dx-{arch}-{target}'
        base = 'https://github.com/DioxusLabs/dioxus/releases/download/v0.7.5/'
        print('Fetching matching Dioxus CLI 0.7.5…', flush=True)
        with urlopen(base+artifact+'.sha256',timeout=30) as response:
            expected = response.read().decode().split()[0]
        with urlopen(base+artifact+'.tar.gz',timeout=60) as response:
            payload = response.read()
        if hashlib.sha256(payload).hexdigest() != expected:
            raise SystemExit('Dioxus CLI checksum did not match')
        with tarfile.open(fileobj=io.BytesIO(payload),mode='r:gz') as archive:
            member = next(m for m in archive.getmembers() if pathlib.PurePosixPath(m.name).name == 'dx' and m.isfile())
            binary = archive.extractfile(member).read()
        cached.parent.mkdir(parents=True,exist_ok=True)
        cached.write_bytes(binary)
        cached.chmod(0o755)
    return str(cached)

if not a.skip_frontend:
    env = dict(os.environ)
    env['CARGO_TARGET_DIR'] = str(source/'target')
    subprocess.run([dioxus_cli(),'build','--release','--package','pasion-frontend','--base-path','/_pasion/','--debug-symbols','false','--locked'], cwd=source, env=env, check=True)
    public = source/'target/dx/pasion-frontend/release/web/public'
    shutil.copytree(public,output/'public',dirs_exist_ok=True)
(output/'SOURCE').write_text(str(source)+'\n')
print('Prepared Pasion resources:',output)
