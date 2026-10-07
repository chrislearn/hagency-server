CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
CREATE TABLE users (id TEXT PRIMARY KEY, mxid TEXT NOT NULL UNIQUE, active INTEGER NOT NULL CHECK(active IN(0,1))) STRICT;
CREATE TABLE projects (
 id TEXT PRIMARY KEY, space_id TEXT NOT NULL UNIQUE, active INTEGER NOT NULL CHECK(active IN(0,1)),
 creation_policy TEXT NOT NULL CHECK(json_valid(creation_policy)), revision INTEGER NOT NULL CHECK(revision>0)
) STRICT;
CREATE TABLE rooms (
 room_id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
 active INTEGER NOT NULL CHECK(active IN(0,1)), creation_policy TEXT NOT NULL CHECK(json_valid(creation_policy)),
 revision INTEGER NOT NULL CHECK(revision>0)
) STRICT;
CREATE TABLE agents (
 id TEXT PRIMARY KEY, owner_user_id TEXT NOT NULL REFERENCES users(id), puppet_mxid TEXT NOT NULL UNIQUE,
 display_name TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN('creating','active','suspended','retiring','retired'))
) STRICT;
CREATE TABLE bindings (
 id TEXT PRIMARY KEY, agent_id TEXT NOT NULL REFERENCES agents(id), project_id TEXT NOT NULL REFERENCES projects(id),
 room_id TEXT NOT NULL REFERENCES rooms(room_id),
 state TEXT NOT NULL CHECK(state IN('joining','active','suspended','left','revoked')),
 generation INTEGER NOT NULL CHECK(generation>0), UNIQUE(agent_id,room_id)
) STRICT;
CREATE TABLE commands (
 actor TEXT NOT NULL REFERENCES users(id), operation TEXT NOT NULL, key TEXT NOT NULL,
 digest TEXT NOT NULL, result TEXT NOT NULL CHECK(json_valid(result)), PRIMARY KEY(actor,operation,key)
) STRICT;
CREATE TABLE audit (
 sequence INTEGER PRIMARY KEY, actor TEXT NOT NULL REFERENCES users(id), operation TEXT NOT NULL,
 object_id TEXT NOT NULL, at INTEGER NOT NULL CHECK(at>=0)
) STRICT;
CREATE TRIGGER immutable_agent_owner BEFORE UPDATE OF owner_user_id ON agents
 WHEN NEW.owner_user_id != OLD.owner_user_id BEGIN SELECT RAISE(ABORT,'agent_owner_immutable'); END;
CREATE TRIGGER immutable_agent_identity BEFORE UPDATE OF id,puppet_mxid ON agents
 WHEN NEW.id != OLD.id OR NEW.puppet_mxid != OLD.puppet_mxid BEGIN SELECT RAISE(ABORT,'agent_identity_immutable'); END;
CREATE TRIGGER retain_agent_identity BEFORE DELETE ON agents
 BEGIN SELECT RAISE(ABORT,'agent_identity_reserved'); END;
CREATE TRIGGER immutable_user_identity BEFORE UPDATE OF id,mxid ON users
 WHEN NEW.id != OLD.id OR NEW.mxid != OLD.mxid BEGIN SELECT RAISE(ABORT,'user_identity_immutable'); END;
CREATE TRIGGER retain_user_identity BEFORE DELETE ON users
 BEGIN SELECT RAISE(ABORT,'user_identity_reserved'); END;
CREATE TRIGGER binding_project_insert BEFORE INSERT ON bindings
 WHEN NOT EXISTS(SELECT 1 FROM rooms WHERE room_id=NEW.room_id AND project_id=NEW.project_id)
 BEGIN SELECT RAISE(ABORT,'binding_project_mismatch'); END;
CREATE TRIGGER immutable_binding_scope BEFORE UPDATE OF id,agent_id,project_id,room_id ON bindings
 WHEN NEW.id!=OLD.id OR NEW.agent_id!=OLD.agent_id OR NEW.project_id!=OLD.project_id OR NEW.room_id!=OLD.room_id
 BEGIN SELECT RAISE(ABORT,'binding_scope_immutable'); END;
