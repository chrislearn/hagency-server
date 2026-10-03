// An isolated Compose deployment of the built image; never touches the normal stack.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {mkdtemp,writeFile,readFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,resolve} from 'node:path';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {randomBytes} from 'node:crypto';
const image=process.env.HAGENCY_TEST_IMAGE??'hagency-server:integration-check';
const reservation=createServer().listen(0,'127.0.0.1');await once(reservation,'listening');const port=reservation.address().port;await new Promise(r=>reservation.close(r));
const base=`http://127.0.0.1:${port}`,name=`localhost:${port}`,password=randomBytes(24).toString('base64url');
const dir=await mkdtemp(join(tmpdir(),'hagency-compose-')),compose=join(dir,'compose.yaml'),project='hagency-test-'+randomBytes(6).toString('hex');
const run=(...args)=>execFileSync('docker',['compose','--project-name',project,'-f',compose,...args],{encoding:'utf8',stdio:['ignore','pipe','pipe']});
const config=`listen = "0.0.0.0:8088"\npublic_origin = "${base}"\ndata_dir = "/app/data"\n[matrix]\nserver_name = "${name}"\nallow_registration = false\n[matrix.db]\nurl = "postgres://hagency:compose_test_only@postgres:5432/hagency"\npool_size = 10\n[matrix.well_known]\nclient = "${base}"\nserver = "${name}"\n[matrix.storage]\nbackend = "fs"\nroot = "/app/data/media"\n[pasion]\ndatabase_url = "postgres://hagency:compose_test_only@postgres:5432/pasion"\nresources_dir = "/app/resources/pasion"\ndelegate_matrix_auth = false\n`;
await writeFile(join(dir,'init.sql'),'CREATE DATABASE pasion OWNER hagency;\n');
await writeFile(join(dir,'config.toml'),config,{mode:0o600});await writeFile(join(dir,'admin-password'),password,{mode:0o600});
function definition(bootstrap){return `services:\n  postgres:\n    image: postgres:18.6-alpine\n    environment:\n      POSTGRES_USER: hagency\n      POSTGRES_PASSWORD: compose_test_only\n      POSTGRES_DB: hagency\n    volumes: ["database:/var/lib/postgresql", "${dir}/init.sql:/docker-entrypoint-initdb.d/20-pasion.sql:ro"]\n    healthcheck:\n      test: ["CMD-SHELL", "pg_isready -U hagency -d hagency"]\n      interval: 1s\n      timeout: 3s\n      retries: 30\n  server:\n    image: ${image}\n    depends_on:\n      postgres: {condition: service_healthy}\n    ports: ["127.0.0.1:${port}:8088"]\n    volumes:\n      - "${dir}/config.toml:/app/config.toml:ro"\n      - "${dir}/admin-password:/app/bootstrap-password:ro"\n      - "server-data:/app/data"\n    command: ${JSON.stringify(['--config','/app/config.toml',...(bootstrap?['--bootstrap-admin','admin','--bootstrap-password-file','/run/hagency/bootstrap-password']:[])])}\nvolumes:\n  database: {}\n  server-data: {}\n`;}
async function ready(){let last;for(let i=0;i<600;i++){try{if((await fetch(base+'/healthz')).status===200)return;}catch(e){last=e;}await new Promise(r=>setTimeout(r,100));}throw last??new Error('Startup timed out');}
async function login(){const r=await fetch(base+'/api/login',{method:'POST',headers:{'Content-Type':'application/json',Origin:base},body:JSON.stringify({username:`@admin:${name}`,password})});assert.equal(r.status,200);assert.equal((await r.json()).isAdmin,true);}
try{
 await writeFile(compose,definition(true));run('up','-d');await ready();await login();
 assert.equal(await(await fetch(base)).text(),await readFile(resolve('public/index.html'),'utf8'));
 assert.equal((await fetch(base+'/_matrix/client/versions')).status,200);
 const discovery=await(await fetch(base+'/_pasion/.well-known/openid-configuration')).json();assert.equal(discovery.issuer,base+'/_pasion/');
 assert.equal((await fetch(base+'/_pasion/login')).status,200);
 const oauthKeys=await(await fetch(base+'/_pasion/oauth2/keys.json')).json();assert.ok(oauthKeys.keys.length);
 const uid=run('exec','-T','server','sh','-c',"awk '/^Uid:/ {print $2}' /proc/1/status; stat -c %a /run/hagency/config.toml; stat -c %a /app/data/pasion-secrets.json").trim().split('\n');assert.deepEqual(uid,['10001','600','600']);
 const key=run('exec','-T','server','cat','/app/data/matrix-signing-key.json');
 run('stop','server');await writeFile(compose,definition(false));run('up','-d','server');await ready();await login();
 assert.equal(run('exec','-T','server','cat','/app/data/matrix-signing-key.json'),key);
 assert.deepEqual(await(await fetch(base+'/_pasion/oauth2/keys.json')).json(),oauthKeys);
 console.log('PASS isolated Compose: non-root Rust process, protected configuration, same-port UI/Matrix/admin/Pasion discovery and login and persistent restart');
}catch(e){console.error(run('logs','--no-color','--tail','60','server'));throw e;}finally{try{run('down','--volumes','--remove-orphans');}finally{await rm(dir,{recursive:true,force:true});}}
