#!/usr/bin/env python3
"""Compile Rust changes, then gracefully replace the development process."""
import argparse, hashlib, json, os, pathlib, signal, subprocess, time

root = pathlib.Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser()
p.add_argument('--config', default='config.dev.toml')
p.add_argument('--palpo-source', type=pathlib.Path, help='local Palpo checkout containing the embedding API')
p.add_argument('--pasion-source', type=pathlib.Path, help='local Pasion checkout containing PasionServer')
a = p.parse_args()
os.chdir(root)
config = pathlib.Path(a.config).resolve()
if not config.exists():
    raise SystemExit('Create a development config with: python3 scripts/init-config.py --dev')
cargo = ['cargo']
# Development assets are served from disk; editing them needs only a browser refresh.
watch_roots = [root / 'src', root / 'Cargo.toml', config]
patches = []
if a.palpo_source:
    source = a.palpo_source.resolve()
    if not (source / 'crates/server/src/lib.rs').exists():
        raise SystemExit('Palpo checkout must expose MatrixServer')
    patches += ['[patch."https://github.com/palpo-im/palpo.git"]', 'palpo = { path = ' + json.dumps(str(source / 'crates/server')) + ' }']
    watch_roots += [source / 'crates', source / 'Cargo.toml']
if a.pasion_source:
    source = a.pasion_source.resolve()
    if not (source / 'crates/backend/src/embedded.rs').exists():
        raise SystemExit('Pasion checkout must expose PasionServer')
    patches += ['[patch."https://github.com/meldry-com/pasion.git"]', 'pasion-backend = { path = ' + json.dumps(str(source / 'crates/backend')) + ' }', 'pasion-config = { path = ' + json.dumps(str(source / 'crates/config')) + ' }']
    watch_roots += [source / 'crates', source / 'Cargo.toml', source / 'templates', source / 'translations', source / 'policies']
if patches:
    patch = root / '.run/dev-cargo.toml'
    patch.parent.mkdir(exist_ok=True)
    patch.write_text('\n'.join(patches) + '\n')
    cargo += ['--config', str(patch)]
# Query cargo rather than assume target/ when CARGO_TARGET_DIR is customized.
metadata = json.loads(subprocess.check_output(cargo + ['metadata', '--format-version', '1', '--no-deps'], text=True))
binary = pathlib.Path(metadata['target_directory']) / 'debug/hagency-server'

def snapshot():
    files = []
    for base in watch_roots:
        if base.is_file(): files.append(base)
        elif base.exists():
            files.extend(f for f in base.rglob('*') if f.is_file() and not any(part in {'target', '.git'} for part in f.parts) and f.suffix in {'.rs', '.toml', '.js', '.css', '.html', '.json', '.cedar', '.ftl'})
    return {str(f): hashlib.sha256(f.read_bytes()).hexdigest() for f in files}

child = None
stopping = False

def stop(*_):
    global stopping
    stopping = True

signal.signal(signal.SIGINT, stop)
signal.signal(signal.SIGTERM, stop)

def stop_child():
    global child
    if child and child.poll() is None:
        child.terminate()
        try: child.wait(timeout=20)
        except subprocess.TimeoutExpired: child.kill(); child.wait()
    child = None

try:
    before = None
    while not stopping:
        current = snapshot()
        if current != before:
            before = current
            # Serve the last successful build while compiling. A failed build
            # does not tear down a working server; the next edit retries it.
            print('Building hagency-server…', flush=True)
            built = subprocess.run(cargo + ['build', '--bin', 'hagency-server'])
            if built.returncode == 0 and not stopping:
                if a.pasion_source:
                    # Rebuild WASM and copy edited templates/assets without an image.
                    prepared = subprocess.run(['python3', 'scripts/prepare-pasion.py', '--source', str(a.pasion_source)])
                    if prepared.returncode: continue
                checked = subprocess.run([str(binary), '--config', str(config), '--check-config'])
                if checked.returncode == 0:
                    stop_child()
                    child = subprocess.Popen([str(binary), '--config', str(config)])
        if child and child.poll() is not None:
            print('Server stopped; edit source/configuration to retry.', flush=True)
            child = None
        time.sleep(.5)
finally:
    stop_child()
