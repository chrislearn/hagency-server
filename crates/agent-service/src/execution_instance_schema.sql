CREATE TABLE hagency_agent_v1.execution_instances (
 agent_id text PRIMARY KEY, id text NOT NULL UNIQUE,
 owner_user_id text NOT NULL REFERENCES hagency_agent_v1.users(id),
 device_id text NOT NULL, name text NOT NULL, generation bigint NOT NULL CHECK(generation>0),
 FOREIGN KEY(agent_id,owner_user_id) REFERENCES hagency_agent_v1.agents(id,owner_user_id),
 FOREIGN KEY(device_id,owner_user_id) REFERENCES hagency_agent_v1.devices(id,user_id)
);
CREATE FUNCTION hagency_agent_v1.retain_execution_instance() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'permanent_execution_instance'; END IF;
 IF NEW.id<>OLD.id OR NEW.agent_id<>OLD.agent_id OR NEW.owner_user_id<>OLD.owner_user_id OR NEW.generation<OLD.generation THEN RAISE EXCEPTION 'immutable_execution_instance_identity'; END IF;
 IF NEW.device_id<>OLD.device_id AND NEW.generation<=OLD.generation THEN RAISE EXCEPTION 'execution_instance_generation_required'; END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER retain_execution_instance BEFORE UPDATE OR DELETE ON hagency_agent_v1.execution_instances FOR EACH ROW EXECUTE FUNCTION hagency_agent_v1.retain_execution_instance();
