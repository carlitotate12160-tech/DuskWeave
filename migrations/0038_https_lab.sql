-- DW-M1-HTTPS-LAB-IMPLEMENT-R1: closed HTTPS LAB admission, attempt-keyed live custody,
-- typed live HTTPS results/Terrain acceptance, and multi-step acquisition/refresh lineage.
BEGIN;

-- 1. Extend execution.live_qualifications for HTTPS_LOOP_LAB pinned to 192.0.2.53.
DO $$
DECLARE r record;
BEGIN
    FOR r IN
        SELECT conname, pg_catalog.pg_get_constraintdef(oid) AS def
        FROM pg_catalog.pg_constraint
        WHERE conrelid = 'execution.live_qualifications'::regclass AND contype = 'c'
    LOOP
        IF r.def LIKE '%autonomous-dns-genai-live-v1%'
           OR (r.def LIKE '%purpose%' AND r.def LIKE '%LAB%') THEN
            EXECUTE format('ALTER TABLE execution.live_qualifications DROP CONSTRAINT %I', r.conname);
        END IF;
    END LOOP;
END $$;

ALTER TABLE execution.live_qualifications ADD CONSTRAINT live_qualifications_run_profile_check
    CHECK (run_profile IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1', 'autonomous-https-genai-lab-v1'));
ALTER TABLE execution.live_qualifications ADD CONSTRAINT live_qualifications_purpose_check
    CHECK (purpose IN ('LIVE', 'LAB', 'HTTPS_LOOP_LAB'));
ALTER TABLE execution.live_qualifications ADD CONSTRAINT live_qualifications_purpose_pair_check
    CHECK ((run_profile, purpose) IN (
        ('autonomous-dns-genai-live-v1', 'LIVE'),
        ('autonomous-dns-genai-lab-v1', 'LAB'),
        ('autonomous-https-genai-lab-v1', 'HTTPS_LOOP_LAB')
    ));
ALTER TABLE execution.live_qualifications ADD CONSTRAINT live_qualifications_lab_resolver_check
    CHECK (purpose NOT IN ('LAB', 'HTTPS_LOOP_LAB') OR resolver_ipv4 = '192.0.2.53'::inet);

-- 2. Rekey live_effect_* and live_dns_results by attempt while preserving legacy rows.
ALTER TABLE execution.live_effect_sessions
    ADD COLUMN run_kind text NOT NULL DEFAULT 'legacy'
        CHECK (run_kind IN ('legacy', 'm1_https_loop_v1')),
    ADD COLUMN step_index integer NOT NULL DEFAULT 0
        CHECK (step_index BETWEEN 0 AND 4),
    ADD COLUMN attempt_kind text NOT NULL DEFAULT 'dns'
        CHECK (attempt_kind IN ('dns', 'dns_refresh', 'head')),
    ADD COLUMN expected_claim_revision integer NOT NULL DEFAULT 0
        CHECK (expected_claim_revision >= 0),
    ADD CONSTRAINT live_effect_sessions_kind_step_check CHECK (
        (run_kind = 'legacy' AND step_index = 0 AND attempt_kind = 'dns')
        OR (run_kind = 'm1_https_loop_v1' AND step_index BETWEEN 1 AND 3)
    );

ALTER TABLE execution.live_effect_control
    ADD COLUMN attempt uuid,
    ADD COLUMN granted_phase text
        CHECK (granted_phase IS NULL OR granted_phase IN ('connect', 'tls', 'head'));

UPDATE execution.live_effect_control c
SET attempt = s.attempt
FROM execution.live_effect_sessions s
WHERE s.run_id = c.run_id;

ALTER TABLE execution.live_effect_control
    ALTER COLUMN attempt SET NOT NULL;

ALTER TABLE execution.live_effect_events
    ADD COLUMN attempt uuid;

UPDATE execution.live_effect_events ev
SET attempt = s.attempt
FROM execution.live_effect_sessions s
WHERE s.run_id = ev.run_id;

ALTER TABLE execution.live_effect_events
    ALTER COLUMN attempt SET NOT NULL;

ALTER TABLE execution.live_dns_results
    ADD COLUMN run_kind text NOT NULL DEFAULT 'legacy'
        CHECK (run_kind IN ('legacy', 'm1_https_loop_v1')),
    ADD COLUMN step_index integer NOT NULL DEFAULT 0
        CHECK (step_index BETWEEN 0 AND 4),
    ADD COLUMN attempt_kind text NOT NULL DEFAULT 'dns'
        CHECK (attempt_kind IN ('dns', 'dns_refresh'));

DO $$
DECLARE r record;
BEGIN
    FOR r IN
        SELECT conrelid::regclass AS tbl, conname
        FROM pg_catalog.pg_constraint
        WHERE confrelid = 'execution.live_effect_sessions'::regclass AND contype = 'f'
    LOOP
        EXECUTE format('ALTER TABLE %s DROP CONSTRAINT %I', r.tbl, r.conname);
    END LOOP;
    FOR r IN
        SELECT conname
        FROM pg_catalog.pg_constraint
        WHERE conrelid = 'execution.live_effect_control'::regclass AND contype = 'p'
    LOOP
        EXECUTE format('ALTER TABLE execution.live_effect_control DROP CONSTRAINT %I', r.conname);
    END LOOP;
    FOR r IN
        SELECT conname
        FROM pg_catalog.pg_constraint
        WHERE conrelid = 'execution.live_effect_events'::regclass AND contype IN ('p', 'c')
          AND pg_catalog.pg_get_constraintdef(oid) ~ '(PRIMARY KEY|phase)'
    LOOP
        EXECUTE format('ALTER TABLE execution.live_effect_events DROP CONSTRAINT %I', r.conname);
    END LOOP;
    FOR r IN
        SELECT conname
        FROM pg_catalog.pg_constraint
        WHERE conrelid = 'execution.live_dns_results'::regclass AND contype = 'u'
          AND pg_catalog.pg_get_constraintdef(oid) = 'UNIQUE (run_id)'
    LOOP
        EXECUTE format('ALTER TABLE execution.live_dns_results DROP CONSTRAINT %I', r.conname);
    END LOOP;
    FOR r IN
        SELECT conname
        FROM pg_catalog.pg_constraint
        WHERE conrelid = 'execution.live_effect_sessions'::regclass AND contype IN ('p', 'u')
          AND pg_catalog.pg_get_constraintdef(oid) IN ('PRIMARY KEY (run_id)', 'UNIQUE (attempt)')
    LOOP
        EXECUTE format('ALTER TABLE execution.live_effect_sessions DROP CONSTRAINT %I', r.conname);
    END LOOP;
END $$;

ALTER TABLE execution.live_effect_sessions
    ADD PRIMARY KEY (attempt),
    ADD CONSTRAINT live_effect_sessions_attempt_generation_key UNIQUE (attempt, generation);

CREATE UNIQUE INDEX live_effect_sessions_legacy_run_idx
    ON execution.live_effect_sessions (run_id) WHERE run_kind = 'legacy';
CREATE UNIQUE INDEX live_effect_sessions_https_step_idx
    ON execution.live_effect_sessions (run_id, step_index, attempt_kind) WHERE run_kind = 'm1_https_loop_v1';

ALTER TABLE execution.live_effect_control
    ADD PRIMARY KEY (attempt),
    ADD CONSTRAINT live_effect_control_attempt_fkey FOREIGN KEY (attempt) REFERENCES execution.live_effect_sessions (attempt);

ALTER TABLE execution.live_effect_events
    ADD CONSTRAINT live_effect_events_phase_check
        CHECK (phase IN ('reserved', 'may_start', 'result_recorded', 'stopped', 'closed', 'phase_connect', 'phase_tls', 'phase_head')),
    ADD PRIMARY KEY (attempt, phase),
    ADD CONSTRAINT live_effect_events_attempt_fkey FOREIGN KEY (attempt) REFERENCES execution.live_effect_sessions (attempt),
    ADD CONSTRAINT live_effect_events_run_id_generation_fkey FOREIGN KEY (run_id, generation) REFERENCES execution.live_effect_sessions (run_id, generation);

ALTER TABLE execution.live_dns_results
    ADD CONSTRAINT live_dns_results_attempt_fkey FOREIGN KEY (attempt) REFERENCES execution.live_effect_sessions (attempt),
    ADD CONSTRAINT live_dns_results_run_id_session_generation_fkey FOREIGN KEY (run_id, session_generation) REFERENCES execution.live_effect_sessions (run_id, generation),
    ADD CONSTRAINT live_dns_results_step_attempt_key UNIQUE (run_id, step_index, attempt_kind);

CREATE UNIQUE INDEX live_dns_results_legacy_run_idx
    ON execution.live_dns_results (run_id) WHERE run_kind = 'legacy';

CREATE TABLE execution.live_https_results (
    result uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES execution.autonomous_runs (run_id),
    step_index integer NOT NULL CHECK (step_index BETWEEN 1 AND 3),
    attempt uuid NOT NULL UNIQUE REFERENCES execution.live_effect_sessions (attempt),
    session_generation bigint NOT NULL,
    safe_result jsonb NOT NULL CHECK (jsonb_typeof(safe_result) = 'object'),
    result_payload jsonb NOT NULL CHECK (jsonb_typeof(result_payload) = 'object'),
    received_at_ms bigint NOT NULL,
    publication_obligation text NOT NULL CHECK (publication_obligation = 'terrain.issued_https.v2'),
    UNIQUE (run_id, step_index),
    FOREIGN KEY (run_id, session_generation) REFERENCES execution.live_effect_sessions (run_id, generation)
);

REVOKE ALL ON execution.live_https_results FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_terrain, dw_m1_trajectory, dw_m1_stop_writer;
GRANT SELECT ON execution.live_https_results TO dw_m1_broker, dw_m1_terrain;

DO $$
DECLARE r record;
BEGIN
    FOR r IN
        SELECT conname
        FROM pg_catalog.pg_constraint
        WHERE conrelid = 'terrain.https_observations'::regclass AND contype = 'c'
          AND pg_catalog.pg_get_constraintdef(oid) LIKE '%source_kind%'
    LOOP
        EXECUTE format('ALTER TABLE terrain.https_observations DROP CONSTRAINT %I', r.conname);
    END LOOP;
END $$;

ALTER TABLE terrain.https_observations
    ADD CONSTRAINT https_observations_source_kind_check
        CHECK (source_kind IN ('fixture_https_acquisition', 'issued_https_acquisition'));

-- 3. Authority fence and private run-control scope updated for attempt-keyed sessions.
CREATE OR REPLACE FUNCTION execution.private_live_control_scope(r uuid, operation text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; s execution.live_effect_sessions; actor oid;
BEGIN
    IF operation NOT IN ('stop_run', 'run_unknown') OR operation IS NULL
       OR NOT execution.private_live_actor('dw_m1_broker') THEN
        RAISE EXCEPTION 'live_control_refused';
    END IF;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    SELECT * INTO x FROM execution.autonomous_runs a WHERE a.run_id = r;
    IF NOT FOUND OR x.writer_oid IS DISTINCT FROM actor THEN
        RAISE EXCEPTION 'live_control_refused';
    END IF;
    IF x.request->>'execution_mode' IS DISTINCT FROM 'live' THEN
        RETURN;
    END IF;
    SELECT a.* INTO s
    FROM execution.live_effect_sessions a
    JOIN execution.live_effect_control b ON b.attempt = a.attempt
    WHERE (a.e, a.c) = (x.e, x.c)
      AND (b.phase <> 'closed' OR (b.possibly_started AND NOT EXISTS (
          SELECT 1 FROM execution.live_effect_events z
          WHERE z.attempt = a.attempt AND z.phase = 'result_recorded'
      )))
    ORDER BY a.generation DESC LIMIT 1;
    IF s.attempt IS NULL THEN
        RETURN;
    END IF;
    IF s.run_id <> r THEN
        RAISE EXCEPTION 'live_control_refused';
    END IF;
    INSERT INTO execution.private_live_writer_context
        (backend_pid, transaction_id, e, c, generation, actor_oid, operation)
    VALUES (pg_backend_pid(), pg_current_xact_id(), x.e, x.c, s.generation, actor, operation);
END $$;
REVOKE ALL ON FUNCTION execution.private_live_control_scope(uuid, text) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.private_live_mutation_guard() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    payload jsonb; prior_payload jsonb; e uuid; c uuid; old_e uuid; old_c uuid; lock_key bigint;
    s execution.live_effect_sessions; x execution.live_effect_control;
    allowed boolean; control_op text; r_run execution.autonomous_runs; now_ms bigint;
BEGIN
    IF TG_OP = 'DELETE' THEN payload := to_jsonb(OLD); ELSE payload := to_jsonb(NEW); END IF;
    IF TG_OP = 'UPDATE' THEN prior_payload := to_jsonb(OLD); END IF;
    IF payload ? 'run_id' THEN
        SELECT a.e, a.c INTO e, c FROM execution.autonomous_runs a WHERE a.run_id = (payload->>'run_id')::uuid;
    ELSE
        e := coalesce(payload->>'engagement_id', payload->>'e')::uuid;
        c := coalesce(payload->>'campaign_id', payload->>'c')::uuid;
    END IF;
    IF e IS NULL OR c IS NULL THEN RAISE EXCEPTION 'live_mutation_scope_refused'; END IF;
    IF prior_payload IS NOT NULL THEN
        IF prior_payload ? 'run_id' THEN
            SELECT a.e, a.c INTO old_e, old_c FROM execution.autonomous_runs a WHERE a.run_id = (prior_payload->>'run_id')::uuid;
        ELSE
            old_e := coalesce(prior_payload->>'engagement_id', prior_payload->>'e')::uuid;
            old_c := coalesce(prior_payload->>'campaign_id', prior_payload->>'c')::uuid;
        END IF;
    END IF;

    -- Positive liveness-only renewal fast path before generic campaign locking (0034 invariant).
    IF TG_TABLE_SCHEMA = 'execution' AND TG_TABLE_NAME = 'autonomous_control' AND TG_OP = 'UPDATE'
       AND (payload->>'lease_until_ms')::bigint > 0
       AND payload - 'lease_until_ms' = prior_payload - 'lease_until_ms' THEN
        SELECT a.* INTO r_run FROM execution.autonomous_runs a WHERE a.run_id = (payload->>'run_id')::uuid;
        IF r_run.run_id IS NULL OR (r_run.e, r_run.c) IS DISTINCT FROM (e, c)
           OR r_run.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
           OR NOT execution.private_live_actor('dw_m1_broker')
           OR prior_payload->>'status' IS DISTINCT FROM 'running'
           OR payload->>'status' IS DISTINCT FROM 'running' THEN
            RAISE EXCEPTION 'run_not_current';
        END IF;
        now_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
        IF (prior_payload->>'lease_until_ms')::bigint <= now_ms
           OR r_run.deadline_at_ms <= now_ms
           OR (payload->>'lease_until_ms')::bigint < (prior_payload->>'lease_until_ms')::bigint
           OR (payload->>'lease_until_ms')::bigint > least(now_ms + 2000, r_run.deadline_at_ms) THEN
            RAISE EXCEPTION 'run_not_current';
        END IF;
        RETURN NEW;
    END IF;

    FOR lock_key IN SELECT DISTINCT k FROM unnest(ARRAY[
        hashtextextended(e::text || ':' || c::text, 0), hashtextextended(old_e::text || ':' || old_c::text, 0)]) k
        WHERE k IS NOT NULL ORDER BY k LOOP
        PERFORM pg_advisory_xact_lock(lock_key);
    END LOOP;
    IF (old_e, old_c) IS DISTINCT FROM (e, c) AND EXISTS (
        SELECT 1 FROM execution.live_effect_sessions a
        JOIN execution.live_effect_control b ON b.attempt = a.attempt
        WHERE (a.e, a.c) = (old_e, old_c)
          AND (b.phase <> 'closed' OR (b.possibly_started AND NOT EXISTS (
              SELECT 1 FROM execution.live_effect_events z WHERE z.attempt = a.attempt AND z.phase = 'result_recorded'
          )))
    ) THEN
        RAISE EXCEPTION 'live_mutation_scope_refused';
    END IF;
    SELECT a.* INTO s
    FROM execution.live_effect_sessions a
    JOIN execution.live_effect_control b ON b.attempt = a.attempt
    WHERE (a.e, a.c) = (e, c)
      AND (b.phase <> 'closed' OR (b.possibly_started AND NOT EXISTS (
          SELECT 1 FROM execution.live_effect_events z WHERE z.attempt = a.attempt AND z.phase = 'result_recorded'
      )))
    ORDER BY a.generation DESC LIMIT 1;
    IF s.attempt IS NULL THEN
        RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
    END IF;
    SELECT * INTO STRICT x FROM execution.live_effect_control b WHERE b.attempt = s.attempt;
    IF prior_payload IS NOT NULL AND (
        coalesce(prior_payload->>'engagement_id', prior_payload->>'e') IS DISTINCT FROM coalesce(payload->>'engagement_id', payload->>'e')
        OR coalesce(prior_payload->>'campaign_id', prior_payload->>'c') IS DISTINCT FROM coalesce(payload->>'campaign_id', payload->>'c')
        OR prior_payload->'run_id' IS DISTINCT FROM payload->'run_id') THEN
        RAISE EXCEPTION 'live_mutation_scope_refused';
    END IF;
    SELECT w.operation INTO control_op FROM execution.private_live_writer_context w
    WHERE w.backend_pid = pg_backend_pid() AND w.transaction_id = pg_current_xact_id()
      AND (w.e, w.c, w.generation) = (s.e, s.c, s.generation)
      AND w.actor_oid = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user);
    allowed := control_op = 'withdraw';
    allowed := allowed AND (
        (TG_TABLE_SCHEMA = 'mission' AND TG_TABLE_NAME = 'withdrawals' AND TG_OP = 'INSERT')
        OR (TG_TABLE_SCHEMA = 'execution' AND TG_TABLE_NAME = 'autonomous_control' AND TG_OP = 'UPDATE'
            AND payload - ARRAY['status', 'lease_until_ms'] = prior_payload - ARRAY['status', 'lease_until_ms']
            AND payload->>'status' = 'stopped' AND payload->>'lease_until_ms' = '0')
        OR (TG_TABLE_SCHEMA = 'execution' AND TG_TABLE_NAME = 'session_fences' AND (
            (TG_OP = 'INSERT' AND payload->>'operation_id' IS NULL AND payload->>'phase' = 'idle')
            OR (TG_OP = 'UPDATE' AND payload - 'epoch' = prior_payload - 'epoch'
                AND (payload->>'epoch')::bigint = (prior_payload->>'epoch')::bigint + 1)))
    );
    IF control_op IN ('stop_run', 'run_unknown')
       AND s.writer_oid = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
       AND execution.private_live_actor('dw_m1_broker') THEN
        allowed := (
            TG_TABLE_SCHEMA = 'execution' AND TG_TABLE_NAME = 'autonomous_control' AND TG_OP = 'UPDATE'
            AND payload - ARRAY['status', 'lease_until_ms'] = prior_payload - ARRAY['status', 'lease_until_ms']
            AND payload->>'status' = CASE control_op WHEN 'stop_run' THEN 'stopped' ELSE 'unknown' END
            AND payload->>'lease_until_ms' = '0'
        ) OR (
            TG_TABLE_SCHEMA = 'execution' AND TG_TABLE_NAME = 'session_fences' AND TG_OP = 'UPDATE'
            AND payload - 'epoch' = prior_payload - 'epoch'
            AND (payload->>'epoch')::bigint = (prior_payload->>'epoch')::bigint + 1
        );
    END IF;
    IF TG_TABLE_SCHEMA = 'terrain' AND TG_TABLE_NAME IN ('claim_revisions', 'current_claims')
       AND TG_OP IN ('INSERT', 'UPDATE') AND execution.private_live_actor('dw_m1_terrain')
       AND EXISTS (
           SELECT 1 FROM execution.autonomous_runs a
           JOIN execution.live_dns_results b ON b.run_id = a.run_id
           LEFT JOIN execution.https_loop_steps st ON st.run_id = a.run_id AND st.step_index = b.step_index
           WHERE a.run_id = s.run_id AND b.attempt = s.attempt AND b.result = s.result_ref
             AND b.publication_obligation = 'terrain.issued_dns.v2'
             AND payload->>'operation_id' = CASE
                 WHEN a.request->>'version' = 'm1_https_loop_v1' THEN st.terrain_operation_id::text
                 ELSE a.terrain_operation::text END
       ) THEN
        allowed := true;
    END IF;
    IF TG_TABLE_SCHEMA = 'terrain' AND TG_TABLE_NAME = 'integrity_conflicts'
       AND TG_OP = 'INSERT' AND execution.private_live_actor('dw_m1_terrain')
       AND EXISTS (
           SELECT 1 FROM execution.autonomous_runs a
           JOIN execution.live_dns_results b ON b.run_id = a.run_id
           LEFT JOIN execution.https_loop_steps st ON st.run_id = a.run_id AND st.step_index = b.step_index
           WHERE a.run_id = s.run_id AND b.attempt = s.attempt AND b.result = s.result_ref
             AND b.publication_obligation = 'terrain.issued_dns.v2'
             AND payload->>'operation_id' = CASE
                 WHEN a.request->>'version' = 'm1_https_loop_v1' THEN st.terrain_operation_id::text
                 ELSE a.terrain_operation::text END
       ) THEN
        allowed := true;
    END IF;
    IF NOT coalesce(allowed, false) THEN RAISE EXCEPTION 'active_live_authority_fence'; END IF;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END $$;
REVOKE ALL ON FUNCTION execution.private_live_mutation_guard() FROM PUBLIC;

-- 4. Inference permission, reservation, timing window, intent and ContextPack admission.
CREATE OR REPLACE FUNCTION execution.inference_profile_matches(p jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
    SELECT coalesce(CASE WHEN p->>'version' = 'm1_inference_permission_v6' THEN
        (p->>'profile', p->>'recipient') IN (
            ('deepseek-genai-responses-low-v1', 'https://api.deepseek.com/responses'),
            ('deepseek-genai-v4-pro-low-v1', 'https://api.deepseek.com/responses'),
            ('deepseek-genai-v4-pro-low-v2', 'https://api.deepseek.com/responses'),
            ('fixture-genai-responses-v1', 'http://127.0.0.1:18083/responses')
        )
        AND jsonb_typeof(p#>'{operation_limits,request_bytes}') = 'number'
        AND p#>>'{operation_limits,request_bytes}' = '16384'
        AND (p->>'run_profile', p->>'execution_mode') IN (('autonomous-dns-genai-live-v1', 'live'), ('autonomous-dns-genai-lab-v1', 'live'))
        AND (p->>'profile' IN ('deepseek-genai-responses-low-v1', 'deepseek-genai-v4-pro-low-v1', 'deepseek-genai-v4-pro-low-v2')) = (p->>'run_profile' = 'autonomous-dns-genai-live-v1')
        AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback', 'admitted_repair_fields_codes']
        AND jsonb_typeof(p->'live_binding') = 'object'
        AND (p->'live_binding') - ARRAY['qualification_ref', 'release_sha256', 'resolver_ipv4'] = '{}'::jsonb
        AND (p#>>'{live_binding,qualification_ref}')::uuid <> '00000000-0000-0000-0000-000000000000'::uuid
        AND p#>>'{live_binding,release_sha256}' ~ '^[0-9a-f]{64}$'
        AND host((p#>>'{live_binding,resolver_ipv4}')::inet) = p#>>'{live_binding,resolver_ipv4}'
        AND family((p#>>'{live_binding,resolver_ipv4}')::inet) = 4
        AND (p->>'run_profile' = 'autonomous-dns-genai-lab-v1') = (p#>>'{live_binding,resolver_ipv4}' = '192.0.2.53')
        AND NOT EXISTS (
            SELECT 1 FROM (VALUES
                ('output_tokens_per_call', CASE WHEN p->>'profile' = 'deepseek-genai-v4-pro-low-v2' THEN 8192 ELSE 4096 END),
                ('decision_wall_time_ms', CASE WHEN p->>'profile' = 'deepseek-genai-v4-pro-low-v2' THEN 249000 ELSE 129000 END),
                ('per_call_wall_time_ms', CASE WHEN p->>'profile' = 'deepseek-genai-v4-pro-low-v2' THEN 122000 ELSE 62000 END),
                ('soft_completion_ms', CASE WHEN p->>'profile' = 'deepseek-genai-v4-pro-low-v2' THEN 120000 ELSE 60000 END),
                ('shared_wall_time_ms', CASE WHEN p->>'profile' = 'deepseek-genai-v4-pro-low-v2' THEN 244000 ELSE 124000 END),
                ('phase_wall_time_ms', CASE WHEN p->>'profile' = 'deepseek-genai-v4-pro-low-v2' THEN 254000 ELSE 134000 END),
                ('cancel_grace_ms', 2000), ('accounting_window_ms', 3000)
            ) b(k, v)
            WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
               OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text
        )
    WHEN p->>'version' = 'm1_inference_permission_v5' THEN
        (p->>'profile', p->>'recipient') IN (
            ('deepseek-genai-responses-low-v1', 'https://api.deepseek.com/responses'),
            ('deepseek-genai-v4-pro-low-v1', 'https://api.deepseek.com/responses'),
            ('deepseek-genai-v4-pro-low-v2', 'https://api.deepseek.com/responses'),
            ('fixture-genai-responses-v1', 'http://127.0.0.1:18083/responses'),
            ('fixture-genai-v4-pro-low-v2', 'http://127.0.0.1:18083/responses')
        )
        AND jsonb_typeof(p#>'{operation_limits,request_bytes}') = 'number'
        AND p#>>'{operation_limits,request_bytes}' = '16384'
        AND (p->>'run_profile', p->>'execution_mode') = ('autonomous-dns-fixture-v2', 'controlled_fixture')
        AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback', 'admitted_repair_fields_codes']
        AND NOT EXISTS (
            SELECT 1 FROM (VALUES
                ('output_tokens_per_call', CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 8192 ELSE 4096 END),
                ('decision_wall_time_ms', CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 249000 ELSE 129000 END),
                ('per_call_wall_time_ms', CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 122000 ELSE 62000 END),
                ('soft_completion_ms', CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 120000 ELSE 60000 END),
                ('shared_wall_time_ms', CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 244000 ELSE 124000 END),
                ('phase_wall_time_ms', CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 254000 ELSE 134000 END),
                ('cancel_grace_ms', 2000), ('accounting_window_ms', 3000)
            ) b(k, v)
            WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
               OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text
        )
    WHEN p->>'version' = 'm1_https_loop_v1' THEN
        (p->>'profile', p->>'recipient') = ('fixture-genai-v4-pro-low-v2', 'http://127.0.0.1:18083/responses')
        AND jsonb_typeof(p#>'{operation_limits,request_bytes}') = 'number'
        AND p#>>'{operation_limits,request_bytes}' = '16384'
        AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback', 'admitted_repair_fields_codes']
        AND NOT (p ? 'qualification_purpose')
        AND (
            ((p->>'run_profile', p->>'execution_mode') = ('m1_https_loop_v1', 'controlled_fixture')
                AND NOT (p ? 'live_binding'))
            OR ((p->>'run_profile', p->>'execution_mode') = ('autonomous-https-genai-lab-v1', 'live')
                AND jsonb_typeof(p->'live_binding') = 'object'
                AND (p->'live_binding') - ARRAY['qualification_ref', 'release_sha256', 'resolver_ipv4'] = '{}'::jsonb
                AND (p#>>'{live_binding,qualification_ref}')::uuid <> '00000000-0000-0000-0000-000000000000'::uuid
                AND p#>>'{live_binding,release_sha256}' ~ '^[0-9a-f]{64}$'
                AND p#>>'{live_binding,resolver_ipv4}' = '192.0.2.53')
        )
        AND NOT EXISTS (
            SELECT 1 FROM (VALUES
                ('output_tokens_per_call', 8192), ('decision_wall_time_ms', 249000),
                ('per_call_wall_time_ms', 122000), ('soft_completion_ms', 120000),
                ('shared_wall_time_ms', 244000), ('phase_wall_time_ms', 254000),
                ('cancel_grace_ms', 2000), ('accounting_window_ms', 3000)
            ) b(k, v)
            WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
               OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text
        )
    WHEN p->>'version' = 'm1_inference_permission_v4' THEN
        (p->>'profile', p->>'recipient') IN (
            ('deepseek-responses-reasoning-v1', 'https://api.deepseek.com/responses'),
            ('deepseek-responses-low-v1', 'https://api.deepseek.com/responses'),
            ('fixture-responses-v1', 'urn:duskweave:fixture:responses:v1'),
            ('fixture-chat-completions-v1', 'urn:duskweave:fixture:chat-completions:v1')
        )
        AND (p->>'run_profile', p->>'execution_mode') = ('autonomous-dns-fixture-v2', 'controlled_fixture')
        AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback', 'admitted_repair_fields_codes']
        AND NOT EXISTS (
            SELECT 1 FROM (VALUES
                ('decision_wall_time_ms', 129000), ('per_call_wall_time_ms', 62000),
                ('soft_completion_ms', 60000), ('shared_wall_time_ms', 124000),
                ('phase_wall_time_ms', 134000), ('cancel_grace_ms', 2000), ('accounting_window_ms', 3000)
            ) b(k, v)
            WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
               OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text
        )
    WHEN p->>'version' = 'm1_inference_permission_v3' THEN
        (p->>'run_profile', p->>'execution_mode', p->>'profile', p->>'recipient') IN (
            ('autonomous-dns-fixture-v1', 'controlled_fixture', 'deterministic-autonomous-v1', 'urn:duskweave:fixture:inference:v1'),
            ('autonomous-dns-provider-fixture-v1', 'controlled_fixture', 'deepseek-responses-autonomous-v1', 'https://api.deepseek.com/responses'),
            ('autonomous-dns-v1', 'live', 'deepseek-responses-autonomous-v1', 'https://api.deepseek.com/responses')
        )
        AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback', 'admitted_repair_fields_codes']
        AND NOT EXISTS (
            SELECT 1 FROM (VALUES
                ('decision_wall_time_ms', 45000), ('per_call_wall_time_ms', 20000),
                ('shared_wall_time_ms', 40000), ('phase_wall_time_ms', 50000),
                ('cancel_grace_ms', 2000), ('accounting_window_ms', 3000)
            ) b(k, v)
            WHERE p->'operation_limits'->b.k IS DISTINCT FROM to_jsonb(b.v)
        )
    ELSE (p->>'version', p->>'profile') IN (('m1_inference_permission_v1', 'deepseek-responses-v1'), ('m1_inference_permission_v2', 'deepseek-responses-envelope32-v1'))
        AND p->>'recipient' = 'https://api.deepseek.com/responses'
        AND NOT p ?| ARRAY['run_profile', 'execution_mode']
    END, false)
$$;

CREATE OR REPLACE FUNCTION execution.inference_reservation(
    action text,
    op uuid,
    q jsonb,
    expected_source jsonb
) RETURNS execution.inference_reservations
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    e uuid; c uuid;
    actor oid := (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user);
    prior execution.inference_reservations;
    source jsonb; consent jsonb; limits jsonb; totals jsonb;
    cap jsonb; held jsonb; prot jsonb;
    now_s bigint; next_generation bigint; claimed uuid; pro_v2 boolean; auto_ver boolean; has_live_binding boolean;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER')
       OR actor IS NULL
       OR EXISTS (SELECT 1 FROM pg_catalog.pg_roles WHERE oid = actor AND (rolsuper OR rolbypassrls OR rolcreatedb OR rolcreaterole))
       OR current_setting('fsync') <> 'on' OR current_setting('full_page_writes') <> 'on'
       OR current_setting('synchronous_commit') <> 'on'
       OR action NOT IN ('reserve', 'recover', 'cancel') OR action IS NULL
       OR op IS NULL OR op = '00000000-0000-0000-0000-000000000000'
       OR jsonb_typeof(q) IS DISTINCT FROM 'object'
       OR q->'version' IS DISTINCT FROM '"m1_inference_reservation_request_v1"'::jsonb
       OR q->'expected_mission_revision' IS DISTINCT FROM '1'::jsonb
       OR q->>'expected_mission_revision' IS DISTINCT FROM '1'
       OR coalesce(q->>'exercise_mode', '') NOT IN ('blind', 'defender_informed')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['engagement_id','campaign_id','operator_ref','permission_id','goal_ref']) k
           WHERE jsonb_typeof(q->k) IS DISTINCT FROM 'string'
           OR coalesce(q->>k, '') !~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
           OR q->>k = '00000000-0000-0000-0000-000000000000')
       OR q - ARRAY['version','engagement_id','campaign_id','operator_ref',
                   'expected_mission_revision','permission_id','goal_ref','exercise_mode'] <> '{}' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_request_refused';
    END IF;
    e := (q->>'engagement_id')::uuid; c := (q->>'campaign_id')::uuid;
    IF action <> 'recover' THEN
        PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    END IF;
    SELECT * INTO prior FROM execution.inference_reservations
    WHERE engagement_id = e AND campaign_id = c AND operation_id = op
    ORDER BY record_revision DESC LIMIT 1;
    IF FOUND THEN
        IF prior.writer_oid <> actor OR prior.request IS DISTINCT FROM q THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_identity_conflict';
        END IF;
        IF action <> 'cancel' OR prior.kind = 'cancelled_never_sent' THEN RETURN prior; END IF;
    ELSIF action = 'recover' THEN RETURN NULL;
    ELSIF action = 'cancel' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_missing_reservation';
    END IF;
    UPDATE execution.session_fences SET epoch = epoch + 1
    WHERE engagement_id = e AND campaign_id = c RETURNING operation_id INTO claimed;
    IF NOT FOUND THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_mission_missing';
    END IF;
    IF prior.operation_id IS NOT NULL THEN
        IF prior.kind <> 'reserved_not_sent' THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_cancellation_refused';
        END IF;
        prior.kind := 'cancelled_never_sent'; prior.record_revision := 2;
        prior.recorded_at := clock_timestamp();
        INSERT INTO execution.inference_reservations SELECT prior.* RETURNING * INTO prior;
        RETURN prior;
    END IF;
    IF claimed IS NOT NULL THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_busy';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.session_history WHERE engagement_id = e AND campaign_id = c AND operation_id = op)
       OR EXISTS (SELECT 1 FROM execution.prepared_withdrawals WHERE engagement_id = e AND campaign_id = c AND operation_id = op)
       OR EXISTS (SELECT 1 FROM cognition.reasoning_decisions rd WHERE (rd.e, rd.c, rd.d) = (e, c, op))
       OR EXISTS (SELECT 1 FROM cognition.reasoning_outbox ro JOIN cognition.reasoning_decisions rd ON rd.event = ro.event WHERE (rd.e, rd.c, rd.d) = (e, c, op)) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_identity_conflict';
    END IF;
    source := mission.current_inference_publication(e, c, op);
    IF source IS NULL OR source IS DISTINCT FROM expected_source THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_authority_unavailable';
    END IF;
    consent := source->'fields'->'m1_inference_permission';
    IF consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1') THEN
        IF NOT coalesce(consent->'field_classes' ? 'admitted_repair_fields_codes', false)
           OR NOT EXISTS (
               SELECT 1 FROM execution.native_input_bindings i
               WHERE (i.e, i.c, i.d, i.writer_oid) = (e, c, op, actor)
                 AND (consent->>'version' <> 'm1_https_loop_v1' OR (
                     i.pack_digest ~ '^[0-9a-f]{64}$'
                     AND i.request_id IS NOT NULL
                     AND i.private_endpoint_map IS NOT NULL
                     AND i.map_version = 'm1_https_endpoint_map_v1'
                     AND i.semantic_template IS NOT NULL
                     AND i.template_digest ~ '^[0-9a-f]{64}$'
                 ))
           ) THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_input_or_consent_refused';
        END IF;
        PERFORM execution.inference_window_ms(e, c, op);
    END IF;
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    IF consent IS NULL OR jsonb_typeof(consent) <> 'object'
       OR q IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1',
           'engagement_id',e,'campaign_id',c,'operator_ref',source->'fields'->'operator_ref',
           'expected_mission_revision',1,'permission_id',consent->'permission_id',
           'goal_ref',source->'fields'->'goal_ref','exercise_mode',source->'fields'->'exercise_mode')
       OR (source->'fields'->>'starts_at')::bigint > now_s
       OR now_s >= (source->'fields'->>'ends_at')::bigint
       OR (consent->>'starts_at')::bigint > now_s OR now_s >= (consent->>'ends_at')::bigint
       OR NOT execution.inference_profile_matches(consent)
       OR consent->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_consent_refused';
    END IF;
    IF (consent->>'provider_terms_ref')::uuid = '00000000-0000-0000-0000-000000000000'
       OR (consent->'engagement_id', consent->'campaign_id', consent->'operator_ref',
           consent->'authority_ref', consent->'authority_revision', consent->'goal_ref',
           consent->'exercise_mode', consent->'mission_revision') IS DISTINCT FROM
          (source->'engagement_id', source->'campaign_id', source->'fields'->'operator_ref',
           source->'fields'->'authority_ref', source->'fields'->'authority_revision',
           source->'fields'->'goal_ref', source->'fields'->'exercise_mode', '1'::jsonb)
       OR (consent->>'starts_at')::bigint < (source->'fields'->>'starts_at')::bigint
       OR (consent->>'ends_at')::bigint > (source->'fields'->>'ends_at')::bigint THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_consent_refused';
    END IF;
    limits := consent->'operation_limits'; totals := consent->'campaign_limits';
    auto_ver := consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1');
    has_live_binding := consent->>'version' = 'm1_inference_permission_v6'
        OR (consent->>'version' = 'm1_https_loop_v1' AND consent->>'execution_mode' = 'live');
    pro_v2 := coalesce(consent->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1')
        AND consent->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2'), false);
    IF (SELECT count(*) FROM jsonb_object_keys(consent)) <> (CASE WHEN auto_ver THEN CASE WHEN has_live_binding THEN 22 ELSE 21 END ELSE 19 END)
       OR jsonb_typeof(consent->'provider_terms_ref') IS DISTINCT FROM 'string'
       OR (consent->>'provider_terms_ref')::uuid IS NULL
       OR jsonb_typeof(consent->'field_classes') IS DISTINCT FROM 'array'
       OR jsonb_array_length(consent->'field_classes') NOT BETWEEN 1 AND (CASE WHEN auto_ver THEN 9 ELSE 8 END)
       OR (SELECT count(DISTINCT v) FROM jsonb_array_elements(consent->'field_classes') v)
          <> jsonb_array_length(consent->'field_classes')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(consent->'field_classes') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(consent->'field_classes') v
           WHERE v NOT IN ('mission_question_constraints','scoped_opaque_references',
               'owner_qualified_summaries','admitted_operator_premises','manifest_availability',
               'omissions_unknown','capacity','admitted_repair_fields_codes')
             AND NOT (auto_ver AND v = 'admitted_campaign_decision_feedback'))
       OR (SELECT count(*) FROM jsonb_object_keys(limits)) <> (CASE WHEN auto_ver THEN 22 ELSE 16 END)
       OR (SELECT count(*) FROM jsonb_object_keys(totals)) <> 5
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['starts_at','ends_at','authority_revision','mission_revision']) k
           WHERE jsonb_typeof(consent->k) IS DISTINCT FROM 'number' OR consent->>k !~ '^-?[0-9]+$') THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_consent_refused';
    END IF;
    IF EXISTS (SELECT 1 FROM (VALUES ('request_bytes',1,16384),('response_bytes',1,65536),
        ('decision_wall_time_ms',1,CASE WHEN pro_v2 THEN 249000 WHEN consent->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1') THEN 129000 WHEN consent->>'version' = 'm1_inference_permission_v3' THEN 45000 ELSE 20000 END),
        ('context_bytes',1,CASE WHEN auto_ver THEN 8192 ELSE 4096 END),('candidates',1,8),('configuration_bytes',1,16384),
        ('connect_time_ms',1,5000),('proposal_bytes',1,8192),('repair_bytes',1,2048),('text_field_bytes',1,1024),
        ('repair_error_codes',1,8),('json_depth',1,CASE WHEN consent->>'version' = 'm1_inference_permission_v1' THEN 8 ELSE 32 END),
        ('collection_entries',1,64),('calls_per_decision',2,2),('output_tokens_per_call',CASE WHEN pro_v2 THEN 8192 ELSE 4096 END,CASE WHEN pro_v2 THEN 8192 ELSE 4096 END),('concurrency',1,1)) AS b(name,floor,ceiling)
        WHERE CASE WHEN jsonb_typeof(limits->b.name) = 'number' AND limits->>b.name ~ '^[0-9]+$'
            THEN (limits->>b.name)::numeric < b.floor OR (limits->>b.name)::numeric > b.ceiling ELSE true END) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_limits_refused';
    END IF;
    IF (limits->>'connect_time_ms')::bigint > (limits->>'decision_wall_time_ms')::bigint THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_limits_refused';
    END IF;
    cap := jsonb_build_object('calls',2,'request_bytes',2*(limits->>'request_bytes')::bigint,
        'response_bytes',2*(limits->>'response_bytes')::bigint,'output_token_capacity',2*(limits->>'output_tokens_per_call')::bigint,
        'wall_time_ms',(limits->>'decision_wall_time_ms')::bigint);
    prot := execution.https_loop_protected_held(e, c, op, true);
    SELECT jsonb_object_agg(k, used) INTO held FROM (
        SELECT v.key AS k, coalesce(sum((h.capacity->>v.key)::numeric), 0) + (prot->>v.key)::numeric AS used
        FROM jsonb_each(cap) v
        LEFT JOIN execution.inference_reservations h
          ON h.engagement_id = e AND h.campaign_id = c AND h.record_revision = 1
          AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x WHERE (x.engagement_id, x.campaign_id, x.operation_id) = (e, c, h.operation_id) AND x.kind = 'cancelled_never_sent')
        GROUP BY v.key, prot
    ) s;
    IF EXISTS (SELECT 1 FROM jsonb_each(cap) v
        WHERE jsonb_typeof(totals->v.key) IS DISTINCT FROM 'number' OR totals->>v.key !~ '^[0-9]+$'
           OR (totals->>v.key)::numeric <= 0 OR (totals->>v.key)::numeric > 9223372036854775807
           OR (held->>v.key)::numeric + (v.value::text)::numeric > (totals->>v.key)::numeric) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_capacity_exhausted';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.inference_reservations h
        WHERE h.engagement_id = e AND h.campaign_id = c AND h.record_revision = 1
          AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x WHERE (x.engagement_id, x.campaign_id, x.operation_id) = (e, c, h.operation_id) AND x.kind = 'cancelled_never_sent')
          AND NOT EXISTS (SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = (e, c, h.operation_id))) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_busy';
    END IF;
    SELECT coalesce(max(generation), 0) + 1 INTO next_generation FROM execution.inference_reservations WHERE engagement_id = e AND campaign_id = c;
    INSERT INTO execution.inference_reservations
        (engagement_id, campaign_id, operation_id, generation, record_revision, writer_oid,
         request, registration_operation_id, registration_event_id, generation_call_id, repair_call_id, capacity, kind)
    VALUES (e, c, op, next_generation, 1, actor, q, (source->>'operation_id')::uuid,
        (source->>'event_id')::uuid, gen_random_uuid(), gen_random_uuid(), cap, 'reserved_not_sent')
    RETURNING * INTO prior;
    RETURN prior;
END $$;

CREATE OR REPLACE FUNCTION execution.inference_window_ms(e uuid, c uuid, d uuid) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    s jsonb; p jsonb; n bigint; l jsonb; run uuid; phase text;
    started bigint; phase_end bigint; decision_end bigint;
    w execution.inference_decision_windows; ar execution.autonomous_runs;
    lq execution.live_qualifications; pro_v2 boolean; is_live_qual boolean; exp_purpose text;
BEGIN
    s := mission.current_inference_publication(e, c, d);
    p := s->'fields'->'m1_inference_permission';
    l := p->'operation_limits';
    IF EXISTS (
        SELECT 1 FROM execution.inference_closures z LEFT JOIN cognition.reasoning_decisions q
            ON (q.e, q.c, q.d) = (z.e, z.c, z.d)
        WHERE (z.e, z.c) = (e, c) AND (
            q.event IS NULL OR NOT EXISTS (
                SELECT 1 FROM trajectory.reasoning_history h
                WHERE (h.engagement_id, h.campaign_id, h.operation_id, h.event_id, h.status, h.contract) = (z.e, z.c, z.d, q.event, 'accepted', q.payload)
            ) OR EXISTS (
                SELECT 1 FROM trajectory.reasoning_history h
                WHERE (h.engagement_id, h.campaign_id) = (z.e, z.c)
                  AND (h.operation_id = z.d OR h.event_id = q.event) AND h.status = 'anomaly'
            )
        )
    ) THEN
        RAISE EXCEPTION 'inference_prior_history_unqualified';
    END IF;
    IF p->>'version' IN ('m1_inference_permission_v3', 'm1_inference_permission_v4', 'm1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN
        ar := execution.find_decision_run(e, c, d);
        run := ar.run_id;
        IF ar.request->>'version' = 'm1_https_loop_v1' THEN
            SELECT st.decision_phase INTO phase FROM execution.https_loop_steps st
            WHERE st.run_id = run AND st.decision_id = d;
        ELSE
            phase := CASE WHEN ar.decision_one = d THEN 'decision_one' ELSE 'decision_two' END;
        END IF;
        PERFORM execution.run_current(run);
        IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = run AND y.phase = phase AND y.claimed) THEN
            RAISE EXCEPTION 'run_phase_refused';
        END IF;
    END IF;
    pro_v2 := coalesce(p->>'version' IN ('m1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1')
        AND p->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2'), false);
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF s IS NULL OR NOT execution.inference_profile_matches(p) OR p->>'version' = 'm1_inference_permission_v1'
       OR p->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb
       OR NOT coalesce(p->'field_classes' ? 'admitted_repair_fields_codes', false)
       OR n < (s->'fields'->>'starts_at')::bigint * 1000 OR n < (p->>'starts_at')::bigint * 1000
       OR EXISTS (
           SELECT 1 FROM (VALUES
               ('json_depth', 32), ('collection_entries', 64), ('response_bytes', 65536),
               ('proposal_bytes', 8192), ('text_field_bytes', 1024), ('repair_bytes', 2048),
               ('repair_error_codes', 8), ('connect_time_ms', 5000), ('calls_per_decision', 2),
               ('output_tokens_per_call', CASE WHEN pro_v2 THEN 8192 ELSE 4096 END), ('concurrency', 1)
           ) b(k, v)
           WHERE l->b.k IS DISTINCT FROM to_jsonb(b.v)
       ) THEN
        RAISE EXCEPTION 'inference_authority_unavailable';
    END IF;
    is_live_qual := p->>'version' = 'm1_inference_permission_v6'
        OR (p->>'version' = 'm1_https_loop_v1' AND ar.request->>'execution_mode' = 'live');
    IF is_live_qual THEN
        exp_purpose := CASE ar.request->>'run_profile'
            WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE'
            WHEN 'autonomous-https-genai-lab-v1' THEN 'HTTPS_LOOP_LAB'
            ELSE 'LAB' END;
        SELECT * INTO lq FROM execution.live_qualifications z WHERE z.qualification_ref = ar.qualification_ref;
        IF NOT FOUND OR n < lq.valid_from_ms OR n >= lq.valid_until_ms
           OR ar.qualification_ref IS DISTINCT FROM (ar.request#>>'{live_binding,qualification_ref}')::uuid
           OR ar.request#>>'{live_binding,release_sha256}' IS DISTINCT FROM ar.release_sha256
           OR lq.run_profile IS DISTINCT FROM (ar.request->>'run_profile')
           OR lq.purpose IS DISTINCT FROM exp_purpose
           OR lq.release_sha256 IS DISTINCT FROM ar.release_sha256
           OR host(lq.resolver_ipv4) IS DISTINCT FROM (ar.request#>>'{live_binding,resolver_ipv4}') THEN
            RAISE EXCEPTION 'live_qualification_unavailable';
        END IF;
    END IF;
    IF p->>'version' IN ('m1_inference_permission_v4', 'm1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN
        SELECT * INTO w FROM execution.inference_decision_windows a WHERE (a.e, a.c, a.d) = (e, c, d);
        IF NOT FOUND THEN
            SELECT recorded_at_ms INTO started FROM execution.autonomous_phases WHERE run_id = run AND autonomous_phases.phase = phase AND status = 'started';
            IF started IS NULL OR n >= started + 5000 THEN RAISE EXCEPTION 'inference_preparation_expired'; END IF;
            SELECT least(started + CASE WHEN pro_v2 THEN 254000 ELSE 134000 END, x.deadline_at_ms, (p->>'ends_at')::bigint * 1000, (s#>>'{fields,ends_at}')::bigint * 1000, lq.valid_until_ms)
                INTO phase_end FROM execution.autonomous_runs x WHERE x.run_id = run;
            decision_end := least(n + CASE WHEN pro_v2 THEN 249000 ELSE 129000 END, phase_end, lq.valid_until_ms);
            INSERT INTO execution.inference_decision_windows
            VALUES (e, c, d, (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user),
                n, least(n + CASE WHEN pro_v2 THEN 244000 ELSE 124000 END, decision_end - 5000, lq.valid_until_ms), decision_end, phase_end)
            ON CONFLICT DO NOTHING;
            SELECT * INTO w FROM execution.inference_decision_windows a WHERE (a.e, a.c, a.d) = (e, c, d);
        END IF;
        IF w.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) OR n >= w.active_until_ms THEN
            RAISE EXCEPTION 'inference_deadline';
        END IF;
        RETURN least(w.active_until_ms - n, (s#>>'{fields,ends_at}')::bigint * 1000 - n, (p->>'ends_at')::bigint * 1000 - n, lq.valid_until_ms - n);
    END IF;
    RETURN least(CASE WHEN p->>'version' IN ('m1_inference_permission_v3', 'm1_inference_permission_v4', 'm1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN CASE WHEN pro_v2 THEN 244000 WHEN p->>'version' IN ('m1_inference_permission_v4', 'm1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN 124000 ELSE 40000 END ELSE 20000 END, (l->>'decision_wall_time_ms')::bigint, (s->'fields'->>'ends_at')::bigint * 1000 - n, (p->>'ends_at')::bigint * 1000 - n);
END $$;

CREATE OR REPLACE FUNCTION execution.genai_expected_intent(p jsonb, r execution.inference_reservations, k uuid, codes jsonb) RETURNS jsonb
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    i jsonb; recipient text; pro_v2 boolean; out_tokens integer;
    soft_ms bigint; hard_ms bigint; shared_ms bigint; decision_ms bigint; phase_ms bigint;
    https_loop boolean; nb execution.native_input_bindings;
BEGIN
    recipient := CASE p->>'inference_profile'
        WHEN 'fixture-genai-responses-v1' THEN 'http://127.0.0.1:18083/responses'
        WHEN 'fixture-genai-v4-pro-low-v2' THEN 'http://127.0.0.1:18083/responses'
        WHEN 'deepseek-genai-responses-low-v1' THEN 'https://api.deepseek.com/responses'
        WHEN 'deepseek-genai-v4-pro-low-v1' THEN 'https://api.deepseek.com/responses'
        WHEN 'deepseek-genai-v4-pro-low-v2' THEN 'https://api.deepseek.com/responses'
        ELSE NULL END;
    IF recipient IS NULL OR k NOT IN (r.generation_call_id, r.repair_call_id) OR r.generation < 1
       OR r.generation_call_id = r.repair_call_id OR r.generation_call_id = '00000000-0000-0000-0000-000000000000'::uuid
       OR r.repair_call_id = '00000000-0000-0000-0000-000000000000'::uuid
       OR p->>'version' NOT IN ('m1_context_pack_autonomous_v3', 'm1_context_pack_autonomous_v4', 'm1_https_loop_v1')
       OR p->>'engagement_id' IS DISTINCT FROM r.engagement_id::text
       OR p->>'campaign_id' IS DISTINCT FROM r.campaign_id::text
       OR p->>'decision_id' IS DISTINCT FROM r.operation_id::text
       OR p->'permission_id' IS DISTINCT FROM r.request->'permission_id'
       OR p->'mission_revision' IS DISTINCT FROM r.request->'expected_mission_revision'
       OR jsonb_typeof(codes) IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'inference_intent_refused';
    END IF;
    IF (k = r.generation_call_id AND codes <> '[]') OR (k = r.repair_call_id AND jsonb_array_length(codes) NOT BETWEEN 1 AND 8)
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(codes) v
           WHERE jsonb_typeof(v) IS DISTINCT FROM 'string'
              OR v#>>'{}' NOT IN ('json_size', 'json_structure', 'json_trailing', 'proposal_shape', 'proposal_version', 'unexpected_view_request', 'proposal_text')
       ) THEN
        RAISE EXCEPTION 'inference_intent_refused';
    END IF;
    https_loop := p->>'version' = 'm1_https_loop_v1';
    IF https_loop THEN
        SELECT * INTO nb FROM execution.native_input_bindings idx
        WHERE (idx.e, idx.c, idx.d) = (r.engagement_id, r.campaign_id, r.operation_id);
        IF NOT FOUND OR nb.writer_oid IS DISTINCT FROM r.writer_oid
           OR nb.semantic_template IS NULL OR nb.template_digest !~ '^[0-9a-f]{64}$'
           OR nb.semantic_template->>'template_version' IS DISTINCT FROM 'm1-https-loop-v1'
           OR nb.semantic_template->>'schema_version' IS DISTINCT FROM 'm1_https_loop_v1'
           OR jsonb_typeof(nb.semantic_template->'instructions') IS DISTINCT FROM 'string'
           OR jsonb_typeof(nb.semantic_template->'schema') IS DISTINCT FROM 'object'
           OR nb.template_digest IS DISTINCT FROM encode(sha256(convert_to(execution.genai_canonical(nb.semantic_template), 'UTF8')), 'hex') THEN
            RAISE EXCEPTION 'inference_intent_refused';
        END IF;
    END IF;
    pro_v2 := p->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2');
    out_tokens := CASE WHEN pro_v2 THEN 8192 ELSE 4096 END;
    soft_ms := CASE WHEN pro_v2 THEN 120000 ELSE 60000 END;
    hard_ms := CASE WHEN pro_v2 THEN 122000 ELSE 62000 END;
    shared_ms := CASE WHEN pro_v2 THEN 244000 ELSE 124000 END;
    decision_ms := CASE WHEN pro_v2 THEN 249000 ELSE 129000 END;
    phase_ms := CASE WHEN pro_v2 THEN 254000 ELSE 134000 END;
    i := jsonb_build_object(
        'version', 'dw-inference-intent-v1',
        'permission_version', CASE WHEN https_loop THEN 'm1_https_loop_v1' WHEN p->>'version' = 'm1_context_pack_autonomous_v4' THEN 'm1_inference_permission_v6' ELSE 'm1_inference_permission_v5' END,
        'run_version', CASE WHEN https_loop THEN 'm1_https_loop_v1' WHEN p->>'version' = 'm1_context_pack_autonomous_v4' THEN 'm1_autonomous_run_request_v4' ELSE 'm1_autonomous_run_request_v3' END,
        'engagement_id', p->'engagement_id', 'campaign_id', p->'campaign_id', 'decision_id', p->'decision_id',
        'permission_id', p->'permission_id', 'mission_revision', p->'mission_revision',
        'generation', r.generation, 'generation_call_id', r.generation_call_id, 'repair_call_id', r.repair_call_id,
        'call_id', k, 'call_role', CASE WHEN k = r.generation_call_id THEN 'generation' ELSE 'repair' END,
        'run_profile', p->'run_profile', 'execution_mode', p->'execution_mode', 'protocol', 'openai-resp-v1',
        'profile', p->'inference_profile', 'recipient', recipient,
        'model', CASE WHEN p->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v1', 'deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 'deepseek-v4-pro' ELSE 'deepseek-flash' END,
        'adapter_release', 'genai-a9504f1-dw1',
        'template_version', CASE WHEN https_loop THEN nb.semantic_template->>'template_version' ELSE 'm1-autonomous-inference-v3' END,
        'schema_version', CASE WHEN https_loop THEN nb.semantic_template->>'schema_version' ELSE 'm1_inference_proposal_v1' END,
        'timing_profile', CASE WHEN pro_v2 THEN 'reasoning-bounded-pro-v2' ELSE 'reasoning-bounded-v1' END,
        'instructions', CASE WHEN https_loop THEN nb.semantic_template->>'instructions'
            ELSE 'DuskWeave autonomous inference v1: propose one objective-led hypothesis, alternative, expected and falsifying observation and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not playbooks or permission. Operator seeds are premises, observations are not facts, and historical decisions do not grant current authority. Use exact references[].id and qualified manifests[].reference values only. observe_more with a DNS manifest proposes one observation for one seed; observe_more with view_request requests only an owner view. Do not combine them. Do not invent endpoints, references, availability, access or scope. Missing, stale, conflicted and fixture sources retain their limits. Explain uncertainty and any retention/revision from predecessor feedback. Retain, defer and stop are valid choices. Synthesis/unsupported effects are explanations of a capability gap, never permission to execute. Return concise safe explanation, never private reasoning or raw client content. Output grants no dispatch. Fixture results cannot establish live target facts.' END,
        'schema', CASE WHEN https_loop THEN nb.semantic_template->'schema' ELSE execution.genai_proposal_schema(p) END,
        'repair_error_codes', codes,
        'settings', jsonb_build_object('reasoning_effort', 'low', 'max_output_tokens', out_tokens, 'stream', false, 'store', false, 'tools', '[]'::jsonb),
        'limits', jsonb_build_object(
            'request_bytes', 16384, 'response_bytes', 65536, 'context_bytes', 8192, 'configuration_bytes', 16384,
            'connect_time_ms', 5000, 'soft_completion_ms', soft_ms, 'per_call_wall_time_ms', hard_ms, 'shared_wall_time_ms', shared_ms,
            'decision_wall_time_ms', decision_ms, 'phase_wall_time_ms', phase_ms, 'cancel_grace_ms', 2000, 'accounting_window_ms', 3000,
            'output_tokens_per_call', out_tokens, 'calls_per_decision', 2, 'concurrency', 1
        )
    );
    IF p->>'version' = 'm1_context_pack_autonomous_v4' OR (https_loop AND p->>'execution_mode' = 'live') THEN
        i := i || jsonb_build_object('live_binding', p->'live_binding', 'qualification_purpose', p->'qualification_purpose');
    END IF;
    RETURN i;
END $$;

CREATE OR REPLACE FUNCTION execution.genai_binding_valid(e uuid, c uuid, d uuid, k uuid, p text, i text, b text, h text) RETURNS boolean
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE r execution.inference_reservations; ar execution.autonomous_runs; request jsonb; permission jsonb; expected jsonb; p_json jsonb;
BEGIN
    SELECT * INTO r FROM execution.inference_reservations a WHERE (a.engagement_id, a.campaign_id, a.operation_id) = (e, c, d) AND a.record_revision = 1;
    ar := execution.find_decision_run(e, c, d);
    SELECT x.request, m.contract#>'{fields,m1_inference_permission}' INTO request, permission
    FROM execution.autonomous_runs x JOIN mission.registration_outbox m ON (m.engagement_id, m.campaign_id) = (x.e, x.c)
    WHERE x.run_id = ar.run_id;
    p_json := p::jsonb;
    IF r.operation_id IS NULL OR request->>'version' NOT IN ('m1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4', 'm1_https_loop_v1')
       OR permission->>'version' IS DISTINCT FROM (CASE request->>'version' WHEN 'm1_https_loop_v1' THEN 'm1_https_loop_v1' WHEN 'm1_autonomous_run_request_v4' THEN 'm1_inference_permission_v6' ELSE 'm1_inference_permission_v5' END)
       OR NOT execution.inference_profile_matches(permission)
       OR request->>'inference_profile' IS DISTINCT FROM permission->>'profile'
       OR p_json->>'inference_profile' IS DISTINCT FROM permission->>'profile'
       OR (request->>'version' = 'm1_autonomous_run_request_v3' AND (request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v2' OR request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'))
       OR (request->>'version' = 'm1_https_loop_v1' AND (
           (request->>'execution_mode' = 'controlled_fixture' AND (
               request->>'run_profile' IS DISTINCT FROM 'm1_https_loop_v1'
               OR (request ? 'live_binding') OR (permission ? 'live_binding') OR (p_json ?| ARRAY['live_binding', 'qualification_purpose'])
           )) OR (request->>'execution_mode' = 'live' AND (
               request->>'run_profile' IS DISTINCT FROM 'autonomous-https-genai-lab-v1'
               OR request->'live_binding' IS DISTINCT FROM permission->'live_binding'
               OR p_json->'live_binding' IS DISTINCT FROM permission->'live_binding'
               OR p_json->>'qualification_purpose' IS DISTINCT FROM 'HTTPS_LOOP_LAB'
           )) OR request->>'execution_mode' NOT IN ('controlled_fixture', 'live')
       ))
       OR (request->>'version' = 'm1_autonomous_run_request_v4' AND (
           request->>'run_profile' NOT IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1')
           OR request->>'execution_mode' IS DISTINCT FROM 'live'
           OR request->'live_binding' IS DISTINCT FROM permission->'live_binding'
           OR p_json->'live_binding' IS DISTINCT FROM permission->'live_binding'
           OR p_json->>'qualification_purpose' IS DISTINCT FROM CASE request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END
       ))
       OR p_json->>'run_profile' IS DISTINCT FROM request->>'run_profile'
       OR p_json->>'execution_mode' IS DISTINCT FROM request->>'execution_mode'
       OR p_json->'permission_id' IS DISTINCT FROM permission->'permission_id'
       OR octet_length(p) > 8192 OR octet_length(i) > 16384 OR b <> '' THEN
        RETURN false;
    END IF;
    IF request->>'version' = 'm1_https_loop_v1' AND NOT EXISTS (
        SELECT 1 FROM execution.native_input_bindings nb
        WHERE (nb.e, nb.c, nb.d) = (e, c, d)
          AND nb.writer_oid = r.writer_oid AND nb.request_id = ar.run_id
          AND nb.pack_digest = encode(sha256(convert_to(p, 'UTF8')), 'hex')
          AND nb.private_endpoint_map IS NOT NULL AND nb.map_version = 'm1_https_endpoint_map_v1'
          AND nb.semantic_template IS NOT NULL AND nb.template_digest ~ '^[0-9a-f]{64}$'
    ) THEN
        RETURN false;
    END IF;
    expected := execution.genai_expected_intent(p_json, r, k, i::jsonb->'repair_error_codes');
    RETURN coalesce(i = execution.genai_canonical(expected) AND h = execution.genai_binding_digest(p, i), false);
EXCEPTION WHEN OTHERS THEN
    RETURN false;
END $$;

CREATE OR REPLACE FUNCTION execution.assert_successor_pack(p jsonb, reg jsonb, request jsonb) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    ref jsonb; candidate jsonb; original jsonb; expected jsonb; ordinal integer;
    https_loop boolean := request->>'version' = 'm1_https_loop_v1';
    is_live boolean := request->>'version' = 'm1_autonomous_run_request_v4'
        OR (request->>'version' = 'm1_https_loop_v1' AND request->>'execution_mode' = 'live');
    exp_purpose text := CASE
        WHEN request->>'version' = 'm1_https_loop_v1' THEN 'HTTPS_LOOP_LAB'
        WHEN request->>'run_profile' = 'autonomous-dns-genai-live-v1' THEN 'LIVE'
        ELSE 'LAB' END;
    exp_https_source text := CASE WHEN request->>'execution_mode' = 'live' THEN 'issued_https_acquisition' ELSE 'fixture_https_acquisition' END;
    keys text[] := ARRAY['version','engagement_id','campaign_id','decision_id','permission_id','mission_revision','goal_ref',
        'exercise_mode','scope','question','evaluated_at','owner_views','references','candidates','omissions','pending_unknown',
        'manifests','remaining_capacity','execution_mode','run_profile','inference_profile','predecessor']
        || CASE WHEN is_live THEN ARRAY['live_binding','qualification_purpose'] ELSE '{}'::text[] END;
    cand_keys text[] := CASE WHEN https_loop
        THEN ARRAY['reference','priority','status','terrain_references','endpoint_references','https_references']
        ELSE ARRAY['reference','priority','status','terrain_references'] END;
BEGIN
    IF NOT is_live AND (p ?| ARRAY['live_binding', 'qualification_purpose']) THEN
        RAISE EXCEPTION 'run_pack_source_refused';
    END IF;
    IF is_live AND (
        p->'live_binding' IS DISTINCT FROM reg#>'{fields,m1_inference_permission,live_binding}'
        OR p->>'qualification_purpose' IS DISTINCT FROM exp_purpose
    ) THEN
        RAISE EXCEPTION 'run_pack_source_refused';
    END IF;
    IF jsonb_typeof(p) IS DISTINCT FROM 'object' OR NOT p ?& keys OR p - keys <> '{}'
       OR p->'permission_id' IS DISTINCT FROM reg#>'{fields,m1_inference_permission,permission_id}'
       OR p->>'mission_revision' IS DISTINCT FROM '1' OR jsonb_typeof(p->'mission_revision') IS DISTINCT FROM 'number'
       OR p->'goal_ref' IS DISTINCT FROM reg#>'{fields,goal_ref}'
       OR p->'exercise_mode' IS DISTINCT FROM reg#>'{fields,exercise_mode}'
       OR p->'scope' IS DISTINCT FROM jsonb_build_object('owner', 'mission', 'publication_ref', reg->'event_id')
       OR (https_loop AND p->>'question' IS DISTINCT FROM 'Select next authorized candidate for dns_udp or single-endpoint https-head-v1 (endpoint_N), or retain/defer/stop/capability_gap. Use issued IDs only; HEAD tests one endpoint and never proves whole-host reachability or objective completion.')
       OR (NOT https_loop AND p->>'question' IS DISTINCT FROM 'Which already-authorized, mission-relevant HTTPS candidate is the best next subject for access-validation planning within the permitted contact envelope, and which routing or response premise still needs observation, a different candidate, or deferral?'
           AND (request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4'
               OR p->>'version' IS DISTINCT FROM 'm1_context_pack_autonomous_v4'
               OR p->>'question' IS DISTINCT FROM 'Which mission-relevant routing uncertainty warrants a bounded DNS observation, or is retain, defer or stop better? An admitted_operator_seed reference identifies a hostname seed held by the Broker; select that asset reference with dns_udp without inventing or disclosing the name. A seed is a premise, not observed DNS and not an execution grant; the Broker rechecks current input binding, scope and permission. Only DNS observation is executable in this run; HTTPS contact is unavailable. Read Terrain availability and omissions together: shown_claims=0 means no eligible claims are displayed, not never queried or not loaded. A view_request records owner_view_requested and ends this run without fetching a view, executing DNS or another decision; it does not refresh an already loaded empty view. With committed predecessor feedback, explain whether evidence supports or revises the hypothesis; further effects remain proposals. DNS is optional.'))
       OR jsonb_typeof(p->'evaluated_at') IS DISTINCT FROM 'number' OR coalesce(p->>'evaluated_at', '') !~ '^[1-9][0-9]*$'
       OR jsonb_typeof(p->'references') IS DISTINCT FROM 'array' OR jsonb_typeof(p->'candidates') IS DISTINCT FROM 'array'
       OR jsonb_typeof(p->'owner_views') IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'run_pack_source_refused';
    END IF;
    IF jsonb_array_length(p->'candidates') NOT BETWEEN 1 AND 8 OR jsonb_array_length(p->'owner_views') <> 6
       OR (SELECT count(DISTINCT v->>'owner') FROM jsonb_array_elements(p->'owner_views') v) <> 6
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(p->'owner_views') v WHERE coalesce(v->>'owner', '') NOT IN
           ('mission', 'cyber_terrain', 'foothold_graph', 'attack_path_view', 'objective_state', 'campaign_trajectory'))
       OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(p->'owner_views') v WHERE v = jsonb_build_object('owner', 'mission',
           'availability', 'available', 'revision', 1, 'frontier', reg->'event_id', 'evaluated_at', p->'evaluated_at'))
       OR (SELECT count(*) FROM jsonb_array_elements(p->'references') v WHERE v->>'owner' = 'mission') <> 1
       OR (SELECT count(*) FROM jsonb_array_elements(p->'references') v WHERE v->>'owner' = 'operator') <> jsonb_array_length(p->'candidates') THEN
        RAISE EXCEPTION 'run_pack_source_refused';
    END IF;
    FOR ref IN SELECT value FROM jsonb_array_elements(p->'references') LOOP
        CASE ref->>'owner'
            WHEN 'mission' THEN
                expected := jsonb_build_object('id', 'mission', 'owner', 'mission', 'entity', reg->'campaign_id', 'revision', 1,
                    'lineage', jsonb_build_array(reg->'event_id', reg->'operation_id'), 'perceived_at', reg->'occurred_at',
                    'time_uncertainty', 'unspecified_by_owner', 'status', 'owner_published', 'tier', NULL,
                    'freshness', 'as_of_snapshot', 'completeness', 'scope_reference');
                IF ref IS DISTINCT FROM expected THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
            WHEN 'operator' THEN
                SELECT value INTO original FROM jsonb_array_elements(request#>'{context,candidates}') WHERE value->'asset_ref' = ref->'entity';
                IF original IS NULL OR NOT (reg#>'{fields,included_assets}') @> jsonb_build_array(ref->'entity')
                   OR (reg#>'{fields,excluded_assets}') @> jsonb_build_array(ref->'entity') THEN
                    RAISE EXCEPTION 'run_pack_source_refused';
                END IF;
                expected := jsonb_build_object('id', ref->'id', 'owner', 'operator', 'entity', original->'asset_ref', 'revision', NULL, 'lineage', '[]'::jsonb,
                    'source', 'admitted_operator_seed', 'perceived_at', NULL, 'time_uncertainty', 'unknown', 'status', 'premise',
                    'tier', NULL, 'freshness', 'unknown', 'completeness', 'declared_only');
                IF ref IS DISTINCT FROM expected THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
            WHEN 'cyber_terrain' THEN
                IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(p->'references') a WHERE a->>'owner' = 'operator' AND a->'entity' = ref->'asset_ref') THEN
                    RAISE EXCEPTION 'run_pack_source_refused';
                END IF;
                IF https_loop AND coalesce(ref->>'id', '') ~ '^endpoint_[0-9]+$' THEN
                    IF NOT ref ?& ARRAY['id','owner','source','entity','asset_ref','parent_revision','port','method','approved_path','valid_until','ttl_seconds','freshness','status']
                       OR ref - ARRAY['id','owner','source','entity','asset_ref','parent_revision','port','method','approved_path','valid_until','ttl_seconds','freshness','status'] <> '{}'
                       OR ref->>'source' IS DISTINCT FROM 'issued_endpoint_binding'
                       OR ref->'port' IS DISTINCT FROM '443'::jsonb OR ref->>'method' IS DISTINCT FROM 'HEAD' THEN
                        RAISE EXCEPTION 'run_pack_source_refused';
                    END IF;
                ELSIF https_loop AND coalesce(ref->>'id', '') ~ '^https_[0-9]+$' THEN
                    IF NOT ref ?& ARRAY['id','owner','source','entity','asset_ref','revision','parent_revision','endpoint_ref','outcome','status_code','redirect','endpoint_reachable','whole_host_proven','objective_completed','perceived_at','evaluated_at','limitations','status']
                       OR ref - ARRAY['id','owner','source','entity','asset_ref','revision','parent_revision','endpoint_ref','outcome','status_code','redirect','endpoint_reachable','whole_host_proven','objective_completed','perceived_at','evaluated_at','limitations','status'] <> '{}'
                       OR ref->>'source' IS DISTINCT FROM exp_https_source
                       OR ref->'whole_host_proven' IS DISTINCT FROM 'false'::jsonb
                       OR ref->'objective_completed' IS DISTINCT FROM 'false'::jsonb THEN
                        RAISE EXCEPTION 'run_pack_source_refused';
                    END IF;
                ELSIF ref - ARRAY['id','owner','source','entity','asset_ref','vantage_ref','revision','lineage','perceived_at','time_uncertainty',
                    'evaluated_at','temporal_frontier','valid_until','ttl_seconds','status','tier','freshness','completeness','limitations','corroborated','conflicted','integrity_hold'] <> '{}' THEN
                    RAISE EXCEPTION 'run_pack_source_refused';
                END IF;
            WHEN 'campaign_trajectory' THEN
                IF p#>>'{predecessor,availability}' IS DISTINCT FROM 'committed' OR ref->>'id' IS DISTINCT FROM 'previous_decision'
                   OR ref - ARRAY['id','owner','source','entity','event','recorded_at','status'] <> '{}' THEN
                    RAISE EXCEPTION 'run_pack_source_refused';
                END IF;
            ELSE
                RAISE EXCEPTION 'run_pack_source_refused';
        END CASE;
    END LOOP;
    FOR candidate, ordinal IN SELECT value, n::integer FROM jsonb_array_elements(p->'candidates') WITH ORDINALITY a(value, n) LOOP
        SELECT value INTO ref FROM jsonb_array_elements(p->'references') WHERE value->>'owner' = 'operator' AND value->'id' = candidate->'reference';
        SELECT value INTO original FROM jsonb_array_elements(request#>'{context,candidates}') WHERE value->'asset_ref' = ref->'entity';
        IF ref IS NULL OR original IS NULL OR candidate->>'reference' IS DISTINCT FROM 'asset_' || (ordinal - 1)::text
           OR candidate->>'status' IS DISTINCT FROM 'operator_premise' OR candidate->'priority' IS DISTINCT FROM original->'priority'
           OR candidate - cand_keys <> '{}' THEN
            RAISE EXCEPTION 'run_pack_source_refused';
        END IF;
    END LOOP;
END $$;

CREATE OR REPLACE FUNCTION execution.assert_autonomous_pack(e uuid, c uuid, d uuid, p jsonb, cfg jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; expected jsonb; prior jsonb; seed text; permission jsonb;
    registration jsonb; expected_timing text; step_idx integer; has_eps boolean; expected_manifests jsonb;
BEGIN
    x := execution.find_decision_run(e, c, d);
    IF x.run_id IS NULL THEN RAISE EXCEPTION 'run_pack_binding_refused'; END IF;
    registration := execution.run_current(x.run_id);
    permission := registration#>'{fields,m1_inference_permission}';
    IF x.request->>'version' IN ('m1_autonomous_run_request_v2', 'm1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4', 'm1_https_loop_v1') THEN
        PERFORM execution.assert_successor_pack(p, registration, x.request);
        IF (p->>'evaluated_at')::bigint > floor(extract(epoch FROM clock_timestamp()))::bigint
           OR (p->>'evaluated_at')::bigint < (registration->>'occurred_at')::bigint THEN
            RAISE EXCEPTION 'run_pack_source_refused';
        END IF;
    END IF;
    expected_timing := CASE WHEN x.request->>'version' IN ('m1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4', 'm1_https_loop_v1')
        AND x.request->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2')
        THEN 'reasoning-bounded-pro-v2' ELSE 'reasoning-bounded-v1' END;
    IF x.request->>'version' = 'm1_https_loop_v1' THEN
        has_eps := EXISTS (
            SELECT 1 FROM jsonb_array_elements(p->'references') ref
            WHERE ref->>'owner' = 'cyber_terrain' AND coalesce(ref->>'id', '') ~ '^endpoint_[0-9]+$'
        );
        IF x.request->>'execution_mode' = 'live' THEN
            expected_manifests := jsonb_build_array(
                jsonb_build_object('availability', 'qualified', 'execution_mode', 'live', 'profile', 'dns-routing-v2',
                    'qualification_ref', x.qualification_ref, 'reference', 'dns_udp', 'release_sha256', x.release_sha256,
                    'schema', 'm1_dns_ipc_v1', 'transport', 'udp'),
                jsonb_build_object('availability', CASE WHEN has_eps THEN 'qualified' ELSE 'requires_dns_premise' END,
                    'execution_mode', 'live', 'profile', 'https-head-v1',
                    'qualification_ref', x.qualification_ref, 'reference', 'https-head-v1', 'release_sha256', x.release_sha256,
                    'schema', 'm1_https_ipc_v1', 'transport', 'tcp_tls_http11_head')
            );
        ELSE
            expected_manifests := jsonb_build_array(
                jsonb_build_object('availability', 'qualified', 'execution_mode', 'controlled_fixture', 'profile', 'dns-routing-fixture-v1',
                    'reference', 'dns_udp', 'schema', 'm1_dns_ipc_v1', 'transport', 'udp'),
                jsonb_build_object('availability', CASE WHEN has_eps THEN 'qualified' ELSE 'requires_dns_premise' END,
                    'execution_mode', 'controlled_fixture', 'profile', 'https-head-fixture-v1',
                    'reference', 'https-head-v1', 'schema', 'm1_https_head_request_v1', 'transport', 'tcp_tls_http11_head')
            );
        END IF;
        IF p->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
           OR p->>'inference_profile' IS DISTINCT FROM permission->>'profile'
           OR cfg->>'timing_profile' IS DISTINCT FROM expected_timing
           OR (p->>'run_profile', p->>'execution_mode') IS DISTINCT FROM (x.request->>'run_profile', x.request->>'execution_mode')
           OR (cfg->>'run_profile', cfg->>'execution_mode') IS DISTINCT FROM (p->>'run_profile', p->>'execution_mode')
           OR (cfg->>'profile', cfg->>'recipient') IS DISTINCT FROM (permission->>'profile', permission->>'recipient')
           OR (p->>'engagement_id', p->>'campaign_id', p->>'decision_id') IS DISTINCT FROM (e::text, c::text, d::text)
           OR (x.request->>'execution_mode' = 'live' AND (p->'live_binding' IS DISTINCT FROM permission->'live_binding'
               OR p->>'qualification_purpose' IS DISTINCT FROM 'HTTPS_LOOP_LAB'))
           OR p->'manifests' IS DISTINCT FROM expected_manifests THEN
            RAISE EXCEPTION 'run_pack_binding_refused';
        END IF;
        SELECT st.step_index INTO step_idx FROM execution.https_loop_steps st WHERE st.run_id = x.run_id AND st.decision_id = d;
        IF step_idx IS NULL THEN RAISE EXCEPTION 'run_pack_binding_refused'; END IF;
        IF step_idx = 1 THEN
            IF p->'predecessor' IS DISTINCT FROM '{"availability":"absent","reason":"first_decision"}'::jsonb THEN
                RAISE EXCEPTION 'run_predecessor_refused';
            END IF;
        ELSE
            expected := execution.https_loop_step_predecessor(x.run_id, step_idx);
            prior := p->'predecessor';
            SELECT ref->>'id' INTO seed FROM jsonb_array_elements(p->'references') ref
            WHERE ref->>'owner' = 'operator' AND ref->'entity' = expected->'selected_seed_entity';
            expected := jsonb_set(expected, '{selected_seed_reference}', to_jsonb(seed));
            IF prior->>'hypothesis_summary_omission' = 'byte_bound' AND NOT prior ? 'hypothesis_summary' THEN
                expected := (expected - 'hypothesis_summary') || '{"hypothesis_summary_omission":"byte_bound"}'::jsonb;
            END IF;
            IF seed IS NULL OR prior IS DISTINCT FROM (expected - ARRAY['decision_entity','decision_event','recorded_at','selected_seed_entity'])
               OR NOT EXISTS (
                   SELECT 1 FROM jsonb_array_elements(p->'references') ref
                   WHERE ref->>'id' = 'previous_decision' AND ref->'entity' = expected->'decision_entity'
                     AND ref->'event' = expected->'decision_event' AND ref->>'status' = 'committed_historical_decision'
                     AND ref->>'source' = 'cognition' AND ref->'recorded_at' = expected->'recorded_at'
               ) THEN
                RAISE EXCEPTION 'run_predecessor_refused';
            END IF;
        END IF;
        RETURN;
    END IF;
    IF p->>'version' IS DISTINCT FROM (CASE WHEN x.request->>'version' = 'm1_autonomous_run_request_v4' THEN 'm1_context_pack_autonomous_v4' WHEN x.request->>'version' = 'm1_autonomous_run_request_v3' THEN 'm1_context_pack_autonomous_v3' WHEN x.request->>'version' = 'm1_autonomous_run_request_v2' THEN 'm1_context_pack_autonomous_v2' ELSE 'm1_context_pack_autonomous_v1' END)
       OR (x.request->>'version' IN ('m1_autonomous_run_request_v2', 'm1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4') AND (p->>'inference_profile' IS DISTINCT FROM permission->>'profile' OR cfg->>'timing_profile' IS DISTINCT FROM expected_timing))
       OR (p->>'run_profile', p->>'execution_mode') IS DISTINCT FROM (x.request->>'run_profile', x.request->>'execution_mode')
       OR (cfg->>'run_profile', cfg->>'execution_mode') IS DISTINCT FROM (p->>'run_profile', p->>'execution_mode')
       OR (cfg->>'profile', cfg->>'recipient') IS DISTINCT FROM (permission->>'profile', permission->>'recipient')
       OR (p->>'engagement_id', p->>'campaign_id', p->>'decision_id') IS DISTINCT FROM (e::text, c::text, d::text)
       OR p->'manifests' IS DISTINCT FROM jsonb_build_array(CASE WHEN x.request->>'version' = 'm1_autonomous_run_request_v4'
           THEN jsonb_build_object('availability', 'qualified', 'execution_mode', 'live', 'profile', 'dns-routing-v2', 'qualification_ref', x.qualification_ref, 'reference', 'dns_udp', 'release_sha256', x.release_sha256, 'schema', 'm1_dns_ipc_v1', 'transport', 'udp')
           ELSE jsonb_build_object('availability', 'qualified', 'execution_mode', 'controlled_fixture', 'profile', 'dns-routing-fixture-v1', 'qualification_ref', x.qualification_ref, 'reference', 'dns_udp', 'release_sha256', x.release_sha256, 'schema', 'm1_dns_ipc_v1', 'transport', 'udp') END)
       OR (x.request->>'version' = 'm1_autonomous_run_request_v4' AND (p->'live_binding' IS DISTINCT FROM permission->'live_binding'
           OR p->>'qualification_purpose' IS DISTINCT FROM CASE x.request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END)) THEN
        RAISE EXCEPTION 'run_pack_binding_refused';
    END IF;
    IF d = x.decision_one THEN
        IF p->'predecessor' IS DISTINCT FROM '{"availability":"absent","reason":"first_decision"}'::jsonb THEN
            RAISE EXCEPTION 'run_predecessor_refused';
        END IF;
    ELSE
        expected := execution.run_predecessor(x.run_id);
        prior := p->'predecessor';
        SELECT ref->>'id' INTO seed FROM jsonb_array_elements(p->'references') ref WHERE ref->>'owner' = 'operator' AND ref->'entity' = expected->'selected_seed_entity';
        expected := jsonb_set(expected, '{selected_seed_reference}', to_jsonb(seed));
        IF x.request->>'version' IN ('m1_autonomous_run_request_v2', 'm1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4')
           AND prior->>'hypothesis_summary_omission' = 'byte_bound' AND NOT prior ? 'hypothesis_summary' THEN
            expected := (expected - 'hypothesis_summary') || '{"hypothesis_summary_omission":"byte_bound"}'::jsonb;
        END IF;
        IF seed IS NULL OR prior IS DISTINCT FROM (expected - ARRAY['decision_entity','decision_event','recorded_at','selected_seed_entity'])
           OR NOT EXISTS (
               SELECT 1 FROM jsonb_array_elements(p->'references') ref
               WHERE ref->>'id' = 'previous_decision' AND ref->'entity' = expected->'decision_entity'
                 AND ref->'event' = expected->'decision_event' AND ref->>'status' = 'committed_historical_decision'
                 AND ref->>'source' = 'cognition' AND ref->'recorded_at' = expected->'recorded_at'
           ) THEN
            RAISE EXCEPTION 'run_predecessor_refused';
        END IF;
    END IF;
END $$;

CREATE OR REPLACE FUNCTION cognition.successor_reasoning_identity(x jsonb) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT coalesce(EXISTS (
        SELECT 1 FROM execution.autonomous_runs r JOIN mission.registration_outbox m ON (m.engagement_id, m.campaign_id) = (r.e, r.c)
        JOIN execution.native_input_bindings i ON (i.e, i.c, i.d) = ((x->>'engagement')::uuid, (x->>'campaign')::uuid, (x->>'decision')::uuid)
        WHERE r.run_id = (execution.find_decision_run((x->>'engagement')::uuid, (x->>'campaign')::uuid, (x->>'decision')::uuid)).run_id
          AND i.writer_oid = r.writer_oid
          AND r.request->>'version' IN ('m1_autonomous_run_request_v2', 'm1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4', 'm1_https_loop_v1')
          AND r.request->>'run_profile' IN ('autonomous-dns-fixture-v2', 'autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1', 'm1_https_loop_v1', 'autonomous-https-genai-lab-v1')
          AND (r.request->>'version' = 'm1_autonomous_run_request_v4') = (r.request->>'run_profile' IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1'))
          AND (r.request->>'version' = 'm1_https_loop_v1') = (r.request->>'run_profile' IN ('m1_https_loop_v1', 'autonomous-https-genai-lab-v1'))
          AND r.request->'live_binding' IS NOT DISTINCT FROM m.contract#>'{fields,m1_inference_permission,live_binding}'
          AND (m.contract#>>'{fields,m1_inference_permission,version}', r.request->>'version') IN
              (('m1_inference_permission_v4', 'm1_autonomous_run_request_v2'),
               ('m1_inference_permission_v5', 'm1_autonomous_run_request_v3'),
               ('m1_inference_permission_v6', 'm1_autonomous_run_request_v4'),
               ('m1_https_loop_v1', 'm1_https_loop_v1'))
          AND (m.contract#>>'{fields,m1_inference_permission,profile}') = (r.request->>'inference_profile')
          AND (m.contract#>>'{fields,m1_inference_permission,permission_id}') = (x->>'permission')
          AND r.request#>>'{context,mission,permission_id}' = x->>'permission'
          AND r.request#>>'{context,mission,exercise_mode}' = x->>'source_mode'
          AND m.contract#>>'{fields,exercise_mode}' = x->>'source_mode'
          AND r.request#>>'{context,mission,expected_mission_revision}' = x->>'mission_revision'
          AND (m.contract->>'owner_revision') = (x->>'mission_revision')
          AND (x->'version') = '2'::jsonb AND x->>'version' = '2'
          AND jsonb_typeof(x->'provenance') = 'array'
          AND NOT EXISTS (
              SELECT 1 FROM execution.inference_calls a WHERE (a.e, a.c, a.d) = (i.e, i.c, i.d)
                AND ((a.p::jsonb->>'version') IS DISTINCT FROM (CASE
                    WHEN r.request->>'version' = 'm1_https_loop_v1' THEN 'm1_https_loop_v1'
                    WHEN r.request->>'version' = 'm1_autonomous_run_request_v4' THEN 'm1_context_pack_autonomous_v4'
                    WHEN r.request->>'version' = 'm1_autonomous_run_request_v3' THEN 'm1_context_pack_autonomous_v3'
                    ELSE 'm1_context_pack_autonomous_v2' END)
                  OR (a.p::jsonb->>'inference_profile') IS DISTINCT FROM r.request->>'inference_profile')
          )
    ), false)
$$;

-- 5. Run lifecycle, refresh check, fixture-only guards, summary and head result reader.
CREATE OR REPLACE FUNCTION execution.run_current(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; y execution.autonomous_control;
    s jsonb; p jsonb; n bigint; is_live_qual boolean; exp_purpose text;
BEGIN
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT * INTO y FROM execution.autonomous_control WHERE run_id = r;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF x.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
       OR NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR y.status IS DISTINCT FROM 'running'
       OR n >= least(x.deadline_at_ms, y.lease_until_ms) THEN
        RAISE EXCEPTION 'run_not_current';
    END IF;
    s := mission.current_inference_publication(x.e, x.c, x.decision_one); p := s->'fields'->'m1_inference_permission';
    is_live_qual := x.request->>'version' = 'm1_autonomous_run_request_v4'
        OR (x.request->>'version' = 'm1_https_loop_v1' AND x.request->>'execution_mode' = 'live');
    exp_purpose := CASE x.request->>'run_profile'
        WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE'
        WHEN 'autonomous-https-genai-lab-v1' THEN 'HTTPS_LOOP_LAB'
        ELSE 'LAB' END;
    IF s IS NULL OR NOT execution.inference_profile_matches(p)
       OR (p->>'version', p->>'run_profile', p->>'execution_mode') IS DISTINCT FROM (
           CASE WHEN x.request->>'version' = 'm1_https_loop_v1' THEN 'm1_https_loop_v1'
                WHEN x.request->>'version' = 'm1_autonomous_run_request_v4' THEN 'm1_inference_permission_v6'
                WHEN x.request->>'version' = 'm1_autonomous_run_request_v3' THEN 'm1_inference_permission_v5'
                WHEN x.request->>'version' = 'm1_autonomous_run_request_v2' THEN 'm1_inference_permission_v4'
                ELSE 'm1_inference_permission_v3' END,
           x.request->>'run_profile', x.request->>'execution_mode')
       OR (x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') AND x.request->>'inference_profile' IS DISTINCT FROM p->>'profile')
       OR p->>'permission_id' IS DISTINCT FROM x.request#>>'{context,mission,permission_id}'
       OR (is_live_qual AND (
           x.request->'live_binding' IS DISTINCT FROM p->'live_binding'
           OR NOT EXISTS (SELECT 1 FROM execution.live_qualifications z WHERE z.qualification_ref = x.qualification_ref
               AND n >= z.valid_from_ms AND n < z.valid_until_ms AND z.release_sha256 = x.release_sha256
               AND z.run_profile = x.request->>'run_profile' AND host(z.resolver_ipv4) = x.request#>>'{live_binding,resolver_ipv4}'
               AND z.purpose = exp_purpose)))
       OR p->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb
       OR (p->>'provider_terms_ref')::uuid IS NULL OR (p->>'provider_terms_ref')::uuid = '00000000-0000-0000-0000-000000000000'::uuid
       OR jsonb_typeof(p->'field_classes') IS DISTINCT FROM 'array' OR jsonb_array_length(p->'field_classes') <> 9
       OR NOT p->'field_classes' ?& ARRAY['mission_question_constraints','scoped_opaque_references','owner_qualified_summaries','admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes','admitted_campaign_decision_feedback']
       OR (p->'engagement_id',p->'campaign_id',p->'operator_ref',p->'authority_ref',p->'authority_revision',p->'goal_ref',p->'exercise_mode',p->'mission_revision') IS DISTINCT FROM
          (s->'engagement_id',s->'campaign_id',s->'fields'->'operator_ref',s->'fields'->'authority_ref',s->'fields'->'authority_revision',s->'fields'->'goal_ref',s->'fields'->'exercise_mode','1'::jsonb)
       OR n >= (p->>'ends_at')::bigint * 1000 OR n < (p->>'starts_at')::bigint * 1000
       OR n >= (s->'fields'->>'ends_at')::bigint * 1000 OR n < (s->'fields'->>'starts_at')::bigint * 1000
       OR NOT EXISTS (SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id,h.operation_id,h.event_id,h.status,h.contract) = (x.e,x.c,(s->>'operation_id')::uuid,(s->>'event_id')::uuid,'accepted',s))
       OR EXISTS (SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id) = (x.e,x.c) AND h.status = 'anomaly') THEN
        RAISE EXCEPTION 'run_authority_unavailable';
    END IF;
    IF p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1') AND EXISTS (
        SELECT 1 FROM execution.autonomous_phases a WHERE a.run_id = r AND a.phase = y.phase AND a.status = 'started'
        AND n >= a.recorded_at_ms + CASE
            WHEN a.phase IN ('acquisition','acquisition_one','acquisition_two','acquisition_three') THEN 60000
            WHEN x.request->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1')
                 AND x.request->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 254000
            ELSE 134000 END
    ) THEN
        RAISE EXCEPTION 'run_phase_expired';
    END IF;
    RETURN s;
END $$;

CREATE OR REPLACE FUNCTION execution.run_start(q jsonb, release text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; r uuid; e uuid; c uuid; n bigint;
    s jsonb; p jsonb; deadline bigint; pro_v2 boolean;
    hl boolean; hl_live boolean; is_live boolean; exp_purpose text;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR release !~ '^[0-9a-f]{64}$'
       OR jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text) > 16384
       OR q - ARRAY['version','run_id','run_profile','execution_mode','deadline_at_ms','context','seed_names','inference_profile','live_binding'] <> '{}'::jsonb
       OR NOT coalesce((q->>'version', q->>'run_profile') IN (
           ('m1_autonomous_run_request_v1','autonomous-dns-fixture-v1'),
           ('m1_autonomous_run_request_v1','autonomous-dns-provider-fixture-v1'),
           ('m1_autonomous_run_request_v2','autonomous-dns-fixture-v2'),
           ('m1_autonomous_run_request_v3','autonomous-dns-fixture-v2'),
           ('m1_autonomous_run_request_v4','autonomous-dns-genai-live-v1'),
           ('m1_autonomous_run_request_v4','autonomous-dns-genai-lab-v1'),
           ('m1_https_loop_v1','m1_https_loop_v1'),
           ('m1_https_loop_v1','autonomous-https-genai-lab-v1')
       ), false)
       OR (q->>'version' = 'm1_autonomous_run_request_v1' AND q ? 'inference_profile')
       OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') AND jsonb_typeof(q->'inference_profile') IS DISTINCT FROM 'string') THEN
        RAISE EXCEPTION 'run_request_refused';
    END IF;
    hl := q->>'version' = 'm1_https_loop_v1';
    hl_live := hl AND q->>'run_profile' = 'autonomous-https-genai-lab-v1';
    is_live := q->>'version' = 'm1_autonomous_run_request_v4' OR hl_live;
    IF (NOT is_live AND q ? 'live_binding')
       OR (is_live AND (jsonb_typeof(q->'live_binding') IS DISTINCT FROM 'object'
           OR NOT q->'live_binding' ?& ARRAY['qualification_ref','release_sha256','resolver_ipv4']
           OR (q->'live_binding') - ARRAY['qualification_ref','release_sha256','resolver_ipv4'] <> '{}'::jsonb))
       OR q->>'execution_mode' IS DISTINCT FROM (CASE WHEN is_live THEN 'live' ELSE 'controlled_fixture' END)
       OR jsonb_typeof(q->'seed_names') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'seed_names') NOT BETWEEN 1 AND 8
       OR jsonb_array_length(q#>'{context,candidates}') <> jsonb_array_length(q->'seed_names')
       OR (hl AND q->>'inference_profile' IS DISTINCT FROM 'fixture-genai-v4-pro-low-v2') THEN
        RAISE EXCEPTION 'run_request_refused';
    END IF;
    IF q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') AND (
        jsonb_typeof(q->'context') IS DISTINCT FROM 'object'
        OR (q->'context') - ARRAY['version','mission','candidates'] <> '{}'::jsonb
        OR q#>>'{context,version}' IS DISTINCT FROM 'm1_context_prepare_request_v1'
        OR jsonb_typeof(q#>'{context,candidates}') IS DISTINCT FROM 'array'
        OR EXISTS (SELECT 1 FROM jsonb_array_elements(q#>'{context,candidates}') v WHERE
            jsonb_typeof(v) IS DISTINCT FROM 'object' OR v - ARRAY['asset_ref','priority'] <> '{}'::jsonb
            OR jsonb_typeof(v->'asset_ref') IS DISTINCT FROM 'string' OR coalesce(v->>'priority','') NOT IN ('high','medium','low'))
    ) THEN
        RAISE EXCEPTION 'run_request_refused';
    END IF;
    IF q->>'version' = 'm1_autonomous_run_request_v4' AND (
        jsonb_array_length(q->'seed_names') <> 1 OR jsonb_array_length(q#>'{context,candidates}') <> 1
        OR (q->>'run_profile' = 'autonomous-dns-genai-lab-v1' AND (q#>>'{seed_names,0,name}' IS NULL OR q#>>'{seed_names,0,name}' !~ '^.+\.duskweave\.test$'))
    ) THEN
        RAISE EXCEPTION 'run_request_refused';
    END IF;
    IF hl_live AND (
        jsonb_array_length(q->'seed_names') <> 1 OR jsonb_array_length(q#>'{context,candidates}') <> 1
        OR q#>>'{seed_names,0,name}' IS DISTINCT FROM 'head.duskweave.test'
        OR q#>>'{live_binding,resolver_ipv4}' IS DISTINCT FROM '192.0.2.53'
    ) THEN
        RAISE EXCEPTION 'run_request_refused';
    END IF;
    r := (q->>'run_id')::uuid; e := (q#>>'{context,mission,engagement_id}')::uuid; c := (q#>>'{context,mission,campaign_id}')::uuid;
    IF '00000000-0000-0000-0000-000000000000'::uuid IN (r, e, c) THEN RAISE EXCEPTION 'run_request_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF FOUND THEN
        IF x.request IS DISTINCT FROM q OR x.release_sha256 <> release
           OR x.writer_oid <> (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
            RAISE EXCEPTION 'run_identity_conflict';
        END IF;
        RETURN false;
    END IF;
    IF hl THEN
        IF EXISTS (
            SELECT 1 FROM execution.autonomous_runs ar
            WHERE (ar.e, ar.c) = (e, c)
              AND (ar.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
                   OR ar.request->>'run_profile' IS DISTINCT FROM (q->>'run_profile')
                   OR ar.request->>'execution_mode' IS DISTINCT FROM (q->>'execution_mode'))
        ) OR EXISTS (
            SELECT 1 FROM execution.autonomous_runs ar
            LEFT JOIN execution.autonomous_control ac USING (run_id)
            WHERE (ar.e, ar.c) = (e, c)
              AND coalesce(ac.status, 'running') IN ('running', 'unknown', 'unknown_supervisor_lost')
        ) OR (hl_live AND EXISTS (
            SELECT 1 FROM execution.live_effect_sessions a
            JOIN execution.live_effect_control b ON b.attempt = a.attempt
            WHERE (a.e, a.c) = (e, c)
              AND (b.phase <> 'closed' OR (b.possibly_started AND NOT EXISTS (
                  SELECT 1 FROM execution.live_effect_events z WHERE z.attempt = a.attempt AND z.phase = 'result_recorded'
              )))
        )) THEN
            RAISE EXCEPTION 'run_identity_conflict';
        END IF;
    ELSIF EXISTS (SELECT 1 FROM execution.autonomous_runs ar WHERE (ar.e, ar.c) = (e, c)) THEN
        RAISE EXCEPTION 'run_identity_conflict';
    END IF;
    pro_v2 := q->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1')
        AND q->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2');
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint; deadline := (q->>'deadline_at_ms')::bigint;
    IF jsonb_typeof(q->'deadline_at_ms') IS DISTINCT FROM 'number' OR q->>'deadline_at_ms' !~ '^[1-9][0-9]*$'
       OR deadline <= n OR deadline > n + (CASE WHEN pro_v2 THEN 640000 WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') THEN 400000 ELSE 150000 END) THEN
        RAISE EXCEPTION 'run_deadline_refused';
    END IF;
    exp_purpose := CASE q->>'run_profile'
        WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE'
        WHEN 'autonomous-https-genai-lab-v1' THEN 'HTTPS_LOOP_LAB'
        ELSE 'LAB' END;
    s := mission.current_inference_publication(e, c, r); p := s->'fields'->'m1_inference_permission';
    IF s IS NULL OR NOT execution.inference_profile_matches(p)
       OR (p->>'version', p->>'run_profile', p->>'execution_mode') IS DISTINCT FROM (
           CASE WHEN hl THEN 'm1_https_loop_v1' WHEN q->>'version' = 'm1_autonomous_run_request_v4' THEN 'm1_inference_permission_v6' WHEN q->>'version' = 'm1_autonomous_run_request_v3' THEN 'm1_inference_permission_v5' WHEN q->>'version' = 'm1_autonomous_run_request_v2' THEN 'm1_inference_permission_v4' ELSE 'm1_inference_permission_v3' END,
           q->>'run_profile', q->>'execution_mode')
       OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') AND q->>'inference_profile' IS DISTINCT FROM p->>'profile')
       OR (is_live AND (q->'live_binding' IS DISTINCT FROM p->'live_binding' OR q#>>'{live_binding,release_sha256}' IS DISTINCT FROM release
           OR NOT EXISTS (SELECT 1 FROM execution.live_qualifications z WHERE z.qualification_ref = (q#>>'{live_binding,qualification_ref}')::uuid
               AND z.run_profile = q->>'run_profile' AND z.release_sha256 = release AND host(z.resolver_ipv4) = q#>>'{live_binding,resolver_ipv4}'
               AND n >= z.valid_from_ms AND n < z.valid_until_ms AND z.purpose = exp_purpose)))
       OR q#>'{context,mission}' IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1','engagement_id',e,'campaign_id',c,'operator_ref',s->'fields'->'operator_ref','expected_mission_revision',1,'permission_id',p->'permission_id','goal_ref',s->'fields'->'goal_ref','exercise_mode',s->'fields'->'exercise_mode')
       OR NOT p->'field_classes' ?& ARRAY['mission_question_constraints','scoped_opaque_references','owner_qualified_summaries','admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes','admitted_campaign_decision_feedback']
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(q->'seed_names') WITH ORDINALITY a(v, i)
           WHERE v - ARRAY['asset_ref','name'] <> '{}'::jsonb OR v->'asset_ref' IS DISTINCT FROM q#>'{context,candidates}'->(i::int - 1)->'asset_ref'
              OR NOT s->'fields'->'included_assets' @> jsonb_build_array(v->'asset_ref') OR s->'fields'->'excluded_assets' @> jsonb_build_array(v->'asset_ref'))
       OR EXISTS (SELECT 1 FROM (VALUES
           ('calls', CASE WHEN hl THEN 2 ELSE 4 END),
           ('request_bytes', CASE WHEN hl THEN 32768 ELSE 65536 END),
           ('response_bytes', CASE WHEN hl THEN 131072 ELSE 262144 END),
           ('output_token_capacity', CASE WHEN hl THEN 16384 WHEN pro_v2 THEN 32768 ELSE 16384 END),
           ('wall_time_ms', CASE WHEN hl THEN 249000 WHEN pro_v2 THEN 498000 WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN 258000 ELSE 90000 END)
       ) b(k, v) WHERE NOT coalesce((p->'campaign_limits'->>b.k)::numeric >= (execution.inference_capacity_snapshot(e, c)->'charged'->>b.k)::numeric + b.v, false)) THEN
        RAISE EXCEPTION 'run_authority_unavailable';
    END IF;
    deadline := least(deadline, (s->'fields'->>'ends_at')::bigint * 1000, (p->>'ends_at')::bigint * 1000);
    INSERT INTO execution.autonomous_runs VALUES (
        r, e, c, (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user), q,
        gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), gen_random_uuid(), gen_random_uuid(),
        release, CASE WHEN is_live THEN (q#>>'{live_binding,qualification_ref}')::uuid ELSE gen_random_uuid() END, deadline
    ) RETURNING * INTO x;
    INSERT INTO execution.autonomous_control VALUES (r, least(n + 2000, deadline), 'running', NULL, false);
    IF hl THEN
        INSERT INTO execution.https_loop_steps (run_id, e, c, step_index, decision_id, next_decision_id, decision_phase, acquisition_phase, status, protection_state, updated_at_ms)
        VALUES (r, e, c, 1, x.decision_one, NULL, 'decision_one', 'acquisition_one', 'pending', 'none', n);
    END IF;
    PERFORM execution.run_current(r);
    RETURN true;
END $$;

CREATE OR REPLACE FUNCTION execution.run_phase_complete(r uuid, wanted text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; d uuid; st execution.https_loop_steps; is_live boolean;
BEGIN
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    PERFORM execution.run_current(r);
    IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control WHERE run_id = r AND phase = wanted AND claimed) THEN
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    is_live := x.request->>'execution_mode' = 'live';
    IF x.request->>'version' = 'm1_https_loop_v1' THEN
        SELECT * INTO st FROM execution.https_loop_steps s WHERE s.run_id = r AND (s.decision_phase = wanted OR s.acquisition_phase = wanted);
        IF NOT FOUND THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
        IF wanted LIKE 'decision%' THEN
            IF NOT EXISTS (SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = (x.e, x.c, st.decision_id)) THEN
                RAISE EXCEPTION 'run_inference_pending';
            END IF;
            UPDATE execution.https_loop_steps SET status = CASE WHEN st.step_index = 4 THEN 'completed' ELSE 'decision_completed' END,
                updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint WHERE run_id = r AND step_index = st.step_index;
        ELSIF is_live THEN
            IF st.next_intent_kind = 'dns_udp' THEN
                IF NOT EXISTS (
                    SELECT 1 FROM execution.live_dns_results b
                    JOIN execution.live_effect_control xc ON xc.attempt = b.attempt
                    JOIN terrain.outbox t ON (t.engagement_id, t.campaign_id, t.operation_id) = (x.e, x.c, st.terrain_operation_id)
                    JOIN trajectory.terrain_history h ON h.event_id = t.event_id
                    WHERE b.run_id = r AND b.step_index = st.step_index AND b.attempt_kind = 'dns'
                      AND b.result = st.dns_result_id AND b.attempt = st.dns_attempt_id
                      AND xc.phase = 'closed' AND xc.possibly_started AND xc.disposal_confirmed
                      AND t.contract#>>'{observation,source_kind}' = 'issued_dns_acquisition'
                      AND t.contract#>>'{observation,correlation_id}' = b.result::text
                      AND h.status = 'accepted' AND h.contract = t.contract
                ) THEN
                    RAISE EXCEPTION 'run_terrain_pending';
                END IF;
            ELSIF st.next_intent_kind = 'https-head-v1' THEN
                IF st.refresh_attempt_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM execution.live_effect_control xcr
                    WHERE xcr.attempt = st.refresh_attempt_id AND xcr.phase = 'closed' AND xcr.disposal_confirmed
                ) THEN
                    RAISE EXCEPTION 'run_terrain_pending';
                END IF;
                IF coalesce(st.head_outcome, '') IN ('blocked', 'unknown_refresh') THEN
                    IF st.refresh_result_id IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM execution.live_dns_results b
                        JOIN execution.live_effect_control xc ON xc.attempt = b.attempt
                        JOIN terrain.outbox t ON (t.engagement_id, t.campaign_id, t.operation_id) = (x.e, x.c, st.terrain_operation_id)
                        JOIN trajectory.terrain_history h ON h.event_id = t.event_id
                        WHERE b.run_id = r AND b.step_index = st.step_index AND b.attempt_kind = 'dns_refresh'
                          AND b.result = st.refresh_result_id AND b.attempt = st.refresh_attempt_id
                          AND xc.phase = 'closed' AND xc.possibly_started AND xc.disposal_confirmed
                          AND t.contract#>>'{observation,source_kind}' = 'issued_dns_acquisition'
                          AND t.contract#>>'{observation,correlation_id}' = b.result::text
                          AND h.status = 'accepted' AND h.contract = t.contract
                    ) THEN
                        RAISE EXCEPTION 'run_terrain_pending';
                    END IF;
                ELSIF NOT EXISTS (
                    SELECT 1 FROM execution.live_https_results hr
                    JOIN execution.live_effect_control xc ON xc.attempt = hr.attempt
                    JOIN terrain.https_outbox ho ON (ho.engagement_id, ho.campaign_id, ho.operation_id) = (x.e, x.c, st.head_terrain_operation_id)
                    JOIN trajectory.terrain_history th ON th.event_id = ho.event_id
                    WHERE hr.run_id = r AND hr.step_index = st.step_index
                      AND hr.result = st.head_result_id AND hr.attempt = st.head_attempt_id
                      AND xc.phase = 'closed' AND xc.possibly_started AND xc.disposal_confirmed
                      AND th.status = 'accepted' AND th.contract = ho.contract
                ) THEN
                    RAISE EXCEPTION 'run_terrain_pending';
                END IF;
            ELSE
                RAISE EXCEPTION 'run_phase_refused';
            END IF;
            IF st.status = 'unknown' OR st.head_outcome = 'unknown_refresh' THEN
                UPDATE execution.https_loop_steps SET status = 'unknown',
                    updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint WHERE run_id = r AND step_index = st.step_index;
                UPDATE execution.autonomous_control SET status = 'unknown', lease_until_ms = 0 WHERE run_id = r AND status = 'running';
            ELSE
                UPDATE execution.https_loop_steps SET status = 'completed',
                    updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint WHERE run_id = r AND step_index = st.step_index;
            END IF;
        ELSE
            IF st.next_intent_kind = 'dns_udp' THEN
                IF NOT EXISTS (SELECT 1 FROM execution.fixture_dns_results b JOIN terrain.outbox t ON t.operation_id = st.terrain_operation_id
                    JOIN trajectory.terrain_history h ON h.event_id = t.event_id WHERE b.run_id = r AND b.result = st.dns_result_id AND h.status = 'accepted' AND h.contract = t.contract) THEN
                    RAISE EXCEPTION 'run_terrain_pending';
                END IF;
            ELSIF st.next_intent_kind = 'https-head-v1' THEN
                IF coalesce(st.head_outcome, '') IN ('blocked', 'unknown_refresh') THEN
                    IF st.refresh_result_id IS NOT NULL AND NOT EXISTS (
                        SELECT 1 FROM execution.fixture_dns_results b JOIN terrain.outbox t ON t.operation_id = st.terrain_operation_id
                        JOIN trajectory.terrain_history h ON h.event_id = t.event_id
                        WHERE b.run_id = r AND b.result = st.refresh_result_id AND h.status = 'accepted' AND h.contract = t.contract
                    ) THEN
                        RAISE EXCEPTION 'run_terrain_pending';
                    END IF;
                ELSIF NOT EXISTS (SELECT 1 FROM execution.fixture_https_results hr JOIN terrain.https_outbox ho ON ho.operation_id = st.head_terrain_operation_id
                    JOIN trajectory.terrain_history th ON th.event_id = ho.event_id WHERE hr.run_id = r AND hr.step_index = st.step_index AND th.status = 'accepted' AND th.contract = ho.contract) THEN
                    RAISE EXCEPTION 'run_terrain_pending';
                END IF;
            ELSE
                RAISE EXCEPTION 'run_phase_refused';
            END IF;
            IF st.status = 'unknown' OR st.head_outcome = 'unknown_refresh' THEN
                UPDATE execution.https_loop_steps SET status = 'unknown',
                    updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint WHERE run_id = r AND step_index = st.step_index;
                UPDATE execution.autonomous_control SET status = 'unknown', lease_until_ms = 0 WHERE run_id = r AND status = 'running';
            ELSE
                UPDATE execution.https_loop_steps SET status = 'completed',
                    updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint WHERE run_id = r AND step_index = st.step_index;
            END IF;
        END IF;
        INSERT INTO execution.autonomous_phases VALUES (r, wanted, 'completed', floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint);
        IF wanted = 'decision_four' THEN
            UPDATE execution.autonomous_control SET status = 'completed', lease_until_ms = 0 WHERE run_id = r AND status = 'running';
        END IF;
        RETURN;
    END IF;
    IF wanted IN ('decision_one', 'decision_two') THEN
        d := CASE WHEN wanted = 'decision_one' THEN x.decision_one ELSE x.decision_two END;
        IF NOT EXISTS (SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = (x.e, x.c, d)) THEN RAISE EXCEPTION 'run_inference_pending'; END IF;
    ELSIF x.request->>'version' = 'm1_autonomous_run_request_v4' THEN
        IF NOT EXISTS (SELECT 1 FROM execution.live_dns_results b JOIN execution.live_effect_control xc ON xc.attempt = b.attempt
            JOIN terrain.outbox t ON (t.engagement_id, t.campaign_id, t.operation_id) = (x.e, x.c, x.terrain_operation)
            JOIN trajectory.terrain_history h ON h.event_id = t.event_id
            WHERE b.run_id = r AND b.result = x.result AND b.attempt = x.attempt AND xc.phase = 'closed' AND xc.possibly_started AND xc.disposal_confirmed
              AND t.contract#>>'{observation,source_kind}' = 'issued_dns_acquisition' AND t.contract#>>'{observation,correlation_id}' = b.result::text
              AND h.status = 'accepted' AND h.contract = t.contract) THEN
            RAISE EXCEPTION 'run_terrain_pending';
        END IF;
    ELSE
        IF NOT EXISTS (SELECT 1 FROM execution.fixture_dns_results b JOIN terrain.outbox t ON t.operation_id = x.terrain_operation
            JOIN trajectory.terrain_history h ON h.event_id = t.event_id WHERE b.run_id = r AND h.status = 'accepted' AND h.contract = t.contract) THEN
            RAISE EXCEPTION 'run_terrain_pending';
        END IF;
    END IF;
    INSERT INTO execution.autonomous_phases VALUES (r, wanted, 'completed', floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint);
    IF wanted = 'acquisition' AND x.request->>'version' <> 'm1_autonomous_run_request_v4' THEN PERFORM execution.close_fixture_session(r, 'completed'); END IF;
    IF wanted = 'decision_two' THEN UPDATE execution.autonomous_control SET status = 'completed', lease_until_ms = 0 WHERE run_id = r AND status = 'running'; END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.run_unknown(r uuid, wanted text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE x execution.autonomous_runs;
BEGIN
    IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    PERFORM execution.private_live_control_scope(r, 'run_unknown');
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control WHERE run_id = r AND phase = wanted) THEN
        DELETE FROM execution.private_live_writer_context w WHERE w.backend_pid = pg_backend_pid()
          AND w.transaction_id = pg_current_xact_id() AND (w.e, w.c) = (x.e, x.c);
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.autonomous_phases WHERE run_id = r AND phase = wanted AND status = 'completed') THEN
        DELETE FROM execution.private_live_writer_context w WHERE w.backend_pid = pg_backend_pid()
          AND w.transaction_id = pg_current_xact_id() AND (w.e, w.c) = (x.e, x.c);
        RETURN;
    END IF;
    INSERT INTO execution.autonomous_phases VALUES (r, wanted, 'unknown', floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint) ON CONFLICT DO NOTHING;
    UPDATE execution.autonomous_control SET status = 'unknown', lease_until_ms = 0 WHERE run_id = r AND status = 'running';
    UPDATE execution.https_loop_steps SET status = 'unknown' WHERE run_id = r AND (decision_phase = wanted OR acquisition_phase = wanted) AND status NOT IN ('completed', 'terminal');
    IF x.request->>'execution_mode' = 'controlled_fixture' THEN
        IF x.request->>'version' = 'm1_https_loop_v1' THEN PERFORM execution.close_https_loop_effect_session(r, 'unknown');
        ELSE PERFORM execution.close_fixture_session(r, 'unknown'); END IF;
    END IF;
    DELETE FROM execution.private_live_writer_context w WHERE w.backend_pid = pg_backend_pid()
      AND w.transaction_id = pg_current_xact_id() AND (w.e, w.c) = (x.e, x.c);
END $$;

CREATE OR REPLACE FUNCTION execution.run_stop(r uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE x execution.autonomous_runs;
BEGIN
    IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    PERFORM execution.private_live_control_scope(r, 'stop_run');
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    UPDATE execution.autonomous_control SET status = 'stopped', lease_until_ms = 0 WHERE run_id = r AND status = 'running';
    UPDATE execution.https_loop_steps SET status = 'stopped' WHERE run_id = r AND status IN ('pending', 'decision_running', 'acquisition_pending', 'acquisition_running');
    IF x.request->>'execution_mode' = 'controlled_fixture' THEN
        IF x.request->>'version' = 'm1_https_loop_v1' THEN PERFORM execution.close_https_loop_effect_session(r, 'stopped');
        ELSE PERFORM execution.close_fixture_session(r, 'stopped'); END IF;
    END IF;
    DELETE FROM execution.private_live_writer_context w WHERE w.backend_pid = pg_backend_pid()
      AND w.transaction_id = pg_current_xact_id() AND (w.e, w.c) = (x.e, x.c);
END $$;

CREATE OR REPLACE FUNCTION execution.https_loop_step_predecessor(r uuid, idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    x execution.autonomous_runs; prev execution.https_loop_steps;
    d jsonb; dns_t terrain.outbox; https_t terrain.https_outbox; prior jsonb;
    seed_ref text; out_label text;
BEGIN
    IF execution.run_record(r) IS NULL OR idx NOT BETWEEN 2 AND 4 THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT * INTO prev FROM execution.https_loop_steps WHERE run_id = r AND step_index = idx - 1;
    IF NOT FOUND OR prev.status NOT IN ('completed', 'terminal') THEN RAISE EXCEPTION 'run_feedback_pending'; END IF;
    SELECT cd.payload INTO d FROM cognition.reasoning_decisions cd WHERE (cd.e, cd.c, cd.d) = (x.e, x.c, prev.decision_id);
    IF d IS NULL OR NOT EXISTS (SELECT 1 FROM execution.inference_closures ic WHERE (ic.e, ic.c, ic.d) = (x.e, x.c, prev.decision_id))
       OR NOT EXISTS (SELECT 1 FROM trajectory.reasoning_history h WHERE (h.engagement_id, h.campaign_id, h.operation_id) = (x.e, x.c, prev.decision_id) AND h.status = 'accepted' AND h.contract = d) THEN
        RAISE EXCEPTION 'run_feedback_pending';
    END IF;
    IF prev.dns_result_id IS NOT NULL OR prev.refresh_result_id IS NOT NULL THEN
        SELECT * INTO dns_t FROM terrain.outbox WHERE (engagement_id, campaign_id, operation_id) = (x.e, x.c, prev.terrain_operation_id);
        IF NOT FOUND OR NOT EXISTS (SELECT 1 FROM trajectory.terrain_history h WHERE h.event_id = dns_t.event_id AND h.status = 'accepted' AND h.contract = dns_t.contract) THEN
            RAISE EXCEPTION 'run_feedback_pending';
        END IF;
    END IF;
    IF prev.head_result_id IS NOT NULL THEN
        SELECT * INTO https_t FROM terrain.https_outbox WHERE (engagement_id, campaign_id, operation_id) = (x.e, x.c, prev.head_terrain_operation_id);
        IF NOT FOUND OR NOT EXISTS (SELECT 1 FROM trajectory.terrain_history h WHERE h.event_id = https_t.event_id AND h.status = 'accepted' AND h.contract = https_t.contract) THEN
            RAISE EXCEPTION 'run_feedback_pending';
        END IF;
    END IF;
    SELECT 'asset_' || (ord - 1)::text INTO seed_ref
    FROM jsonb_array_elements(x.request->'seed_names') WITH ORDINALITY a(elem, ord)
    WHERE (elem->>'asset_ref')::uuid = prev.resolved_asset_ref LIMIT 1;
    IF seed_ref IS NULL THEN RAISE EXCEPTION 'run_feedback_pending'; END IF;
    out_label := CASE
        WHEN prev.head_result_id IS NOT NULL THEN coalesce(https_t.contract#>>'{claim,outcome}', 'https_outcome_recorded')
        WHEN coalesce(prev.head_outcome, '') = 'blocked' THEN coalesce(prev.disposition, 'head_blocked')
        WHEN dns_t.event_id IS NOT NULL THEN 'owner_qualified_observed'
        ELSE coalesce(prev.disposition, 'recorded')
    END;
    prior := jsonb_build_object(
        'availability', 'committed',
        'decision_reference', 'previous_decision',
        'selected_seed_reference', seed_ref,
        'selected_seed_entity', prev.resolved_asset_ref,
        'selected_intent', CASE WHEN prev.next_intent_kind = 'dns_udp' THEN 'observe_more' ELSE 'validate' END,
        'execution_mode', x.request->>'execution_mode',
        'outcome', out_label,
        'hypothesis_summary', d#>'{proposal,hypothesis}',
        'decision_entity', d->'decision',
        'decision_event', d->'event',
        'recorded_at', ((d->>'recorded_at_ms')::bigint / 1000)
    );
    IF prev.resolved_endpoint_ref IS NOT NULL THEN
        prior := prior || jsonb_build_object('selected_endpoint_reference', prev.resolved_endpoint_ref);
    END IF;
    IF prev.dns_attempt_id IS NOT NULL AND prev.dns_result_id IS NOT NULL THEN
        prior := prior || jsonb_build_object('attempt_ref', prev.dns_attempt_id, 'result_ref', prev.dns_result_id);
    END IF;
    IF prev.refresh_attempt_id IS NOT NULL THEN
        prior := prior || jsonb_build_object('refresh_attempt_ref', prev.refresh_attempt_id, 'refresh_result_ref', prev.refresh_result_id, 'refresh_outcome', prev.refresh_outcome);
    END IF;
    IF prev.head_attempt_id IS NOT NULL THEN
        prior := prior || jsonb_build_object('head_attempt_ref', prev.head_attempt_id, 'head_result_ref', prev.head_result_id);
    END IF;
    IF https_t.event_id IS NOT NULL THEN
        prior := prior || jsonb_build_object('terrain_publication_ref', https_t.event_id);
    ELSIF dns_t.event_id IS NOT NULL THEN
        prior := prior || jsonb_build_object('terrain_publication_ref', dns_t.event_id);
    END IF;
    IF coalesce(prev.head_outcome, '') = 'blocked' AND prev.disposition IS NOT NULL THEN
        prior := prior || jsonb_build_object('head_block_reason', prev.disposition);
    END IF;
    RETURN prior;
END $$;

CREATE OR REPLACE FUNCTION execution.https_loop_refresh_check(r uuid, idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps;
    s jsonb; pol jsonb; lim jsonb; cc terrain.current_claims; ev terrain.evaluations;
    now_s bigint; cap_until bigint; seed_idx integer; is_live boolean;
    run_dns_used integer; camp_dns_used integer; ref_used integer; camp_ref_used integer;
    run_ep_used integer; camp_ep_used integer;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    is_live := x.request->>'execution_mode' = 'live';
    s := execution.run_current(r);
    pol := s#>'{fields,m1_permission}';
    lim := pol->'campaign_limits';
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    SELECT * INTO st FROM execution.https_loop_steps WHERE run_id = r AND step_index = idx;
    IF NOT FOUND OR st.next_intent_kind IS DISTINCT FROM 'https-head-v1' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = CASE WHEN is_live THEN 'live_effect_refused' ELSE 'fixture_release_refused' END;
    END IF;
    IF pol IS NULL OR jsonb_typeof(pol) <> 'object' OR pol->>'policy_version' IS DISTINCT FROM '1'
       OR now_s < (s->'fields'->>'starts_at')::bigint OR now_s >= (s->'fields'->>'ends_at')::bigint
       OR now_s < (pol->>'starts_at')::bigint OR now_s >= (pol->>'ends_at')::bigint
       OR NOT (s#>'{fields,included_assets}' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR (s#>'{fields,excluded_assets}' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR st.resolved_path IS DISTINCT FROM (pol->>'approved_path')
       OR NOT EXISTS (
           SELECT 1 FROM jsonb_array_elements(pol->'contact_rules') cr
           WHERE (cr->>'asset_ref')::uuid = st.resolved_asset_ref
             AND (cr->'rule'->>'exact' = st.resolved_hostname
                  OR cr->'rule'->>'label_suffix' = st.resolved_hostname
                  OR right(st.resolved_hostname, length(cr->'rule'->>'label_suffix') + 1) = '.' || (cr->'rule'->>'label_suffix'))
       )
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(pol->'excluded_names') ex
           WHERE ex->>'exact' = st.resolved_hostname
              OR ex->>'label_suffix' = st.resolved_hostname
              OR right(st.resolved_hostname, length(ex->>'label_suffix') + 1) = '.' || (ex->>'label_suffix')
       ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = CASE WHEN is_live THEN 'live_policy_denied' ELSE 'fixture_policy_denied' END;
    END IF;
    IF EXISTS (
        SELECT 1 FROM execution.https_loop_steps p1
        WHERE p1.run_id = r AND p1.step_index <> idx
          AND (p1.dns_attempt_id IS NOT NULL OR p1.refresh_attempt_id IS NOT NULL OR p1.head_attempt_id IS NOT NULL)
          AND (p1.resolved_asset_ref IS DISTINCT FROM st.resolved_asset_ref OR p1.resolved_hostname IS DISTINCT FROM st.resolved_hostname)
    ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = CASE WHEN is_live THEN 'live_effect_scope_refused' ELSE 'fixture_hostname_mismatch' END;
    END IF;
    SELECT * INTO cc FROM terrain.current_claims WHERE engagement_id = x.e AND campaign_id = x.c AND claim_id = st.parent_claim_id;
    IF NOT FOUND THEN RETURN '{"decision":"blocked","reason":"non_ttl_invalidity"}'::jsonb; END IF;
    SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = x.e AND campaign_id = x.c AND operation_id = cc.operation_id;
    IF NOT FOUND OR cc.claim_status <> 'observed' OR cc.conflicted OR cc.corroborated
       OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision
       OR cc.asset_ref IS DISTINCT FROM st.resolved_asset_ref
       OR cc.subject_name IS DISTINCT FROM st.resolved_hostname
       OR cc.vantage_ref IS DISTINCT FROM (pol->>'vantage_ref')::uuid
       OR cc.resolver_ipv4 IS DISTINCT FROM (pol->>'resolver_ipv4')
       OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0
       OR now_s < cc.perceived_at
       OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = x.e AND ic.campaign_id = x.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
        RETURN '{"decision":"blocked","reason":"non_ttl_invalidity"}'::jsonb;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) a WHERE a->>'ipv4' = st.resolved_ipv4) THEN
        RETURN '{"decision":"blocked","reason":"ip_missing_from_claim"}'::jsonb;
    END IF;
    cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
    IF now_s < cap_until THEN
        RETURN '{"decision":"not_needed"}'::jsonb;
    END IF;
    IF st.dependency_refresh IS DISTINCT FROM 'dns_once_if_stale' THEN
        RETURN '{"decision":"blocked","reason":"stale_without_refresh"}'::jsonb;
    END IF;
    IF st.refresh_outcome = 'completed' AND st.refresh_result_id IS NOT NULL THEN
        RETURN '{"decision":"not_needed"}'::jsonb;
    END IF;
    SELECT count(*) INTO ref_used FROM (
        SELECT attempt FROM execution.fixture_dns_attempts WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'refresh'
        UNION ALL
        SELECT attempt FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'dns_refresh'
    ) rf;
    IF st.refresh_attempt_id IS NOT NULL OR ref_used >= 1 THEN
        RETURN '{"decision":"blocked","reason":"refresh_already_attempted"}'::jsonb;
    END IF;
    SELECT count(*) INTO run_dns_used FROM (
        SELECT attempt FROM execution.fixture_dns_attempts WHERE run_id = r AND run_kind = 'm1_https_loop_v1'
        UNION ALL
        SELECT attempt FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind IN ('dns', 'dns_refresh')
    ) rd;
    SELECT count(*) INTO camp_dns_used FROM (
        SELECT fa.attempt FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c)
        UNION ALL
        SELECT ls.attempt FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (x.e, x.c) AND ls.attempt_kind IN ('dns', 'dns_refresh')
    ) cd;
    SELECT count(*) INTO camp_ref_used FROM (
        SELECT fa.attempt FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c) AND fa.attempt_kind = 'refresh'
        UNION ALL
        SELECT ls.attempt FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (x.e, x.c) AND ls.attempt_kind = 'dns_refresh'
    ) crf;
    SELECT count(*) INTO run_ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    SELECT count(*) INTO camp_ep_used FROM execution.https_loop_steps WHERE (e, c) = (x.e, x.c) AND episode_index IS NOT NULL;
    camp_ep_used := camp_ep_used
        + (SELECT count(*) FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c) AND fa.run_kind = 'legacy')
        + (SELECT count(*) FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (x.e, x.c) AND ls.run_kind = 'legacy');
    IF run_dns_used >= 3 OR camp_dns_used >= (lim->>'dns_questions')::int
       OR camp_ref_used >= (lim->>'dns_followups')::int
       OR (st.episode_index IS NULL AND (run_ep_used >= 3 OR camp_ep_used >= (lim->>'episodes')::int)) THEN
        RETURN '{"decision":"blocked","reason":"dns_budget_exhausted"}'::jsonb;
    END IF;
    SELECT (ord - 1)::integer INTO seed_idx FROM jsonb_array_elements(x.request->'seed_names') WITH ORDINALITY a(elem, ord)
    WHERE (elem->>'asset_ref')::uuid = st.resolved_asset_ref AND elem->>'name' = st.resolved_hostname LIMIT 1;
    IF seed_idx IS NULL THEN RETURN '{"decision":"blocked","reason":"seed_ineligible"}'::jsonb; END IF;
    RETURN jsonb_build_object('decision', 'need_refresh', 'seed_index', seed_idx);
END $$;

-- Enforce fixture-only mode on fixture DNS/HEAD reserve & commit ports.
CREATE OR REPLACE FUNCTION execution.reserve_https_loop_dns(r uuid, idx integer, kind text, seed_idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
    s jsonb; pol jsonb; lim jsonb; seed jsonb; req jsonb;
    cc terrain.current_claims; ev terrain.evaluations;
    attempt_id uuid; result_id uuid; cur_rev integer; n_ms bigint; now_s bigint; ep_num integer; cap_until bigint;
    init_used integer; ref_used integer; run_dns_used integer; camp_dns_used integer; camp_ref_used integer;
    run_ep_used integer; camp_ep_used integer;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 OR kind NOT IN ('initial', 'refresh') THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
       OR x.request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    s := execution.run_current(r);
    pol := s#>'{fields,m1_permission}';
    lim := pol->'campaign_limits';
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND OR NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = r AND y.phase = st.acquisition_phase AND y.claimed) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    IF kind = 'initial' AND st.next_intent_kind IS DISTINCT FROM 'dns_udp' THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused'; END IF;
    IF kind = 'refresh' AND (st.next_intent_kind IS DISTINCT FROM 'https-head-v1' OR st.dependency_refresh IS DISTINCT FROM 'dns_once_if_stale') THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    seed := x.request->'seed_names'->seed_idx;
    IF seed IS NULL OR (seed->>'asset_ref')::uuid IS DISTINCT FROM st.resolved_asset_ref OR seed->>'name' IS DISTINCT FROM st.resolved_hostname THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_scope_refused';
    END IF;
    IF kind = 'initial' AND st.dns_attempt_id IS NOT NULL THEN RETURN st.dns_request; END IF;
    IF kind = 'refresh' AND st.refresh_attempt_id IS NOT NULL THEN RETURN st.refresh_request; END IF;
    IF st.protection_state IS DISTINCT FROM 'protected' OR st.next_decision_id IS NULL OR st.protected_capacity IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    now_s := n_ms / 1000;
    IF pol IS NULL OR jsonb_typeof(pol) <> 'object' OR pol->>'policy_version' IS DISTINCT FROM '1'
       OR now_s < (s->'fields'->>'starts_at')::bigint OR now_s >= (s->'fields'->>'ends_at')::bigint
       OR now_s < (pol->>'starts_at')::bigint OR now_s >= (pol->>'ends_at')::bigint
       OR NOT (s#>'{fields,included_assets}' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR (s#>'{fields,excluded_assets}' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR NOT EXISTS (
           SELECT 1 FROM jsonb_array_elements(pol->'contact_rules') cr
           WHERE (cr->>'asset_ref')::uuid = st.resolved_asset_ref
             AND (cr->'rule'->>'exact' = st.resolved_hostname
                  OR cr->'rule'->>'label_suffix' = st.resolved_hostname
                  OR right(st.resolved_hostname, length(cr->'rule'->>'label_suffix') + 1) = '.' || (cr->'rule'->>'label_suffix'))
       )
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(pol->'excluded_names') ex
           WHERE ex->>'exact' = st.resolved_hostname
              OR ex->>'label_suffix' = st.resolved_hostname
              OR right(st.resolved_hostname, length(ex->>'label_suffix') + 1) = '.' || (ex->>'label_suffix')
       ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_policy_denied';
    END IF;
    IF EXISTS (
        SELECT 1 FROM execution.https_loop_steps p1
        WHERE p1.run_id = r AND p1.step_index <> idx
          AND (p1.dns_attempt_id IS NOT NULL OR p1.refresh_attempt_id IS NOT NULL OR p1.head_attempt_id IS NOT NULL)
          AND (p1.resolved_asset_ref IS DISTINCT FROM st.resolved_asset_ref OR p1.resolved_hostname IS DISTINCT FROM st.resolved_hostname)
    ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_hostname_mismatch';
    END IF;
    SELECT count(*) INTO init_used FROM execution.fixture_dns_attempts WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'initial';
    SELECT count(*) INTO ref_used FROM execution.fixture_dns_attempts WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'refresh';
    IF kind = 'initial' AND init_used >= 1 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_dns_exhausted'; END IF;
    SELECT count(*) INTO camp_ref_used FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c) AND fa.attempt_kind = 'refresh';
    IF kind = 'refresh' AND (ref_used >= 1 OR camp_ref_used >= (lim->>'dns_followups')::int) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_refresh_exhausted';
    END IF;
    SELECT count(*) INTO run_dns_used FROM execution.fixture_dns_attempts WHERE run_id = r AND run_kind = 'm1_https_loop_v1';
    SELECT count(*) INTO camp_dns_used FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c);
    SELECT count(*) INTO run_ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    SELECT count(*) INTO camp_ep_used FROM execution.https_loop_steps WHERE (e, c) = (x.e, x.c) AND episode_index IS NOT NULL;
    camp_ep_used := camp_ep_used
        + (SELECT count(*) FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c) AND fa.run_kind = 'legacy')
        + (SELECT count(*) FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (x.e, x.c) AND ls.run_kind = 'legacy');
    IF run_dns_used >= 3 OR camp_dns_used >= (lim->>'dns_questions')::int
       OR (st.episode_index IS NULL AND (run_ep_used >= 3 OR camp_ep_used >= (lim->>'episodes')::int)) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_budget_exhausted';
    END IF;
    IF kind = 'refresh' THEN
        SELECT * INTO cc FROM terrain.current_claims WHERE engagement_id = x.e AND campaign_id = x.c AND claim_id = st.parent_claim_id;
        IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible'; END IF;
        SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = x.e AND campaign_id = x.c AND operation_id = cc.operation_id;
        IF NOT FOUND OR cc.claim_status <> 'observed' OR cc.conflicted OR cc.corroborated
           OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision
           OR cc.asset_ref IS DISTINCT FROM st.resolved_asset_ref
           OR cc.subject_name IS DISTINCT FROM st.resolved_hostname
           OR cc.vantage_ref IS DISTINCT FROM (pol->>'vantage_ref')::uuid
           OR cc.resolver_ipv4 IS DISTINCT FROM (pol->>'resolver_ipv4')
           OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0
           OR now_s < cc.perceived_at
           OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) a WHERE a->>'ipv4' = st.resolved_ipv4)
           OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = x.e AND ic.campaign_id = x.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible';
        END IF;
        cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
        IF now_s < cap_until THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible'; END IF;
    END IF;
    IF st.episode_deadline_ms IS NULL THEN
        IF (x.deadline_at_ms - n_ms) < 314000 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_time_insufficient'; END IF;
        ep_num := run_ep_used + 1;
        UPDATE execution.https_loop_steps
        SET episode_index = ep_num, episode_deadline_ms = least(n_ms + 60000, x.deadline_at_ms - 254000)
        WHERE run_id = r AND step_index = idx;
    ELSE
        ep_num := st.episode_index;
        IF n_ms >= st.episode_deadline_ms OR (x.deadline_at_ms - n_ms) < 254000 THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_time_insufficient';
        END IF;
    END IF;
    SELECT coalesce(max(cr.claim_revision), 0) INTO cur_rev FROM terrain.claim_revisions cr
    WHERE cr.engagement_id = x.e AND cr.campaign_id = x.c AND cr.subject_name = st.resolved_hostname
      AND cr.claim_kind = 'dns_hostname_ipv4_resolution' AND cr.vantage_ref = (pol->>'vantage_ref')::uuid
      AND cr.resolver_ipv4 = pol->>'resolver_ipv4';
    attempt_id := gen_random_uuid(); result_id := gen_random_uuid();
    req := jsonb_build_object(
        'version', 'm1_terrain_dns_issued_observation_v1', 'engagement_id', x.e, 'campaign_id', x.c,
        'operator_ref', s#>'{fields,operator_ref}', 'expected_mission_revision', 1,
        'goal_ref', s#>'{fields,goal_ref}', 'exercise_mode', s#>'{fields,exercise_mode}',
        'correlation_id', result_id, 'source_kind', 'fixture_dns_acquisition',
        'vantage_ref', pol->'vantage_ref', 'resolver_ipv4', pol->'resolver_ipv4',
        'subject_name', st.resolved_hostname, 'asset_ref', st.resolved_asset_ref,
        'expected_claim_revision', cur_rev, 'perceived_at', (n_ms / 1000)::bigint, 'uncertainty_seconds', 0,
        'transport', 'udp', 'qtype', 'A', 'qclass', 'IN', 'rcode', 'noerror',
        'truncated', false, 'completeness', 'complete', 'limitations', '["single_vantage_only"]'::jsonb, 'answers', '[]'::jsonb
    );
    INSERT INTO execution.fixture_dns_attempts
        (attempt, run_id, decision, seed_index, profile, mode, release_sha256, session_generation, reserved_at_ms, episode_count, dns_questions, run_kind, step_index, attempt_kind)
    VALUES (attempt_id, r, st.decision_id, seed_idx, 'dns-routing-fixture-v1', 'controlled_fixture', x.release_sha256, ep_num, n_ms, 1, 1, 'm1_https_loop_v1', idx, kind);
    IF kind = 'initial' THEN
        UPDATE execution.https_loop_steps
        SET dns_attempt_id = attempt_id, dns_request = req, terrain_operation_id = coalesce(terrain_operation_id, gen_random_uuid()),
            status = 'acquisition_running', updated_at_ms = n_ms
        WHERE run_id = r AND step_index = idx;
    ELSE
        UPDATE execution.https_loop_steps
        SET refresh_attempt_id = attempt_id, refresh_request = req, terrain_operation_id = gen_random_uuid(),
            status = 'acquisition_running', updated_at_ms = n_ms
        WHERE run_id = r AND step_index = idx;
    END IF;
    RETURN req;
END $$;

CREATE OR REPLACE FUNCTION execution.commit_https_loop_dns(r uuid, idx integer, kind text, req jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
    att execution.fixture_dns_attempts; prev_res execution.fixture_dns_results;
    base jsonb; n_ms bigint; res_id uuid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 OR kind NOT IN ('initial', 'refresh')
       OR req IS NULL OR jsonb_typeof(req) <> 'object' THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
       OR x.request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture' THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    SELECT * INTO att FROM execution.fixture_dns_attempts a WHERE a.run_id = r AND a.step_index = idx AND a.run_kind = 'm1_https_loop_v1' AND a.attempt_kind = kind FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'fixture_result_refused'; END IF;
    base := CASE WHEN kind = 'initial' THEN st.dns_request ELSE st.refresh_request END;
    IF base IS NULL THEN RAISE EXCEPTION 'fixture_result_refused'; END IF;
    res_id := (base->>'correlation_id')::uuid;
    IF (req - 'answers') IS DISTINCT FROM (base - 'answers')
       OR (SELECT count(*) FROM jsonb_object_keys(req)) <> (SELECT count(*) FROM jsonb_object_keys(base))
       OR jsonb_typeof(req->'answers') IS DISTINCT FROM 'array'
       OR jsonb_array_length(req->'answers') > 4
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(req->'answers') a
           WHERE jsonb_typeof(a) IS DISTINCT FROM 'object' OR a - ARRAY['ipv4','ttl_seconds'] <> '{}'::jsonb
              OR (SELECT count(*) FROM jsonb_object_keys(a)) <> 2
              OR jsonb_typeof(a->'ipv4') IS DISTINCT FROM 'string' OR jsonb_typeof(a->'ttl_seconds') IS DISTINCT FROM 'number'
              OR (a->>'ttl_seconds') !~ '^(0|[1-9][0-9]*)$' OR (a->>'ttl_seconds')::bigint > 86400
              OR family((a->>'ipv4')::inet) <> 4 OR host((a->>'ipv4')::inet) IS DISTINCT FROM (a->>'ipv4')
              OR (a->>'ipv4')::inet << '0.0.0.0/8'::inet
       )
       OR (SELECT count(DISTINCT a->>'ipv4') FROM jsonb_array_elements(req->'answers') a) <> jsonb_array_length(req->'answers') THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    SELECT * INTO prev_res FROM execution.fixture_dns_results r1 WHERE r1.attempt = att.attempt FOR UPDATE;
    IF FOUND THEN
        IF prev_res.result = res_id AND prev_res.request = req THEN RETURN; END IF;
        RAISE EXCEPTION 'fixture_result_conflict';
    END IF;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    INSERT INTO execution.fixture_dns_results
        (result, run_id, attempt, profile, mode, release_sha256, received_at_ms, publication_obligation, request, run_kind, step_index, attempt_kind)
    VALUES (res_id, r, att.attempt, 'dns-routing-fixture-v1', 'controlled_fixture', x.release_sha256, n_ms, 'terrain.issued_dns.v1', req, 'm1_https_loop_v1', idx, kind);
    IF kind = 'initial' THEN
        UPDATE execution.https_loop_steps SET dns_result_id = res_id, updated_at_ms = n_ms WHERE run_id = r AND step_index = idx;
    ELSE
        UPDATE execution.https_loop_steps SET refresh_result_id = res_id, refresh_outcome = 'completed', updated_at_ms = n_ms WHERE run_id = r AND step_index = idx;
    END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.reserve_https_loop_head(r uuid, idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
    s jsonb; pol jsonb; lim jsonb; cc terrain.current_claims; ev terrain.evaluations;
    n_ms bigint; now_s bigint; cap_until bigint;
    attempt_id uuid; req jsonb;
    run_head_used integer; camp_head_used integer; run_ep_used integer; camp_ep_used integer; ep_num integer;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
       OR x.request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    s := execution.run_current(r);
    pol := s#>'{fields,m1_permission}';
    lim := pol->'campaign_limits';
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND OR st.next_intent_kind IS DISTINCT FROM 'https-head-v1'
       OR NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = r AND y.phase = st.acquisition_phase AND y.claimed) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    IF st.head_attempt_id IS NOT NULL THEN
        SELECT a.request INTO req FROM execution.fixture_https_attempts a WHERE a.attempt = st.head_attempt_id;
        RETURN req;
    END IF;
    IF st.protection_state IS DISTINCT FROM 'protected' OR st.next_decision_id IS NULL OR st.protected_capacity IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_release_refused';
    END IF;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    now_s := n_ms / 1000;
    IF pol IS NULL OR jsonb_typeof(pol) <> 'object' OR pol->>'policy_version' IS DISTINCT FROM '1'
       OR now_s < (s->'fields'->>'starts_at')::bigint OR now_s >= (s->'fields'->>'ends_at')::bigint
       OR now_s < (pol->>'starts_at')::bigint OR now_s >= (pol->>'ends_at')::bigint
       OR NOT (s#>'{fields,included_assets}' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR (s#>'{fields,excluded_assets}' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR st.resolved_path IS DISTINCT FROM (pol->>'approved_path')
       OR NOT EXISTS (
           SELECT 1 FROM jsonb_array_elements(pol->'contact_rules') cr
           WHERE (cr->>'asset_ref')::uuid = st.resolved_asset_ref
             AND (cr->'rule'->>'exact' = st.resolved_hostname
                  OR cr->'rule'->>'label_suffix' = st.resolved_hostname
                  OR right(st.resolved_hostname, length(cr->'rule'->>'label_suffix') + 1) = '.' || (cr->'rule'->>'label_suffix'))
       )
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(pol->'excluded_names') ex
           WHERE ex->>'exact' = st.resolved_hostname
              OR ex->>'label_suffix' = st.resolved_hostname
              OR right(st.resolved_hostname, length(ex->>'label_suffix') + 1) = '.' || (ex->>'label_suffix')
       ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_policy_denied';
    END IF;
    IF EXISTS (
        SELECT 1 FROM execution.https_loop_steps p1
        WHERE p1.run_id = r AND p1.step_index <> idx
          AND (p1.dns_attempt_id IS NOT NULL OR p1.refresh_attempt_id IS NOT NULL OR p1.head_attempt_id IS NOT NULL)
          AND (p1.resolved_asset_ref IS DISTINCT FROM st.resolved_asset_ref OR p1.resolved_hostname IS DISTINCT FROM st.resolved_hostname)
    ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_hostname_mismatch';
    END IF;
    SELECT count(*) INTO run_head_used FROM execution.fixture_https_attempts WHERE run_id = r;
    SELECT count(*) INTO camp_head_used FROM execution.fixture_https_attempts ha JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c);
    IF run_head_used >= 2 OR camp_head_used >= least((lim->>'head_requests')::int, (lim->>'tcp_connections')::int) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_head_exhausted';
    END IF;
    SELECT count(*) INTO run_ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    SELECT count(*) INTO camp_ep_used FROM execution.https_loop_steps WHERE (e, c) = (x.e, x.c) AND episode_index IS NOT NULL;
    camp_ep_used := camp_ep_used
        + (SELECT count(*) FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (x.e, x.c) AND fa.run_kind = 'legacy')
        + (SELECT count(*) FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (x.e, x.c) AND ls.run_kind = 'legacy');
    IF st.episode_index IS NULL AND (run_ep_used >= 3 OR camp_ep_used >= (lim->>'episodes')::int) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_budget_exhausted';
    END IF;
    IF st.episode_deadline_ms IS NULL THEN
        IF (x.deadline_at_ms - n_ms) < 314000 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_time_insufficient'; END IF;
        ep_num := run_ep_used + 1;
    ELSE
        ep_num := st.episode_index;
        IF n_ms >= st.episode_deadline_ms OR (x.deadline_at_ms - n_ms) < 254000 OR (x.deadline_at_ms - st.episode_deadline_ms) < 254000 THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_time_insufficient';
        END IF;
    END IF;
    IF st.refresh_result_id IS NULL THEN
        SELECT * INTO cc FROM terrain.current_claims WHERE engagement_id = x.e AND campaign_id = x.c AND claim_id = st.parent_claim_id;
        IF NOT FOUND OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible';
        END IF;
        SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = x.e AND campaign_id = x.c AND operation_id = cc.operation_id;
        IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible'; END IF;
    ELSE
        SELECT cc1.* INTO cc
        FROM terrain.observations ob1
        JOIN terrain.evaluations ev1 ON (ev1.engagement_id, ev1.campaign_id, ev1.operation_id, ev1.observation_id) = (ob1.engagement_id, ob1.campaign_id, ob1.operation_id, ob1.observation_id)
        JOIN terrain.current_claims cc1 ON (cc1.engagement_id, cc1.campaign_id, cc1.operation_id, cc1.claim_id) = (ev1.engagement_id, ev1.campaign_id, ev1.operation_id, ev1.claim_id)
        WHERE ob1.engagement_id = x.e AND ob1.campaign_id = x.c AND ob1.operation_id = st.terrain_operation_id
          AND ob1.correlation_id = st.refresh_result_id AND ob1.source_kind = 'fixture_dns_acquisition';
        IF NOT FOUND OR cc.claim_revision <= st.parent_claim_revision THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible';
        END IF;
        SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = x.e AND campaign_id = x.c AND operation_id = cc.operation_id;
        IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible'; END IF;
    END IF;
    IF cc.claim_status <> 'observed' OR cc.conflicted OR cc.corroborated
       OR cc.asset_ref IS DISTINCT FROM st.resolved_asset_ref OR cc.subject_name IS DISTINCT FROM st.resolved_hostname
       OR cc.vantage_ref IS DISTINCT FROM (pol->>'vantage_ref')::uuid OR cc.resolver_ipv4 IS DISTINCT FROM (pol->>'resolver_ipv4')
       OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0 OR now_s < cc.perceived_at
       OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) a WHERE a->>'ipv4' = st.resolved_ipv4)
       OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = x.e AND ic.campaign_id = x.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible';
    END IF;
    cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
    IF now_s >= cap_until THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'fixture_premise_ineligible'; END IF;
    attempt_id := gen_random_uuid();
    req := jsonb_build_object(
        'version', 'm1_https_head_request_v1', 'engagement_id', x.e, 'campaign_id', x.c, 'run_id', r, 'decision_id', st.decision_id,
        'attempt_id', attempt_id, 'asset_ref', st.resolved_asset_ref, 'endpoint_ref', st.resolved_endpoint_ref,
        'parent_claim_id', cc.claim_id, 'parent_claim_revision', cc.claim_revision,
        'hostname', st.resolved_hostname, 'ipv4', st.resolved_ipv4, 'port', 443,
        'approved_path', st.resolved_path, 'method', 'HEAD', 'resolver_ipv4', pol->'resolver_ipv4', 'vantage_ref', pol->'vantage_ref',
        'dependency_refresh', st.dependency_refresh, 'deadline_at_ms', coalesce(st.episode_deadline_ms, least(n_ms + 60000, x.deadline_at_ms - 254000))
    );
    INSERT INTO execution.fixture_https_attempts
        (attempt, run_id, step_index, session_generation, decision, asset_ref, endpoint_ref, hostname, ipv4, approved_path,
         parent_claim_id, parent_claim_revision, profile, mode, release_sha256, reserved_at_ms, request)
    VALUES (attempt_id, r, idx, ep_num, st.decision_id, st.resolved_asset_ref, st.resolved_endpoint_ref, st.resolved_hostname,
        st.resolved_ipv4, st.resolved_path, cc.claim_id, cc.claim_revision, 'https-head-fixture-v1', 'controlled_fixture',
        x.release_sha256, n_ms, req);
    UPDATE execution.https_loop_steps
    SET episode_index = ep_num, episode_deadline_ms = coalesce(episode_deadline_ms, least(n_ms + 60000, x.deadline_at_ms - 254000)),
        head_attempt_id = attempt_id, head_terrain_operation_id = coalesce(head_terrain_operation_id, gen_random_uuid()),
        status = 'acquisition_running', updated_at_ms = n_ms
    WHERE run_id = r AND step_index = idx;
    RETURN req;
END $$;

CREATE OR REPLACE FUNCTION execution.commit_https_loop_head(r uuid, idx integer, res jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps; att execution.fixture_https_attempts;
    prev_hr execution.fixture_https_results; actor oid; n_ms bigint; res_id uuid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3
       OR res IS NULL OR jsonb_typeof(res) <> 'object' THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
       OR x.request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture' THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    IF (SELECT count(*) FROM jsonb_object_keys(res)) <> 23
       OR res - ARRAY['version','engagement_id','campaign_id','run_id','decision_id','attempt_id','result_id',
           'asset_ref','endpoint_ref','hostname','ipv4','port','approved_path','method',
           'parent_claim_id','parent_claim_revision','perceived_at','uncertainty_seconds',
           'outcome','status_code','redirect','source_kind','limitations'] <> '{}'::jsonb THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    SELECT * INTO att FROM execution.fixture_https_attempts a WHERE a.run_id = r AND a.step_index = idx FOR UPDATE;
    IF NOT FOUND OR res->>'version' IS DISTINCT FROM 'm1_https_head_result_v1'
       OR (res->>'attempt_id')::uuid IS DISTINCT FROM att.attempt
       OR (res->>'engagement_id')::uuid IS DISTINCT FROM x.e OR (res->>'campaign_id')::uuid IS DISTINCT FROM x.c
       OR (res->>'run_id')::uuid IS DISTINCT FROM r OR (res->>'decision_id')::uuid IS DISTINCT FROM att.decision
       OR (res->>'asset_ref')::uuid IS DISTINCT FROM att.asset_ref OR res->>'endpoint_ref' IS DISTINCT FROM att.endpoint_ref
       OR res->>'hostname' IS DISTINCT FROM att.hostname OR res->>'ipv4' IS DISTINCT FROM att.ipv4
       OR jsonb_typeof(res->'port') IS DISTINCT FROM 'number' OR res->>'port' IS DISTINCT FROM '443'
       OR res->>'approved_path' IS DISTINCT FROM att.approved_path OR res->>'method' IS DISTINCT FROM 'HEAD'
       OR (res->>'parent_claim_id')::uuid IS DISTINCT FROM att.parent_claim_id
       OR jsonb_typeof(res->'parent_claim_revision') IS DISTINCT FROM 'number'
       OR (res->>'parent_claim_revision')::int IS DISTINCT FROM att.parent_claim_revision
       OR jsonb_typeof(res->'perceived_at') IS DISTINCT FROM 'number' OR (res->>'perceived_at') !~ '^[1-9][0-9]*$'
       OR jsonb_typeof(res->'uncertainty_seconds') IS DISTINCT FROM 'number' OR (res->>'uncertainty_seconds') !~ '^(0|[1-9][0-9]*)$'
       OR coalesce(res->>'outcome', '') NOT IN ('head_response','connection_failed','tls_failed','timed_out','execution_refused')
       OR (CASE WHEN res->>'outcome' = 'head_response'
            THEN jsonb_typeof(res->'status_code') IS DISTINCT FROM 'number' OR (res->>'status_code') !~ '^[1-9][0-9]*$' OR (res->>'status_code')::int NOT BETWEEN 100 AND 599
            ELSE res->'status_code' IS DISTINCT FROM 'null'::jsonb END)
       OR coalesce(res->>'redirect', '') NOT IN ('none','refused_redirect')
       OR res->>'source_kind' IS DISTINCT FROM 'fixture_https_acquisition'
       OR jsonb_typeof(res->'limitations') IS DISTINCT FROM 'array'
       OR jsonb_array_length(res->'limitations') NOT BETWEEN 1 AND 8
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(res->'limitations') l
           WHERE jsonb_typeof(l) IS DISTINCT FROM 'string' OR length(l#>>'{}') NOT BETWEEN 1 AND 96
              OR position('://' in (l#>>'{}')) > 0 OR position(chr(10) in (l#>>'{}')) > 0
       )
       OR jsonb_typeof(res->'result_id') IS DISTINCT FROM 'string'
       OR (res->>'result_id')::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    res_id := (res->>'result_id')::uuid;
    SELECT * INTO prev_hr FROM execution.fixture_https_results hr WHERE hr.run_id = r AND hr.step_index = idx FOR UPDATE;
    IF FOUND THEN
        IF prev_hr.result = res_id AND prev_hr.attempt = att.attempt AND prev_hr.result_payload = res THEN RETURN; END IF;
        RAISE EXCEPTION 'fixture_result_conflict';
    END IF;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    INSERT INTO execution.fixture_https_results
        (result, run_id, step_index, attempt, request, result_payload, profile, mode, release_sha256, received_at_ms, publication_obligation)
    VALUES (res_id, r, idx, att.attempt, att.request, res, 'https-head-fixture-v1', 'controlled_fixture', x.release_sha256, n_ms, 'terrain.issued_https.v1');
    UPDATE execution.https_loop_steps SET head_result_id = res_id, head_outcome = res->>'outcome', updated_at_ms = n_ms WHERE run_id = r AND step_index = idx;
END $$;

CREATE OR REPLACE FUNCTION execution.read_https_loop_head_result(r uuid, idx integer) RETURNS jsonb
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; actor oid; payload jsonb;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN
        RAISE EXCEPTION 'run_inspect_unknown';
    END IF;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    IF x.writer_oid IS DISTINCT FROM actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN
        RAISE EXCEPTION 'run_inspect_unknown';
    END IF;
    IF x.request->>'execution_mode' = 'live' THEN
        SELECT lr.result_payload INTO payload FROM execution.live_https_results lr WHERE lr.run_id = r AND lr.step_index = idx;
    ELSE
        SELECT fr.result_payload INTO payload FROM execution.fixture_https_results fr WHERE fr.run_id = r AND fr.step_index = idx;
    END IF;
    RETURN payload;
END $$;
REVOKE ALL ON FUNCTION execution.read_https_loop_head_result(uuid, integer) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.read_https_loop_head_result(uuid, integer) TO dw_m1_broker;

CREATE OR REPLACE FUNCTION execution.run_summary_without_diagnostics(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    x execution.autonomous_runs; y execution.autonomous_control; status text; summary jsonb;
    n bigint := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    exp_purpose text; qual_ok boolean;
BEGIN
    IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT * INTO y FROM execution.autonomous_control WHERE run_id = r;
    status := CASE WHEN y.status = 'running' AND least(y.lease_until_ms, x.deadline_at_ms) <= n THEN 'unknown_supervisor_lost' ELSE y.status END;
    exp_purpose := CASE x.request->>'run_profile'
        WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE'
        WHEN 'autonomous-https-genai-lab-v1' THEN 'HTTPS_LOOP_LAB'
        ELSE 'LAB' END;
    qual_ok := x.request->>'execution_mode' = 'live' AND coalesce((
        SELECT n >= z.valid_from_ms AND n < z.valid_until_ms AND z.run_profile = x.request->>'run_profile'
           AND z.release_sha256 = x.release_sha256 AND host(z.resolver_ipv4) = x.request#>>'{live_binding,resolver_ipv4}'
           AND z.purpose = exp_purpose
        FROM execution.live_qualifications z WHERE z.qualification_ref = x.qualification_ref
    ), false);
    IF x.request->>'version' = 'm1_https_loop_v1' THEN
        summary := jsonb_build_object(
            'kind', 'm1_https_loop_run_receipt_v1', 'run_id', r, 'run_profile', x.request->'run_profile',
            'execution_mode', x.request->'execution_mode', 'inference_profile', x.request->'inference_profile',
            'status', status, 'phase', y.phase, 'dispatch_granted', false,
            'phases', (SELECT coalesce(jsonb_agg(jsonb_build_object('phase', p.phase, 'status', p.status, 'recorded_at_ms', p.recorded_at_ms) ORDER BY p.recorded_at_ms), '[]') FROM execution.autonomous_phases p WHERE p.run_id = r),
            'steps', execution.https_loop_steps_for_run(r),
            'inference', (SELECT coalesce(jsonb_agg(jsonb_build_object('decision', ic.d, 'call', ic.call, 'binding', ic.binding, 'state', ic.state, 'reported', ic.usage) ORDER BY ic.d, ic.call), '[]')
                FROM execution.inference_calls ic JOIN execution.https_loop_steps st ON st.run_id = r AND st.decision_id = ic.d WHERE (ic.e, ic.c) = (x.e, x.c)),
            'dns_effect_count', (SELECT count(*) FROM execution.fixture_dns_results WHERE run_id = r)
                + (SELECT count(*) FROM execution.live_dns_results WHERE run_id = r),
            'head_effect_count', (SELECT count(*) FROM execution.fixture_https_results WHERE run_id = r)
                + (SELECT count(*) FROM execution.live_https_results WHERE run_id = r),
            'decision_count', (SELECT count(*) FROM cognition.reasoning_decisions cd JOIN execution.https_loop_steps st ON st.run_id = r AND st.decision_id = cd.d WHERE (cd.e, cd.c) = (x.e, x.c)),
            'decision_history_count', (SELECT count(*) FROM trajectory.reasoning_history h JOIN execution.https_loop_steps st ON st.run_id = r AND st.decision_id = h.operation_id WHERE (h.engagement_id, h.campaign_id) = (x.e, x.c) AND h.status = 'accepted'),
            'terrain_history_count', (SELECT count(*) FROM trajectory.terrain_history th JOIN execution.https_loop_steps st ON st.run_id = r AND th.operation_id IN (st.terrain_operation_id, st.head_terrain_operation_id) WHERE th.status = 'accepted'),
            'recorded_decisions', (SELECT coalesce(jsonb_agg(jsonb_build_object('step_index', st.step_index, 'decision', cd.d, 'event', cd.event,
                'version', cd.payload->'version', 'disposition', cd.payload->'disposition', 'proposal', cd.payload->'proposal',
                'occurred_at_ms', cd.payload->'occurred_at_ms', 'recorded_at_ms', cd.payload->'recorded_at_ms', 'dispatch_granted', false,
                'history', CASE WHEN EXISTS (SELECT 1 FROM trajectory.reasoning_history h WHERE (h.engagement_id, h.campaign_id, h.operation_id, h.event_id, h.status, h.contract) = (cd.e, cd.c, cd.d, cd.event, 'accepted', cd.payload)) THEN 'completed' ELSE 'pending_unknown' END)
                ORDER BY st.step_index), '[]')
                FROM execution.https_loop_steps st JOIN cognition.reasoning_decisions cd ON (cd.e, cd.c, cd.d) = (x.e, x.c, st.decision_id) WHERE st.run_id = r),
            'https_results', (SELECT coalesce(jsonb_agg(res_row ORDER BY (res_row->>'step_index')::int), '[]') FROM (
                SELECT jsonb_build_object('step_index', fr.step_index, 'attempt', fr.attempt, 'result', fr.result,
                    'endpoint_ref', fr.result_payload->'endpoint_ref', 'outcome', fr.result_payload->'outcome', 'status_code', fr.result_payload->'status_code',
                    'whole_host_proven', false, 'objective_completed', false) AS res_row
                FROM execution.fixture_https_results fr WHERE fr.run_id = r
                UNION ALL
                SELECT jsonb_build_object('step_index', lr.step_index, 'attempt', lr.attempt, 'result', lr.result,
                    'endpoint_ref', lr.result_payload->'endpoint_ref', 'outcome', lr.result_payload->'outcome', 'status_code', lr.result_payload->'status_code',
                    'whole_host_proven', false, 'objective_completed', false) AS res_row
                FROM execution.live_https_results lr WHERE lr.run_id = r
            ) combined)
        );
        IF x.request->>'execution_mode' = 'live' THEN
            summary := summary || jsonb_build_object('live_qualified', qual_ok);
        END IF;
        RETURN summary;
    END IF;
    summary := jsonb_build_object(
        'kind', 'm1_autonomous_run_receipt_v1', 'run_id', r, 'run_profile', x.request->'run_profile',
        'execution_mode', x.request->'execution_mode', 'status', status, 'phase', y.phase, 'dispatch_granted', false,
        'live_qualified', qual_ok,
        'decision_one', x.decision_one, 'decision_two', x.decision_two, 'attempt', x.attempt, 'result', x.result, 'terrain_operation', x.terrain_operation,
        'phases', (SELECT coalesce(jsonb_agg(jsonb_build_object('phase', p.phase, 'status', p.status, 'recorded_at_ms', p.recorded_at_ms) ORDER BY p.recorded_at_ms), '[]') FROM execution.autonomous_phases p WHERE run_id = r),
        'inference', (SELECT coalesce(jsonb_agg(jsonb_build_object('decision', ic.d, 'call', ic.call, 'binding', ic.binding, 'state', ic.state, 'reported', ic.usage) ORDER BY ic.d, ic.call), '[]') FROM execution.inference_calls ic WHERE (ic.e, ic.c) = (x.e, x.c) AND ic.d IN (x.decision_one, x.decision_two)),
        'acquisition', CASE WHEN x.request->>'version' = 'm1_autonomous_run_request_v4' THEN CASE WHEN EXISTS (SELECT 1 FROM execution.live_dns_results WHERE run_id = r) THEN 'committed_live_result' WHEN EXISTS (SELECT 1 FROM execution.live_effect_sessions WHERE run_id = r) THEN 'reserved_outcome_unknown' ELSE 'not_reserved' END
            WHEN EXISTS (SELECT 1 FROM execution.fixture_dns_results WHERE run_id = r) THEN 'committed_fixture_result' WHEN EXISTS (SELECT 1 FROM execution.fixture_dns_attempts WHERE run_id = r) THEN 'reserved_outcome_unknown' ELSE 'not_reserved' END,
        'terrain', CASE WHEN EXISTS (SELECT 1 FROM terrain.outbox WHERE operation_id = x.terrain_operation) THEN 'committed' ELSE 'pending_or_not_requested' END,
        'effect_count', (SELECT count(*) FROM execution.fixture_dns_results WHERE run_id = r) + (SELECT count(*) FROM execution.live_dns_results WHERE run_id = r),
        'decision_count', (SELECT count(*) FROM cognition.reasoning_decisions cd WHERE (cd.e, cd.c) = (x.e, x.c) AND cd.d IN (x.decision_one, x.decision_two)),
        'decision_history_count', (SELECT count(*) FROM trajectory.reasoning_history h WHERE (h.engagement_id, h.campaign_id) = (x.e, x.c) AND h.operation_id IN (x.decision_one, x.decision_two) AND h.status = 'accepted'),
        'terrain_history_count', (SELECT count(*) FROM trajectory.terrain_history h WHERE h.operation_id = x.terrain_operation AND h.status = 'accepted')
    );
    IF x.request->>'version' IN ('m1_autonomous_run_request_v2', 'm1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4') THEN
        summary := summary || jsonb_build_object('inference_profile', x.request->'inference_profile',
            'recorded_decisions', (SELECT coalesce(jsonb_agg(jsonb_build_object('decision', cd.d, 'event', cd.event,
                'version', cd.payload->'version', 'disposition', cd.payload->'disposition', 'proposal', cd.payload->'proposal',
                'occurred_at_ms', cd.payload->'occurred_at_ms', 'recorded_at_ms', cd.payload->'recorded_at_ms', 'dispatch_granted', false,
                'current_use', CASE WHEN cd.payload->>'disposition' = 'recorded_stale_premises' THEN 'blocked_stale_premises' WHEN cd.payload->>'disposition' = 'validated_proposal' THEN 'requires_current_owner_admission' ELSE 'not_admitted' END,
                'history', CASE WHEN EXISTS (SELECT 1 FROM trajectory.reasoning_history h WHERE (h.engagement_id, h.campaign_id, h.operation_id, h.event_id, h.status, h.contract) = (cd.e, cd.c, cd.d, cd.event, 'accepted', cd.payload)) THEN 'completed' ELSE 'pending_unknown' END)
                ORDER BY CASE WHEN cd.d = x.decision_one THEN 0 ELSE 1 END), '[]') FROM cognition.reasoning_decisions cd WHERE (cd.e, cd.c) = (x.e, x.c) AND cd.d IN (x.decision_one, x.decision_two)));
    END IF;
    RETURN summary;
END $$;

-- 6. Preserve legacy v4 single-attempt live APIs with run_kind = 'legacy' & attempt joins.
CREATE OR REPLACE FUNCTION execution.live_effect_record(r uuid) RETURNS jsonb
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT jsonb_build_object('run_id', s.run_id, 'request', s.request, 'result_ref', s.result_ref,
        'phase', x.phase, 'possibly_started', x.possibly_started,
        'attempt_deadline_at_ms', x.attempt_deadline_at_ms, 'result', b.projection, 'disposal_confirmed', x.disposal_confirmed)
    FROM execution.live_effect_sessions s
    JOIN execution.live_effect_control x ON x.attempt = s.attempt
    LEFT JOIN execution.live_dns_results b ON b.attempt = s.attempt
    WHERE s.run_id = r AND s.run_kind = 'legacy'
      AND s.writer_oid = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
      AND execution.private_live_actor('dw_m1_broker')
$$;

CREATE OR REPLACE FUNCTION execution.live_effect_reserve(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    a execution.autonomous_runs; prior execution.live_effect_sessions;
    reg jsonb; f jsonb; p jsonb; seed jsonb; proposal jsonb; pack jsonb;
    bound bigint; n bigint; g bigint; request jsonb; actor oid;
BEGIN
    SELECT * INTO a FROM execution.autonomous_runs WHERE run_id = r;
    IF a.run_id IS NULL THEN RAISE EXCEPTION 'live_effect_refused'; END IF;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF a.request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4' THEN
        RAISE EXCEPTION 'live_effect_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(a.e::text || ':' || a.c::text, 0));
    SELECT * INTO prior FROM execution.live_effect_sessions s WHERE s.run_id = r AND s.run_kind = 'legacy';
    IF FOUND THEN
        IF prior.writer_oid IS DISTINCT FROM actor OR NOT execution.private_live_actor('dw_m1_broker') THEN
            RAISE EXCEPTION 'live_effect_refused';
        END IF;
        RETURN execution.live_effect_record(r);
    END IF;
    bound := execution.private_live_deadline(r);
    reg := execution.run_current(r); f := reg->'fields'; p := f->'m1_permission';
    seed := a.request->'seed_names'->0;
    IF p IS NULL OR jsonb_typeof(p) <> 'object' OR p->>'policy_version' IS DISTINCT FROM '1'
       OR p->'resolver_ipv4' IS DISTINCT FROM a.request#>'{live_binding,resolver_ipv4}'
       OR (a.request->>'run_profile' = 'autonomous-dns-genai-lab-v1' AND p->>'resolver_ipv4' IS DISTINCT FROM '192.0.2.53')
       OR floor(extract(epoch FROM clock_timestamp()))::bigint < (p->>'starts_at')::bigint
       OR floor(extract(epoch FROM clock_timestamp()))::bigint >= (p->>'ends_at')::bigint
       OR seed IS NULL THEN
        RAISE EXCEPTION 'live_effect_refused';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = r AND y.phase = 'acquisition' AND y.claimed)
       OR EXISTS (SELECT 1 FROM execution.inference_reservations h WHERE (h.engagement_id, h.campaign_id) = (a.e, a.c)
           AND h.record_revision = 1 AND NOT EXISTS (SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = (a.e, a.c, h.operation_id)))
       OR EXISTS (SELECT 1 FROM execution.fixture_dns_attempts WHERE run_id = r)
       OR EXISTS (SELECT 1 FROM execution.live_effect_sessions WHERE (e, c) = (a.e, a.c)) THEN
        RAISE EXCEPTION 'live_sender_not_closed';
    END IF;
    SELECT payload->'proposal' INTO proposal FROM cognition.reasoning_decisions d
    WHERE (d.e, d.c, d.d) = (a.e, a.c, a.decision_one) AND payload->>'disposition' = 'validated_proposal'
      AND EXISTS (SELECT 1 FROM trajectory.reasoning_history h WHERE (h.engagement_id, h.campaign_id, h.operation_id, h.event_id, h.status, h.contract) = (a.e, a.c, a.decision_one, d.event, 'accepted', d.payload));
    SELECT call.p::jsonb INTO pack FROM execution.inference_calls call JOIN execution.inference_reservations h ON h.generation_call_id = call.call
    WHERE (call.e, call.c, call.d) = (a.e, a.c, a.decision_one) AND call.state = 'completed';
    IF proposal IS NULL OR proposal#>>'{next_intent,category}' IS DISTINCT FROM 'observe_more'
       OR proposal#>>'{next_intent,manifest_reference}' IS DISTINCT FROM 'dns_udp'
       OR NOT coalesce(proposal#>'{next_intent,references}' @> '["asset_0"]'::jsonb, false)
       OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(pack->'references') v WHERE v->>'id' = 'asset_0' AND v->>'owner' = 'operator' AND v->>'source' = 'admitted_operator_seed' AND v->'entity' = seed->'asset_ref')
       OR coalesce(proposal#>'{next_intent,view_request}', 'null'::jsonb) <> 'null'::jsonb
       OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(p->'contact_rules') rule WHERE rule->'asset_ref' = seed->'asset_ref'
           AND (rule->'rule'->>'exact' = seed->>'name' OR rule->'rule'->>'label_suffix' = seed->>'name'
                OR right(seed->>'name', length(rule->'rule'->>'label_suffix') + 1) = '.' || (rule->'rule'->>'label_suffix')))
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(p->'excluded_names') rule WHERE rule->>'exact' = seed->>'name'
           OR rule->>'label_suffix' = seed->>'name' OR right(seed->>'name', length(rule->'label_suffix') + 1) = '.' || (rule->>'label_suffix'))
       OR NOT coalesce((p#>>'{campaign_limits,episodes}')::bigint >= 1, false)
       OR NOT coalesce((p#>>'{campaign_limits,dns_questions}')::bigint >= 1, false) THEN
        RAISE EXCEPTION 'live_effect_scope_refused';
    END IF;
    UPDATE execution.session_fences SET generation = generation + 1, epoch = epoch + 1
    WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id IS NULL AND phase = 'idle' RETURNING generation INTO g;
    IF g IS NULL THEN RAISE EXCEPTION 'live_effect_session_busy'; END IF;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    request := jsonb_build_object('schema', 'm1_dns_ipc_v1', 'kind', 'REQUEST', 'attempt', a.attempt,
        'session_generation', g, 'sequence', 0, 'engagement_id', a.e, 'campaign_id', a.c, 'decision_id', a.decision_one,
        'profile', 'dns-routing-v2', 'execution_mode', 'live', 'run_profile', a.request->'run_profile',
        'release_sha256', a.release_sha256, 'qualification_ref', a.qualification_ref, 'name', seed->'name',
        'resolver_ipv4', p->'resolver_ipv4', 'transport', 'udp', 'query_id', mod(get_byte(uuid_send(a.attempt), 0) * 256 + get_byte(uuid_send(a.attempt), 1), 65536),
        'allowed_names', jsonb_build_array(seed->'name'), 'deadline_at_ms', bound, 'max_duration_ms', 2000);
    INSERT INTO execution.live_effect_sessions
        (run_id, e, c, generation, writer_oid, mission_revision, decision, attempt, result_ref, request, qualification_ref, deadline_at_ms, run_kind, step_index, attempt_kind, expected_claim_revision)
    VALUES (r, a.e, a.c, g, a.writer_oid, 1, a.decision_one, a.attempt, a.result, request, a.qualification_ref, bound, 'legacy', 0, 'dns', 0);
    INSERT INTO execution.live_effect_control
        (run_id, phase, possibly_started, attempt_deadline_at_ms, disposal_confirmed, attempt, granted_phase)
    VALUES (r, 'reserved', false, bound, false, a.attempt, NULL);
    INSERT INTO execution.live_effect_events
        (run_id, generation, phase, occurred_at_ms, attempt)
    VALUES (r, g, 'reserved', n, a.attempt);
    RETURN execution.live_effect_record(r);
END $$;

CREATE OR REPLACE FUNCTION execution.live_effect_current(r uuid, g bigint) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; bound bigint;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'legacy';
    IF g IS NULL OR s.generation IS DISTINCT FROM g THEN RAISE EXCEPTION 'live_effect_identity_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = s.attempt FOR UPDATE;
    bound := execution.private_live_deadline(r);
    IF x.phase NOT IN ('reserved', 'may_start') OR x.disposal_confirmed
       OR (x.phase = 'may_start' AND floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint >= x.attempt_deadline_at_ms) THEN
        RAISE EXCEPTION 'live_effect_stopped';
    END IF;
    RETURN CASE WHEN x.phase = 'may_start' THEN least(bound, x.attempt_deadline_at_ms) ELSE bound END;
END $$;

CREATE OR REPLACE FUNCTION execution.live_effect_may_start(r uuid, g bigint) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; bound bigint; n bigint; att uuid;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'legacy';
    IF g IS NULL OR s.generation IS DISTINCT FROM g THEN RAISE EXCEPTION 'live_effect_identity_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    bound := execution.live_effect_current(r, g);
    att := s.attempt;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    UPDATE execution.live_effect_control
    SET phase = 'may_start', possibly_started = true, attempt_deadline_at_ms = least(n + 2000, bound)
    WHERE attempt = att AND phase = 'reserved' AND NOT possibly_started;
    IF NOT FOUND THEN RAISE EXCEPTION 'live_effect_already_charged'; END IF;
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt) VALUES (r, g, 'may_start', n, att);
    RETURN (SELECT attempt_deadline_at_ms FROM execution.live_effect_control WHERE attempt = att);
END $$;

CREATE OR REPLACE FUNCTION execution.live_dns_result(r uuid, g bigint, v jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; b execution.live_dns_results;
 n bigint; started bigint;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'legacy';
    IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
        RAISE EXCEPTION 'live_result_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO b FROM execution.live_dns_results WHERE attempt = s.attempt;
    IF FOUND THEN
        IF b.projection IS DISTINCT FROM v THEN RAISE EXCEPTION 'live_result_conflict'; END IF;
        RETURN;
    END IF;
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = s.attempt FOR UPDATE;
    SELECT occurred_at_ms INTO started FROM execution.live_effect_events WHERE attempt = s.attempt AND phase = 'may_start';
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF x.phase NOT IN ('may_start', 'stopped') OR NOT x.possibly_started OR x.disposal_confirmed OR started IS NULL
       OR NOT execution.private_live_result_shape(v, s.request, least(n, x.attempt_deadline_at_ms), started) THEN
        RAISE EXCEPTION 'live_result_refused';
    END IF;
    INSERT INTO execution.live_dns_results
        (result, run_id, attempt, session_generation, projection, received_at_ms, publication_obligation, run_kind, step_index, attempt_kind)
    VALUES (s.result_ref, r, s.attempt, g, v, n, 'terrain.issued_dns.v2', 'legacy', 0, 'dns');
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt) VALUES (r, g, 'result_recorded', n, s.attempt);
    UPDATE execution.live_effect_control SET phase = 'result_recorded' WHERE attempt = s.attempt AND phase = 'may_start';
END $$;

CREATE OR REPLACE FUNCTION execution.live_effect_stop(r uuid, g bigint, disposed boolean) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; n bigint; target text;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'legacy';
    IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) OR disposed IS NULL THEN
        RAISE EXCEPTION 'live_stop_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = s.attempt FOR UPDATE;
    IF x.phase = 'closed' THEN RETURN; END IF;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    target := CASE WHEN disposed THEN 'closed' ELSE 'stopped' END;
    UPDATE execution.live_effect_control SET phase = target, disposal_confirmed = disposed WHERE attempt = s.attempt;
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt) VALUES (r, g, target, n, s.attempt) ON CONFLICT DO NOTHING;
END $$;

-- 7. Attempt-keyed live APIs and private HTTPS IPC/result validator for m1_https_loop_v1.
CREATE FUNCTION execution.private_live_attempt_bound(s execution.live_effect_sessions) RETURNS bigint
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    a execution.autonomous_runs; q execution.live_qualifications; reg jsonb; p jsonb; st execution.https_loop_steps;
    cc terrain.current_claims; ev terrain.evaluations;
    n bigint; now_s bigint; bound bigint; cap_until bigint;
    bound_claim_id uuid; bound_claim_rev integer; bound_claim_until_s bigint;
BEGIN
    IF NOT execution.private_live_actor('dw_m1_broker') OR s.attempt IS NULL THEN
        RAISE EXCEPTION 'live_effect_refused';
    END IF;
    SELECT * INTO a FROM execution.autonomous_runs WHERE run_id = s.run_id;
    SELECT * INTO q FROM execution.live_qualifications WHERE qualification_ref = a.qualification_ref;
    reg := execution.run_current(s.run_id); p := reg->'fields'->'m1_permission';
    SELECT * INTO st FROM execution.https_loop_steps WHERE run_id = s.run_id AND step_index = s.step_index;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    now_s := n / 1000;
    IF q.qualification_ref IS NULL OR n < q.valid_from_ms OR n >= q.valid_until_ms
       OR q.run_profile IS DISTINCT FROM 'autonomous-https-genai-lab-v1'
       OR q.purpose IS DISTINCT FROM 'HTTPS_LOOP_LAB'
       OR q.release_sha256 IS DISTINCT FROM a.release_sha256
       OR host(q.resolver_ipv4) IS DISTINCT FROM '192.0.2.53'
       OR st.acquisition_phase IS NULL THEN
        RAISE EXCEPTION 'live_qualification_unavailable';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = s.run_id AND y.phase = st.acquisition_phase AND y.claimed) THEN
        RAISE EXCEPTION 'live_effect_refused';
    END IF;
    bound := least(
        a.deadline_at_ms,
        q.valid_until_ms,
        (reg->'fields'->>'ends_at')::bigint * 1000,
        (p->>'ends_at')::bigint * 1000,
        (reg#>>'{fields,m1_inference_permission,ends_at}')::bigint * 1000,
        st.episode_deadline_ms
    );
    IF s.attempt_kind = 'dns_refresh' THEN
        SELECT * INTO cc FROM terrain.current_claims
        WHERE engagement_id = a.e AND campaign_id = a.c AND claim_id = st.parent_claim_id;
        IF NOT FOUND THEN RAISE EXCEPTION 'live_premise_ineligible'; END IF;
        SELECT * INTO ev FROM terrain.evaluations
        WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id = cc.operation_id;
        IF NOT FOUND OR cc.claim_status <> 'observed' OR cc.conflicted OR cc.corroborated
           OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision
           OR cc.asset_ref IS DISTINCT FROM st.resolved_asset_ref
           OR cc.subject_name IS DISTINCT FROM st.resolved_hostname
           OR cc.vantage_ref IS DISTINCT FROM (p->>'vantage_ref')::uuid
           OR cc.resolver_ipv4 IS DISTINCT FROM (p->>'resolver_ipv4')
           OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0
           OR now_s < cc.perceived_at
           OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) ans WHERE ans->>'ipv4' = st.resolved_ipv4)
           OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = a.e AND ic.campaign_id = a.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
            RAISE EXCEPTION 'live_premise_ineligible';
        END IF;
        cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
        IF now_s < cap_until THEN RAISE EXCEPTION 'live_premise_ineligible'; END IF;
    ELSIF s.attempt_kind = 'head' THEN
        bound_claim_id := (s.request->>'parent_claim_id')::uuid;
        bound_claim_rev := (s.request->>'parent_claim_revision')::integer;
        bound_claim_until_s := (s.request->>'claim_valid_until_s')::bigint;
        IF bound_claim_id IS NULL OR bound_claim_rev IS DISTINCT FROM s.expected_claim_revision THEN
            RAISE EXCEPTION 'live_premise_ineligible';
        END IF;
        IF st.refresh_result_id IS NULL THEN
            IF bound_claim_id IS DISTINCT FROM st.parent_claim_id
               OR bound_claim_rev IS DISTINCT FROM st.parent_claim_revision THEN
                RAISE EXCEPTION 'live_premise_ineligible';
            END IF;
            SELECT * INTO cc FROM terrain.current_claims
            WHERE engagement_id = a.e AND campaign_id = a.c AND claim_id = st.parent_claim_id;
            IF NOT FOUND THEN RAISE EXCEPTION 'live_premise_ineligible'; END IF;
        ELSE
            SELECT cc1.* INTO cc
            FROM terrain.observations ob1
            JOIN terrain.evaluations ev1
              ON (ev1.engagement_id, ev1.campaign_id, ev1.operation_id, ev1.observation_id)
               = (ob1.engagement_id, ob1.campaign_id, ob1.operation_id, ob1.observation_id)
            JOIN terrain.current_claims cc1
              ON (cc1.engagement_id, cc1.campaign_id, cc1.operation_id, cc1.claim_id)
               = (ev1.engagement_id, ev1.campaign_id, ev1.operation_id, ev1.claim_id)
            WHERE ob1.engagement_id = a.e AND ob1.campaign_id = a.c
              AND ob1.operation_id = st.terrain_operation_id
              AND ob1.correlation_id = st.refresh_result_id
              AND ob1.source_kind = 'issued_dns_acquisition';
            IF NOT FOUND OR cc.claim_revision <= st.parent_claim_revision THEN
                RAISE EXCEPTION 'live_premise_ineligible';
            END IF;
        END IF;
        IF cc.claim_id IS DISTINCT FROM bound_claim_id
           OR cc.claim_revision IS DISTINCT FROM bound_claim_rev THEN
            RAISE EXCEPTION 'live_premise_ineligible';
        END IF;
        SELECT * INTO ev FROM terrain.evaluations
        WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id = cc.operation_id;
        IF NOT FOUND OR cc.claim_status <> 'observed' OR cc.conflicted OR cc.corroborated
           OR cc.asset_ref IS DISTINCT FROM st.resolved_asset_ref
           OR cc.subject_name IS DISTINCT FROM st.resolved_hostname
           OR cc.vantage_ref IS DISTINCT FROM (p->>'vantage_ref')::uuid
           OR cc.resolver_ipv4 IS DISTINCT FROM (p->>'resolver_ipv4')
           OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0
           OR now_s < cc.perceived_at
           OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) ans WHERE ans->>'ipv4' = st.resolved_ipv4)
           OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = a.e AND ic.campaign_id = a.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
            RAISE EXCEPTION 'live_premise_ineligible';
        END IF;
        cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
        cap_until := least(cap_until, bound_claim_until_s);
        IF n >= cap_until * 1000 THEN
            RAISE EXCEPTION 'live_premise_ineligible';
        END IF;
        bound := least(bound, cap_until * 1000);
    END IF;
    IF bound IS NULL OR bound <= n THEN RAISE EXCEPTION 'live_effect_expired'; END IF;
    RETURN bound;
END $$;
REVOKE ALL ON FUNCTION execution.private_live_attempt_bound(execution.live_effect_sessions) FROM PUBLIC;

CREATE FUNCTION execution.private_live_https_result_shape(v jsonb, q jsonb, granted text, n_ms bigint) RETURNS boolean
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    dl bigint; max_dur bigint; earliest bigint; time_cap bigint; handle_end bigint; obs_ms bigint;
    outcome text; code bigint; redir text; tls_cat text; exp_lim text; sub_lim text;
BEGIN
    IF granted NOT IN ('connect', 'tls', 'head') OR jsonb_typeof(v) IS DISTINCT FROM 'object' OR octet_length(v::text) > 16384
       OR NOT v ?& ARRAY['schema','kind','attempt','session_generation','sequence','outcome','observed_at_ms',
           'network_bytes','header_bytes','status_code','tls_category','redirect','limitations']
       OR v - ARRAY['schema','kind','attempt','session_generation','sequence','outcome','observed_at_ms',
           'network_bytes','header_bytes','status_code','tls_category','redirect','limitations'] <> '{}'
       OR (v->>'schema', v->>'kind', v->>'attempt', v->'session_generation', v->>'sequence')
          IS DISTINCT FROM ('m1_https_ipc_v1', 'RESULT', q->>'attempt', q->'session_generation', '7')
       OR EXISTS (
           SELECT 1 FROM unnest(ARRAY['session_generation','sequence','observed_at_ms','network_bytes','header_bytes','status_code']) k
           WHERE jsonb_typeof(v->k) IS DISTINCT FROM 'number' OR v->>k !~ '^(0|[1-9][0-9]*)$'
       )
       OR jsonb_typeof(v->'outcome') IS DISTINCT FROM 'string' OR jsonb_typeof(v->'tls_category') IS DISTINCT FROM 'string'
       OR jsonb_typeof(v->'redirect') IS DISTINCT FROM 'string' OR jsonb_typeof(v->'limitations') IS DISTINCT FROM 'array' THEN
        RETURN false;
    END IF;
    dl := (q->>'deadline_at_ms')::bigint; max_dur := (q->>'max_duration_ms')::bigint; obs_ms := (v->>'observed_at_ms')::bigint;
    earliest := dl - max_dur; handle_end := dl + 1000; outcome := v->>'outcome';
    time_cap := CASE WHEN outcome = 'timed_out' THEN handle_end ELSE dl END;
    IF obs_ms < earliest OR obs_ms > time_cap OR n_ms > handle_end OR obs_ms > n_ms
       OR (v->>'network_bytes')::bigint > (q#>>'{limits,network_inbound_bytes}')::bigint
       OR (v->>'header_bytes')::bigint > (q#>>'{limits,header_bytes}')::bigint THEN
        RETURN false;
    END IF;
    IF jsonb_array_length(v->'limitations') NOT BETWEEN 2 AND 3
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(v->'limitations') l
           WHERE jsonb_typeof(l) IS DISTINCT FROM 'string' OR length(l#>>'{}') NOT BETWEEN 1 AND 96
              OR position('://' in (l#>>'{}')) > 0 OR position(chr(10) in (l#>>'{}')) > 0)
       OR (v->'limitations'->>0) IS DISTINCT FROM 'synthetic_https_lab_acquisition_not_public_live' THEN
        RETURN false;
    END IF;
    exp_lim := CASE outcome
        WHEN 'head_response' THEN 'head_response_does_not_prove_access_or_objective_completion'
        WHEN 'connection_failed' THEN 'single_endpoint_connection_failed_not_whole_host'
        WHEN 'tls_failed' THEN 'single_endpoint_tls_failed_not_whole_host'
        WHEN 'timed_out' THEN 'single_endpoint_timeout_not_whole_host'
        WHEN 'execution_refused' THEN 'single_endpoint_execution_refused_not_whole_host'
        ELSE NULL END;
    IF exp_lim IS NULL OR (v->'limitations'->>1) IS DISTINCT FROM exp_lim THEN RETURN false; END IF;
    code := (v->>'status_code')::bigint; redir := v->>'redirect'; tls_cat := v->>'tls_category';
    sub_lim := v->'limitations'->>2;
    CASE outcome
        WHEN 'head_response' THEN
            IF granted <> 'head' OR (v->>'header_bytes')::bigint <= 0 OR code NOT BETWEEN 200 AND 599
               OR tls_cat <> 'none' OR jsonb_array_length(v->'limitations') <> 2
               OR redir IS DISTINCT FROM (CASE WHEN code BETWEEN 300 AND 399 THEN 'refused_redirect' ELSE 'none' END) THEN
                RETURN false;
            END IF;
        WHEN 'connection_failed' THEN
            IF granted NOT IN ('connect', 'head') OR (v->>'header_bytes')::bigint <> 0 OR code <> 0
               OR tls_cat <> 'none' OR redir <> 'none' OR jsonb_array_length(v->'limitations') <> 2 THEN
                RETURN false;
            END IF;
        WHEN 'tls_failed' THEN
            IF granted <> 'tls' OR (v->>'header_bytes')::bigint <> 0 OR code <> 0 OR redir <> 'none'
               OR tls_cat NOT IN ('unknown_root', 'hostname_mismatch', 'expired_or_not_yet_valid', 'protocol', 'other_validation')
               OR jsonb_array_length(v->'limitations') <> 3 OR sub_lim IS DISTINCT FROM ('tls_' || tls_cat) THEN
                RETURN false;
            END IF;
        WHEN 'timed_out' THEN
            IF (v->>'header_bytes')::bigint <> 0 OR code <> 0 OR tls_cat <> 'none' OR redir <> 'none'
               OR jsonb_array_length(v->'limitations') <> 2 THEN
                RETURN false;
            END IF;
        WHEN 'execution_refused' THEN
            IF (v->>'header_bytes')::bigint <> 0 OR code <> 0 OR tls_cat <> 'none' OR redir <> 'none' THEN
                RETURN false;
            END IF;
            IF jsonb_array_length(v->'limitations') = 3 THEN
                IF NOT (
                    (granted = 'tls' AND sub_lim IN ('network_byte_limit_exceeded', 'unsupported_http_response'))
                    OR (granted = 'head' AND sub_lim IN ('http_header_limit_exceeded', 'network_byte_limit_exceeded', 'unsupported_http_response', 'malformed_http_response'))
                ) THEN
                    RETURN false;
                END IF;
            END IF;
        ELSE RETURN false;
    END CASE;
    RETURN true;
EXCEPTION WHEN invalid_text_representation OR numeric_value_out_of_range THEN RETURN false;
END $$;
REVOKE ALL ON FUNCTION execution.private_live_https_result_shape(jsonb, jsonb, text, bigint) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.live_attempt_record(att uuid) RETURNS jsonb
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT jsonb_build_object('run_id', s.run_id, 'request', s.request, 'result_ref', s.result_ref,
        'phase', x.phase, 'possibly_started', x.possibly_started,
        'attempt_deadline_at_ms', x.attempt_deadline_at_ms, 'result', b.projection, 'disposal_confirmed', x.disposal_confirmed)
    FROM execution.live_effect_sessions s
    JOIN execution.live_effect_control x ON x.attempt = s.attempt
    LEFT JOIN execution.live_dns_results b ON b.attempt = s.attempt
    WHERE s.attempt = att AND s.run_kind = 'm1_https_loop_v1' AND s.attempt_kind IN ('dns', 'dns_refresh')
      AND s.writer_oid = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
      AND execution.private_live_actor('dw_m1_broker')
$$;

CREATE OR REPLACE FUNCTION execution.live_attempt_head_record(att uuid) RETURNS jsonb
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
    SELECT jsonb_build_object('run_id', s.run_id, 'step_index', s.step_index, 'attempt_id', s.attempt,
        'request', s.request, 'result_ref', s.result_ref, 'phase', x.phase, 'possibly_started', x.possibly_started,
        'granted_phase', x.granted_phase, 'attempt_deadline_at_ms', x.attempt_deadline_at_ms,
        'result', b.safe_result, 'disposal_confirmed', x.disposal_confirmed)
    FROM execution.live_effect_sessions s
    JOIN execution.live_effect_control x ON x.attempt = s.attempt
    LEFT JOIN execution.live_https_results b ON b.attempt = s.attempt
    WHERE s.attempt = att AND s.run_kind = 'm1_https_loop_v1' AND s.attempt_kind = 'head'
      AND s.writer_oid = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
      AND execution.private_live_actor('dw_m1_broker')
$$;

CREATE OR REPLACE FUNCTION execution.live_https_loop_dns_reserve(r uuid, idx integer, kind text) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    a execution.autonomous_runs; st execution.https_loop_steps; prior execution.live_effect_sessions;
    q execution.live_qualifications; reg jsonb; f jsonb; p jsonb; lim jsonb;
    cc terrain.current_claims; ev terrain.evaluations;
    actor oid; n_ms bigint; now_s bigint; bound bigint; g bigint;
    attempt_id uuid; result_id uuid; request jsonb; cur_rev integer;
    init_used integer; ref_used integer; run_dns_used integer; camp_dns_used integer; camp_ref_used integer;
    run_ep_used integer; camp_ep_used integer; ep_num integer; ep_dl bigint; cap_until bigint;
BEGIN
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT execution.private_live_actor('dw_m1_broker') OR idx NOT BETWEEN 1 AND 3 OR kind NOT IN ('dns', 'dns_refresh') THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    SELECT * INTO a FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND OR a.writer_oid <> actor OR a.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
       OR a.request->>'run_profile' IS DISTINCT FROM 'autonomous-https-genai-lab-v1'
       OR a.request->>'execution_mode' IS DISTINCT FROM 'live' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(a.e::text || ':' || a.c::text, 0));
    SELECT * INTO prior FROM execution.live_effect_sessions s
    WHERE s.run_id = r AND s.step_index = idx AND s.attempt_kind = kind AND s.run_kind = 'm1_https_loop_v1';
    IF FOUND THEN
        IF prior.writer_oid IS DISTINCT FROM actor THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused'; END IF;
        RETURN execution.live_attempt_record(prior.attempt);
    END IF;
    reg := execution.run_current(r); f := reg->'fields'; p := f->'m1_permission'; lim := p->'campaign_limits';
    SELECT * INTO q FROM execution.live_qualifications z WHERE z.qualification_ref = a.qualification_ref;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    now_s := n_ms / 1000;
    IF q.qualification_ref IS NULL OR n_ms < q.valid_from_ms OR n_ms >= q.valid_until_ms
       OR q.run_profile IS DISTINCT FROM 'autonomous-https-genai-lab-v1' OR q.purpose IS DISTINCT FROM 'HTTPS_LOOP_LAB'
       OR q.release_sha256 IS DISTINCT FROM a.release_sha256 OR host(q.resolver_ipv4) IS DISTINCT FROM '192.0.2.53' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_qualification_unavailable';
    END IF;
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND OR NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = r AND y.phase = st.acquisition_phase AND y.claimed) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    IF kind = 'dns' AND (st.next_intent_kind IS DISTINCT FROM 'dns_udp' OR st.dns_attempt_id IS NOT NULL) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    IF kind = 'dns_refresh' AND (st.next_intent_kind IS DISTINCT FROM 'https-head-v1'
       OR st.dependency_refresh IS DISTINCT FROM 'dns_once_if_stale' OR st.refresh_attempt_id IS NOT NULL) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    IF st.protection_state IS DISTINCT FROM 'protected' OR st.next_decision_id IS NULL OR st.protected_capacity IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    IF EXISTS (
        SELECT 1 FROM execution.live_effect_sessions ls
        JOIN execution.live_effect_control lc ON lc.attempt = ls.attempt
        WHERE (ls.e, ls.c) = (a.e, a.c)
          AND (lc.phase <> 'closed' OR NOT lc.disposal_confirmed OR (lc.possibly_started AND NOT EXISTS (
              SELECT 1 FROM execution.live_effect_events z WHERE z.attempt = ls.attempt AND z.phase = 'result_recorded'
          )))
    ) OR EXISTS (
        SELECT 1 FROM execution.inference_reservations h WHERE (h.engagement_id, h.campaign_id) = (a.e, a.c)
          AND h.record_revision = 1 AND NOT EXISTS (SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = (a.e, a.c, h.operation_id))
    ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_sender_not_closed';
    END IF;
    IF p IS NULL OR jsonb_typeof(p) <> 'object' OR p->>'policy_version' IS DISTINCT FROM '1'
       OR p->>'resolver_ipv4' IS DISTINCT FROM '192.0.2.53'
       OR now_s < (f->>'starts_at')::bigint OR now_s >= (f->>'ends_at')::bigint
       OR now_s < (p->>'starts_at')::bigint OR now_s >= (p->>'ends_at')::bigint
       OR NOT (f->'included_assets' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR (f->'excluded_assets' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR st.resolved_hostname IS DISTINCT FROM 'head.duskweave.test'
       OR NOT EXISTS (
           SELECT 1 FROM jsonb_array_elements(p->'contact_rules') cr
           WHERE (cr->>'asset_ref')::uuid = st.resolved_asset_ref
             AND (cr->'rule'->>'exact' = st.resolved_hostname OR cr->'rule'->>'label_suffix' = st.resolved_hostname
                  OR right(st.resolved_hostname, length(cr->'rule'->>'label_suffix') + 1) = '.' || (cr->'rule'->>'label_suffix'))
       )
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(p->'excluded_names') ex
           WHERE ex->>'exact' = st.resolved_hostname OR ex->>'label_suffix' = st.resolved_hostname
              OR right(st.resolved_hostname, length(ex->>'label_suffix') + 1) = '.' || (ex->>'label_suffix')
       ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_policy_denied';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM jsonb_array_elements(a.request->'seed_names') s0
        WHERE (s0->>'asset_ref')::uuid = st.resolved_asset_ref AND s0->>'name' = st.resolved_hostname
    ) OR EXISTS (
        SELECT 1 FROM execution.https_loop_steps p1
        WHERE p1.run_id = r AND p1.step_index <> idx
          AND (p1.dns_attempt_id IS NOT NULL OR p1.refresh_attempt_id IS NOT NULL OR p1.head_attempt_id IS NOT NULL)
          AND (p1.resolved_asset_ref IS DISTINCT FROM st.resolved_asset_ref OR p1.resolved_hostname IS DISTINCT FROM st.resolved_hostname)
    ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_scope_refused';
    END IF;
    SELECT count(*) INTO init_used FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'dns';
    SELECT count(*) INTO ref_used FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'dns_refresh';
    IF kind = 'dns' AND init_used >= 1 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_dns_exhausted'; END IF;
    SELECT count(*) INTO camp_ref_used FROM (
        SELECT fa.attempt FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (a.e, a.c) AND fa.attempt_kind = 'refresh'
        UNION ALL
        SELECT ls.attempt FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (a.e, a.c) AND ls.attempt_kind = 'dns_refresh'
    ) r0;
    IF kind = 'dns_refresh' AND (ref_used >= 1 OR camp_ref_used >= (lim->>'dns_followups')::int) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_refresh_exhausted';
    END IF;
    SELECT count(*) INTO run_dns_used FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind IN ('dns', 'dns_refresh');
    SELECT count(*) INTO camp_dns_used FROM (
        SELECT fa.attempt FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (a.e, a.c)
        UNION ALL
        SELECT ls.attempt FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (a.e, a.c) AND ls.attempt_kind IN ('dns', 'dns_refresh')
    ) d0;
    SELECT count(*) INTO run_ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    SELECT count(*) INTO camp_ep_used FROM execution.https_loop_steps WHERE (e, c) = (a.e, a.c) AND episode_index IS NOT NULL;
    camp_ep_used := camp_ep_used
        + (SELECT count(*) FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (a.e, a.c) AND fa.run_kind = 'legacy')
        + (SELECT count(*) FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (a.e, a.c) AND ls.run_kind = 'legacy');
    IF run_dns_used >= 3 OR camp_dns_used >= (lim->>'dns_questions')::int
       OR (st.episode_index IS NULL AND (run_ep_used >= 3 OR camp_ep_used >= (lim->>'episodes')::int)) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_budget_exhausted';
    END IF;
    IF kind = 'dns_refresh' THEN
        SELECT * INTO cc FROM terrain.current_claims WHERE engagement_id = a.e AND campaign_id = a.c AND claim_id = st.parent_claim_id;
        IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible'; END IF;
        SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id = cc.operation_id;
        IF NOT FOUND OR cc.claim_status <> 'observed' OR cc.conflicted OR cc.corroborated
           OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision
           OR cc.asset_ref IS DISTINCT FROM st.resolved_asset_ref OR cc.subject_name IS DISTINCT FROM st.resolved_hostname
           OR cc.vantage_ref IS DISTINCT FROM (p->>'vantage_ref')::uuid OR cc.resolver_ipv4 IS DISTINCT FROM (p->>'resolver_ipv4')
           OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0 OR now_s < cc.perceived_at
           OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) ans WHERE ans->>'ipv4' = st.resolved_ipv4)
           OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = a.e AND ic.campaign_id = a.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible';
        END IF;
        cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
        IF now_s < cap_until THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible'; END IF;
    END IF;
    bound := least(a.deadline_at_ms, q.valid_until_ms, (f->>'ends_at')::bigint * 1000, (p->>'ends_at')::bigint * 1000, (f#>>'{m1_inference_permission,ends_at}')::bigint * 1000);
    IF bound <= n_ms THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_expired'; END IF;
    IF st.episode_deadline_ms IS NULL THEN
        IF (bound - n_ms) < 314000 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_time_insufficient'; END IF;
        ep_num := run_ep_used + 1;
        ep_dl := least(n_ms + 60000, bound - 254000);
        UPDATE execution.https_loop_steps
        SET episode_index = ep_num, episode_deadline_ms = ep_dl
        WHERE run_id = r AND step_index = idx;
    ELSE
        ep_num := st.episode_index; ep_dl := st.episode_deadline_ms;
        IF n_ms >= ep_dl OR (bound - n_ms) < 254000 OR (bound - ep_dl) < 254000 THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_time_insufficient';
        END IF;
    END IF;
    UPDATE execution.session_fences SET generation = generation + 1, epoch = epoch + 1
    WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id IS NULL AND phase = 'idle'
    RETURNING generation INTO g;
    IF g IS NULL THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_session_busy'; END IF;
    SELECT coalesce(max(cr.claim_revision), 0) INTO cur_rev FROM terrain.claim_revisions cr
    WHERE cr.engagement_id = a.e AND cr.campaign_id = a.c AND cr.subject_name = st.resolved_hostname
      AND cr.claim_kind = 'dns_hostname_ipv4_resolution' AND cr.vantage_ref = (p->>'vantage_ref')::uuid
      AND cr.resolver_ipv4 = p->>'resolver_ipv4';
    attempt_id := gen_random_uuid(); result_id := gen_random_uuid();
    request := jsonb_build_object(
        'schema', 'm1_dns_ipc_v1', 'kind', 'REQUEST', 'attempt', attempt_id,
        'session_generation', g, 'sequence', 0, 'engagement_id', a.e, 'campaign_id', a.c, 'decision_id', st.decision_id,
        'profile', 'dns-routing-v2', 'execution_mode', 'live', 'run_profile', 'autonomous-https-genai-lab-v1',
        'release_sha256', a.release_sha256, 'qualification_ref', a.qualification_ref,
        'name', st.resolved_hostname, 'resolver_ipv4', p->'resolver_ipv4', 'transport', 'udp',
        'query_id', mod(get_byte(uuid_send(attempt_id), 0) * 256 + get_byte(uuid_send(attempt_id), 1), 65536),
        'allowed_names', jsonb_build_array(st.resolved_hostname), 'deadline_at_ms', least(bound, ep_dl), 'max_duration_ms', 2000
    );
    INSERT INTO execution.live_effect_sessions
        (run_id, e, c, generation, writer_oid, mission_revision, decision, attempt, result_ref, request, qualification_ref, deadline_at_ms, run_kind, step_index, attempt_kind, expected_claim_revision)
    VALUES (r, a.e, a.c, g, a.writer_oid, 1, st.decision_id, attempt_id, result_id, request, a.qualification_ref, least(bound, ep_dl), 'm1_https_loop_v1', idx, kind, cur_rev);
    INSERT INTO execution.live_effect_control
        (run_id, phase, possibly_started, attempt_deadline_at_ms, disposal_confirmed, attempt, granted_phase)
    VALUES (r, 'reserved', false, least(bound, ep_dl), false, attempt_id, NULL);
    INSERT INTO execution.live_effect_events
        (run_id, generation, phase, occurred_at_ms, attempt)
    VALUES (r, g, 'reserved', n_ms, attempt_id);
    IF kind = 'dns' THEN
        UPDATE execution.https_loop_steps
        SET dns_attempt_id = attempt_id, terrain_operation_id = coalesce(terrain_operation_id, gen_random_uuid()),
            status = 'acquisition_running', updated_at_ms = n_ms
        WHERE run_id = r AND step_index = idx;
    ELSE
        UPDATE execution.https_loop_steps
        SET refresh_attempt_id = attempt_id, terrain_operation_id = gen_random_uuid(),
            status = 'acquisition_running', updated_at_ms = n_ms
        WHERE run_id = r AND step_index = idx;
    END IF;
    RETURN execution.live_attempt_record(attempt_id);
END $$;

CREATE OR REPLACE FUNCTION execution.live_https_loop_head_reserve(r uuid, idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    a execution.autonomous_runs; st execution.https_loop_steps; prior execution.live_effect_sessions;
    q execution.live_qualifications; reg jsonb; f jsonb; p jsonb; lim jsonb;
    cc terrain.current_claims; ev terrain.evaluations;
    actor oid; n_ms bigint; now_s bigint; bound bigint; g bigint;
    attempt_id uuid; result_id uuid; request jsonb;
    run_head_used integer; camp_head_used integer; run_ep_used integer; camp_ep_used integer;
    ep_num integer; ep_dl bigint; cap_until bigint; avail bigint; dl_ms bigint;
BEGIN
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT execution.private_live_actor('dw_m1_broker') OR idx NOT BETWEEN 1 AND 3 THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    SELECT * INTO a FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND OR a.writer_oid <> actor OR a.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
       OR a.request->>'run_profile' IS DISTINCT FROM 'autonomous-https-genai-lab-v1'
       OR a.request->>'execution_mode' IS DISTINCT FROM 'live' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(a.e::text || ':' || a.c::text, 0));
    SELECT * INTO prior FROM execution.live_effect_sessions s
    WHERE s.run_id = r AND s.step_index = idx AND s.attempt_kind = 'head' AND s.run_kind = 'm1_https_loop_v1';
    IF FOUND THEN
        IF prior.writer_oid IS DISTINCT FROM actor THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused'; END IF;
        RETURN execution.live_attempt_head_record(prior.attempt);
    END IF;
    reg := execution.run_current(r); f := reg->'fields'; p := f->'m1_permission'; lim := p->'campaign_limits';
    SELECT * INTO q FROM execution.live_qualifications z WHERE z.qualification_ref = a.qualification_ref;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    now_s := n_ms / 1000;
    IF q.qualification_ref IS NULL OR n_ms < q.valid_from_ms OR n_ms >= q.valid_until_ms
       OR q.run_profile IS DISTINCT FROM 'autonomous-https-genai-lab-v1' OR q.purpose IS DISTINCT FROM 'HTTPS_LOOP_LAB'
       OR q.release_sha256 IS DISTINCT FROM a.release_sha256 OR host(q.resolver_ipv4) IS DISTINCT FROM '192.0.2.53' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_qualification_unavailable';
    END IF;
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND OR st.next_intent_kind IS DISTINCT FROM 'https-head-v1' OR st.head_attempt_id IS NOT NULL
       OR NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = r AND y.phase = st.acquisition_phase AND y.claimed)
       OR st.protection_state IS DISTINCT FROM 'protected' OR st.next_decision_id IS NULL OR st.protected_capacity IS NULL THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_refused';
    END IF;
    IF EXISTS (
        SELECT 1 FROM execution.live_effect_sessions ls
        JOIN execution.live_effect_control lc ON lc.attempt = ls.attempt
        WHERE (ls.e, ls.c) = (a.e, a.c)
          AND (lc.phase <> 'closed' OR NOT lc.disposal_confirmed OR (lc.possibly_started AND NOT EXISTS (
              SELECT 1 FROM execution.live_effect_events z WHERE z.attempt = ls.attempt AND z.phase = 'result_recorded'
          )))
    ) OR EXISTS (
        SELECT 1 FROM execution.inference_reservations h WHERE (h.engagement_id, h.campaign_id) = (a.e, a.c)
          AND h.record_revision = 1 AND NOT EXISTS (SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = (a.e, a.c, h.operation_id))
    ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_sender_not_closed';
    END IF;
    IF p IS NULL OR jsonb_typeof(p) <> 'object' OR p->>'policy_version' IS DISTINCT FROM '1'
       OR p->>'resolver_ipv4' IS DISTINCT FROM '192.0.2.53'
       OR now_s < (f->>'starts_at')::bigint OR now_s >= (f->>'ends_at')::bigint
       OR now_s < (p->>'starts_at')::bigint OR now_s >= (p->>'ends_at')::bigint
       OR NOT (f->'included_assets' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR (f->'excluded_assets' @> jsonb_build_array(to_jsonb(st.resolved_asset_ref)))
       OR st.resolved_path IS DISTINCT FROM (p->>'approved_path') OR st.resolved_path IS DISTINCT FROM '/'
       OR st.resolved_hostname IS DISTINCT FROM 'head.duskweave.test' OR st.resolved_ipv4 IS DISTINCT FROM '198.51.100.10'
       OR NOT EXISTS (
           SELECT 1 FROM jsonb_array_elements(p->'contact_rules') cr
           WHERE (cr->>'asset_ref')::uuid = st.resolved_asset_ref
             AND (cr->'rule'->>'exact' = st.resolved_hostname OR cr->'rule'->>'label_suffix' = st.resolved_hostname
                  OR right(st.resolved_hostname, length(cr->'rule'->>'label_suffix') + 1) = '.' || (cr->'rule'->>'label_suffix'))
       )
       OR EXISTS (
           SELECT 1 FROM jsonb_array_elements(p->'excluded_names') ex
           WHERE ex->>'exact' = st.resolved_hostname OR ex->>'label_suffix' = st.resolved_hostname
              OR right(st.resolved_hostname, length(ex->>'label_suffix') + 1) = '.' || (ex->>'label_suffix')
       ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_policy_denied';
    END IF;
    IF EXISTS (
        SELECT 1 FROM execution.https_loop_steps p1
        WHERE p1.run_id = r AND p1.step_index <> idx
          AND (p1.dns_attempt_id IS NOT NULL OR p1.refresh_attempt_id IS NOT NULL OR p1.head_attempt_id IS NOT NULL)
          AND (p1.resolved_asset_ref IS DISTINCT FROM st.resolved_asset_ref OR p1.resolved_hostname IS DISTINCT FROM st.resolved_hostname)
    ) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_scope_refused';
    END IF;
    SELECT count(*) INTO run_head_used FROM execution.live_effect_sessions WHERE run_id = r AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'head';
    SELECT count(*) INTO camp_head_used FROM (
        SELECT ha.attempt FROM execution.fixture_https_attempts ha JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (a.e, a.c)
        UNION ALL
        SELECT ls.attempt FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (a.e, a.c) AND ls.attempt_kind = 'head'
    ) ch;
    IF run_head_used >= 2 OR camp_head_used >= least((lim->>'head_requests')::int, (lim->>'tcp_connections')::int) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_head_exhausted';
    END IF;
    SELECT count(*) INTO run_ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    SELECT count(*) INTO camp_ep_used FROM execution.https_loop_steps WHERE (e, c) = (a.e, a.c) AND episode_index IS NOT NULL;
    camp_ep_used := camp_ep_used
        + (SELECT count(*) FROM execution.fixture_dns_attempts fa JOIN execution.autonomous_runs ar USING (run_id) WHERE (ar.e, ar.c) = (a.e, a.c) AND fa.run_kind = 'legacy')
        + (SELECT count(*) FROM execution.live_effect_sessions ls WHERE (ls.e, ls.c) = (a.e, a.c) AND ls.run_kind = 'legacy');
    IF st.episode_index IS NULL AND (run_ep_used >= 3 OR camp_ep_used >= (lim->>'episodes')::int) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_budget_exhausted';
    END IF;
    bound := least(a.deadline_at_ms, q.valid_until_ms, (f->>'ends_at')::bigint * 1000, (p->>'ends_at')::bigint * 1000, (f#>>'{m1_inference_permission,ends_at}')::bigint * 1000);
    IF bound <= n_ms THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_expired'; END IF;
    IF st.episode_deadline_ms IS NULL THEN
        IF (bound - n_ms) < 314000 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_time_insufficient'; END IF;
        ep_num := run_ep_used + 1;
        ep_dl := least(n_ms + 60000, bound - 254000);
    ELSE
        ep_num := st.episode_index;
        ep_dl := st.episode_deadline_ms;
        IF n_ms >= ep_dl OR (bound - n_ms) < 254000 OR (bound - ep_dl) < 254000 THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_time_insufficient';
        END IF;
    END IF;
    IF st.refresh_result_id IS NULL THEN
        SELECT * INTO cc FROM terrain.current_claims WHERE engagement_id = a.e AND campaign_id = a.c AND claim_id = st.parent_claim_id;
        IF NOT FOUND OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible';
        END IF;
        SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id = cc.operation_id;
        IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible'; END IF;
    ELSE
        SELECT cc1.* INTO cc
        FROM terrain.observations ob1
        JOIN terrain.evaluations ev1 ON (ev1.engagement_id, ev1.campaign_id, ev1.operation_id, ev1.observation_id) = (ob1.engagement_id, ob1.campaign_id, ob1.operation_id, ob1.observation_id)
        JOIN terrain.current_claims cc1 ON (cc1.engagement_id, cc1.campaign_id, cc1.operation_id, cc1.claim_id) = (ev1.engagement_id, ev1.campaign_id, ev1.operation_id, ev1.claim_id)
        WHERE ob1.engagement_id = a.e AND ob1.campaign_id = a.c AND ob1.operation_id = st.terrain_operation_id
          AND ob1.correlation_id = st.refresh_result_id AND ob1.source_kind = 'issued_dns_acquisition';
        IF NOT FOUND OR cc.claim_revision <= st.parent_claim_revision THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible';
        END IF;
        SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id = cc.operation_id;
        IF NOT FOUND THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible'; END IF;
    END IF;
    IF cc.claim_status <> 'observed' OR cc.conflicted OR cc.corroborated
       OR cc.asset_ref IS DISTINCT FROM st.resolved_asset_ref OR cc.subject_name IS DISTINCT FROM st.resolved_hostname
       OR cc.vantage_ref IS DISTINCT FROM (p->>'vantage_ref')::uuid OR cc.resolver_ipv4 IS DISTINCT FROM (p->>'resolver_ipv4')
       OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0 OR now_s < cc.perceived_at
       OR NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) ans WHERE ans->>'ipv4' = st.resolved_ipv4)
       OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = a.e AND ic.campaign_id = a.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible';
    END IF;
    cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
    IF n_ms >= cap_until * 1000 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_premise_ineligible'; END IF;
    dl_ms := least(n_ms + 5000, ep_dl, bound, cap_until * 1000);
    avail := dl_ms - n_ms;
    IF avail <= 0 THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_time_insufficient'; END IF;
    UPDATE execution.session_fences SET generation = generation + 1, epoch = epoch + 1
    WHERE engagement_id = a.e AND campaign_id = a.c AND operation_id IS NULL AND phase = 'idle'
    RETURNING generation INTO g;
    IF g IS NULL THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'live_effect_session_busy'; END IF;
    attempt_id := gen_random_uuid(); result_id := gen_random_uuid();
    request := jsonb_build_object(
        'schema', 'm1_https_ipc_v1', 'kind', 'REQUEST', 'attempt', attempt_id, 'session_generation', g, 'sequence', 0,
        'engagement_id', a.e, 'campaign_id', a.c, 'decision_id', st.decision_id, 'run_id', r,
        'step_index', idx, 'attempt_kind', 'head', 'result_id', result_id, 'profile', 'https-head-v1',
        'execution_mode', 'live', 'run_profile', 'autonomous-https-genai-lab-v1', 'release_sha256', a.release_sha256,
        'qualification_ref', a.qualification_ref, 'asset_ref', st.resolved_asset_ref, 'endpoint_ref', st.resolved_endpoint_ref,
        'hostname', st.resolved_hostname, 'ipv4', st.resolved_ipv4, 'port', 443, 'approved_path', '/', 'method', 'HEAD',
        'parent_claim_id', cc.claim_id, 'parent_claim_revision', cc.claim_revision,
        'resolver_ipv4', p->'resolver_ipv4', 'vantage_ref', p->'vantage_ref',
        'permission_id', f#>'{m1_inference_permission,permission_id}', 'expected_mission_revision', 1,
        'authority_evaluated_at_ms', n_ms, 'claim_valid_until_s', cap_until,
        'episode_deadline_at_ms', ep_dl, 'deadline_at_ms', dl_ms, 'max_duration_ms', avail,
        'limits', jsonb_build_object(
            'network_inbound_bytes', 65536, 'header_bytes', 8192, 'frame_payload_bytes', 16384,
            'control_payload_bytes', 1024, 'max_worker_frames', 16, 'min_tls_version', '1.2',
            'http_version', 'HTTP/1.1', 'max_connects', 1, 'max_requests', 1
        )
    );
    INSERT INTO execution.live_effect_sessions
        (run_id, e, c, generation, writer_oid, mission_revision, decision, attempt, result_ref, request, qualification_ref, deadline_at_ms, run_kind, step_index, attempt_kind, expected_claim_revision)
    VALUES (r, a.e, a.c, g, a.writer_oid, 1, st.decision_id, attempt_id, result_id, request, a.qualification_ref, ep_dl, 'm1_https_loop_v1', idx, 'head', cc.claim_revision);
    INSERT INTO execution.live_effect_control
        (run_id, phase, possibly_started, attempt_deadline_at_ms, disposal_confirmed, attempt, granted_phase)
    VALUES (r, 'reserved', false, dl_ms, false, attempt_id, NULL);
    INSERT INTO execution.live_effect_events
        (run_id, generation, phase, occurred_at_ms, attempt)
    VALUES (r, g, 'reserved', n_ms, attempt_id);
    UPDATE execution.https_loop_steps
    SET episode_index = ep_num, episode_deadline_ms = ep_dl,
        head_attempt_id = attempt_id, head_terrain_operation_id = coalesce(head_terrain_operation_id, gen_random_uuid()),
        status = 'acquisition_running', updated_at_ms = n_ms
    WHERE run_id = r AND step_index = idx;
    RETURN execution.live_attempt_head_record(attempt_id);
END $$;

CREATE OR REPLACE FUNCTION execution.live_attempt_current(att uuid, g bigint) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; bound bigint; n_ms bigint; eff bigint;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE attempt = att AND run_kind = 'm1_https_loop_v1';
    IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
        RAISE EXCEPTION 'live_effect_identity_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = att FOR UPDATE;
    bound := execution.private_live_attempt_bound(s);
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF x.phase NOT IN ('reserved', 'may_start') OR x.disposal_confirmed
       OR (s.attempt_kind = 'head' AND x.attempt_deadline_at_ms IS DISTINCT FROM (s.request->>'deadline_at_ms')::bigint) THEN
        RAISE EXCEPTION 'live_effect_stopped';
    END IF;
    IF s.attempt_kind = 'head' THEN
        eff := least(bound, x.attempt_deadline_at_ms);
        IF n_ms >= eff THEN RAISE EXCEPTION 'live_effect_stopped'; END IF;
        RETURN eff;
    END IF;
    IF x.phase = 'may_start' AND n_ms >= x.attempt_deadline_at_ms THEN
        RAISE EXCEPTION 'live_effect_stopped';
    END IF;
    RETURN CASE WHEN x.phase = 'may_start' THEN least(bound, x.attempt_deadline_at_ms) ELSE bound END;
END $$;

CREATE OR REPLACE FUNCTION execution.live_attempt_may_start(att uuid, g bigint) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; bound bigint; n_ms bigint;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE attempt = att AND run_kind = 'm1_https_loop_v1';
    IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
        RAISE EXCEPTION 'live_effect_identity_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = att FOR UPDATE;
    bound := execution.live_attempt_current(att, g);
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF s.attempt_kind = 'head' THEN
        UPDATE execution.live_effect_control
        SET phase = 'may_start', possibly_started = true, granted_phase = 'connect'
        WHERE attempt = att AND phase = 'reserved' AND NOT possibly_started AND granted_phase IS NULL;
    ELSE
        UPDATE execution.live_effect_control
        SET phase = 'may_start', possibly_started = true, attempt_deadline_at_ms = least(n_ms + 2000, bound)
        WHERE attempt = att AND phase = 'reserved' AND NOT possibly_started;
    END IF;
    IF NOT FOUND THEN RAISE EXCEPTION 'live_effect_already_charged'; END IF;
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt)
    VALUES (s.run_id, g, 'may_start', n_ms, att);
    RETURN CASE WHEN s.attempt_kind = 'head' THEN bound
        ELSE (SELECT attempt_deadline_at_ms FROM execution.live_effect_control WHERE attempt = att) END;
END $$;

CREATE OR REPLACE FUNCTION execution.live_attempt_head_phase(att uuid, g bigint, wanted text) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; dl bigint; n_ms bigint;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE attempt = att AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'head';
    IF NOT FOUND OR wanted NOT IN ('tls', 'head') OR NOT execution.private_live_actor('dw_m1_broker')
       OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
        RAISE EXCEPTION 'live_effect_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = att FOR UPDATE;
    dl := execution.live_attempt_current(att, g);
    IF x.phase <> 'may_start' OR NOT x.possibly_started OR x.disposal_confirmed
       OR NOT ((x.granted_phase = 'connect' AND wanted = 'tls') OR (x.granted_phase = 'tls' AND wanted = 'head')) THEN
        RAISE EXCEPTION 'live_effect_refused';
    END IF;
    UPDATE execution.live_effect_control SET granted_phase = wanted WHERE attempt = att;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt)
    VALUES (s.run_id, g, 'phase_' || wanted, n_ms, att) ON CONFLICT DO NOTHING;
    RETURN dl;
END $$;

CREATE OR REPLACE FUNCTION execution.live_attempt_dns_result(att uuid, g bigint, v jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; b execution.live_dns_results; n bigint; started bigint;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE attempt = att AND run_kind = 'm1_https_loop_v1' AND attempt_kind IN ('dns', 'dns_refresh');
    IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
        RAISE EXCEPTION 'live_result_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO b FROM execution.live_dns_results WHERE attempt = att;
    IF FOUND THEN
        IF b.projection IS DISTINCT FROM v THEN RAISE EXCEPTION 'live_result_conflict'; END IF;
        RETURN;
    END IF;
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = att FOR UPDATE;
    SELECT occurred_at_ms INTO started FROM execution.live_effect_events WHERE attempt = att AND phase = 'may_start';
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF x.phase NOT IN ('may_start', 'stopped') OR NOT x.possibly_started OR x.disposal_confirmed OR started IS NULL
       OR NOT execution.private_live_result_shape(v, s.request, least(n, x.attempt_deadline_at_ms), started) THEN
        RAISE EXCEPTION 'live_result_refused';
    END IF;
    INSERT INTO execution.live_dns_results
        (result, run_id, attempt, session_generation, projection, received_at_ms, publication_obligation, run_kind, step_index, attempt_kind)
    VALUES (s.result_ref, s.run_id, att, g, v, n, 'terrain.issued_dns.v2', 'm1_https_loop_v1', s.step_index, s.attempt_kind);
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt) VALUES (s.run_id, g, 'result_recorded', n, att);
    UPDATE execution.live_effect_control SET phase = 'result_recorded' WHERE attempt = att AND phase = 'may_start';
    IF s.attempt_kind = 'dns' THEN
        UPDATE execution.https_loop_steps SET dns_result_id = s.result_ref, updated_at_ms = n WHERE run_id = s.run_id AND step_index = s.step_index;
    ELSE
        UPDATE execution.https_loop_steps SET refresh_result_id = s.result_ref, refresh_outcome = 'completed', updated_at_ms = n WHERE run_id = s.run_id AND step_index = s.step_index;
    END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.live_attempt_head_result(att uuid, g bigint, safe_v jsonb, head_v jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    s execution.live_effect_sessions; x execution.live_effect_control; b execution.live_https_results;
    n bigint; started bigint; exp_head jsonb; exp_status jsonb;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE attempt = att AND run_kind = 'm1_https_loop_v1' AND attempt_kind = 'head';
    IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
        RAISE EXCEPTION 'live_result_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO b FROM execution.live_https_results WHERE attempt = att;
    IF FOUND THEN
        IF b.safe_result IS DISTINCT FROM safe_v OR b.result_payload IS DISTINCT FROM head_v THEN
            RAISE EXCEPTION 'live_result_conflict';
        END IF;
        RETURN;
    END IF;
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = att FOR UPDATE;
    SELECT occurred_at_ms INTO started FROM execution.live_effect_events WHERE attempt = att AND phase = 'may_start';
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF x.phase NOT IN ('may_start', 'stopped') OR NOT x.possibly_started OR x.disposal_confirmed OR started IS NULL
       OR NOT execution.private_live_https_result_shape(safe_v, s.request, x.granted_phase, n) THEN
        RAISE EXCEPTION 'live_result_refused';
    END IF;
    exp_status := CASE WHEN safe_v->>'outcome' = 'head_response' THEN safe_v->'status_code' ELSE 'null'::jsonb END;
    exp_head := jsonb_build_object(
        'version', 'm1_https_head_result_v1',
        'engagement_id', s.request->'engagement_id',
        'campaign_id', s.request->'campaign_id',
        'run_id', s.request->'run_id',
        'decision_id', s.request->'decision_id',
        'attempt_id', s.request->'attempt',
        'result_id', s.request->'result_id',
        'asset_ref', s.request->'asset_ref',
        'endpoint_ref', s.request->'endpoint_ref',
        'hostname', s.request->'hostname',
        'ipv4', s.request->'ipv4',
        'port', s.request->'port',
        'approved_path', s.request->'approved_path',
        'method', s.request->'method',
        'parent_claim_id', s.request->'parent_claim_id',
        'parent_claim_revision', s.request->'parent_claim_revision',
        'perceived_at', floor((safe_v->>'observed_at_ms')::numeric / 1000)::bigint,
        'uncertainty_seconds', 1,
        'outcome', safe_v->'outcome',
        'status_code', exp_status,
        'redirect', safe_v->'redirect',
        'source_kind', 'issued_https_acquisition',
        'limitations', safe_v->'limitations'
    );
    IF head_v IS DISTINCT FROM exp_head THEN
        RAISE EXCEPTION 'live_result_refused';
    END IF;
    INSERT INTO execution.live_https_results
        (result, run_id, step_index, attempt, session_generation, safe_result, result_payload, received_at_ms, publication_obligation)
    VALUES (s.result_ref, s.run_id, s.step_index, att, g, safe_v, head_v, n, 'terrain.issued_https.v2');
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt) VALUES (s.run_id, g, 'result_recorded', n, att);
    UPDATE execution.live_effect_control SET phase = 'result_recorded' WHERE attempt = att AND phase = 'may_start';
    UPDATE execution.https_loop_steps SET head_result_id = s.result_ref, head_outcome = head_v->>'outcome', updated_at_ms = n
    WHERE run_id = s.run_id AND step_index = s.step_index;
END $$;

CREATE OR REPLACE FUNCTION execution.live_attempt_stop(att uuid, g bigint, disposed boolean) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE s execution.live_effect_sessions; x execution.live_effect_control; n bigint; target text;
BEGIN
    SELECT * INTO s FROM execution.live_effect_sessions WHERE attempt = att AND run_kind = 'm1_https_loop_v1';
    IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
       OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) OR disposed IS NULL THEN
        RAISE EXCEPTION 'live_stop_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text || ':' || s.c::text, 0));
    SELECT * INTO x FROM execution.live_effect_control WHERE attempt = att FOR UPDATE;
    IF x.phase = 'closed' THEN RETURN; END IF;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    target := CASE WHEN disposed THEN 'closed' ELSE 'stopped' END;
    UPDATE execution.live_effect_control SET phase = target, disposal_confirmed = disposed WHERE attempt = att;
    INSERT INTO execution.live_effect_events (run_id, generation, phase, occurred_at_ms, attempt) VALUES (s.run_id, g, target, n, att) ON CONFLICT DO NOTHING;
END $$;

REVOKE ALL ON FUNCTION
    execution.live_attempt_record(uuid),
    execution.live_attempt_head_record(uuid),
    execution.live_https_loop_dns_reserve(uuid, integer, text),
    execution.live_https_loop_head_reserve(uuid, integer),
    execution.live_attempt_current(uuid, bigint),
    execution.live_attempt_may_start(uuid, bigint),
    execution.live_attempt_head_phase(uuid, bigint, text),
    execution.live_attempt_dns_result(uuid, bigint, jsonb),
    execution.live_attempt_head_result(uuid, bigint, jsonb, jsonb),
    execution.live_attempt_stop(uuid, bigint, boolean)
FROM PUBLIC;

GRANT EXECUTE ON FUNCTION
    execution.live_attempt_record(uuid),
    execution.live_attempt_head_record(uuid),
    execution.live_https_loop_dns_reserve(uuid, integer, text),
    execution.live_https_loop_head_reserve(uuid, integer),
    execution.live_attempt_current(uuid, bigint),
    execution.live_attempt_may_start(uuid, bigint),
    execution.live_attempt_head_phase(uuid, bigint, text),
    execution.live_attempt_dns_result(uuid, bigint, jsonb),
    execution.live_attempt_head_result(uuid, bigint, jsonb, jsonb),
    execution.live_attempt_stop(uuid, bigint, boolean)
TO dw_m1_broker;

-- 8. Terrain issuance & acceptance for multi-step LAB DNS + live HTTPS HEAD.
CREATE OR REPLACE FUNCTION execution.private_live_issued_request(e uuid, c uuid, result_id uuid) RETURNS jsonb
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    b execution.live_dns_results; s execution.live_effect_sessions;
    x execution.autonomous_runs; q execution.live_qualifications; st execution.https_loop_steps;
    m mission.missions%ROWTYPE; reg mission.registration_outbox%ROWTYPE;
    f jsonb; p jsonb; req jsonb; proj jsonb; seed jsonb; received boolean;
    perceived bigint; completeness text; limits jsonb; hl boolean;
BEGIN
    SELECT * INTO b FROM execution.live_dns_results WHERE result = result_id;
    IF b.result IS NULL THEN RETURN NULL; END IF;
    SELECT * INTO s FROM execution.live_effect_sessions WHERE attempt = b.attempt;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = b.run_id;
    SELECT * INTO q FROM execution.live_qualifications WHERE qualification_ref = x.qualification_ref;
    SELECT * INTO m FROM mission.missions WHERE (engagement_id, campaign_id) = (x.e, x.c);
    SELECT * INTO reg FROM mission.registration_outbox WHERE (engagement_id, campaign_id, operation_id) = (x.e, x.c, m.operation_id);
    IF s.attempt IS NULL OR x.run_id IS NULL OR q.qualification_ref IS NULL
       OR m.operation_id IS NULL OR reg.operation_id IS NULL
       OR (x.e, x.c) IS DISTINCT FROM (e, c) THEN
        RETURN NULL;
    END IF;
    req := s.request; proj := b.projection;
    f := reg.contract->'fields'; p := f->'m1_permission';
    hl := x.request->>'version' = 'm1_https_loop_v1';
    IF hl THEN
        SELECT * INTO st FROM execution.https_loop_steps WHERE run_id = b.run_id AND step_index = s.step_index;
        IF NOT FOUND OR b.run_kind <> 'm1_https_loop_v1' OR s.run_kind <> 'm1_https_loop_v1'
           OR b.step_index <> s.step_index OR b.attempt_kind <> s.attempt_kind
           OR s.attempt_kind NOT IN ('dns', 'dns_refresh')
           OR (CASE WHEN s.attempt_kind = 'dns' THEN st.dns_attempt_id ELSE st.refresh_attempt_id END) IS DISTINCT FROM s.attempt
           OR (CASE WHEN s.attempt_kind = 'dns' THEN st.dns_result_id ELSE st.refresh_result_id END) IS DISTINCT FROM b.result
           OR (b.attempt, b.session_generation, b.result) IS DISTINCT FROM (s.attempt, s.generation, s.result_ref)
           OR s.qualification_ref IS DISTINCT FROM x.qualification_ref
           OR s.decision IS DISTINCT FROM st.decision_id OR (s.e, s.c) IS DISTINCT FROM (x.e, x.c)
           OR b.publication_obligation IS DISTINCT FROM 'terrain.issued_dns.v2'
           OR (q.run_profile, q.purpose) IS DISTINCT FROM ('autonomous-https-genai-lab-v1', 'HTTPS_LOOP_LAB')
           OR q.run_profile IS DISTINCT FROM x.request->>'run_profile' OR q.release_sha256 IS DISTINCT FROM x.release_sha256
           OR (req->>'schema', req->>'kind', req->>'profile', req->>'execution_mode', req->>'transport') IS DISTINCT FROM ('m1_dns_ipc_v1', 'REQUEST', 'dns-routing-v2', 'live', 'udp')
           OR (req->>'run_profile', req->>'release_sha256', req->>'qualification_ref') IS DISTINCT FROM (x.request->>'run_profile', x.release_sha256, x.qualification_ref::text)
           OR req->>'attempt' IS DISTINCT FROM s.attempt::text OR req->'session_generation' IS DISTINCT FROM to_jsonb(s.generation)
           OR req->>'sequence' IS DISTINCT FROM '0'
           OR (req->>'engagement_id', req->>'campaign_id', req->>'decision_id') IS DISTINCT FROM (x.e::text, x.c::text, st.decision_id::text)
           OR x.request->'live_binding' IS DISTINCT FROM f#>'{m1_inference_permission,live_binding}'
           OR (x.request#>>'{live_binding,qualification_ref}', x.request#>>'{live_binding,release_sha256}', x.request#>>'{live_binding,resolver_ipv4}')
              IS DISTINCT FROM (x.qualification_ref::text, x.release_sha256, host(q.resolver_ipv4))
           OR req->>'resolver_ipv4' IS DISTINCT FROM host(q.resolver_ipv4) OR req->>'resolver_ipv4' IS DISTINCT FROM p->>'resolver_ipv4'
           OR req->>'resolver_ipv4' IS DISTINCT FROM '192.0.2.53'
           OR req->>'name' IS DISTINCT FROM st.resolved_hostname OR req->>'name' IS DISTINCT FROM 'head.duskweave.test'
           OR req->'allowed_names' IS DISTINCT FROM jsonb_build_array(st.resolved_hostname) THEN
            RETURN NULL;
        END IF;
        SELECT elem INTO seed FROM jsonb_array_elements(x.request->'seed_names') elem
        WHERE (elem->>'asset_ref')::uuid = st.resolved_asset_ref AND elem->>'name' = st.resolved_hostname LIMIT 1;
        IF seed IS NULL THEN RETURN NULL; END IF;
    ELSE
        seed := x.request->'seed_names'->0;
        IF b.run_kind <> 'legacy' OR s.run_kind <> 'legacy'
           OR (b.attempt, b.session_generation, b.result) IS DISTINCT FROM (s.attempt, s.generation, s.result_ref)
           OR (b.attempt, b.run_id) IS DISTINCT FROM (x.attempt, x.run_id) OR b.result IS DISTINCT FROM x.result
           OR s.qualification_ref IS DISTINCT FROM x.qualification_ref
           OR s.decision IS DISTINCT FROM x.decision_one OR (s.e, s.c) IS DISTINCT FROM (x.e, x.c)
           OR b.publication_obligation IS DISTINCT FROM 'terrain.issued_dns.v2'
           OR q.run_profile IS DISTINCT FROM x.request->>'run_profile' OR q.release_sha256 IS DISTINCT FROM x.release_sha256
           OR (q.run_profile, q.purpose) NOT IN (('autonomous-dns-genai-live-v1', 'LIVE'), ('autonomous-dns-genai-lab-v1', 'LAB'))
           OR (req->>'schema', req->>'kind', req->>'profile', req->>'execution_mode', req->>'transport') IS DISTINCT FROM ('m1_dns_ipc_v1', 'REQUEST', 'dns-routing-v2', 'live', 'udp')
           OR (req->>'run_profile', req->>'release_sha256', req->>'qualification_ref') IS DISTINCT FROM (x.request->>'run_profile', x.release_sha256, x.qualification_ref::text)
           OR req->>'attempt' IS DISTINCT FROM x.attempt::text OR req->'session_generation' IS DISTINCT FROM to_jsonb(s.generation)
           OR req->>'sequence' IS DISTINCT FROM '0'
           OR (req->>'engagement_id', req->>'campaign_id', req->>'decision_id') IS DISTINCT FROM (x.e::text, x.c::text, x.decision_one::text)
           OR x.request->'live_binding' IS DISTINCT FROM f#>'{m1_inference_permission,live_binding}'
           OR (x.request#>>'{live_binding,qualification_ref}', x.request#>>'{live_binding,release_sha256}', x.request#>>'{live_binding,resolver_ipv4}')
              IS DISTINCT FROM (x.qualification_ref::text, x.release_sha256, host(q.resolver_ipv4))
           OR req->>'resolver_ipv4' IS DISTINCT FROM host(q.resolver_ipv4) OR req->>'resolver_ipv4' IS DISTINCT FROM p->>'resolver_ipv4'
           OR req->>'name' IS DISTINCT FROM seed->>'name' OR req->'allowed_names' IS DISTINCT FROM jsonb_build_array(seed->'name')
           OR seed->>'asset_ref' IS NULL THEN
            RETURN NULL;
        END IF;
    END IF;
    received := coalesce(proj->>'status' IN ('completed', 'limited'), false);
    IF received THEN
        perceived := floor((proj->>'observed_at_ms')::numeric / 1000)::bigint;
    ELSE
        perceived := floor(b.received_at_ms / 1000)::bigint;
    END IF;
    completeness := CASE
        WHEN proj->>'status' = 'completed' AND proj->'complete' = 'true'::jsonb THEN 'complete'
        WHEN proj->>'status' = 'limited' THEN CASE WHEN proj->'truncated' = 'true'::jsonb THEN 'truncated' ELSE 'incomplete' END
        WHEN proj->>'status' = 'timeout' THEN 'timeout' ELSE 'incomplete' END;
    limits := jsonb_build_array('single_vantage_only', 'resolver_recursion_variance');
    IF jsonb_array_length(proj->'answers') > 0 THEN limits := limits || '"cache_remaining_ttl"'::jsonb; END IF;
    IF coalesce((proj->>'withheld_aliases')::bigint, 0) > 0 OR jsonb_array_length(proj->'aliases') > 0 THEN
        limits := limits || '"partial_alias_chain"'::jsonb;
    END IF;
    RETURN jsonb_build_object(
        'version', 'm1_terrain_dns_issued_observation_v2',
        'engagement_id', x.e, 'campaign_id', x.c, 'operator_ref', f->'operator_ref',
        'expected_mission_revision', 1, 'goal_ref', f->'goal_ref', 'exercise_mode', f->'exercise_mode',
        'correlation_id', b.result, 'source_kind', 'issued_dns_acquisition', 'vantage_ref', p->'vantage_ref',
        'resolver_ipv4', req->'resolver_ipv4', 'subject_name', req->'name', 'asset_ref', seed->'asset_ref',
        'expected_claim_revision', s.expected_claim_revision, 'perceived_at', perceived, 'uncertainty_seconds', 1,
        'transport', 'udp', 'qtype', 'A', 'qclass', 'IN',
        'rcode', CASE (proj->>'rcode')::bigint WHEN 0 THEN 'noerror' WHEN 1 THEN 'formerr'
            WHEN 2 THEN 'servfail' WHEN 3 THEN 'nxdomain' WHEN 4 THEN 'notimp' WHEN 5 THEN 'refused' END,
        'truncated', proj->'truncated', 'completeness', completeness, 'limitations', limits, 'answers', proj->'answers'
    );
END $$;
REVOKE ALL ON FUNCTION execution.private_live_issued_request(uuid, uuid, uuid) FROM PUBLIC;

CREATE OR REPLACE FUNCTION terrain.issued_dns_request(q jsonb) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE raw jsonb; e uuid; c uuid; result_id uuid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_terrain', 'MEMBER') THEN RAISE EXCEPTION 'unqualified_terrain_writer'; END IF;
    IF jsonb_typeof(q) IS DISTINCT FROM 'object' OR q - ARRAY['version','engagement_id','campaign_id','result_ref','run_profile','execution_mode'] <> '{}'
       OR q->>'version' NOT IN ('m1_terrain_dns_issued_observation_v1', 'm1_terrain_dns_issued_observation_v2')
       OR q->>'run_profile' IS NULL THEN
        RAISE EXCEPTION 'issued_reference_refused';
    END IF;
    e := (q->>'engagement_id')::uuid; c := (q->>'campaign_id')::uuid; result_id := (q->>'result_ref')::uuid;
    IF q->>'version' = 'm1_terrain_dns_issued_observation_v2' THEN
        IF q->>'run_profile' NOT IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1', 'autonomous-https-genai-lab-v1')
           OR q->>'execution_mode' IS DISTINCT FROM 'live' THEN
            RAISE EXCEPTION 'issued_reference_refused';
        END IF;
        IF q->>'run_profile' = 'autonomous-https-genai-lab-v1' THEN
            SELECT jsonb_build_object('request', execution.private_live_issued_request(e, c, result_id), 'operation', st.terrain_operation_id) INTO raw
            FROM execution.autonomous_runs x
            JOIN execution.live_dns_results b ON b.run_id = x.run_id
            JOIN execution.https_loop_steps st ON st.run_id = x.run_id AND st.step_index = b.step_index
            WHERE b.result = result_id AND (x.e, x.c) = (e, c)
              AND b.run_kind = 'm1_https_loop_v1'
              AND ((b.attempt_kind = 'dns' AND b.attempt = st.dns_attempt_id AND b.result = st.dns_result_id)
                   OR (b.attempt_kind = 'dns_refresh' AND b.attempt = st.refresh_attempt_id AND b.result = st.refresh_result_id))
              AND x.request->>'version' = 'm1_https_loop_v1'
              AND x.request->>'run_profile' = 'autonomous-https-genai-lab-v1'
              AND x.request->>'execution_mode' = 'live'
              AND b.publication_obligation = 'terrain.issued_dns.v2';
        ELSE
            SELECT jsonb_build_object('request', execution.private_live_issued_request(e, c, result_id), 'operation', x.terrain_operation) INTO raw
            FROM execution.autonomous_runs x JOIN execution.live_dns_results b ON b.run_id = x.run_id
            WHERE b.result = result_id AND (x.e, x.c) = (e, c) AND b.result = x.result AND b.attempt = x.attempt
              AND b.run_kind = 'legacy' AND x.request->>'run_profile' = q->>'run_profile'
              AND x.request->>'execution_mode' = 'live' AND b.publication_obligation = 'terrain.issued_dns.v2';
        END IF;
    ELSIF q->>'run_profile' = 'm1_https_loop_v1' THEN
        IF q->>'execution_mode' IS DISTINCT FROM 'controlled_fixture' THEN RAISE EXCEPTION 'issued_reference_refused'; END IF;
        SELECT jsonb_build_object('request', b.request, 'operation', coalesce(st.terrain_operation_id, x.terrain_operation)) INTO raw
        FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a ON a.attempt = b.attempt
        JOIN execution.autonomous_runs x ON x.run_id = b.run_id
        JOIN execution.https_loop_steps st ON st.run_id = x.run_id AND st.step_index = b.step_index
        WHERE b.result = result_id AND (x.e, x.c) = (e, c)
          AND (x.request->>'run_profile', x.request->>'execution_mode') = (q->>'run_profile', q->>'execution_mode')
          AND b.run_kind = 'm1_https_loop_v1' AND a.run_kind = 'm1_https_loop_v1'
          AND b.step_index = a.step_index AND b.attempt_kind = a.attempt_kind
          AND a.decision = st.decision_id AND a.session_generation > 0
          AND b.release_sha256 = x.release_sha256 AND b.release_sha256 = a.release_sha256
          AND b.mode = a.mode AND b.profile = a.profile AND b.mode = 'controlled_fixture' AND b.profile = 'dns-routing-fixture-v1'
          AND b.publication_obligation = 'terrain.issued_dns.v1'
          AND (b.request->>'engagement_id', b.request->>'campaign_id', b.request->>'correlation_id') = (e::text, c::text, result_id::text)
          AND (b.request->>'version', b.request->>'source_kind') = ('m1_terrain_dns_issued_observation_v1', 'fixture_dns_acquisition');
    ELSE
        IF q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1', 'autonomous-dns-provider-fixture-v1', 'autonomous-dns-fixture-v2')
           OR q->>'execution_mode' IS DISTINCT FROM 'controlled_fixture' THEN
            RAISE EXCEPTION 'issued_reference_refused';
        END IF;
        SELECT jsonb_build_object('request', b.request, 'operation', x.terrain_operation) INTO raw
        FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a USING (run_id)
        JOIN execution.autonomous_runs x USING (run_id) WHERE b.result = result_id AND (x.e, x.c) = (e, c)
          AND (x.request->>'run_profile', x.request->>'execution_mode') = (q->>'run_profile', q->>'execution_mode')
          AND b.result = x.result AND b.attempt = a.attempt AND b.attempt = x.attempt AND a.decision = x.decision_one
          AND a.session_generation > 0 AND b.release_sha256 = x.release_sha256 AND b.release_sha256 = a.release_sha256
          AND b.mode = a.mode AND b.profile = a.profile AND b.mode = 'controlled_fixture' AND b.profile = 'dns-routing-fixture-v1'
          AND b.publication_obligation = 'terrain.issued_dns.v1'
          AND (b.request->>'engagement_id', b.request->>'campaign_id', b.request->>'correlation_id') = (e::text, c::text, result_id::text)
          AND (b.request->>'version', b.request->>'source_kind') = ('m1_terrain_dns_issued_observation_v1', 'fixture_dns_acquisition');
    END IF;
    IF raw IS NULL OR raw->'request' = 'null'::jsonb THEN RAISE EXCEPTION 'issued_result_unavailable'; END IF;
    RETURN raw;
END $$;

CREATE OR REPLACE FUNCTION terrain.admit_dns_request(p_request jsonb)
RETURNS jsonb LANGUAGE plpgsql SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    r jsonb; k text; a jsonb; label text; rule jsonb; rules jsonb;
    m mission.missions%ROWTYPE; reg mission.registration_outbox%ROWTYPE;
    f jsonb; policy jsonb; matched boolean := false;
    now_at bigint := floor(extract(epoch FROM statement_timestamp()))::bigint;
    serialized text;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_terrain', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user, g, 'MEMBER'))
       OR EXISTS (SELECT 1 FROM pg_catalog.pg_roles p WHERE pg_has_role(session_user, p.oid, 'SET') AND (p.rolsuper OR p.rolbypassrls OR p.rolcreatedb OR p.rolcreaterole)) THEN
        RAISE EXCEPTION 'unqualified_terrain_writer' USING ERRCODE = '42501';
    END IF;
    IF current_setting('transaction_isolation') <> 'serializable' OR current_setting('synchronous_commit') <> 'on' THEN
        RAISE EXCEPTION 'terrain_transaction_refused';
    END IF;
    IF jsonb_typeof(p_request) IS DISTINCT FROM 'object' OR octet_length(p_request::text) > 16384 THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    r := '{"asset_ref":null,"expected_claim_revision":0,"uncertainty_seconds":0,"qclass":"IN","limitations":[]}'::jsonb || p_request;
    IF (r - ARRAY['version','engagement_id','campaign_id','operator_ref','expected_mission_revision','goal_ref','exercise_mode',
        'correlation_id','source_kind','vantage_ref','resolver_ipv4','subject_name','asset_ref','expected_claim_revision',
        'perceived_at','uncertainty_seconds','transport','qtype','qclass','rcode','truncated','completeness','limitations','answers']) <> '{}'::jsonb THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    FOREACH k IN ARRAY ARRAY['version','engagement_id','campaign_id','operator_ref','goal_ref','exercise_mode','correlation_id',
        'source_kind','vantage_ref','resolver_ipv4','subject_name','transport','qtype','qclass','rcode','completeness'] LOOP
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'string' THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
    END LOOP;
    FOREACH k IN ARRAY ARRAY['engagement_id','campaign_id','operator_ref','goal_ref','correlation_id','vantage_ref','asset_ref'] LOOP
        IF k = 'asset_ref' AND r->k = 'null'::jsonb THEN CONTINUE; END IF;
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'string' OR (r->>k) !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           OR (r->>k)::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    FOREACH k IN ARRAY ARRAY['expected_mission_revision','expected_claim_revision','perceived_at','uncertainty_seconds'] LOOP
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'number' OR (r->>k) !~ '^(0|[1-9][0-9]*)$' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF NOT (
        (r->>'version', r->>'source_kind') = ('m1_terrain_dns_observation_v1', 'operator_reviewed_dns')
        OR (r->>'version', r->>'source_kind') = ('m1_terrain_dns_issued_observation_v1', 'fixture_dns_acquisition') AND (
            EXISTS (
                SELECT 1 FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a USING (run_id)
                JOIN execution.autonomous_runs x USING (run_id) WHERE b.result = (r->>'correlation_id')::uuid
                  AND b.request = r AND b.result = x.result AND b.attempt = a.attempt AND b.attempt = x.attempt
                  AND a.decision = x.decision_one AND a.session_generation > 0 AND b.profile = a.profile AND b.mode = a.mode
                  AND b.profile = 'dns-routing-fixture-v1' AND b.mode = 'controlled_fixture'
                  AND b.release_sha256 = a.release_sha256 AND b.release_sha256 = x.release_sha256
                  AND (x.e, x.c) = ((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid)
                  AND x.request->>'execution_mode' = b.mode
                  AND x.request->>'run_profile' IN ('autonomous-dns-fixture-v1', 'autonomous-dns-provider-fixture-v1', 'autonomous-dns-fixture-v2')
                  AND b.publication_obligation = 'terrain.issued_dns.v1'
            ) OR EXISTS (
                SELECT 1 FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a ON a.attempt = b.attempt
                JOIN execution.autonomous_runs x ON x.run_id = b.run_id
                JOIN execution.https_loop_steps st ON st.run_id = x.run_id AND st.step_index = b.step_index
                WHERE b.result = (r->>'correlation_id')::uuid AND b.request = r
                  AND b.run_kind = 'm1_https_loop_v1' AND a.run_kind = 'm1_https_loop_v1'
                  AND b.step_index = a.step_index AND b.attempt_kind = a.attempt_kind
                  AND a.decision = st.decision_id AND a.session_generation > 0 AND b.profile = a.profile AND b.mode = a.mode
                  AND b.profile = 'dns-routing-fixture-v1' AND b.mode = 'controlled_fixture'
                  AND b.release_sha256 = a.release_sha256 AND b.release_sha256 = x.release_sha256
                  AND (x.e, x.c) = ((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid)
                  AND x.request->>'execution_mode' = b.mode AND x.request->>'run_profile' = 'm1_https_loop_v1'
                  AND b.publication_obligation = 'terrain.issued_dns.v1'
            )
        ) OR (r->>'version', r->>'source_kind') = ('m1_terrain_dns_issued_observation_v2', 'issued_dns_acquisition') AND (
            EXISTS (
                SELECT 1 FROM execution.live_dns_results b JOIN execution.autonomous_runs x ON x.run_id = b.run_id
                WHERE b.result = (r->>'correlation_id')::uuid AND b.result = x.result AND b.attempt = x.attempt AND b.run_kind = 'legacy'
                  AND (x.e, x.c) = ((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid)
                  AND x.request->>'execution_mode' = 'live'
                  AND x.request->>'run_profile' IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1')
                  AND b.publication_obligation = 'terrain.issued_dns.v2'
                  AND r = execution.private_live_issued_request((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid, (r->>'correlation_id')::uuid)
            ) OR EXISTS (
                SELECT 1 FROM execution.live_dns_results b
                JOIN execution.autonomous_runs x ON x.run_id = b.run_id
                JOIN execution.https_loop_steps st ON st.run_id = x.run_id AND st.step_index = b.step_index
                WHERE b.result = (r->>'correlation_id')::uuid AND b.run_kind = 'm1_https_loop_v1'
                  AND ((b.attempt_kind = 'dns' AND b.attempt = st.dns_attempt_id AND b.result = st.dns_result_id)
                       OR (b.attempt_kind = 'dns_refresh' AND b.attempt = st.refresh_attempt_id AND b.result = st.refresh_result_id))
                  AND (x.e, x.c) = ((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid)
                  AND x.request->>'version' = 'm1_https_loop_v1'
                  AND x.request->>'execution_mode' = 'live'
                  AND x.request->>'run_profile' = 'autonomous-https-genai-lab-v1'
                  AND b.publication_obligation = 'terrain.issued_dns.v2'
                  AND r = execution.private_live_issued_request((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid, (r->>'correlation_id')::uuid)
            )
        )
    )
       OR r->>'expected_mission_revision' <> '1' OR (r->>'expected_claim_revision')::bigint > 10000
       OR (r->>'uncertainty_seconds')::bigint > 300 OR (r->>'perceived_at')::bigint <= 0
       OR (r->>'perceived_at')::bigint > now_at OR r->>'exercise_mode' NOT IN ('blind', 'defender_informed')
       OR r->>'transport' NOT IN ('udp', 'tcp') OR r->>'qtype' <> 'A' OR r->>'qclass' <> 'IN'
       OR r->>'rcode' NOT IN ('noerror', 'nxdomain', 'servfail', 'refused', 'formerr', 'notimp')
       OR r->>'completeness' NOT IN ('complete', 'incomplete', 'truncated', 'timeout')
       OR jsonb_typeof(r->'truncated') IS DISTINCT FROM 'boolean' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF length(r->>'subject_name') > 253 OR position('.' in r->>'subject_name') = 0
       OR r->>'subject_name' ~ '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    FOREACH label IN ARRAY string_to_array(r->>'subject_name', '.') LOOP
        IF length(label) > 63 OR label !~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$' OR label LIKE 'xn--%' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF family((r->>'resolver_ipv4')::inet) <> 4 OR host((r->>'resolver_ipv4')::inet) <> r->>'resolver_ipv4' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF jsonb_typeof(r->'answers') IS DISTINCT FROM 'array' OR jsonb_typeof(r->'limitations') IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF jsonb_array_length(r->'answers') > 4 OR jsonb_array_length(r->'limitations') > 4 THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
    FOR a IN SELECT value FROM jsonb_array_elements(r->'answers') LOOP
        IF jsonb_typeof(a) IS DISTINCT FROM 'object' OR (a - ARRAY['ipv4','ttl_seconds']) <> '{}'::jsonb
           OR jsonb_typeof(a->'ipv4') IS DISTINCT FROM 'string' OR jsonb_typeof(a->'ttl_seconds') IS DISTINCT FROM 'number'
           OR (a->>'ttl_seconds') !~ '^(0|[1-9][0-9]*)$' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
        IF (a->>'ttl_seconds')::bigint > 86400 OR family((a->>'ipv4')::inet) <> 4 OR host((a->>'ipv4')::inet) <> a->>'ipv4' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF (SELECT count(DISTINCT value->>'ipv4') FROM jsonb_array_elements(r->'answers')) <> jsonb_array_length(r->'answers')
       OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(r->'limitations')) <> jsonb_array_length(r->'limitations')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(r->'limitations') x WHERE jsonb_typeof(x.value) <> 'string'
           OR x.value#>>'{}' NOT IN ('cache_remaining_ttl', 'single_vantage_only', 'partial_alias_chain', 'resolver_recursion_variance')) THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    SELECT * INTO m FROM mission.missions WHERE engagement_id = (r->>'engagement_id')::uuid AND campaign_id = (r->>'campaign_id')::uuid FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    SELECT * INTO reg FROM mission.registration_outbox WHERE engagement_id = m.engagement_id AND campaign_id = m.campaign_id AND operation_id = m.operation_id;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    f := reg.contract->'fields'; policy := f->'m1_permission';
    IF ((reg.contract @> jsonb_build_object('engagement_id', m.engagement_id, 'campaign_id', m.campaign_id, 'operation_id', m.operation_id,
            'event_id', reg.event_id, 'producer', 'mission', 'kind', 'mission_registered', 'owner_revision', m.revision)) IS NOT TRUE
        OR (f @> jsonb_build_object('operator_ref', m.operator_ref, 'goal_ref', m.goal_ref, 'authority_ref', m.authority_ref,
            'authority_revision', m.authority_revision, 'included_assets', m.included_assets, 'excluded_assets', m.excluded_assets,
            'exercise_mode', m.exercise_mode, 'starts_at', m.starts_at, 'ends_at', m.ends_at)) IS NOT TRUE
        OR m.revision <> 1 OR r->>'operator_ref' <> m.operator_ref::text OR r->>'goal_ref' <> m.goal_ref::text
        OR r->>'exercise_mode' <> m.exercise_mode OR policy->>'policy_version' IS DISTINCT FROM '1'
        OR policy->>'vantage_ref' IS DISTINCT FROM r->>'vantage_ref' OR policy->>'resolver_ipv4' IS DISTINCT FROM r->>'resolver_ipv4'
        OR NOT (m.starts_at <= now_at AND now_at < m.ends_at AND m.starts_at <= (r->>'perceived_at')::bigint AND (r->>'perceived_at')::bigint < m.ends_at)
        OR NOT ((policy->>'starts_at')::bigint <= now_at AND now_at < (policy->>'ends_at')::bigint
            AND (policy->>'starts_at')::bigint <= (r->>'perceived_at')::bigint AND (r->>'perceived_at')::bigint < (policy->>'ends_at')::bigint)
        OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE engagement_id = m.engagement_id AND campaign_id = m.campaign_id)) IS NOT FALSE THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    IF r->'asset_ref' = 'null'::jsonb THEN
        rules := policy->'discovery_rules';
    ELSE
        IF NOT (m.included_assets @> jsonb_build_array(r->'asset_ref')) OR m.excluded_assets @> jsonb_build_array(r->'asset_ref') THEN
            RAISE EXCEPTION 'terrain_authority_unavailable';
        END IF;
        SELECT coalesce(jsonb_agg(value->'rule'), '[]'::jsonb) INTO rules FROM jsonb_array_elements(policy->'contact_rules') WHERE value->'asset_ref' = r->'asset_ref';
    END IF;
    FOR rule IN SELECT value FROM jsonb_array_elements(rules) LOOP
        matched := matched OR rule->>'exact' = r->>'subject_name' OR rule->>'label_suffix' = r->>'subject_name'
            OR right(r->>'subject_name', length(rule->>'label_suffix') + 1) = '.' || (rule->>'label_suffix');
    END LOOP;
    IF matched IS NOT TRUE OR EXISTS (
        SELECT 1 FROM jsonb_array_elements(policy->'excluded_names') x
        WHERE x.value->>'exact' = r->>'subject_name' OR x.value->>'label_suffix' = r->>'subject_name'
           OR right(r->>'subject_name', length(x.value->>'label_suffix') + 1) = '.' || (x.value->>'label_suffix')
    ) THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    serialized := replace(json_build_object(
        'version', r->'version', 'engagement_id', r->'engagement_id', 'campaign_id', r->'campaign_id',
        'operator_ref', r->'operator_ref', 'expected_mission_revision', r->'expected_mission_revision', 'goal_ref', r->'goal_ref',
        'exercise_mode', r->'exercise_mode', 'correlation_id', r->'correlation_id', 'source_kind', r->'source_kind',
        'vantage_ref', r->'vantage_ref', 'resolver_ipv4', r->'resolver_ipv4', 'subject_name', r->'subject_name',
        'asset_ref', r->'asset_ref', 'expected_claim_revision', r->'expected_claim_revision', 'perceived_at', r->'perceived_at',
        'uncertainty_seconds', r->'uncertainty_seconds', 'transport', r->'transport', 'qtype', r->'qtype', 'qclass', r->'qclass',
        'rcode', r->'rcode', 'truncated', r->'truncated', 'completeness', r->'completeness',
        'limitations', r->'limitations', 'answers', r->'answers'
    )::text, ' ', '');
    RETURN jsonb_build_object('request', r, 'registration', reg.contract, 'now', now_at,
        'request_hash', encode(sha256(convert_to(serialized, 'UTF8')), 'hex'));
END $$;

CREATE OR REPLACE FUNCTION terrain.accept_https_observation(req jsonb, req_hash text, payload jsonb, reg_op uuid)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    v_e uuid; v_c uuid; v_op uuid; existing terrain.https_outbox;
    ev_id uuid; obs_id uuid; mat_id uuid; eval_id uuid; clm_id uuid;
    reg mission.registration_outbox; clm jsonb; evl jsonb; max_rev integer;
    res_payload jsonb; res_e uuid; res_c uuid;
    tag text; digest bytea; ids jsonb := '{}'::jsonb;
    pol jsonb; scope_ok boolean; name_ok boolean; exp_ev_outcome text; exp_usable boolean;
    exp_reach_eval jsonb; exp_status text; exp_reach_clm boolean;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_terrain', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user, g, 'MEMBER'))
       OR EXISTS (SELECT 1 FROM pg_catalog.pg_roles p WHERE pg_has_role(session_user, p.oid, 'SET') AND (p.rolsuper OR p.rolbypassrls OR p.rolcreatedb OR p.rolcreaterole)) THEN
        RAISE EXCEPTION 'unqualified_terrain_writer' USING ERRCODE = '42501';
    END IF;
    IF current_setting('transaction_isolation') <> 'serializable' OR current_setting('synchronous_commit') <> 'on' THEN
        RAISE EXCEPTION 'terrain_transaction_refused';
    END IF;
    IF jsonb_typeof(req) <> 'object' OR jsonb_typeof(payload) <> 'object'
       OR req_hash IS NULL OR req_hash !~ '^[0-9a-f]{64}$'
       OR req_hash IS DISTINCT FROM encode(sha256(convert_to(execution.genai_canonical(req), 'UTF8')), 'hex') THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    v_e := (payload->>'engagement_id')::uuid;
    v_c := (payload->>'campaign_id')::uuid;
    v_op := (payload->>'operation_id')::uuid;
    IF (req->>'engagement_id')::uuid IS DISTINCT FROM v_e
       OR (req->>'campaign_id')::uuid IS DISTINCT FROM v_c
       OR payload->>'kind' IS DISTINCT FROM 'terrain_changed'
       OR payload->>'publication_obligation' IS DISTINCT FROM 'trajectory.terrain_history.v1' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    SELECT * INTO reg FROM mission.registration_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = reg_op;
    IF NOT FOUND OR (reg.contract->>'engagement_id')::uuid IS DISTINCT FROM v_e OR (reg.contract->>'campaign_id')::uuid IS DISTINCT FROM v_c THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    IF req->>'source_kind' = 'fixture_https_acquisition' THEN
        SELECT hr.result_payload, (hr.result_payload->>'engagement_id')::uuid, (hr.result_payload->>'campaign_id')::uuid
        INTO res_payload, res_e, res_c
        FROM execution.fixture_https_results hr WHERE hr.result = (req->>'result_id')::uuid;
    ELSIF req->>'source_kind' = 'issued_https_acquisition' THEN
        SELECT lr.result_payload, (lr.result_payload->>'engagement_id')::uuid, (lr.result_payload->>'campaign_id')::uuid
        INTO res_payload, res_e, res_c
        FROM execution.live_https_results lr
        JOIN execution.live_effect_control xc ON xc.attempt = lr.attempt
        JOIN execution.live_effect_sessions ls ON ls.attempt = lr.attempt
        WHERE lr.result = (req->>'result_id')::uuid
          AND lr.publication_obligation = 'terrain.issued_https.v2'
          AND ls.run_kind = 'm1_https_loop_v1' AND ls.attempt_kind = 'head'
          AND xc.possibly_started;
    ELSE
        RAISE EXCEPTION 'issued_result_unavailable';
    END IF;
    IF res_payload IS NULL OR res_payload IS DISTINCT FROM req OR res_e IS DISTINCT FROM v_e OR res_c IS DISTINCT FROM v_c THEN
        RAISE EXCEPTION 'issued_result_unavailable';
    END IF;
    SELECT * INTO existing FROM terrain.https_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op FOR UPDATE;
    IF FOUND THEN
        IF existing.request_hash = req_hash AND existing.contract = payload THEN RETURN existing.contract; END IF;
        RAISE EXCEPTION 'terrain_identity_conflict';
    END IF;
    FOREACH tag IN ARRAY ARRAY['observation','material','evaluation','claim','event'] LOOP
        digest := substring(sha256(convert_to('dw-terrain-id-v1','UTF8')||decode('00','hex')||uuid_send(v_e)||uuid_send(v_c)||uuid_send(v_op)||convert_to(tag,'UTF8')) FROM 1 FOR 16);
        digest := set_byte(digest, 6, (get_byte(digest, 6) & 15) | 64);
        digest := set_byte(digest, 8, (get_byte(digest, 8) & 63) | 128);
        ids := ids || jsonb_build_object(tag, encode(digest, 'hex')::uuid);
    END LOOP;
    ev_id := (ids->>'event')::uuid;
    obs_id := (ids->>'observation')::uuid;
    mat_id := (ids->>'material')::uuid;
    eval_id := (ids->>'evaluation')::uuid;
    clm_id := (ids->>'claim')::uuid;
    clm := payload->'claim'; evl := payload->'evaluation';
    IF jsonb_typeof(clm) <> 'object' OR jsonb_typeof(evl) <> 'object'
       OR (payload->>'event_id')::uuid IS DISTINCT FROM ev_id
       OR (evl->>'observation_id')::uuid IS DISTINCT FROM obs_id
       OR (evl->>'material_id')::uuid IS DISTINCT FROM mat_id
       OR (evl->>'evaluation_id')::uuid IS DISTINCT FROM eval_id
       OR (evl->>'claim_id')::uuid IS DISTINCT FROM clm_id
       OR (clm->>'claim_id')::uuid IS DISTINCT FROM clm_id
       OR (clm->>'evaluation_id')::uuid IS DISTINCT FROM eval_id
       OR (clm->>'observation_id')::uuid IS DISTINCT FROM obs_id
       OR (clm->>'material_id')::uuid IS DISTINCT FROM mat_id
       OR clm->'asset_ref' IS DISTINCT FROM req->'asset_ref'
       OR clm->>'hostname' IS DISTINCT FROM req->>'hostname'
       OR clm->>'ipv4' IS DISTINCT FROM req->>'ipv4'
       OR clm->'port' IS DISTINCT FROM req->'port'
       OR clm->>'approved_path' IS DISTINCT FROM req->>'approved_path'
       OR clm->>'method' IS DISTINCT FROM req->>'method'
       OR clm->'parent_claim_id' IS DISTINCT FROM req->'parent_claim_id'
       OR clm->'parent_claim_revision' IS DISTINCT FROM req->'parent_claim_revision'
       OR clm->>'claim_kind' IS DISTINCT FROM 'https_endpoint_head_observation'
       OR clm->>'source_kind' IS DISTINCT FROM req->>'source_kind'
       OR clm->>'trust_tier' IS DISTINCT FROM 'tier_1'
       OR clm->>'outcome' IS DISTINCT FROM req->>'outcome'
       OR clm->'status_code' IS DISTINCT FROM req->'status_code'
       OR clm->>'redirect' IS DISTINCT FROM req->>'redirect'
       OR clm->'perceived_at' IS DISTINCT FROM req->'perceived_at'
       OR (clm->>'evaluated_at')::bigint < (req->>'perceived_at')::bigint
       OR clm->'uncertainty_seconds' IS DISTINCT FROM req->'uncertainty_seconds'
       OR clm->'temporal_frontier' IS DISTINCT FROM req->'perceived_at'
       OR clm->'limitations' IS DISTINCT FROM req->'limitations' THEN
        RAISE EXCEPTION 'terrain_event_refused';
    END IF;
    pol := reg.contract#>'{fields,m1_permission}';
    IF pol IS NULL THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    scope_ok := (reg.contract#>'{fields,included_assets}' @> jsonb_build_array(req->'asset_ref'))
        AND NOT (reg.contract#>'{fields,excluded_assets}' @> jsonb_build_array(req->'asset_ref'));
    name_ok := (req->>'perceived_at')::bigint >= (pol->>'starts_at')::bigint
        AND (req->>'perceived_at')::bigint < (pol->>'ends_at')::bigint
        AND EXISTS (
            SELECT 1 FROM jsonb_array_elements(pol->'contact_rules') cr
            WHERE (cr->>'asset_ref')::uuid = (req->>'asset_ref')::uuid
              AND (cr->'rule'->>'exact' = req->>'hostname'
                   OR cr->'rule'->>'label_suffix' = req->>'hostname'
                   OR right(req->>'hostname', length(cr->'rule'->>'label_suffix') + 1) = '.' || (cr->'rule'->>'label_suffix'))
        )
        AND NOT EXISTS (
            SELECT 1 FROM jsonb_array_elements(pol->'excluded_names') ex
            WHERE ex->>'exact' = req->>'hostname'
               OR ex->>'label_suffix' = req->>'hostname'
               OR right(req->>'hostname', length(ex->>'label_suffix') + 1) = '.' || (ex->>'label_suffix')
        );
    exp_reach_clm := (req->>'outcome' = 'head_response');
    IF req->>'outcome' = 'execution_refused' THEN
        exp_ev_outcome := 'execution_refused'; exp_usable := false; exp_reach_eval := 'null'::jsonb; exp_status := 'withheld';
    ELSIF scope_ok AND name_ok THEN
        exp_ev_outcome := 'qualified_https_observation'; exp_usable := true; exp_reach_eval := to_jsonb(exp_reach_clm); exp_status := 'observed';
    ELSE
        exp_ev_outcome := 'scope_or_policy_withheld'; exp_usable := false; exp_reach_eval := 'null'::jsonb; exp_status := 'withheld';
    END IF;
    IF evl->>'outcome' IS DISTINCT FROM exp_ev_outcome
       OR evl->'usable_endpoint_premise' IS DISTINCT FROM to_jsonb(exp_usable)
       OR evl->'endpoint_reachable' IS DISTINCT FROM exp_reach_eval
       OR clm->>'claim_status' IS DISTINCT FROM exp_status
       OR clm->'endpoint_reachable' IS DISTINCT FROM to_jsonb(exp_reach_clm)
       OR clm->'whole_host_proven' IS DISTINCT FROM 'false'::jsonb
       OR clm->'objective_completed' IS DISTINCT FROM 'false'::jsonb THEN
        RAISE EXCEPTION 'terrain_event_refused';
    END IF;
    SELECT coalesce(max(claim_revision), 0) INTO max_rev FROM terrain.https_observations
    WHERE engagement_id = v_e AND campaign_id = v_c AND asset_ref = (req->>'asset_ref')::uuid
      AND hostname = req->>'hostname' AND ipv4 = req->>'ipv4' AND approved_path = req->>'approved_path';
    IF (clm->>'claim_revision')::int IS DISTINCT FROM max_rev + 1 THEN RAISE EXCEPTION 'claim_revision_mismatch'; END IF;
    INSERT INTO terrain.https_observations (
        engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id,
        claim_revision, asset_ref, hostname, ipv4, port, approved_path, parent_claim_id, parent_claim_revision,
        source_kind, outcome, status_code, redirect, endpoint_reachable, whole_host_proven, objective_completed,
        perceived_at, evaluated_at, request_hash, payload
    ) VALUES (
        v_e, v_c, v_op, ev_id, obs_id, mat_id, eval_id, clm_id, (clm->>'claim_revision')::int,
        (req->>'asset_ref')::uuid, req->>'hostname', req->>'ipv4', (req->>'port')::int, req->>'approved_path',
        (req->>'parent_claim_id')::uuid, (req->>'parent_claim_revision')::int, req->>'source_kind', req->>'outcome',
        CASE WHEN req->'status_code' = 'null'::jsonb THEN NULL ELSE (req->>'status_code')::int END,
        req->>'redirect', exp_reach_clm, false, false,
        (req->>'perceived_at')::bigint, (clm->>'evaluated_at')::bigint, req_hash, payload
    );
    INSERT INTO terrain.https_outbox (
        engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id,
        claim_revision, registration_operation_id, registration_event_id, request_hash, contract
    ) VALUES (
        v_e, v_c, v_op, ev_id, obs_id, mat_id, eval_id, clm_id, (clm->>'claim_revision')::int,
        reg.operation_id, reg.event_id, req_hash, payload
    );
    RETURN payload;
END $$;

COMMIT;
