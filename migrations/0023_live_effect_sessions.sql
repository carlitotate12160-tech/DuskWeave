-- Distinct live sessions and administrator-installed deployment qualification.
-- Neither fixture state nor an ordinary caller can create qualification.
CREATE TABLE execution.live_qualifications (
 qualification_ref uuid PRIMARY KEY,
 run_profile text NOT NULL CHECK (run_profile IN ('autonomous-dns-genai-live-v1','autonomous-dns-genai-lab-v1')),
 purpose text NOT NULL CHECK (purpose IN ('LIVE','LAB')),
 release_sha256 text NOT NULL CHECK (release_sha256 ~ '^[0-9a-f]{64}$'),
 launcher_sha256 text NOT NULL CHECK (launcher_sha256 ~ '^[0-9a-f]{64}$'),
 capture_sha256 text NOT NULL CHECK (capture_sha256 ~ '^[0-9a-f]{64}$'),
 exporter_sha256 text NOT NULL CHECK (exporter_sha256 ~ '^[0-9a-f]{64}$'),
 image_digest text NOT NULL CHECK (image_digest ~ '^sha256:[0-9a-f]{64}$'),
 manifest_sha256 text NOT NULL CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'),
 boot_id uuid NOT NULL,
 resolver_ipv4 inet NOT NULL CHECK (family(resolver_ipv4)=4 AND masklen(resolver_ipv4)=32),
 valid_from_ms bigint NOT NULL,
 valid_until_ms bigint NOT NULL CHECK (valid_until_ms>valid_from_ms),
 CHECK ((run_profile,purpose) IN (('autonomous-dns-genai-live-v1','LIVE'),('autonomous-dns-genai-lab-v1','LAB'))),
 CHECK (purpose<>'LAB' OR resolver_ipv4='192.0.2.53'::inet)
);

CREATE TABLE execution.live_effect_sessions (
 run_id uuid PRIMARY KEY REFERENCES execution.autonomous_runs,
 e uuid NOT NULL,
 c uuid NOT NULL,
 generation bigint NOT NULL CHECK (generation>0),
 writer_oid oid NOT NULL,
 mission_revision bigint NOT NULL CHECK (mission_revision=1),
 decision uuid NOT NULL,
 attempt uuid NOT NULL UNIQUE,
 result_ref uuid NOT NULL UNIQUE,
 request jsonb NOT NULL CHECK (jsonb_typeof(request)='object'),
 qualification_ref uuid NOT NULL REFERENCES execution.live_qualifications,
 deadline_at_ms bigint NOT NULL,
 UNIQUE(e,c,generation),
 UNIQUE(run_id,generation)
);
CREATE TABLE execution.live_effect_control (
 run_id uuid PRIMARY KEY REFERENCES execution.live_effect_sessions,
 phase text NOT NULL CHECK (phase IN ('reserved','may_start','result_recorded','stopped','closed')),
 possibly_started boolean NOT NULL DEFAULT false,
 attempt_deadline_at_ms bigint NOT NULL,
 disposal_confirmed boolean NOT NULL DEFAULT false,
 CHECK (phase<>'closed' OR disposal_confirmed)
);
CREATE TABLE execution.live_effect_events (
 run_id uuid NOT NULL REFERENCES execution.live_effect_sessions,
 generation bigint NOT NULL,
 phase text NOT NULL CHECK (phase IN ('reserved','may_start','result_recorded','stopped','closed')),
 occurred_at_ms bigint NOT NULL,
 PRIMARY KEY(run_id,phase),
 FOREIGN KEY(run_id,generation) REFERENCES execution.live_effect_sessions(run_id,generation)
);

-- Exists only inside a restricted owner operation and is removed before return.
-- Runtime roles cannot insert a context or leave one open for a later write.
CREATE TABLE execution.private_live_writer_context (
 backend_pid integer NOT NULL,
 transaction_id xid8 NOT NULL,
 e uuid NOT NULL,
 c uuid NOT NULL,
 generation bigint NOT NULL,
 actor_oid oid NOT NULL,
 operation text NOT NULL CHECK (operation IN ('withdraw','stop_run','run_unknown')),
 PRIMARY KEY(backend_pid,transaction_id,e,c)
);
REVOKE ALL ON execution.live_qualifications,execution.live_effect_sessions,
 execution.live_effect_control,execution.live_effect_events,execution.private_live_writer_context
 FROM PUBLIC,dw_runtime,dw_m1_broker,dw_m1_cognition,dw_m1_trajectory,dw_m1_terrain;
GRANT SELECT ON execution.live_qualifications TO dw_runtime,dw_m1_broker;
GRANT SELECT ON execution.live_effect_sessions,execution.live_effect_control,
 execution.live_effect_events TO dw_m1_broker;

CREATE FUNCTION execution.private_live_actor(role_name text) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT coalesce(pg_has_role(session_user,role_name,'MEMBER')
 AND pg_has_role(session_user,role_name,'USAGE')
 AND NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname=session_user
 AND (rolsuper OR rolbypassrls OR rolcreaterole OR rolcreatedb OR rolreplication)),false)
$$;
REVOKE ALL ON FUNCTION execution.private_live_actor(text) FROM PUBLIC;

CREATE FUNCTION execution.live_effect_record(r uuid) RETURNS jsonb
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT jsonb_build_object('run_id',s.run_id,'request',s.request,'result_ref',s.result_ref,
 'phase',x.phase,'possibly_started',x.possibly_started,
 'attempt_deadline_at_ms',x.attempt_deadline_at_ms,'result',NULL,'disposal_confirmed',x.disposal_confirmed)
 FROM execution.live_effect_sessions s JOIN execution.live_effect_control x USING(run_id)
 WHERE s.run_id=r AND s.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user)
 AND execution.private_live_actor('dw_m1_broker')
$$;
REVOKE ALL ON FUNCTION execution.live_effect_record(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.live_effect_record(uuid) TO dw_m1_broker;
