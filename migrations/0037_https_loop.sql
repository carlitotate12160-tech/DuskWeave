-- HTTPS-loop successor (`m1_https_loop_v1`), indexed steps, protected feedback capacity,
-- write-once Broker-private endpoint bindings, and typed HTTPS Terrain/Trajectory integration.
BEGIN;

-- 1. Extend execution.native_input_bindings while restricting Cognition to original safe columns.
ALTER TABLE execution.native_input_bindings
    ADD COLUMN pack_digest text,
    ADD COLUMN request_id uuid,
    ADD COLUMN private_endpoint_map jsonb,
    ADD COLUMN map_version text;

REVOKE SELECT ON execution.native_input_bindings FROM dw_m1_cognition;
GRANT SELECT (e, c, d, input_hash, writer_oid) ON execution.native_input_bindings TO dw_m1_cognition;

-- 2. Extend fixture DNS attempt/result cardinality only for explicit `m1_https_loop_v1` runs.
ALTER TABLE execution.fixture_dns_results DROP CONSTRAINT fixture_dns_results_run_id_fkey;
ALTER TABLE execution.fixture_dns_results DROP CONSTRAINT fixture_dns_results_run_id_key;
ALTER TABLE execution.fixture_dns_attempts DROP CONSTRAINT fixture_dns_attempts_pkey;

ALTER TABLE execution.fixture_dns_attempts
    ADD PRIMARY KEY (attempt),
    ADD COLUMN run_kind text NOT NULL DEFAULT 'legacy' CHECK (run_kind IN ('legacy', 'm1_https_loop_v1')),
    ADD COLUMN step_index integer CHECK (step_index BETWEEN 1 AND 4),
    ADD COLUMN attempt_kind text NOT NULL DEFAULT 'initial' CHECK (attempt_kind IN ('initial', 'refresh'));

ALTER TABLE execution.fixture_dns_results
    ADD COLUMN run_kind text NOT NULL DEFAULT 'legacy' CHECK (run_kind IN ('legacy', 'm1_https_loop_v1')),
    ADD COLUMN step_index integer CHECK (step_index BETWEEN 1 AND 4),
    ADD COLUMN attempt_kind text NOT NULL DEFAULT 'initial' CHECK (attempt_kind IN ('initial', 'refresh')),
    ADD CONSTRAINT fixture_dns_results_attempt_fkey FOREIGN KEY (attempt) REFERENCES execution.fixture_dns_attempts(attempt);

CREATE UNIQUE INDEX fixture_dns_attempts_legacy_uidx
    ON execution.fixture_dns_attempts (run_id) WHERE run_kind = 'legacy';
CREATE UNIQUE INDEX fixture_dns_attempts_https_uidx
    ON execution.fixture_dns_attempts (run_id, step_index, attempt_kind) WHERE run_kind = 'm1_https_loop_v1';
CREATE UNIQUE INDEX fixture_dns_results_legacy_uidx
    ON execution.fixture_dns_results (run_id) WHERE run_kind = 'legacy';
CREATE UNIQUE INDEX fixture_dns_results_https_uidx
    ON execution.fixture_dns_results (run_id, step_index, attempt_kind) WHERE run_kind = 'm1_https_loop_v1';

ALTER TABLE execution.autonomous_control DROP CONSTRAINT autonomous_control_status_check;
ALTER TABLE execution.autonomous_control
    ADD CONSTRAINT autonomous_control_status_check CHECK (
        status IN (
            'running', 'completed', 'stopped', 'unknown', 'deferred',
            'retained', 'capability_gap', 'owner_view_requested',
            'blocked_stale_premises', 'blocked_budget_or_time'
        )
    );

ALTER TABLE execution.run_phase_diagnostics DROP CONSTRAINT run_phase_diagnostics_phase_check;
ALTER TABLE execution.run_phase_diagnostics
    ADD CONSTRAINT run_phase_diagnostics_phase_check CHECK (
        phase IN (
            'decision_one', 'acquisition', 'decision_two',
            'acquisition_one', 'acquisition_two', 'decision_three', 'acquisition_three', 'decision_four'
        )
    );

-- 3. Bounded indexed run steps and protected next-decision capacity.
CREATE TABLE execution.https_loop_steps (
    run_id uuid NOT NULL REFERENCES execution.autonomous_runs(run_id),
    e uuid NOT NULL,
    c uuid NOT NULL,
    step_index integer NOT NULL CHECK (step_index BETWEEN 1 AND 4),
    decision_id uuid NOT NULL UNIQUE,
    next_decision_id uuid UNIQUE,
    decision_phase text NOT NULL,
    acquisition_phase text,
    status text NOT NULL CHECK (
        status IN (
            'pending', 'decision_running', 'decision_completed',
            'acquisition_pending', 'acquisition_running', 'completed',
            'terminal', 'unknown', 'stopped'
        )
    ),
    protection_state text NOT NULL DEFAULT 'none'
        CHECK (protection_state IN ('none', 'protected', 'converted', 'released', 'unknown')),
    protected_capacity jsonb,
    protection_epoch bigint,
    next_intent_kind text,
    resolved_asset_ref uuid,
    resolved_hostname text,
    resolved_ipv4 text,
    resolved_endpoint_ref text,
    resolved_path text,
    parent_claim_id uuid,
    parent_claim_revision integer,
    dependency_refresh text,
    episode_index integer CHECK (episode_index BETWEEN 1 AND 3),
    episode_deadline_ms bigint,
    dns_attempt_id uuid UNIQUE,
    dns_result_id uuid UNIQUE,
    dns_request jsonb,
    refresh_attempt_id uuid UNIQUE,
    refresh_result_id uuid UNIQUE,
    refresh_request jsonb,
    refresh_outcome text,
    head_attempt_id uuid UNIQUE,
    head_result_id uuid UNIQUE,
    head_outcome text,
    terrain_operation_id uuid,
    head_terrain_operation_id uuid,
    disposition text,
    updated_at_ms bigint NOT NULL,
    PRIMARY KEY (run_id, step_index),
    UNIQUE (e, c, decision_id)
);

CREATE TABLE execution.fixture_https_attempts (
    attempt uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES execution.autonomous_runs(run_id),
    step_index integer NOT NULL CHECK (step_index BETWEEN 1 AND 4),
    session_generation bigint NOT NULL,
    decision uuid NOT NULL,
    asset_ref uuid NOT NULL,
    endpoint_ref text NOT NULL,
    hostname text NOT NULL,
    ipv4 text NOT NULL,
    approved_path text NOT NULL,
    parent_claim_id uuid NOT NULL,
    parent_claim_revision integer NOT NULL CHECK (parent_claim_revision >= 1),
    profile text NOT NULL CHECK (profile = 'https-head-fixture-v1'),
    mode text NOT NULL CHECK (mode = 'controlled_fixture'),
    release_sha256 text NOT NULL,
    reserved_at_ms bigint NOT NULL,
    request jsonb NOT NULL,
    UNIQUE (run_id, step_index)
);

CREATE TABLE execution.fixture_https_results (
    result uuid PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES execution.autonomous_runs(run_id),
    step_index integer NOT NULL CHECK (step_index BETWEEN 1 AND 4),
    attempt uuid NOT NULL UNIQUE REFERENCES execution.fixture_https_attempts(attempt),
    request jsonb NOT NULL,
    result_payload jsonb NOT NULL,
    profile text NOT NULL CHECK (profile = 'https-head-fixture-v1'),
    mode text NOT NULL CHECK (mode = 'controlled_fixture'),
    release_sha256 text NOT NULL,
    received_at_ms bigint NOT NULL,
    publication_obligation text NOT NULL CHECK (publication_obligation = 'terrain.issued_https.v1'),
    UNIQUE (run_id, step_index)
);

REVOKE ALL ON execution.https_loop_steps, execution.fixture_https_attempts, execution.fixture_https_results FROM PUBLIC;
GRANT SELECT ON execution.https_loop_steps TO dw_m1_broker, dw_runtime;
GRANT SELECT ON execution.fixture_https_attempts, execution.fixture_https_results TO dw_m1_broker, dw_m1_terrain, dw_runtime;

-- 4. Typed HTTPS Terrain persistence and outbox.
CREATE TABLE terrain.https_observations (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL UNIQUE,
    observation_id uuid NOT NULL UNIQUE,
    material_id uuid NOT NULL UNIQUE,
    evaluation_id uuid NOT NULL UNIQUE,
    claim_id uuid NOT NULL UNIQUE,
    claim_revision integer NOT NULL CHECK (claim_revision > 0),
    asset_ref uuid NOT NULL,
    hostname text NOT NULL,
    ipv4 text NOT NULL,
    port integer NOT NULL CHECK (port = 443),
    approved_path text NOT NULL,
    parent_claim_id uuid NOT NULL,
    parent_claim_revision integer NOT NULL CHECK (parent_claim_revision > 0),
    source_kind text NOT NULL CHECK (source_kind = 'fixture_https_acquisition'),
    outcome text NOT NULL CHECK (
        outcome IN ('head_response', 'connection_failed', 'tls_failed', 'timed_out', 'execution_refused')
    ),
    status_code integer,
    redirect text NOT NULL CHECK (redirect IN ('none', 'refused_redirect')),
    endpoint_reachable boolean NOT NULL,
    whole_host_proven boolean NOT NULL CHECK (whole_host_proven = false),
    objective_completed boolean NOT NULL CHECK (objective_completed = false),
    perceived_at bigint NOT NULL CHECK (perceived_at > 0),
    evaluated_at bigint NOT NULL CHECK (evaluated_at >= perceived_at),
    request_hash text NOT NULL,
    payload jsonb NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    UNIQUE (engagement_id, campaign_id, asset_ref, hostname, ipv4, approved_path, claim_revision)
);

CREATE TABLE terrain.https_outbox (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL UNIQUE,
    observation_id uuid NOT NULL UNIQUE,
    material_id uuid NOT NULL UNIQUE,
    evaluation_id uuid NOT NULL UNIQUE,
    claim_id uuid NOT NULL UNIQUE,
    claim_revision integer NOT NULL CHECK (claim_revision > 0),
    registration_operation_id uuid NOT NULL,
    registration_event_id uuid NOT NULL,
    request_hash text NOT NULL,
    contract jsonb NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.https_observations (engagement_id, campaign_id, operation_id)
);

REVOKE ALL ON terrain.https_observations, terrain.https_outbox
    FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
GRANT SELECT ON terrain.https_observations, terrain.https_outbox TO dw_m1_terrain, dw_m1_trajectory;

-- 5. Decision resolution helper and capacity protection accounting.
CREATE OR REPLACE FUNCTION execution.find_decision_run(e uuid, c uuid, d uuid)
RETURNS execution.autonomous_runs LANGUAGE sql STABLE SET search_path = pg_catalog AS $$
    SELECT x.*
    FROM execution.autonomous_runs x
    WHERE (x.e, x.c) = ($1, $2)
      AND (
          $3 IN (x.decision_one, x.decision_two)
          OR EXISTS (
              SELECT 1 FROM execution.https_loop_steps s
              WHERE s.run_id = x.run_id AND (s.e, s.c, s.decision_id) = ($1, $2, $3)
          )
      )
    LIMIT 1
$$;

CREATE OR REPLACE FUNCTION execution.https_loop_protected_held(
    e uuid,
    c uuid,
    exclude_decision uuid DEFAULT NULL,
    exclude_converted boolean DEFAULT false
) RETURNS jsonb LANGUAGE sql STABLE SET search_path = pg_catalog AS $$
    SELECT jsonb_build_object(
        'calls', coalesce(sum((s.protected_capacity->>'calls')::bigint), 0),
        'request_bytes', coalesce(sum((s.protected_capacity->>'request_bytes')::bigint), 0),
        'response_bytes', coalesce(sum((s.protected_capacity->>'response_bytes')::bigint), 0),
        'output_token_capacity', coalesce(sum((s.protected_capacity->>'output_token_capacity')::bigint), 0),
        'wall_time_ms', coalesce(sum((s.protected_capacity->>'wall_time_ms')::bigint), 0)
    )
    FROM execution.https_loop_steps s
    WHERE (s.e, s.c) = ($1, $2)
      AND s.protected_capacity IS NOT NULL
      AND (
          s.protection_state = 'unknown'
          OR (
              s.protection_state = 'protected'
              AND NOT ($3 IS NOT NULL AND NOT $4 AND s.next_decision_id = $3)
          )
          OR (
              s.protection_state = 'converted'
              AND NOT EXISTS (
                  SELECT 1 FROM execution.inference_reservations ir
                  WHERE ir.engagement_id = $1 AND ir.campaign_id = $2 AND ir.operation_id = s.next_decision_id
              )
              AND NOT ($3 IS NOT NULL AND $4 AND s.next_decision_id = $3)
          )
      )
$$;

CREATE OR REPLACE FUNCTION execution.inference_capacity_snapshot(e uuid, c uuid)
RETURNS jsonb LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$
    WITH prot AS (SELECT execution.https_loop_protected_held($1, $2, NULL, false) AS p)
    SELECT jsonb_build_object(
        'version', 'm1_inference_capacity_snapshot_v2',
        'epoch', f.epoch,
        'session_claimed', f.operation_id IS NOT NULL,
        'active_holds', count(h.operation_id) FILTER (WHERE NOT EXISTS (
            SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = ($1, $2, h.operation_id))),
        'charged', jsonb_build_object(
            'calls', coalesce(sum((h.capacity->>'calls')::bigint), 0) + (SELECT (p->>'calls')::bigint FROM prot),
            'request_bytes', coalesce(sum((h.capacity->>'request_bytes')::bigint), 0) + (SELECT (p->>'request_bytes')::bigint FROM prot),
            'response_bytes', coalesce(sum((h.capacity->>'response_bytes')::bigint), 0) + (SELECT (p->>'response_bytes')::bigint FROM prot),
            'output_token_capacity', coalesce(sum((h.capacity->>'output_token_capacity')::bigint), 0) + (SELECT (p->>'output_token_capacity')::bigint FROM prot),
            'wall_time_ms', coalesce(sum((h.capacity->>'wall_time_ms')::bigint), 0) + (SELECT (p->>'wall_time_ms')::bigint FROM prot)
        )
    )
    FROM execution.session_fences f
    LEFT JOIN execution.inference_reservations h
        ON h.engagement_id = $1 AND h.campaign_id = $2 AND h.record_revision = 1
        AND NOT EXISTS (
            SELECT 1 FROM execution.inference_reservations x
            WHERE (x.engagement_id, x.campaign_id, x.operation_id) = ($1, $2, h.operation_id)
              AND x.kind = 'cancelled_never_sent'
        )
    WHERE f.engagement_id = $1 AND f.campaign_id = $2
    GROUP BY f.epoch, f.operation_id
$$;

CREATE OR REPLACE FUNCTION execution.inference_capacity_snapshot(e uuid, c uuid, d uuid)
RETURNS jsonb LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$
    WITH prot AS (SELECT execution.https_loop_protected_held($1, $2, $3, false) AS p)
    SELECT jsonb_build_object(
        'version', 'm1_inference_capacity_snapshot_v2',
        'epoch', f.epoch,
        'session_claimed', f.operation_id IS NOT NULL,
        'active_holds', count(h.operation_id) FILTER (WHERE NOT EXISTS (
            SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = ($1, $2, h.operation_id))),
        'charged', jsonb_build_object(
            'calls', coalesce(sum((h.capacity->>'calls')::bigint), 0) + (SELECT (p->>'calls')::bigint FROM prot),
            'request_bytes', coalesce(sum((h.capacity->>'request_bytes')::bigint), 0) + (SELECT (p->>'request_bytes')::bigint FROM prot),
            'response_bytes', coalesce(sum((h.capacity->>'response_bytes')::bigint), 0) + (SELECT (p->>'response_bytes')::bigint FROM prot),
            'output_token_capacity', coalesce(sum((h.capacity->>'output_token_capacity')::bigint), 0) + (SELECT (p->>'output_token_capacity')::bigint FROM prot),
            'wall_time_ms', coalesce(sum((h.capacity->>'wall_time_ms')::bigint), 0) + (SELECT (p->>'wall_time_ms')::bigint FROM prot)
        )
    )
    FROM execution.session_fences f
    LEFT JOIN execution.inference_reservations h
        ON h.engagement_id = $1 AND h.campaign_id = $2 AND h.record_revision = 1
        AND NOT EXISTS (
            SELECT 1 FROM execution.inference_reservations x
            WHERE (x.engagement_id, x.campaign_id, x.operation_id) = ($1, $2, h.operation_id)
              AND x.kind = 'cancelled_never_sent'
        )
    WHERE f.engagement_id = $1 AND f.campaign_id = $2
    GROUP BY f.epoch, f.operation_id
$$;

REVOKE ALL ON FUNCTION execution.inference_capacity_snapshot(uuid, uuid, uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.inference_capacity_snapshot(uuid, uuid, uuid) TO dw_m1_broker;

-- 6. Profile matching & reservation writer with protected capacity enforcement.
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
        AND (p->>'run_profile', p->>'execution_mode') = ('m1_https_loop_v1', 'controlled_fixture')
        AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback', 'admitted_repair_fields_codes']
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
    prior execution.inference_reservations;
    source jsonb; consent jsonb; totals jsonb; limits jsonb;
    cap jsonb; held jsonb; prot jsonb;
    actor oid; next_generation bigint; now_s bigint; pro_v2 boolean; auto_ver boolean;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER')
       OR action NOT IN ('reserve', 'cancel_never_sent')
       OR op IS NULL OR op = '00000000-0000-0000-0000-000000000000'
       OR q IS NULL OR jsonb_typeof(q) <> 'object' THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_request_invalid';
    END IF;
    IF q->>'version' IS DISTINCT FROM 'm1_inference_reservation_request_v1'
       OR (SELECT count(*) FROM jsonb_object_keys(q)) <> 8
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['engagement_id','campaign_id','operator_ref','permission_id','goal_ref']) k
           WHERE jsonb_typeof(q->k) IS DISTINCT FROM 'string' OR (q->>k)::uuid = '00000000-0000-0000-0000-000000000000')
       OR jsonb_typeof(q->'expected_mission_revision') IS DISTINCT FROM 'number'
       OR q->>'expected_mission_revision' <> '1'
       OR q->>'exercise_mode' NOT IN ('blind', 'defender_informed') THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_request_invalid';
    END IF;
    e := (q->>'engagement_id')::uuid; c := (q->>'campaign_id')::uuid;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF actor IS NULL THEN RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_request_invalid'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = e AND campaign_id = c;

    SELECT * INTO prior FROM execution.inference_reservations x
    WHERE x.engagement_id = e AND x.campaign_id = c AND x.operation_id = op ORDER BY x.record_revision DESC LIMIT 1;
    IF action = 'cancel_never_sent' THEN
        IF NOT FOUND OR prior.request <> q OR prior.writer_oid <> actor THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_cancel_refused';
        END IF;
        IF EXISTS (SELECT 1 FROM execution.inference_calls x WHERE (x.e, x.c, x.d) = (e, c, op) AND x.state <> 'failed_never_released') THEN
            RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_cancel_refused';
        END IF;
        IF prior.record_revision = 2 THEN RETURN prior; END IF;
        prior.record_revision := 2; prior.kind := 'cancelled_never_sent'; prior.recorded_at := now();
        INSERT INTO execution.inference_reservations SELECT prior.* RETURNING * INTO prior;
        RETURN prior;
    END IF;
    IF FOUND THEN
        IF prior.record_revision = 1 AND prior.request = q AND prior.writer_oid = actor THEN RETURN prior; END IF;
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_operation_conflict';
    END IF;
    source := mission.current_inference_publication(e, c, op);
    IF source IS NULL OR source IS DISTINCT FROM expected_source THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_mission_unavailable';
    END IF;
    consent := source->'fields'->'m1_inference_permission';
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    IF consent IS NULL OR jsonb_typeof(consent) <> 'object'
       OR (consent->>'permission_id')::uuid IS DISTINCT FROM (q->>'permission_id')::uuid
       OR (source->'fields'->>'starts_at')::bigint > now_s OR now_s >= (source->'fields'->>'ends_at')::bigint
       OR (consent->>'starts_at')::bigint > now_s OR now_s >= (consent->>'ends_at')::bigint
       OR NOT execution.inference_profile_matches(consent)
       OR consent->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_consent_refused';
    END IF;
    IF (consent->>'provider_terms_ref')::uuid = '00000000-0000-0000-0000-000000000000'
       OR (consent->'engagement_id', consent->'campaign_id', consent->'operator_ref', consent->'authority_ref',
           consent->'authority_revision', consent->'goal_ref', consent->'exercise_mode', consent->'mission_revision')
          IS DISTINCT FROM (source->'engagement_id', source->'campaign_id', source->'fields'->'operator_ref',
           source->'fields'->'authority_ref', source->'fields'->'authority_revision', source->'fields'->'goal_ref',
           source->'fields'->'exercise_mode', '1'::jsonb)
       OR (consent->>'starts_at')::bigint < (source->'fields'->>'starts_at')::bigint
       OR (consent->>'ends_at')::bigint > (source->'fields'->>'ends_at')::bigint THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_consent_refused';
    END IF;
    limits := consent->'operation_limits'; totals := consent->'campaign_limits';
    auto_ver := consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1');
    pro_v2 := coalesce(consent->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1')
       AND consent->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2'), false);
    IF (SELECT count(*) FROM jsonb_object_keys(consent)) <> (CASE WHEN auto_ver THEN CASE WHEN consent->>'version' = 'm1_inference_permission_v6' THEN 22 ELSE 21 END ELSE 19 END)
       OR jsonb_typeof(consent->'provider_terms_ref') IS DISTINCT FROM 'string' OR (consent->>'provider_terms_ref')::uuid IS NULL
       OR jsonb_typeof(consent->'field_classes') IS DISTINCT FROM 'array'
       OR jsonb_array_length(consent->'field_classes') NOT BETWEEN 1 AND (CASE WHEN auto_ver THEN 9 ELSE 8 END)
       OR (SELECT count(DISTINCT v) FROM jsonb_array_elements(consent->'field_classes') v) <> jsonb_array_length(consent->'field_classes')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(consent->'field_classes') v
           WHERE v NOT IN ('mission_question_constraints','scoped_opaque_references','owner_qualified_summaries','admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes')
             AND NOT (auto_ver AND v = 'admitted_campaign_decision_feedback'))
       OR (SELECT count(*) FROM jsonb_object_keys(limits)) <> (CASE WHEN auto_ver THEN 22 ELSE 16 END)
       OR (SELECT count(*) FROM jsonb_object_keys(totals)) <> 5 THEN
        RAISE EXCEPTION USING ERRCODE = 'P0001', MESSAGE = 'inference_consent_refused';
    END IF;
    IF EXISTS (SELECT 1 FROM (VALUES ('request_bytes',1,16384),('response_bytes',1,65536),
        ('decision_wall_time_ms',1,CASE WHEN pro_v2 THEN 249000 WHEN consent->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6','m1_https_loop_v1') THEN 129000 WHEN consent->>'version'='m1_inference_permission_v3' THEN 45000 ELSE 20000 END),
        ('context_bytes',1,CASE WHEN auto_ver THEN 8192 ELSE 4096 END),('candidates',1,8),('configuration_bytes',1,16384),
        ('connect_time_ms',1,5000),('proposal_bytes',1,8192),('repair_bytes',1,2048),('text_field_bytes',1,1024),
        ('repair_error_codes',1,8),('json_depth',1,CASE WHEN consent->>'version'='m1_inference_permission_v1' THEN 8 ELSE 32 END),
        ('collection_entries',1,64),('calls_per_decision',2,2),('output_tokens_per_call',CASE WHEN pro_v2 THEN 8192 ELSE 4096 END,CASE WHEN pro_v2 THEN 8192 ELSE 4096 END),('concurrency',1,1)) AS b(name,floor,ceiling)
        WHERE CASE WHEN jsonb_typeof(limits->b.name)='number' AND limits->>b.name ~ '^[0-9]+$' THEN (limits->>b.name)::numeric < b.floor OR (limits->>b.name)::numeric > b.ceiling ELSE true END) THEN
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

-- 7. Write-once private endpoint map binding and Broker read.
CREATE OR REPLACE FUNCTION execution.bind_https_loop_input(
    e uuid, c uuid, d uuid, run_id uuid, input_hash text, p_pack_digest text, private_map jsonb
) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE actor oid; cur execution.native_input_bindings; x execution.autonomous_runs;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER')
       OR input_hash !~ '^[0-9a-f]{64}$' OR p_pack_digest !~ '^[0-9a-f]{64}$' OR input_hash = p_pack_digest
       OR jsonb_typeof(private_map) IS DISTINCT FROM 'object'
       OR private_map->>'version' IS DISTINCT FROM 'm1_https_endpoint_map_v1'
       OR (private_map->>'engagement_id')::uuid IS DISTINCT FROM e
       OR (private_map->>'campaign_id')::uuid IS DISTINCT FROM c
       OR (private_map->>'decision_id')::uuid IS DISTINCT FROM d
       OR (private_map->>'request_id')::uuid IS DISTINCT FROM run_id
       OR private_map->>'pack_digest' IS DISTINCT FROM p_pack_digest
       OR jsonb_typeof(private_map->'bindings') IS DISTINCT FROM 'array'
       OR jsonb_array_length(private_map->'bindings') > 4 THEN
        RAISE EXCEPTION 'inference_input_conflict';
    END IF;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    x := execution.find_decision_run(e, c, d);
    IF x.run_id IS NULL OR x.run_id <> run_id OR x.writer_oid <> actor OR x.request->>'version' <> 'm1_https_loop_v1' THEN
        RAISE EXCEPTION 'inference_input_conflict';
    END IF;
    SELECT * INTO cur FROM execution.native_input_bindings i WHERE (i.e, i.c, i.d) = (e, c, d) FOR UPDATE;
    IF NOT FOUND THEN
        INSERT INTO execution.native_input_bindings
            (e, c, d, input_hash, writer_oid, pack_digest, request_id, private_endpoint_map, map_version)
        VALUES (e, c, d, input_hash, actor, p_pack_digest, run_id, private_map, 'm1_https_endpoint_map_v1');
        RETURN;
    END IF;
    IF cur.writer_oid <> actor OR cur.input_hash <> input_hash THEN
        RAISE EXCEPTION 'inference_input_conflict';
    END IF;
    IF cur.pack_digest IS NULL THEN
        UPDATE execution.native_input_bindings i
        SET pack_digest = p_pack_digest, request_id = run_id,
            private_endpoint_map = private_map, map_version = 'm1_https_endpoint_map_v1'
        WHERE (i.e, i.c, i.d) = (e, c, d);
    ELSIF cur.pack_digest IS DISTINCT FROM p_pack_digest OR cur.request_id IS DISTINCT FROM run_id
       OR cur.private_endpoint_map IS DISTINCT FROM private_map OR cur.map_version IS DISTINCT FROM 'm1_https_endpoint_map_v1' THEN
        RAISE EXCEPTION 'inference_input_conflict';
    END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.read_https_endpoint_map(p_e uuid, p_c uuid, p_d uuid)
RETURNS jsonb LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT i.private_endpoint_map
    FROM execution.native_input_bindings i
    JOIN execution.autonomous_runs x ON (x.e, x.c) = (i.e, i.c)
    WHERE (i.e, i.c, i.d) = (p_e, p_c, p_d)
      AND (p_d IN (x.decision_one, x.decision_two) OR EXISTS (
          SELECT 1 FROM execution.https_loop_steps s WHERE s.run_id = x.run_id AND s.decision_id = p_d
      ))
      AND x.writer_oid = (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
      AND i.writer_oid = x.writer_oid
      AND pg_has_role(session_user, 'dw_m1_broker', 'MEMBER')
$$;

REVOKE ALL ON FUNCTION execution.bind_https_loop_input(uuid, uuid, uuid, uuid, text, text, jsonb),
    execution.read_https_endpoint_map(uuid, uuid, uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.bind_https_loop_input(uuid, uuid, uuid, uuid, text, text, jsonb),
    execution.read_https_endpoint_map(uuid, uuid, uuid) TO dw_m1_broker;

-- 8. Protected next-decision capacity lifecycle (protect -> convert once -> release/unknown).
CREATE OR REPLACE FUNCTION execution.protect_next_decision_capacity(r uuid, step_idx integer)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; s jsonb; p jsonb; totals jsonb; limits jsonb;
    cap jsonb; held jsonb; prot jsonb; cur execution.https_loop_steps;
    next_id uuid; next_phase text; fence_epoch bigint; n bigint; deadline_rem bigint;
BEGIN
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' OR step_idx NOT BETWEEN 1 AND 3 THEN
        RAISE EXCEPTION 'protected_capacity_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c
    RETURNING epoch INTO fence_epoch;
    s := execution.run_current(r);
    p := s->'fields'->'m1_inference_permission';
    totals := p->'campaign_limits'; limits := p->'operation_limits';
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    deadline_rem := x.deadline_at_ms - n;
    IF deadline_rem < 314000 THEN
        RAISE EXCEPTION 'run_remaining_time_insufficient';
    END IF;
    SELECT * INTO cur FROM execution.https_loop_steps st WHERE st.run_id = r AND st.step_index = step_idx FOR UPDATE;
    IF NOT FOUND OR cur.protection_state <> 'none' THEN
        RAISE EXCEPTION 'protected_capacity_conflict';
    END IF;
    cap := jsonb_build_object(
        'calls', 2,
        'request_bytes', 2 * (limits->>'request_bytes')::bigint,
        'response_bytes', 2 * (limits->>'response_bytes')::bigint,
        'output_token_capacity', 2 * (limits->>'output_tokens_per_call')::bigint,
        'wall_time_ms', (limits->>'decision_wall_time_ms')::bigint
    );
    prot := execution.https_loop_protected_held(x.e, x.c, NULL, false);
    SELECT jsonb_object_agg(k, used) INTO held FROM (
        SELECT v.key AS k, coalesce(sum((h.capacity->>v.key)::numeric), 0) + (prot->>v.key)::numeric AS used
        FROM jsonb_each(cap) v
        LEFT JOIN execution.inference_reservations h
          ON h.engagement_id = x.e AND h.campaign_id = x.c AND h.record_revision = 1
          AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations ir WHERE (ir.engagement_id, ir.campaign_id, ir.operation_id) = (x.e, x.c, h.operation_id) AND ir.kind = 'cancelled_never_sent')
        GROUP BY v.key, prot
    ) sub;
    IF EXISTS (SELECT 1 FROM jsonb_each(cap) v WHERE (held->>v.key)::numeric + (v.value::text)::numeric > (totals->>v.key)::numeric) THEN
        RAISE EXCEPTION 'inference_capacity_exhausted';
    END IF;
    next_id := CASE WHEN step_idx = 1 THEN x.decision_two ELSE gen_random_uuid() END;
    next_phase := CASE step_idx + 1 WHEN 2 THEN 'decision_two' WHEN 3 THEN 'decision_three' WHEN 4 THEN 'decision_four' END;
    INSERT INTO execution.https_loop_steps
        (run_id, e, c, step_index, decision_id, next_decision_id, decision_phase, acquisition_phase, status, protection_state, updated_at_ms)
    VALUES (
        r, x.e, x.c, step_idx + 1, next_id, NULL, next_phase,
        CASE WHEN step_idx + 1 < 4 THEN CASE step_idx + 1 WHEN 2 THEN 'acquisition_two' WHEN 3 THEN 'acquisition_three' END ELSE NULL END,
        'pending', 'none', n
    )
    ON CONFLICT (run_id, step_index) DO NOTHING;
    SELECT decision_id INTO next_id FROM execution.https_loop_steps WHERE run_id = r AND step_index = step_idx + 1;
    UPDATE execution.https_loop_steps st
    SET next_decision_id = next_id, protection_state = 'protected',
        protected_capacity = cap, protection_epoch = fence_epoch, updated_at_ms = n
    WHERE st.run_id = r AND st.step_index = step_idx;
    RETURN jsonb_build_object('next_decision_id', next_id, 'protection_state', 'protected', 'capacity', cap);
END $$;

CREATE OR REPLACE FUNCTION execution.convert_protected_capacity(e uuid, c uuid, d uuid)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE s execution.https_loop_steps; next_s execution.https_loop_steps; x execution.autonomous_runs; actor oid; n bigint;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') THEN
        RAISE EXCEPTION 'protected_capacity_unavailable';
    END IF;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    SELECT * INTO s FROM execution.https_loop_steps st WHERE (st.e, st.c, st.next_decision_id) = (e, c, d) FOR UPDATE;
    IF NOT FOUND THEN
        RETURN 'not_applicable';
    END IF;
    IF s.protection_state = 'converted' THEN
        RETURN 'already_converted';
    END IF;
    IF s.protection_state <> 'protected' THEN
        RAISE EXCEPTION 'protected_capacity_unavailable';
    END IF;
    SELECT * INTO next_s FROM execution.https_loop_steps st WHERE st.run_id = s.run_id AND st.decision_id = d;
    SELECT * INTO x FROM execution.autonomous_runs ar WHERE ar.run_id = s.run_id;
    IF x.writer_oid <> actor THEN RAISE EXCEPTION 'protected_capacity_unavailable'; END IF;
    PERFORM execution.run_current(s.run_id);
    IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control ac WHERE ac.run_id = s.run_id AND ac.phase = next_s.decision_phase AND ac.claimed)
       OR NOT EXISTS (SELECT 1 FROM execution.native_input_bindings i WHERE (i.e, i.c, i.d, i.writer_oid) = (e, c, d, actor) AND i.pack_digest IS NOT NULL) THEN
        RAISE EXCEPTION 'protected_capacity_unavailable';
    END IF;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    UPDATE execution.https_loop_steps st
    SET protection_state = 'converted', updated_at_ms = n
    WHERE st.run_id = s.run_id AND st.step_index = s.step_index;
    RETURN 'converted';
END $$;

CREATE OR REPLACE FUNCTION execution.release_protected_capacity(r uuid, step_idx integer, disposition text)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; s execution.https_loop_steps; actor oid; n bigint;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR disposition NOT IN ('released', 'unknown') THEN
        RAISE EXCEPTION 'protected_capacity_unavailable';
    END IF;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND OR x.writer_oid <> actor THEN RAISE EXCEPTION 'protected_capacity_unavailable'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    SELECT * INTO s FROM execution.https_loop_steps st WHERE st.run_id = r AND st.step_index = step_idx FOR UPDATE;
    IF NOT FOUND OR s.protection_state IN ('none', 'released') THEN RETURN; END IF;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF disposition = 'released' THEN
        IF s.protection_state <> 'protected'
           OR EXISTS (SELECT 1 FROM execution.inference_reservations ir WHERE (ir.engagement_id, ir.campaign_id, ir.operation_id) = (x.e, x.c, s.next_decision_id))
           OR EXISTS (SELECT 1 FROM execution.autonomous_phases ap WHERE ap.run_id = r AND ap.phase = (SELECT decision_phase FROM execution.https_loop_steps WHERE run_id = r AND step_index = step_idx + 1)) THEN
            RAISE EXCEPTION 'protected_capacity_unavailable';
        END IF;
        UPDATE execution.https_loop_steps SET protection_state = 'released', updated_at_ms = n WHERE run_id = r AND step_index = step_idx;
    ELSE
        IF s.protection_state IN ('protected', 'converted') AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations ir WHERE (ir.engagement_id, ir.campaign_id, ir.operation_id) = (x.e, x.c, s.next_decision_id)) THEN
            UPDATE execution.https_loop_steps SET protection_state = 'unknown', updated_at_ms = n WHERE run_id = r AND step_index = step_idx;
        END IF;
    END IF;
END $$;

REVOKE ALL ON FUNCTION execution.protect_next_decision_capacity(uuid, integer),
    execution.convert_protected_capacity(uuid, uuid, uuid),
    execution.release_protected_capacity(uuid, integer, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.protect_next_decision_capacity(uuid, integer),
    execution.convert_protected_capacity(uuid, uuid, uuid),
    execution.release_protected_capacity(uuid, integer, text) TO dw_m1_broker;

-- 9. Run lifecycle functions extended for `m1_https_loop_v1` indexed phases.
CREATE OR REPLACE FUNCTION execution.run_current(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; y execution.autonomous_control; s jsonb; p jsonb; n bigint;
BEGIN
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT * INTO y FROM execution.autonomous_control WHERE run_id = r;
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF x.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname = session_user)
       OR NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR y.status IS DISTINCT FROM 'running'
       OR n >= least(x.deadline_at_ms, y.lease_until_ms) THEN
        RAISE EXCEPTION 'run_not_current';
    END IF;
    s := mission.current_inference_publication(x.e, x.c, x.decision_one); p := s->'fields'->'m1_inference_permission';
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
       OR (x.request->>'version' = 'm1_autonomous_run_request_v4' AND (
           x.request->'live_binding' IS DISTINCT FROM p->'live_binding'
           OR NOT EXISTS (SELECT 1 FROM execution.live_qualifications z WHERE z.qualification_ref = x.qualification_ref
               AND n >= z.valid_from_ms AND n < z.valid_until_ms AND z.release_sha256 = x.release_sha256
               AND z.run_profile = x.request->>'run_profile' AND host(z.resolver_ipv4) = x.request#>>'{live_binding,resolver_ipv4}'
               AND z.purpose = CASE x.request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END)))
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
DECLARE x execution.autonomous_runs; r uuid; e uuid; c uuid; n bigint; s jsonb; p jsonb; deadline bigint; pro_v2 boolean; hl boolean; min_cap numeric;
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
           ('m1_https_loop_v1','m1_https_loop_v1')
       ), false)
       OR (q->>'version' = 'm1_autonomous_run_request_v1' AND q ? 'inference_profile')
       OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') AND jsonb_typeof(q->'inference_profile') IS DISTINCT FROM 'string')
       OR (q->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4' AND q ? 'live_binding')
       OR (q->>'version' = 'm1_autonomous_run_request_v4' AND (jsonb_typeof(q->'live_binding') IS DISTINCT FROM 'object'
           OR NOT q->'live_binding' ?& ARRAY['qualification_ref','release_sha256','resolver_ipv4']
           OR (q->'live_binding') - ARRAY['qualification_ref','release_sha256','resolver_ipv4'] <> '{}'::jsonb))
       OR q->>'execution_mode' IS DISTINCT FROM (CASE WHEN q->>'version' = 'm1_autonomous_run_request_v4' THEN 'live' ELSE 'controlled_fixture' END)
       OR jsonb_typeof(q->'seed_names') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'seed_names') NOT BETWEEN 1 AND 8
       OR jsonb_array_length(q#>'{context,candidates}') <> jsonb_array_length(q->'seed_names') THEN
        RAISE EXCEPTION 'run_request_refused';
    END IF;
    hl := q->>'version' = 'm1_https_loop_v1';
    IF hl AND q->>'inference_profile' IS DISTINCT FROM 'fixture-genai-v4-pro-low-v2' THEN
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
    r := (q->>'run_id')::uuid; e := (q#>>'{context,mission,engagement_id}')::uuid; c := (q#>>'{context,mission,campaign_id}')::uuid;
    IF '00000000-0000-0000-0000-000000000000'::uuid IN (r, e, c) THEN RAISE EXCEPTION 'run_request_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r OR (autonomous_runs.e, autonomous_runs.c) = (e, c);
    IF FOUND THEN
        IF x.run_id <> r OR x.request IS DISTINCT FROM q OR x.release_sha256 <> release
           OR x.writer_oid <> (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user) THEN
            RAISE EXCEPTION 'run_identity_conflict';
        END IF;
        RETURN false;
    END IF;
    pro_v2 := q->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1')
        AND q->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2');
    n := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint; deadline := (q->>'deadline_at_ms')::bigint;
    IF jsonb_typeof(q->'deadline_at_ms') IS DISTINCT FROM 'number' OR q->>'deadline_at_ms' !~ '^[1-9][0-9]*$'
       OR deadline <= n OR deadline > n + (CASE WHEN pro_v2 THEN 640000 WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') THEN 400000 ELSE 150000 END) THEN
        RAISE EXCEPTION 'run_deadline_refused';
    END IF;
    s := mission.current_inference_publication(e, c, r); p := s->'fields'->'m1_inference_permission';
    IF s IS NULL OR NOT execution.inference_profile_matches(p)
       OR (p->>'version', p->>'run_profile', p->>'execution_mode') IS DISTINCT FROM (
           CASE WHEN hl THEN 'm1_https_loop_v1' WHEN q->>'version' = 'm1_autonomous_run_request_v4' THEN 'm1_inference_permission_v6' WHEN q->>'version' = 'm1_autonomous_run_request_v3' THEN 'm1_inference_permission_v5' WHEN q->>'version' = 'm1_autonomous_run_request_v2' THEN 'm1_inference_permission_v4' ELSE 'm1_inference_permission_v3' END,
           q->>'run_profile', q->>'execution_mode')
       OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1') AND q->>'inference_profile' IS DISTINCT FROM p->>'profile')
       OR (q->>'version' = 'm1_autonomous_run_request_v4' AND (q->'live_binding' IS DISTINCT FROM p->'live_binding' OR q#>>'{live_binding,release_sha256}' IS DISTINCT FROM release
           OR NOT EXISTS (SELECT 1 FROM execution.live_qualifications z WHERE z.qualification_ref = (q#>>'{live_binding,qualification_ref}')::uuid
               AND z.run_profile = q->>'run_profile' AND z.release_sha256 = release AND host(z.resolver_ipv4) = q#>>'{live_binding,resolver_ipv4}'
               AND n >= z.valid_from_ms AND n < z.valid_until_ms AND z.purpose = CASE q->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END)))
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
        release, CASE WHEN q->>'version' = 'm1_autonomous_run_request_v4' THEN (q#>>'{live_binding,qualification_ref}')::uuid ELSE gen_random_uuid() END, deadline
    ) RETURNING * INTO x;
    INSERT INTO execution.autonomous_control VALUES (r, least(n + 2000, deadline), 'running', NULL, false);
    IF hl THEN
        INSERT INTO execution.https_loop_steps (run_id, e, c, step_index, decision_id, next_decision_id, decision_phase, acquisition_phase, status, protection_state, updated_at_ms)
        VALUES (r, e, c, 1, x.decision_one, NULL, 'decision_one', 'acquisition_one', 'pending', 'none', n);
    END IF;
    PERFORM execution.run_current(r);
    RETURN true;
END $$;

CREATE OR REPLACE FUNCTION execution.run_phase_start(r uuid, wanted text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE previous text; x execution.autonomous_runs; expected_prev text; hl boolean;
BEGIN
    PERFORM execution.run_current(r);
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    hl := x.request->>'version' = 'm1_https_loop_v1';
    SELECT phase INTO previous FROM execution.autonomous_control WHERE run_id = r FOR UPDATE;
    IF hl THEN
        expected_prev := CASE wanted
            WHEN 'decision_one' THEN NULL WHEN 'acquisition_one' THEN 'decision_one'
            WHEN 'decision_two' THEN 'acquisition_one' WHEN 'acquisition_two' THEN 'decision_two'
            WHEN 'decision_three' THEN 'acquisition_two' WHEN 'acquisition_three' THEN 'decision_three'
            WHEN 'decision_four' THEN 'acquisition_three' ELSE '__invalid__' END;
        IF expected_prev = '__invalid__' OR previous IS DISTINCT FROM expected_prev
           OR (previous IS NOT NULL AND NOT EXISTS (SELECT 1 FROM execution.autonomous_phases WHERE run_id = r AND phase = previous AND status = 'completed')) THEN
            RAISE EXCEPTION 'run_phase_refused';
        END IF;
        UPDATE execution.https_loop_steps
        SET status = CASE WHEN wanted LIKE 'acquisition%' THEN 'acquisition_running' ELSE 'decision_running' END,
            updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
        WHERE run_id = r AND (decision_phase = wanted OR acquisition_phase = wanted);
    ELSIF NOT ((previous IS NULL AND wanted = 'decision_one') OR (previous = 'decision_one' AND wanted = 'acquisition') OR (previous = 'acquisition' AND wanted = 'decision_two'))
       OR (previous IS NOT NULL AND NOT EXISTS (SELECT 1 FROM execution.autonomous_phases WHERE run_id = r AND phase = previous AND status = 'completed')) THEN
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    UPDATE execution.autonomous_control SET phase = wanted, claimed = false WHERE run_id = r;
    INSERT INTO execution.autonomous_phases VALUES (r, wanted, 'started', floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint);
END $$;

CREATE OR REPLACE FUNCTION execution.run_phase_complete(r uuid, wanted text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; d uuid; st execution.https_loop_steps;
BEGIN
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    PERFORM execution.run_current(r);
    IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control WHERE run_id = r AND phase = wanted AND claimed) THEN
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    IF x.request->>'version' = 'm1_https_loop_v1' THEN
        SELECT * INTO st FROM execution.https_loop_steps s WHERE s.run_id = r AND (s.decision_phase = wanted OR s.acquisition_phase = wanted);
        IF NOT FOUND THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
        IF wanted LIKE 'decision%' THEN
            IF NOT EXISTS (SELECT 1 FROM execution.inference_closures z WHERE (z.e, z.c, z.d) = (x.e, x.c, st.decision_id)) THEN
                RAISE EXCEPTION 'run_inference_pending';
            END IF;
            UPDATE execution.https_loop_steps SET status = CASE WHEN st.step_index = 4 THEN 'completed' ELSE 'decision_completed' END,
                updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint WHERE run_id = r AND step_index = st.step_index;
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
        IF NOT EXISTS (SELECT 1 FROM execution.live_dns_results b JOIN execution.live_effect_control xc USING (run_id)
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
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE x execution.autonomous_runs;
BEGIN
    IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    IF NOT EXISTS (SELECT 1 FROM execution.autonomous_control WHERE run_id = r AND phase = wanted) THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
    IF EXISTS (SELECT 1 FROM execution.autonomous_phases WHERE run_id = r AND phase = wanted AND status = 'completed') THEN RETURN; END IF;
    INSERT INTO execution.autonomous_phases VALUES (r, wanted, 'unknown', floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint) ON CONFLICT DO NOTHING;
    UPDATE execution.autonomous_control SET status = 'unknown', lease_until_ms = 0 WHERE run_id = r AND status = 'running';
    UPDATE execution.https_loop_steps SET status = 'unknown' WHERE run_id = r AND (decision_phase = wanted OR acquisition_phase = wanted) AND status NOT IN ('completed', 'terminal');
    IF x.request->>'version' = 'm1_autonomous_run_request_v4' THEN PERFORM execution.close_live_session(r, 'unknown');
    ELSIF x.request->>'version' = 'm1_https_loop_v1' THEN PERFORM execution.close_https_loop_effect_session(r, 'unknown');
    ELSE PERFORM execution.close_fixture_session(r, 'unknown'); END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.run_stop(r uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE x execution.autonomous_runs;
BEGIN
    IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    UPDATE execution.autonomous_control SET status = 'stopped', lease_until_ms = 0 WHERE run_id = r AND status = 'running';
    UPDATE execution.https_loop_steps SET status = 'stopped' WHERE run_id = r AND status IN ('pending', 'decision_running', 'acquisition_pending', 'acquisition_running');
    IF x.request->>'version' = 'm1_autonomous_run_request_v4' THEN PERFORM execution.close_live_session(r, 'stopped');
    ELSIF x.request->>'version' = 'm1_https_loop_v1' THEN PERFORM execution.close_https_loop_effect_session(r, 'stopped');
    ELSE PERFORM execution.close_fixture_session(r, 'stopped'); END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.run_phase_diagnostic(q jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
DECLARE r uuid; p text; old jsonb; elapsed_max numeric;
BEGIN
    IF jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text) > 1024 THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
    IF NOT q ?& ARRAY['version','run_id','phase','cause','cause_source','child_error','exit_code','readiness_observed','elapsed_ms']
       OR q - ARRAY['version','run_id','phase','cause','cause_source','child_error','exit_code','readiness_observed','elapsed_ms','reported_guard'] <> '{}'::jsonb
       OR coalesce(q->>'version', '') NOT IN ('m1_run_phase_diagnostic_v1', 'm1_run_phase_diagnostic_v2')
       OR (q->>'version' = 'm1_run_phase_diagnostic_v1' AND q ? 'reported_guard')
       OR (q ? 'reported_guard' AND (jsonb_typeof(q->'reported_guard') IS DISTINCT FROM 'string' OR q->>'reported_guard' NOT IN ('transport_hard_deadline','decision_envelope','permit_expiry','command_watchdog')))
       OR jsonb_typeof(q->'run_id') IS DISTINCT FROM 'string' OR jsonb_typeof(q->'phase') IS DISTINCT FROM 'string'
       OR q->>'phase' NOT IN ('decision_one','acquisition','decision_two','acquisition_one','acquisition_two','decision_three','acquisition_three','decision_four')
       OR jsonb_typeof(q->'cause') IS DISTINCT FROM 'string'
       OR q->>'cause' NOT IN ('child_error','child_exit','readiness_refused','readiness_deadline','phase_deadline','local_stop','supervisor_unavailable','unavailable')
       OR q->>'cause_source' IS DISTINCT FROM (CASE q->>'cause' WHEN 'child_error' THEN 'child_report' WHEN 'unavailable' THEN 'unavailable' ELSE 'parent_observed' END)
       OR jsonb_typeof(q->'readiness_observed') IS DISTINCT FROM 'boolean' THEN
        RAISE EXCEPTION 'run_diagnostic_refused';
    END IF;
    IF q->'child_error' <> 'null'::jsonb AND (jsonb_typeof(q->'child_error') IS DISTINCT FROM 'string' OR q->>'child_error' NOT IN (
        'missing_env','invalid_database_config_mode','database_config_conflict','missing_database_url_file','database_file_platform_unqualified',
        'invalid_dsn','non_loopback_host','missing_credentials','connect_failed','unqualified_database_waits','run_control_unqualified',
        'run_control_stopped','run_child_refused','inference_authority_unavailable','run_inference_pending','child_failed'
    )) OR (q->>'cause' = 'child_error' AND q->'child_error' = 'null'::jsonb) THEN
        RAISE EXCEPTION 'run_diagnostic_refused';
    END IF;
    IF q->'exit_code' <> 'null'::jsonb THEN
        IF jsonb_typeof(q->'exit_code') IS DISTINCT FROM 'number' OR q->>'exit_code' !~ '^-?(0|[1-9][0-9]*)$'
           OR (q->>'exit_code')::numeric NOT BETWEEN -2147483648 AND 2147483647 THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
    END IF;
    r := (q->>'run_id')::uuid; p := q->>'phase';
    elapsed_max:=CASE WHEN q->>'version'='m1_run_phase_diagnostic_v2'
        AND EXISTS(SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id=r
            AND x.request->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1')
            AND x.request->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2'))
        THEN 650000 ELSE 150000 END;
    IF q->'elapsed_ms' <> 'null'::jsonb THEN
        IF jsonb_typeof(q->'elapsed_ms') IS DISTINCT FROM 'number' OR q->>'elapsed_ms' !~ '^(0|[1-9][0-9]*)$'
           OR (q->>'elapsed_ms')::numeric NOT BETWEEN 0 AND elapsed_max THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
    END IF;
    IF execution.run_record(r) IS NULL
       OR (q->>'version' = 'm1_run_phase_diagnostic_v2' AND NOT EXISTS (SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id = r AND x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4','m1_https_loop_v1')))
       OR (p IN ('acquisition_one','acquisition_two','decision_three','acquisition_three','decision_four') AND NOT EXISTS (SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id = r AND x.request->>'version' = 'm1_https_loop_v1'))
       OR NOT EXISTS (SELECT 1 FROM execution.autonomous_phases WHERE run_id = r AND phase = p AND status = 'started') THEN
        RAISE EXCEPTION 'run_diagnostic_writer_refused';
    END IF;
    INSERT INTO execution.run_phase_diagnostics VALUES (r, p, q) ON CONFLICT DO NOTHING;
    SELECT payload INTO old FROM execution.run_phase_diagnostics WHERE run_id = r AND phase = p;
    IF old IS DISTINCT FROM q THEN RAISE EXCEPTION 'run_diagnostic_conflict'; END IF;
END $$;


-- 10. Successor inference window, schema, binding, pack, terrain, and may-send gates.
CREATE OR REPLACE FUNCTION execution.inference_window_ms(e uuid, c uuid, d uuid) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    s jsonb; p jsonb; n bigint; l jsonb; run uuid; phase text;
    started bigint; phase_end bigint; decision_end bigint;
    w execution.inference_decision_windows; ar execution.autonomous_runs;
    lq execution.live_qualifications; pro_v2 boolean;
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
    IF p->>'version' = 'm1_inference_permission_v6' THEN
        SELECT * INTO lq FROM execution.live_qualifications z WHERE z.qualification_ref = ar.qualification_ref;
        IF NOT FOUND OR n < lq.valid_from_ms OR n >= lq.valid_until_ms
           OR ar.qualification_ref IS DISTINCT FROM (ar.request#>>'{live_binding,qualification_ref}')::uuid
           OR ar.request#>>'{live_binding,release_sha256}' IS DISTINCT FROM ar.release_sha256
           OR lq.run_profile IS DISTINCT FROM (ar.request->>'run_profile')
           OR lq.purpose IS DISTINCT FROM (CASE ar.request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END)
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

CREATE OR REPLACE FUNCTION execution.genai_https_loop_schema(p jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog AS $$
DECLARE
    ids jsonb; refs jsonb; manifests jsonb; properties jsonb; variants jsonb := '[]';
    fields jsonb; view_schema jsonb; required jsonb; category text; name text;
    refresh_schema jsonb; manifest_schema jsonb;
BEGIN
    SELECT coalesce(jsonb_agg(v->'id' ORDER BY n), '[]') INTO ids
    FROM jsonb_array_elements(p->'references') WITH ORDINALITY a(v, n);
    refs := jsonb_build_object('type', 'array', 'items', CASE WHEN jsonb_array_length(ids) = 0
        THEN '{"type":"string"}'::jsonb ELSE jsonb_build_object('type', 'string', 'enum', ids) END,
        'maxItems', CASE WHEN jsonb_array_length(ids) = 0 THEN 0 ELSE 64 END);
    SELECT coalesce(jsonb_agg(v->'reference' ORDER BY n), '[]') INTO manifests
    FROM jsonb_array_elements(p->'manifests') WITH ORDINALITY a(v, n);
    view_schema := jsonb_build_object('type', 'object', 'additionalProperties', false, 'required', jsonb_build_array('owner', 'selectors'),
        'properties', jsonb_build_object('owner', '{"type":"string","enum":["cyber_terrain","foothold_graph","attack_path_view","objective_state","campaign_trajectory"]}'::jsonb, 'selectors', refs));
    FOREACH category IN ARRAY ARRAY['validate', 'observe_more', 'synthesize_candidate_implementation', 'retain', 'defer', 'stop'] LOOP
        refresh_schema := CASE WHEN category = 'validate'
            THEN '{"type":"string","enum":["none","dns_once_if_stale"]}'::jsonb
            ELSE '{"type":"string","enum":["none"]}'::jsonb END;
        manifest_schema := CASE
            WHEN category = 'validate' AND manifests @> '["https-head-v1"]'::jsonb
                THEN '{"anyOf":[{"type":"null"},{"type":"string","enum":["https-head-v1"]}]}'::jsonb
            WHEN category = 'observe_more' AND manifests @> '["dns_udp"]'::jsonb
                THEN '{"anyOf":[{"type":"null"},{"type":"string","enum":["dns_udp"]}]}'::jsonb
            ELSE '{"type":"null"}'::jsonb
        END;
        fields := jsonb_build_object(
            'category', jsonb_build_object('type', 'string', 'enum', jsonb_build_array(category)),
            'references', refs,
            'manifest_reference', manifest_schema,
            'dependency_refresh', refresh_schema
        );
        required := '["category","dependency_refresh","manifest_reference","references"]'::jsonb;
        IF category = 'observe_more' THEN
            fields := fields || jsonb_build_object('view_request', jsonb_build_object('anyOf', jsonb_build_array('{"type":"null"}'::jsonb, view_schema)));
            required := '["category","dependency_refresh","manifest_reference","references","view_request"]'::jsonb;
        END IF;
        variants := variants || jsonb_build_array(jsonb_build_object('type', 'object', 'additionalProperties', false, 'required', required, 'properties', fields));
    END LOOP;
    properties := jsonb_build_object('version', '{"type":"string","enum":["m1_https_loop_v1"]}'::jsonb,
        'references', refs, 'next_intent', jsonb_build_object('anyOf', variants));
    FOREACH name IN ARRAY ARRAY['hypothesis', 'considered_alternative', 'expected_observation', 'falsifying_observation', 'uncertainty', 'rationale'] LOOP
        properties := properties || jsonb_build_object(name, '{"type":"string","minLength":1,"maxLength":1024}'::jsonb);
    END LOOP;
    SELECT jsonb_agg(key ORDER BY key COLLATE "C") INTO required FROM jsonb_object_keys(properties) a(key);
    RETURN jsonb_build_object('type', 'object', 'additionalProperties', false, 'required', required, 'properties', properties);
END $$;

CREATE OR REPLACE FUNCTION execution.genai_expected_intent(p jsonb, r execution.inference_reservations, k uuid, codes jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog, pg_temp AS $$
DECLARE
    i jsonb; recipient text; pro_v2 boolean; out_tokens integer;
    soft_ms bigint; hard_ms bigint; shared_ms bigint; decision_ms bigint; phase_ms bigint;
    https_loop boolean;
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
        'template_version', CASE WHEN https_loop THEN 'm1-https-loop-v1' ELSE 'm1-autonomous-inference-v3' END,
        'schema_version', CASE WHEN https_loop THEN 'm1_https_loop_v1' ELSE 'm1_inference_proposal_v1' END,
        'timing_profile', CASE WHEN pro_v2 THEN 'reasoning-bounded-pro-v2' ELSE 'reasoning-bounded-v1' END,
        'instructions', CASE WHEN https_loop THEN 'DuskWeave m1_https_loop_v1: propose one objective-led hypothesis, alternative, expected and falsifying observation, uncertainty, rationale and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not permission. Use exact references[].id (asset_N, endpoint_N, terrain_N, previous_decision, mission) and qualified manifests[].reference (dns_udp, https-head-v1) values only. Never invent hostnames, IPv4s, paths, availability, access or scope. validate with https-head-v1 requires one asset_N and its issued endpoint_N, and may set dependency_refresh to none or dns_once_if_stale; all other categories must set dependency_refresh to none. Return concise safe explanation only; output grants no dispatch.'
            ELSE 'DuskWeave autonomous inference v1: propose one objective-led hypothesis, alternative, expected and falsifying observation and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not playbooks or permission. Operator seeds are premises, observations are not facts, and historical decisions do not grant current authority. Use exact references[].id and qualified manifests[].reference values only. observe_more with a DNS manifest proposes one observation for one seed; observe_more with view_request requests only an owner view. Do not combine them. Do not invent endpoints, references, availability, access or scope. Missing, stale, conflicted and fixture sources retain their limits. Explain uncertainty and any retention/revision from predecessor feedback. Retain, defer and stop are valid choices. Synthesis/unsupported effects are explanations of a capability gap, never permission to execute. Return concise safe explanation, never private reasoning or raw client content. Output grants no dispatch. Fixture results cannot establish live target facts.' END,
        'schema', CASE WHEN https_loop THEN execution.genai_https_loop_schema(p) ELSE execution.genai_proposal_schema(p) END,
        'repair_error_codes', codes,
        'settings', jsonb_build_object('reasoning_effort', 'low', 'max_output_tokens', out_tokens, 'stream', false, 'store', false, 'tools', '[]'::jsonb),
        'limits', jsonb_build_object(
            'request_bytes', 16384, 'response_bytes', 65536, 'context_bytes', 8192, 'configuration_bytes', 16384,
            'connect_time_ms', 5000, 'soft_completion_ms', soft_ms, 'per_call_wall_time_ms', hard_ms, 'shared_wall_time_ms', shared_ms,
            'decision_wall_time_ms', decision_ms, 'phase_wall_time_ms', phase_ms, 'cancel_grace_ms', 2000, 'accounting_window_ms', 3000,
            'output_tokens_per_call', out_tokens, 'calls_per_decision', 2, 'concurrency', 1
        )
    );
    IF p->>'version' = 'm1_context_pack_autonomous_v4' THEN
        i := i || jsonb_build_object('live_binding', p->'live_binding', 'qualification_purpose', p->'qualification_purpose');
    END IF;
    RETURN i;
END $$;

CREATE OR REPLACE FUNCTION execution.genai_binding_valid(e uuid, c uuid, d uuid, k uuid, p text, i text, b text, h text) RETURNS boolean
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE r execution.inference_reservations; ar execution.autonomous_runs; request jsonb; permission jsonb; expected jsonb;
BEGIN
    SELECT * INTO r FROM execution.inference_reservations a WHERE (a.engagement_id, a.campaign_id, a.operation_id) = (e, c, d) AND a.record_revision = 1;
    ar := execution.find_decision_run(e, c, d);
    SELECT x.request, m.contract#>'{fields,m1_inference_permission}' INTO request, permission
    FROM execution.autonomous_runs x JOIN mission.registration_outbox m ON (m.engagement_id, m.campaign_id) = (x.e, x.c)
    WHERE x.run_id = ar.run_id;
    IF r.operation_id IS NULL OR request->>'version' NOT IN ('m1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4', 'm1_https_loop_v1')
       OR permission->>'version' IS DISTINCT FROM (CASE request->>'version' WHEN 'm1_https_loop_v1' THEN 'm1_https_loop_v1' WHEN 'm1_autonomous_run_request_v4' THEN 'm1_inference_permission_v6' ELSE 'm1_inference_permission_v5' END)
       OR NOT execution.inference_profile_matches(permission)
       OR request->>'inference_profile' IS DISTINCT FROM permission->>'profile'
       OR p::jsonb->>'inference_profile' IS DISTINCT FROM permission->>'profile'
       OR (request->>'version' = 'm1_autonomous_run_request_v3' AND (request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v2' OR request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'))
       OR (request->>'version' = 'm1_https_loop_v1' AND (request->>'run_profile' IS DISTINCT FROM 'm1_https_loop_v1' OR request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'))
       OR (request->>'version' = 'm1_autonomous_run_request_v4' AND (
           request->>'run_profile' NOT IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1')
           OR request->>'execution_mode' IS DISTINCT FROM 'live'
           OR request->'live_binding' IS DISTINCT FROM permission->'live_binding'
           OR p::jsonb->'live_binding' IS DISTINCT FROM permission->'live_binding'
           OR p::jsonb->>'qualification_purpose' IS DISTINCT FROM CASE request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END
       ))
       OR p::jsonb->>'run_profile' IS DISTINCT FROM request->>'run_profile'
       OR p::jsonb->>'execution_mode' IS DISTINCT FROM request->>'execution_mode'
       OR p::jsonb->'permission_id' IS DISTINCT FROM permission->'permission_id'
       OR octet_length(p) > 8192 OR octet_length(i) > 16384 OR b <> '' THEN
        RETURN false;
    END IF;
    expected := execution.genai_expected_intent(p::jsonb, r, k, i::jsonb->'repair_error_codes');
    RETURN coalesce(i = execution.genai_canonical(expected) AND h = execution.genai_binding_digest(p, i), false);
EXCEPTION WHEN OTHERS THEN
    RETURN false;
END $$;

CREATE OR REPLACE FUNCTION execution.assert_successor_pack(p jsonb, reg jsonb, request jsonb) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path = pg_catalog AS $$
DECLARE
    ref jsonb; candidate jsonb; original jsonb; expected jsonb; ordinal integer;
    https_loop boolean := request->>'version' = 'm1_https_loop_v1';
    keys text[] := ARRAY['version','engagement_id','campaign_id','decision_id','permission_id','mission_revision','goal_ref',
        'exercise_mode','scope','question','evaluated_at','owner_views','references','candidates','omissions','pending_unknown',
        'manifests','remaining_capacity','execution_mode','run_profile','inference_profile','predecessor']
        || CASE WHEN request->>'version' = 'm1_autonomous_run_request_v4' THEN ARRAY['live_binding','qualification_purpose'] ELSE '{}'::text[] END;
    cand_keys text[] := CASE WHEN request->>'version' = 'm1_https_loop_v1'
        THEN ARRAY['reference','priority','status','terrain_references','endpoint_references','https_references']
        ELSE ARRAY['reference','priority','status','terrain_references'] END;
BEGIN
    IF request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4' AND (p ?& ARRAY['live_binding', 'qualification_purpose']) THEN
        RAISE EXCEPTION 'run_pack_source_refused';
    END IF;
    IF request->>'version' = 'm1_autonomous_run_request_v4' AND (
        p->'live_binding' IS DISTINCT FROM reg#>'{fields,m1_inference_permission,live_binding}'
        OR p->>'qualification_purpose' IS DISTINCT FROM CASE request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END
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
                       OR ref->>'source' IS DISTINCT FROM 'fixture_https_acquisition'
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
        expected_manifests := jsonb_build_array(
            jsonb_build_object('availability', 'qualified', 'execution_mode', 'controlled_fixture', 'profile', 'dns-routing-fixture-v1',
                'reference', 'dns_udp', 'schema', 'm1_dns_ipc_v1', 'transport', 'udp'),
            jsonb_build_object('availability', CASE WHEN has_eps THEN 'qualified' ELSE 'requires_dns_premise' END,
                'execution_mode', 'controlled_fixture', 'profile', 'https-head-fixture-v1',
                'reference', 'https-head-v1', 'schema', 'm1_https_head_request_v1', 'transport', 'tcp_tls_http11_head')
        );
        IF p->>'version' IS DISTINCT FROM 'm1_https_loop_v1'
           OR p->>'inference_profile' IS DISTINCT FROM permission->>'profile'
           OR cfg->>'timing_profile' IS DISTINCT FROM expected_timing
           OR (p->>'run_profile', p->>'execution_mode') IS DISTINCT FROM (x.request->>'run_profile', x.request->>'execution_mode')
           OR (cfg->>'run_profile', cfg->>'execution_mode') IS DISTINCT FROM (p->>'run_profile', p->>'execution_mode')
           OR (cfg->>'profile', cfg->>'recipient') IS DISTINCT FROM (permission->>'profile', permission->>'recipient')
           OR (p->>'engagement_id', p->>'campaign_id', p->>'decision_id') IS DISTINCT FROM (e::text, c::text, d::text)
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

CREATE OR REPLACE FUNCTION execution.assert_current_pack_terrain(e uuid, c uuid, d uuid, p jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    source jsonb; ref jsonb; v jsonb; claim jsonb; q jsonb;
    https_loop boolean := p->>'version' = 'm1_https_loop_v1';
    mats jsonb; mat jsonb; hview jsonb; hclaim jsonb; cap_until bigint; exp_fresh text;
BEGIN
    source := mission.current_inference_publication(e, c, d);
    IF source IS NULL THEN RAISE EXCEPTION 'fixture_authority_expired'; END IF;
    FOR ref IN SELECT value FROM jsonb_array_elements(p->'references') WHERE value->>'owner' = 'cyber_terrain' LOOP
        IF https_loop AND coalesce(ref->>'id', '') LIKE 'endpoint_%' THEN
            mats := terrain.current_dns_endpoint_material(e, c, ARRAY[(ref->>'asset_ref')::uuid]);
            SELECT value INTO mat FROM jsonb_array_elements(mats) WHERE value->'claim_id' = ref->'entity';
            IF mat IS NULL OR mat->>'effective_status' IS DISTINCT FROM 'observed'
               OR mat->'conflicted' IS DISTINCT FROM 'false'::jsonb OR mat->'integrity_hold' IS DISTINCT FROM 'false'::jsonb
               OR mat->'claim_revision' IS DISTINCT FROM ref->'parent_revision' THEN
                RAISE EXCEPTION 'fixture_stale_premises';
            END IF;
            cap_until := least((mat->>'valid_until')::bigint, (mat->>'perceived_at')::bigint + least((mat->>'lowest_ttl_seconds')::bigint, 60));
            exp_fresh := CASE WHEN (p->>'evaluated_at')::bigint < cap_until THEN 'fresh' ELSE 'expired' END;
            IF ref->'valid_until' IS DISTINCT FROM to_jsonb(cap_until) OR ref->>'freshness' IS DISTINCT FROM exp_fresh THEN
                RAISE EXCEPTION 'fixture_stale_premises';
            END IF;
        ELSIF https_loop AND coalesce(ref->>'id', '') LIKE 'https_%' THEN
            hview := terrain.current_https_context_view(e, c, ARRAY[(ref->>'asset_ref')::uuid], (p->>'evaluated_at')::bigint);
            SELECT value INTO hclaim FROM jsonb_array_elements(hview) WHERE value->'claim_id' = ref->'entity';
            IF hclaim IS NULL OR hclaim->'claim_revision' IS DISTINCT FROM ref->'revision'
               OR hclaim->>'claim_status' IS DISTINCT FROM 'observed'
               OR hclaim->>'outcome' IS DISTINCT FROM ref->>'outcome' THEN
                RAISE EXCEPTION 'fixture_stale_premises';
            END IF;
        ELSE
            q := jsonb_build_object('version', 'm1_terrain_context_view_query_v1', 'engagement_id', e, 'campaign_id', c,
                'operator_ref', source#>'{fields,operator_ref}', 'expected_mission_revision', 1, 'goal_ref', source#>'{fields,goal_ref}',
                'exercise_mode', source#>'{fields,exercise_mode}', 'selected_assets', jsonb_build_array(ref->'asset_ref'),
                'claim_ids', jsonb_build_array(ref->'entity'));
            v := terrain.current_dns_context_view(q);
            SELECT value INTO claim FROM jsonb_array_elements(v->'claims') WHERE value->'claim_id' = ref->'entity';
            IF claim IS NULL OR
               (claim->'claim_revision', claim->'asset_ref', claim->'event_id', claim->'operation_id',
                claim->'observation_id', claim->'material_id', claim->'evaluation_id', claim->'perceived_at',
                claim->'valid_until', claim->'temporal_frontier', claim->'source_kind') IS DISTINCT FROM
               (ref->'revision', ref->'asset_ref', ref->'lineage'->0, ref->'lineage'->1,
                ref->'lineage'->2, ref->'lineage'->3, ref->'lineage'->4, ref->'perceived_at',
                ref->'valid_until', ref->'temporal_frontier', ref->'source')
               OR claim->>'claim_status' IS DISTINCT FROM 'observed'
               OR claim->'conflicted' IS DISTINCT FROM 'false'::jsonb
               OR claim->'integrity_hold' IS DISTINCT FROM 'false'::jsonb
               OR claim->>'outcome' IS DISTINCT FROM 'qualified_dns_claim'
               OR (claim->'vantage_ref', claim->'uncertainty_seconds', claim->'lowest_ttl_seconds', claim->'completeness', claim->'limitations') IS DISTINCT FROM
                  (ref->'vantage_ref', ref->'time_uncertainty', ref->'ttl_seconds', ref->'completeness', ref->'limitations')
               OR (ref->>'status', ref->>'tier') IS DISTINCT FROM ('observed', 'tier1')
               OR ref->'conflicted' IS DISTINCT FROM 'false'::jsonb OR ref->'integrity_hold' IS DISTINCT FROM 'false'::jsonb
               OR ref->'corroborated' IS DISTINCT FROM 'false'::jsonb OR ref->'evaluated_at' IS DISTINCT FROM p->'evaluated_at' THEN
                RAISE EXCEPTION 'fixture_stale_premises';
            END IF;
            IF ref->>'freshness' = 'fresh' THEN
                IF (claim->>'valid_until')::bigint <= (v->>'evaluated_at')::bigint THEN RAISE EXCEPTION 'fixture_stale_premises'; END IF;
            ELSIF https_loop AND ref->>'freshness' = 'expired' THEN
                IF (claim->>'valid_until')::bigint > (v->>'evaluated_at')::bigint THEN RAISE EXCEPTION 'fixture_stale_premises'; END IF;
            ELSE
                RAISE EXCEPTION 'fixture_stale_premises';
            END IF;
        END IF;
    END LOOP;
END $$;

CREATE OR REPLACE FUNCTION execution.inference_may_send(e uuid, c uuid, d uuid, k uuid, p text, cfg text, b text, h text)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    r execution.inference_reservations; s jsonb; perm jsonb; now_s bigint;
    actual text; remaining bigint; min_call_ms bigint; ar execution.autonomous_runs;
    nb execution.native_input_bindings;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') THEN RAISE EXCEPTION 'inference_actor_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = e AND campaign_id = c;
    SELECT * INTO r FROM execution.inference_reservations x WHERE (x.engagement_id, x.campaign_id, x.operation_id) = (e, c, d) AND x.record_revision = 1;
    SELECT * INTO nb FROM execution.native_input_bindings i WHERE (i.e, i.c, i.d) = (e, c, d);
    IF r.operation_id IS NULL OR r.writer_oid <> (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
       OR nb.d IS NULL OR nb.writer_oid <> r.writer_oid
       OR k NOT IN (r.generation_call_id, r.repair_call_id)
       OR EXISTS (SELECT 1 FROM execution.inference_reservations x WHERE (x.engagement_id, x.campaign_id, x.operation_id) = (e, c, d) AND x.record_revision = 2) THEN
        RAISE EXCEPTION 'inference_reservation_refused';
    END IF;
    s := mission.current_inference_publication(e, c, d);
    perm := s->'fields'->'m1_inference_permission';
    IF perm->>'version' = 'm1_https_loop_v1' THEN
        ar := execution.find_decision_run(e, c, d);
        IF ar.run_id IS NULL OR nb.request_id IS DISTINCT FROM ar.run_id
           OR nb.private_endpoint_map IS NULL OR nb.map_version IS DISTINCT FROM 'm1_https_endpoint_map_v1'
           OR nb.pack_digest IS DISTINCT FROM encode(sha256(convert_to(p, 'UTF8')), 'hex') THEN
            RAISE EXCEPTION 'inference_input_conflict';
        END IF;
    END IF;
    remaining := execution.inference_window_ms(e, c, d);
    min_call_ms := CASE WHEN perm->>'version' IN ('m1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1')
        AND perm->>'profile' IN ('deepseek-genai-v4-pro-low-v2', 'fixture-genai-v4-pro-low-v2') THEN 122000 ELSE 62000 END;
    IF perm->>'version' IN ('m1_inference_permission_v4', 'm1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') AND remaining < min_call_ms THEN
        RAISE EXCEPTION 'inference_call_window_refused';
    END IF;
    IF NOT coalesce(perm->'field_classes' ? 'admitted_repair_fields_codes', false) THEN RAISE EXCEPTION 'inference_consent_refused'; END IF;
    IF perm->>'version' IN ('m1_inference_permission_v3', 'm1_inference_permission_v4', 'm1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN
        PERFORM execution.assert_autonomous_pack(e, c, d, p::jsonb, cfg::jsonb);
    END IF;
    IF perm->>'version' IN ('m1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN
        IF NOT execution.genai_binding_valid(e, c, d, k, p, cfg, b, h) THEN RAISE EXCEPTION 'inference_intent_refused'; END IF;
        PERFORM execution.assert_current_pack_terrain(e, c, d, p::jsonb);
        IF k = r.repair_call_id AND NOT EXISTS (SELECT 1 FROM execution.inference_calls a WHERE a.call = r.generation_call_id AND a.p = p AND a.state = 'completed') THEN
            RAISE EXCEPTION 'inference_repair_refused';
        END IF;
    ELSIF perm->>'version' = 'm1_inference_permission_v4' THEN
        PERFORM execution.assert_successor_wire(p, cfg::jsonb, b::jsonb, k = r.repair_call_id);
        PERFORM execution.assert_current_pack_terrain(e, c, d, p::jsonb);
        IF k = r.repair_call_id AND NOT EXISTS (SELECT 1 FROM execution.inference_calls a WHERE a.call = r.generation_call_id AND a.p = p AND a.configuration = cfg AND a.state = 'completed') THEN
            RAISE EXCEPTION 'inference_repair_refused';
        END IF;
    END IF;
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    IF s IS NULL OR perm->>'permission_id' IS DISTINCT FROM r.request->>'permission_id'
       OR now_s < (s->'fields'->>'starts_at')::bigint OR now_s >= (s->'fields'->>'ends_at')::bigint
       OR now_s < (perm->>'starts_at')::bigint OR now_s >= (perm->>'ends_at')::bigint
       OR octet_length(p) > (perm->'operation_limits'->>'context_bytes')::int
       OR octet_length(cfg) > (perm->'operation_limits'->>'configuration_bytes')::int
       OR octet_length(b) > (perm->'operation_limits'->>'request_bytes')::int
       OR cfg::jsonb->>'recipient' IS DISTINCT FROM perm->>'recipient'
       OR cfg::jsonb->>'profile' IS DISTINCT FROM perm->>'profile'
       OR (CASE WHEN perm->>'version' IN ('m1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN false
           ELSE (CASE WHEN cfg::jsonb->>'protocol' = 'chat-completions-v1' THEN b::jsonb#>>'{messages,1,content}' ELSE b::jsonb->>'input' END) IS DISTINCT FROM p AND k = r.generation_call_id END) THEN
        RAISE EXCEPTION 'inference_authority_refused';
    END IF;
    IF perm->>'version' IN ('m1_inference_permission_v5', 'm1_inference_permission_v6', 'm1_https_loop_v1') THEN
        actual := execution.genai_binding_digest(p, cfg);
    ELSE
        actual := encode(sha256(convert_to('dw-inference-binding-v1', 'UTF8') || decode('00', 'hex') ||
            int8send(octet_length(p)::bigint) || convert_to(p, 'UTF8') ||
            int8send(octet_length(cfg)::bigint) || convert_to(cfg, 'UTF8') ||
            int8send(octet_length(b)::bigint) || convert_to(b, 'UTF8')), 'hex');
    END IF;
    IF actual <> h THEN RAISE EXCEPTION 'inference_binding_refused'; END IF;
    IF k = r.repair_call_id AND NOT EXISTS (SELECT 1 FROM execution.inference_calls x WHERE x.call = r.generation_call_id AND x.state = 'completed') THEN
        RAISE EXCEPTION 'inference_repair_refused';
    END IF;
    INSERT INTO execution.inference_calls VALUES (e, c, d, k, h, p, cfg, b, 'may_send_uncertain', NULL);
END $$;

-- 11. Cognition identity and publication validation across indexed steps.
CREATE OR REPLACE FUNCTION cognition.successor_reasoning_identity(x jsonb) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT coalesce(EXISTS (
        SELECT 1 FROM execution.autonomous_runs r JOIN mission.registration_outbox m ON (m.engagement_id, m.campaign_id) = (r.e, r.c)
        JOIN execution.native_input_bindings i ON (i.e, i.c, i.d) = ((x->>'engagement')::uuid, (x->>'campaign')::uuid, (x->>'decision')::uuid)
        WHERE r.run_id = (execution.find_decision_run((x->>'engagement')::uuid, (x->>'campaign')::uuid, (x->>'decision')::uuid)).run_id
          AND i.writer_oid = r.writer_oid
          AND r.request->>'version' IN ('m1_autonomous_run_request_v2', 'm1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4', 'm1_https_loop_v1')
          AND r.request->>'run_profile' IN ('autonomous-dns-fixture-v2', 'autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1', 'm1_https_loop_v1')
          AND (r.request->>'version' = 'm1_autonomous_run_request_v4') = (r.request->>'run_profile' IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1'))
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

CREATE OR REPLACE FUNCTION cognition.reasoning_source_matches(k uuid, x jsonb) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE existing jsonb; s jsonb; r execution.inference_reservations; e uuid; c uuid; d uuid;
BEGIN
    SELECT payload INTO s FROM cognition.reasoning_decisions z JOIN cognition.reasoning_outbox o USING (event)
    JOIN execution.native_input_bindings i ON (i.e, i.c, i.d, i.input_hash) = (z.e, z.c, z.d, z.input_hash) WHERE z.event = k;
    IF s IS NULL OR s IS DISTINCT FROM x THEN RAISE EXCEPTION 'inference_source_unavailable'; END IF;
    IF s->>'kind' IS DISTINCT FROM 'reasoning_decision' OR s->>'source_mode' NOT IN ('blind', 'defender_informed')
       OR jsonb_typeof(s->'occurred_at_ms') IS DISTINCT FROM 'number' OR jsonb_typeof(s->'recorded_at_ms') IS DISTINCT FROM 'number'
       OR jsonb_typeof(s->'provenance') NOT IN ('array', 'null') THEN
        RAISE EXCEPTION 'inference_binding_conflict';
    END IF;
    IF jsonb_typeof(s) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
    IF s->>'version' = '2' OR s->>'disposition' = 'recorded_stale_premises' THEN
        IF NOT cognition.successor_reasoning_identity(s) THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
    END IF;
    IF s->>'producer' IS DISTINCT FROM 'cognition' OR (s->'version' IN ('1'::jsonb, '2'::jsonb)) IS DISTINCT FROM true
       OR s->>'publication_obligation' IS DISTINCT FROM 'trajectory.reasoning_history.v1'
       OR jsonb_typeof(s->'producer') IS DISTINCT FROM 'string' OR jsonb_typeof(s->'publication_obligation') IS DISTINCT FROM 'string'
       OR jsonb_typeof(s->'mission_revision') IS DISTINCT FROM 'number' OR jsonb_typeof(s->'call_ids') IS DISTINCT FROM 'array'
       OR jsonb_typeof(s->'bindings') IS DISTINCT FROM 'array' OR jsonb_typeof(s->'disposition') IS DISTINCT FROM 'string'
       OR NOT (s ? 'proposal') THEN
        RAISE EXCEPTION 'inference_binding_conflict';
    END IF;
    IF s->>'mission_revision' !~ '^[0-9]+$' OR (s->>'mission_revision')::numeric NOT BETWEEN 1 AND 9223372036854775807
       OR s->>'disposition' NOT IN (
           'validated_proposal', 'recorded_stale_premises', 'schema_repair_exhausted', 'unsafe_explanation', 'unsupported_premise',
           'reference_ineligible', 'manifest_ineligible', 'view_selector_ineligible', 'current_authority_unavailable',
           'repair_code_ineligible', 'repair_code_limit', 'pack_size', 'pack_utf8', 'repair_encode', 'request_encode', 'request_size',
           'inference_marker_commit_unknown', 'inference_marker_refused', 'inference_marker_unknown', 'inference_sql_ambiguous',
           'inference_sql_privilege', 'inference_sql_function_missing', 'stopped_before_release', 'inference_stop_unknown',
           'inference_authority_unavailable', 'inference_reservation_unknown', 'inference_reservation_missing', 'inference_deadline',
           'inference_guard_missing', 'inference_platform_unqualified', 'inference_guard_busy', 'inference_fixture_endpoint_refused',
           'live_key_missing', 'live_key_invalid', 'inference_curl_missing', 'inference_pack_encode', 'inference_config_encode',
           'inference_body_encode', 'inference_usage_encode', 'inference_accounting_unknown', 'inference_database_config_missing',
           'inference_database_config_invalid', 'inference_database_unavailable', 'provider_incomplete', 'provider_protocol',
           'provider_unsafe_response', 'provider_response_limit', 'provider_transport_unknown', 'provider_cancelled_uncertain',
           'accounting_unknown', 'inference_release_unknown', 'inference_transport_unavailable', 'inference_inspect_unavailable'
       ) THEN
        RAISE EXCEPTION 'inference_binding_conflict';
    END IF;
    IF EXISTS (SELECT 1 FROM unnest(ARRAY['event', 'engagement', 'campaign', 'decision', 'permission']) f
        WHERE jsonb_typeof(s->f) IS DISTINCT FROM 'string' OR (s->>f) IS NULL OR (s->>f) !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(s->'call_ids') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string' OR v#>>'{}' !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(s->'bindings') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string') THEN
        RAISE EXCEPTION 'inference_binding_conflict';
    END IF;
    IF s->>'disposition' IN ('validated_proposal', 'recorded_stale_premises') THEN
        IF jsonb_typeof(s->'proposal') IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
    ELSE
        IF jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
    END IF;
    e := (s->>'engagement')::uuid; c := (s->>'campaign')::uuid; d := (s->>'decision')::uuid;
    IF NOT EXISTS (
        SELECT 1 FROM mission.registration_outbox m JOIN trajectory.registration_history h
            ON (h.engagement_id, h.campaign_id, h.operation_id, h.event_id) = (m.engagement_id, m.campaign_id, m.operation_id, m.event_id)
        WHERE (m.engagement_id, m.campaign_id) = (e, c) AND h.status = 'accepted'
          AND (m.contract->>'owner_revision')::bigint = (s->>'mission_revision')::bigint
          AND m.contract->'fields'->'m1_inference_permission'->>'permission_id' = s->>'permission'
    ) OR EXISTS (SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id, h.campaign_id) = (e, c) AND h.status = 'anomaly') THEN
        RAISE EXCEPTION 'inference_predecessor_missing';
    END IF;
    IF jsonb_typeof(s->'call_ids') IS DISTINCT FROM 'array' OR jsonb_typeof(s->'bindings') IS DISTINCT FROM 'array'
       OR (CASE WHEN s->>'disposition' IN ('validated_proposal', 'recorded_stale_premises') THEN jsonb_typeof(s->'proposal') IS DISTINCT FROM 'object' OR jsonb_array_length(s->'bindings') NOT BETWEEN 1 AND 2
           WHEN s->>'disposition' = 'schema_repair_exhausted' THEN jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' OR jsonb_array_length(s->'bindings') <> 2
           ELSE jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' END) THEN
        RAISE EXCEPTION 'inference_binding_conflict';
    END IF;
    IF jsonb_array_length(s->'call_ids') = 0 THEN
        IF jsonb_array_length(s->'bindings') <> 0 OR EXISTS (SELECT 1 FROM execution.inference_calls a WHERE (a.e, a.c, a.d) = (e, c, d)) THEN
            RAISE EXCEPTION 'inference_binding_conflict';
        END IF;
    ELSE
        SELECT * INTO r FROM execution.inference_reservations y WHERE (y.engagement_id, y.campaign_id, y.operation_id) = (e, c, d) AND y.record_revision = 1;
        IF r.operation_id IS NULL OR s->'call_ids' <> jsonb_build_array(r.generation_call_id, r.repair_call_id)
           OR (SELECT coalesce(jsonb_agg(a.binding ORDER BY CASE WHEN a.call = r.generation_call_id THEN 1 WHEN a.call = r.repair_call_id THEN 2 ELSE 3 END), '[]'::jsonb)
               FROM execution.inference_calls a WHERE (a.e, a.c, a.d) = (e, c, d)) <> s->'bindings'
           OR EXISTS (SELECT 1 FROM execution.inference_calls a WHERE (a.e, a.c, a.d) = (e, c, d) AND a.call NOT IN (r.generation_call_id, r.repair_call_id)) THEN
            RAISE EXCEPTION 'inference_binding_conflict';
        END IF;
    END IF;
    IF EXISTS (
        SELECT 1 FROM execution.inference_calls a WHERE (a.e, a.c, a.d) = (e, c, d)
          AND CASE WHEN EXISTS (
              SELECT 1 FROM execution.autonomous_runs z
              WHERE z.run_id = (execution.find_decision_run(e, c, d)).run_id
                AND z.request->>'version' IN ('m1_autonomous_run_request_v3', 'm1_autonomous_run_request_v4', 'm1_https_loop_v1')
          ) THEN NOT execution.genai_binding_valid(e, c, d, a.call, a.p, a.configuration, a.body, a.binding)
          ELSE encode(sha256(convert_to('dw-inference-binding-v1', 'UTF8') || decode('00', 'hex') ||
              int8send(octet_length(a.p)::bigint) || convert_to(a.p, 'UTF8') ||
              int8send(octet_length(a.configuration)::bigint) || convert_to(a.configuration, 'UTF8') ||
              int8send(octet_length(a.body)::bigint) || convert_to(a.body, 'UTF8')), 'hex') <> a.binding END
    ) THEN
        RAISE EXCEPTION 'inference_binding_conflict';
    END IF;
    RETURN true;
END $$;

-- 12. Indexed HTTPS-loop step queries, intent/terminal persistence, refresh & effect execution ports, and summary.
CREATE OR REPLACE FUNCTION execution.https_loop_step_record(r uuid, idx integer) RETURNS jsonb
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT CASE WHEN execution.run_record(r) IS NULL THEN NULL ELSE (
        SELECT jsonb_build_object(
            'run_id', s.run_id, 'step_index', s.step_index, 'decision_id', s.decision_id,
            'next_decision_id', s.next_decision_id, 'decision_phase', s.decision_phase,
            'acquisition_phase', s.acquisition_phase, 'status', s.status, 'protection_state', s.protection_state,
            'next_intent_kind', s.next_intent_kind, 'resolved_asset_ref', s.resolved_asset_ref,
            'resolved_hostname', s.resolved_hostname, 'resolved_ipv4', s.resolved_ipv4,
            'resolved_endpoint_ref', s.resolved_endpoint_ref, 'resolved_path', s.resolved_path,
            'parent_claim_id', s.parent_claim_id, 'parent_claim_revision', s.parent_claim_revision,
            'dependency_refresh', s.dependency_refresh, 'episode_index', s.episode_index,
            'dns_attempt_id', s.dns_attempt_id, 'dns_result_id', s.dns_result_id,
            'refresh_attempt_id', s.refresh_attempt_id, 'refresh_result_id', s.refresh_result_id,
            'refresh_outcome', s.refresh_outcome, 'head_attempt_id', s.head_attempt_id,
            'head_result_id', s.head_result_id, 'head_outcome', s.head_outcome,
            'terrain_operation_id', s.terrain_operation_id, 'head_terrain_operation_id', s.head_terrain_operation_id,
            'disposition', s.disposition
        )
        FROM execution.https_loop_steps s
        WHERE s.run_id = r AND s.step_index = idx
    ) END
$$;

CREATE OR REPLACE FUNCTION execution.https_loop_steps_for_run(r uuid) RETURNS jsonb
LANGUAGE sql STABLE SECURITY DEFINER SET search_path = pg_catalog AS $$
    SELECT CASE WHEN execution.run_record(r) IS NULL THEN '[]'::jsonb ELSE coalesce((
        SELECT jsonb_agg(execution.https_loop_step_record(r, s.step_index) ORDER BY s.step_index)
        FROM execution.https_loop_steps s WHERE s.run_id = r
    ), '[]'::jsonb) END
$$;

CREATE OR REPLACE FUNCTION execution.https_loop_step_predecessor(r uuid, idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
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
        'execution_mode', 'controlled_fixture',
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

CREATE OR REPLACE FUNCTION execution.record_https_loop_dns_intent(r uuid, idx integer, asset uuid, hostname text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    PERFORM execution.run_current(r);
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(x.request->'seed_names') s WHERE (s->>'asset_ref')::uuid = asset AND s->>'name' = hostname) THEN
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    SELECT * INTO st FROM execution.https_loop_steps s WHERE s.run_id = r AND s.step_index = idx FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
    IF st.next_intent_kind IS NOT NULL THEN
        IF st.next_intent_kind = 'dns_udp' AND st.resolved_asset_ref = asset AND st.resolved_hostname = hostname THEN RETURN; END IF;
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    UPDATE execution.https_loop_steps
    SET next_intent_kind = 'dns_udp', resolved_asset_ref = asset, resolved_hostname = hostname,
        dependency_refresh = 'none', status = 'acquisition_pending',
        updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
    WHERE run_id = r AND step_index = idx;
END $$;

CREATE OR REPLACE FUNCTION execution.record_https_loop_head_intent(
    r uuid, idx integer, asset uuid, hostname text, ipv4 text, endpoint_ref text, approved_path text, claim_id uuid, claim_rev integer, refresh text
) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; st execution.https_loop_steps; actor oid; mp jsonb;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 OR refresh NOT IN ('none', 'dns_once_if_stale') THEN
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    PERFORM execution.run_current(r);
    SELECT * INTO st FROM execution.https_loop_steps s WHERE s.run_id = r AND s.step_index = idx FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
    SELECT nb.private_endpoint_map INTO mp FROM execution.native_input_bindings nb WHERE (nb.e, nb.c, nb.d) = (x.e, x.c, st.decision_id);
    IF mp IS NULL OR NOT EXISTS (
        SELECT 1 FROM jsonb_array_elements(mp->'bindings') b
        WHERE b->>'local_id' = endpoint_ref AND (b->>'asset_ref')::uuid = asset AND b->>'hostname' = hostname
          AND b->>'ipv4' = ipv4 AND b->>'approved_path' = approved_path
          AND (b->>'parent_claim_id')::uuid = claim_id AND (b->>'parent_claim_revision')::int = claim_rev
    ) THEN
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    IF st.next_intent_kind IS NOT NULL THEN
        IF st.next_intent_kind = 'https-head-v1' AND st.resolved_asset_ref = asset AND st.resolved_hostname = hostname
           AND st.resolved_ipv4 = ipv4 AND st.resolved_endpoint_ref = endpoint_ref AND st.resolved_path = approved_path
           AND st.parent_claim_id = claim_id AND st.parent_claim_revision = claim_rev AND st.dependency_refresh = refresh THEN
            RETURN;
        END IF;
        RAISE EXCEPTION 'run_phase_refused';
    END IF;
    UPDATE execution.https_loop_steps
    SET next_intent_kind = 'https-head-v1', resolved_asset_ref = asset, resolved_hostname = hostname,
        resolved_ipv4 = ipv4, resolved_endpoint_ref = endpoint_ref, resolved_path = approved_path,
        parent_claim_id = claim_id, parent_claim_revision = claim_rev, dependency_refresh = refresh,
        status = 'acquisition_pending', updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
    WHERE run_id = r AND step_index = idx;
END $$;

CREATE OR REPLACE FUNCTION execution.record_https_loop_d4_outcome(r uuid, intent_kind text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER')
       OR intent_kind NOT IN ('validate_refused_at_d4', 'observe_more_refused_at_d4', 'retained', 'deferred', 'stopped', 'capability_gap') THEN
        RAISE EXCEPTION 'run_terminal_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'run_terminal_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    SELECT * INTO st FROM execution.https_loop_steps s WHERE s.run_id = r AND s.step_index = 4 FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'run_terminal_refused'; END IF;
    UPDATE execution.https_loop_steps
    SET next_intent_kind = intent_kind, disposition = intent_kind, status = 'completed',
        updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
    WHERE run_id = r AND step_index = 4;
END $$;

CREATE OR REPLACE FUNCTION execution.run_https_loop_terminal(r uuid, idx integer, reason text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; st execution.https_loop_steps;
BEGIN
    IF execution.run_record(r) IS NULL OR idx NOT BETWEEN 1 AND 3
       OR reason NOT IN ('blocked_budget_or_time', 'blocked_stale_premises', 'retained', 'deferred', 'stopped', 'owner_view_requested', 'capability_gap') THEN
        RAISE EXCEPTION 'run_terminal_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    PERFORM execution.run_current(r);
    SELECT * INTO st FROM execution.https_loop_steps s WHERE s.run_id = r AND s.step_index = idx FOR UPDATE;
    IF NOT FOUND OR NOT EXISTS (SELECT 1 FROM execution.autonomous_phases WHERE run_id = r AND phase = st.decision_phase AND status = 'completed') THEN
        RAISE EXCEPTION 'run_terminal_refused';
    END IF;
    UPDATE execution.https_loop_steps
    SET status = 'terminal', disposition = reason, updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
    WHERE run_id = r AND step_index = idx;
    UPDATE execution.autonomous_control SET status = reason, lease_until_ms = 0 WHERE run_id = r AND status = 'running';
END $$;

CREATE OR REPLACE FUNCTION execution.https_loop_refresh_check(r uuid, idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps;
    s jsonb; lim jsonb; cc terrain.current_claims; ev terrain.evaluations;
    now_s bigint; cap_until bigint; seed_idx integer; dns_used integer; ep_used integer;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN RAISE EXCEPTION 'fixture_release_refused'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'fixture_release_refused'; END IF;
    s := execution.run_current(r);
    lim := s#>'{fields,m1_permission,campaign_limits}';
    SELECT * INTO st FROM execution.https_loop_steps WHERE run_id = r AND step_index = idx;
    IF NOT FOUND OR st.next_intent_kind IS DISTINCT FROM 'https-head-v1' THEN RAISE EXCEPTION 'fixture_release_refused'; END IF;
    SELECT * INTO cc FROM terrain.current_claims WHERE engagement_id = x.e AND campaign_id = x.c AND claim_id = st.parent_claim_id;
    IF NOT FOUND THEN RETURN '{"decision":"blocked","reason":"non_ttl_invalidity"}'::jsonb; END IF;
    SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = x.e AND campaign_id = x.c AND operation_id = cc.operation_id;
    IF cc.claim_status <> 'observed' OR cc.conflicted
       OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision
       OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0
       OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = x.e AND ic.campaign_id = x.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
        RETURN '{"decision":"blocked","reason":"non_ttl_invalidity"}'::jsonb;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) a WHERE a->>'ipv4' = st.resolved_ipv4) THEN
        RETURN '{"decision":"blocked","reason":"ip_missing_from_claim"}'::jsonb;
    END IF;
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
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
    IF st.refresh_attempt_id IS NOT NULL THEN
        RETURN '{"decision":"blocked","reason":"refresh_already_attempted"}'::jsonb;
    END IF;
    SELECT count(*) INTO dns_used FROM execution.fixture_dns_attempts WHERE run_id = r AND run_kind = 'm1_https_loop_v1';
    SELECT count(*) INTO ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    IF dns_used >= least(3, (lim->>'dns_questions')::int) OR (st.episode_index IS NULL AND ep_used >= 3) THEN
        RETURN '{"decision":"blocked","reason":"dns_budget_exhausted"}'::jsonb;
    END IF;
    SELECT (ord - 1)::integer INTO seed_idx FROM jsonb_array_elements(x.request->'seed_names') WITH ORDINALITY a(elem, ord)
    WHERE (elem->>'asset_ref')::uuid = st.resolved_asset_ref AND elem->>'name' = st.resolved_hostname LIMIT 1;
    IF seed_idx IS NULL THEN RETURN '{"decision":"blocked","reason":"seed_ineligible"}'::jsonb; END IF;
    RETURN jsonb_build_object('decision', 'need_refresh', 'seed_index', seed_idx);
END $$;

CREATE OR REPLACE FUNCTION execution.reserve_https_loop_dns(r uuid, idx integer, kind text, seed_idx integer) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
    s jsonb; pol jsonb; lim jsonb; seed jsonb; req jsonb;
    attempt_id uuid; result_id uuid; cur_rev integer; n_ms bigint; ep_num integer;
    dns_used integer; ep_used integer;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 OR kind NOT IN ('initial', 'refresh') THEN
        RAISE EXCEPTION 'fixture_release_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN
        RAISE EXCEPTION 'fixture_release_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    s := execution.run_current(r);
    pol := s#>'{fields,m1_permission}';
    lim := s#>'{fields,m1_permission,campaign_limits}';
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND OR NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = r AND y.phase = st.acquisition_phase AND y.claimed) THEN
        RAISE EXCEPTION 'fixture_release_refused';
    END IF;
    IF kind = 'initial' AND st.next_intent_kind IS DISTINCT FROM 'dns_udp' THEN RAISE EXCEPTION 'fixture_release_refused'; END IF;
    IF kind = 'refresh' AND (st.next_intent_kind IS DISTINCT FROM 'https-head-v1' OR st.dependency_refresh IS DISTINCT FROM 'dns_once_if_stale') THEN
        RAISE EXCEPTION 'fixture_release_refused';
    END IF;
    seed := x.request->'seed_names'->seed_idx;
    IF seed IS NULL OR (seed->>'asset_ref')::uuid IS DISTINCT FROM st.resolved_asset_ref OR seed->>'name' IS DISTINCT FROM st.resolved_hostname THEN
        RAISE EXCEPTION 'fixture_release_refused';
    END IF;
    IF kind = 'initial' AND st.dns_attempt_id IS NOT NULL THEN
        RETURN st.dns_request;
    END IF;
    IF kind = 'refresh' AND st.refresh_attempt_id IS NOT NULL THEN
        RETURN st.refresh_request;
    END IF;
    SELECT count(*) INTO dns_used FROM execution.fixture_dns_attempts WHERE run_id = r AND run_kind = 'm1_https_loop_v1';
    SELECT count(*) INTO ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    IF dns_used >= least(3, (lim->>'dns_questions')::int) OR ep_used >= 3 OR (x.deadline_at_ms - n_ms) < 60000 THEN
        RAISE EXCEPTION 'fixture_release_refused';
    END IF;
    ep_num := coalesce(st.episode_index, ep_used + 1);
    IF st.episode_deadline_ms IS NULL THEN
        UPDATE execution.https_loop_steps
        SET episode_index = ep_num, episode_deadline_ms = least(n_ms + 60000, x.deadline_at_ms)
        WHERE run_id = r AND step_index = idx;
    ELSIF n_ms >= st.episode_deadline_ms THEN
        RAISE EXCEPTION 'episode_deadline_expired';
    END IF;
    SELECT coalesce(max(cr.claim_revision), 0) INTO cur_rev FROM terrain.claim_revisions cr
    WHERE cr.engagement_id = x.e AND cr.campaign_id = x.c AND cr.subject_name = st.resolved_hostname
      AND cr.claim_kind = 'dns_hostname_ipv4_resolution' AND cr.vantage_ref = (pol->>'vantage_ref')::uuid
      AND cr.resolver_ipv4 = pol->>'resolver_ipv4';
    attempt_id := gen_random_uuid();
    result_id := gen_random_uuid();
    req := jsonb_build_object(
        'version', 'm1_terrain_dns_issued_observation_v1',
        'engagement_id', x.e, 'campaign_id', x.c,
        'operator_ref', s#>'{fields,operator_ref}',
        'expected_mission_revision', 1,
        'goal_ref', s#>'{fields,goal_ref}',
        'exercise_mode', s#>'{fields,exercise_mode}',
        'correlation_id', result_id,
        'source_kind', 'fixture_dns_acquisition',
        'vantage_ref', pol->'vantage_ref',
        'resolver_ipv4', pol->'resolver_ipv4',
        'subject_name', st.resolved_hostname,
        'asset_ref', st.resolved_asset_ref,
        'expected_claim_revision', cur_rev,
        'perceived_at', (n_ms / 1000)::bigint,
        'uncertainty_seconds', 0,
        'transport', 'udp', 'qtype', 'A', 'qclass', 'IN', 'rcode', 'noerror',
        'truncated', false, 'completeness', 'complete', 'limitations', '["single_vantage_only"]'::jsonb,
        'answers', '[]'::jsonb
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
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
    att execution.fixture_dns_attempts; base jsonb; n_ms bigint; res_id uuid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 OR kind NOT IN ('initial', 'refresh') THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    SELECT * INTO att FROM execution.fixture_dns_attempts a WHERE a.run_id = r AND a.step_index = idx AND a.run_kind = 'm1_https_loop_v1' AND a.attempt_kind = kind FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'fixture_result_refused'; END IF;
    base := CASE WHEN kind = 'initial' THEN st.dns_request ELSE st.refresh_request END;
    IF base IS NULL THEN RAISE EXCEPTION 'fixture_result_refused'; END IF;
    res_id := (base->>'correlation_id')::uuid;
    IF (req - 'answers') IS DISTINCT FROM (base - 'answers') OR jsonb_typeof(req->'answers') IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.fixture_dns_results WHERE attempt = att.attempt) THEN RETURN; END IF;
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
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE
    x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
    s jsonb; pol jsonb; lim jsonb; cc terrain.current_claims; ev terrain.evaluations;
    n_ms bigint; now_s bigint; cap_until bigint;
    attempt_id uuid; req jsonb; head_used integer; ep_used integer; ep_num integer;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN RAISE EXCEPTION 'fixture_release_refused'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'fixture_release_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    s := execution.run_current(r);
    pol := s#>'{fields,m1_permission}';
    lim := s#>'{fields,m1_permission,campaign_limits}';
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND OR st.next_intent_kind IS DISTINCT FROM 'https-head-v1'
       OR NOT EXISTS (SELECT 1 FROM execution.autonomous_control y WHERE y.run_id = r AND y.phase = st.acquisition_phase AND y.claimed) THEN
        RAISE EXCEPTION 'fixture_release_refused';
    END IF;
    IF st.head_attempt_id IS NOT NULL THEN
        SELECT a.request INTO req FROM execution.fixture_https_attempts a WHERE a.attempt = st.head_attempt_id;
        RETURN req;
    END IF;
    SELECT count(*) INTO head_used FROM execution.fixture_https_attempts WHERE run_id = r;
    SELECT count(*) INTO ep_used FROM execution.https_loop_steps WHERE run_id = r AND episode_index IS NOT NULL;
    IF head_used >= least(2, (lim->>'head_requests')::int, (lim->>'tcp_connections')::int)
       OR (st.episode_index IS NULL AND ep_used >= 3) THEN
        RAISE EXCEPTION 'head_budget_exhausted';
    END IF;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    now_s := n_ms / 1000;
    IF st.episode_deadline_ms IS NOT NULL AND n_ms >= st.episode_deadline_ms THEN RAISE EXCEPTION 'episode_deadline_expired'; END IF;
    IF st.episode_index IS NULL AND (x.deadline_at_ms - n_ms) < 60000 THEN RAISE EXCEPTION 'run_remaining_time_insufficient'; END IF;
    SELECT * INTO cc FROM terrain.current_claims
    WHERE engagement_id = x.e AND campaign_id = x.c AND subject_name = st.resolved_hostname
      AND vantage_ref = (pol->>'vantage_ref')::uuid AND resolver_ipv4 = pol->>'resolver_ipv4'
    ORDER BY claim_revision DESC LIMIT 1;
    IF NOT FOUND OR cc.claim_status <> 'observed' OR cc.conflicted THEN RAISE EXCEPTION 'head_premise_invalid'; END IF;
    SELECT * INTO ev FROM terrain.evaluations WHERE engagement_id = x.e AND campaign_id = x.c AND operation_id = cc.operation_id;
    IF NOT FOUND OR ev.outcome <> 'qualified_dns_claim' OR coalesce(ev.lowest_ttl_seconds, 0) <= 0
       OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts ic WHERE ic.engagement_id = x.e AND ic.campaign_id = x.c AND ic.claim_id = cc.claim_id AND ic.blocks_use) THEN
        RAISE EXCEPTION 'head_premise_invalid';
    END IF;
    IF st.refresh_result_id IS NULL THEN
        IF cc.claim_id IS DISTINCT FROM st.parent_claim_id OR cc.claim_revision IS DISTINCT FROM st.parent_claim_revision THEN
            RAISE EXCEPTION 'head_claim_superseded';
        END IF;
    END IF;
    cap_until := least(cc.valid_until, cc.perceived_at + least(ev.lowest_ttl_seconds, 60));
    IF now_s >= cap_until THEN RAISE EXCEPTION 'head_dns_stale'; END IF;
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cc.answers) a WHERE a->>'ipv4' = st.resolved_ipv4) THEN
        RAISE EXCEPTION 'head_ip_missing_from_claim';
    END IF;
    attempt_id := gen_random_uuid();
    ep_num := coalesce(st.episode_index, ep_used + 1);
    req := jsonb_build_object(
        'version', 'm1_https_head_request_v1',
        'engagement_id', x.e, 'campaign_id', x.c, 'run_id', r, 'decision_id', st.decision_id,
        'attempt_id', attempt_id, 'asset_ref', st.resolved_asset_ref, 'endpoint_ref', st.resolved_endpoint_ref,
        'parent_claim_id', cc.claim_id, 'parent_claim_revision', cc.claim_revision,
        'hostname', st.resolved_hostname, 'ipv4', st.resolved_ipv4, 'port', 443,
        'approved_path', st.resolved_path, 'method', 'HEAD',
        'resolver_ipv4', pol->'resolver_ipv4', 'vantage_ref', pol->'vantage_ref',
        'dependency_refresh', st.dependency_refresh,
        'deadline_at_ms', coalesce(st.episode_deadline_ms, least(n_ms + 60000, x.deadline_at_ms))
    );
    INSERT INTO execution.fixture_https_attempts
        (attempt, run_id, step_index, session_generation, decision, asset_ref, endpoint_ref, hostname, ipv4, approved_path,
         parent_claim_id, parent_claim_revision, profile, mode, release_sha256, reserved_at_ms, request)
    VALUES (attempt_id, r, idx, ep_num, st.decision_id, st.resolved_asset_ref, st.resolved_endpoint_ref, st.resolved_hostname,
        st.resolved_ipv4, st.resolved_path, cc.claim_id, cc.claim_revision, 'https-head-fixture-v1', 'controlled_fixture',
        x.release_sha256, n_ms, req);
    UPDATE execution.https_loop_steps
    SET episode_index = ep_num,
        episode_deadline_ms = coalesce(episode_deadline_ms, least(n_ms + 60000, x.deadline_at_ms)),
        head_attempt_id = attempt_id, head_terrain_operation_id = coalesce(head_terrain_operation_id, gen_random_uuid()),
        status = 'acquisition_running', updated_at_ms = n_ms
    WHERE run_id = r AND step_index = idx;
    RETURN req;
END $$;

CREATE OR REPLACE FUNCTION execution.commit_https_loop_head(r uuid, idx integer, res jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; st execution.https_loop_steps; att execution.fixture_https_attempts; actor oid; n_ms bigint; res_id uuid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN RAISE EXCEPTION 'fixture_result_refused'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'fixture_result_refused'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    SELECT * INTO att FROM execution.fixture_https_attempts a WHERE a.run_id = r AND a.step_index = idx FOR UPDATE;
    IF NOT FOUND OR (res->>'attempt_id')::uuid IS DISTINCT FROM att.attempt
       OR (res->>'engagement_id')::uuid IS DISTINCT FROM x.e OR (res->>'campaign_id')::uuid IS DISTINCT FROM x.c
       OR (res->>'run_id')::uuid IS DISTINCT FROM r OR (res->>'decision_id')::uuid IS DISTINCT FROM att.decision
       OR res->>'hostname' IS DISTINCT FROM att.hostname OR res->>'ipv4' IS DISTINCT FROM att.ipv4
       OR res->>'approved_path' IS DISTINCT FROM att.approved_path THEN
        RAISE EXCEPTION 'fixture_result_refused';
    END IF;
    res_id := (res->>'result_id')::uuid;
    IF EXISTS (SELECT 1 FROM execution.fixture_https_results hr WHERE hr.run_id = r AND hr.step_index = idx) THEN RETURN; END IF;
    n_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
    INSERT INTO execution.fixture_https_results
        (result, run_id, step_index, attempt, request, result_payload, profile, mode, release_sha256, received_at_ms, publication_obligation)
    VALUES (res_id, r, idx, att.attempt, att.request, res, 'https-head-fixture-v1', 'controlled_fixture', x.release_sha256, n_ms, 'terrain.issued_https.v1');
    UPDATE execution.https_loop_steps
    SET head_result_id = res_id, head_outcome = res->>'outcome', updated_at_ms = n_ms
    WHERE run_id = r AND step_index = idx;
END $$;

CREATE OR REPLACE FUNCTION execution.block_https_loop_head(r uuid, idx integer, reason text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    UPDATE execution.https_loop_steps
    SET head_outcome = 'blocked', disposition = reason, updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
    WHERE run_id = r AND step_index = idx;
END $$;

CREATE OR REPLACE FUNCTION execution.mark_https_loop_effect_unknown(r uuid, idx integer, effect_kind text, reason text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; st execution.https_loop_steps; actor oid;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') OR idx NOT BETWEEN 1 AND 3 THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT oid INTO actor FROM pg_catalog.pg_roles WHERE rolname = session_user;
    IF NOT FOUND OR x.writer_oid <> actor OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    SELECT * INTO st FROM execution.https_loop_steps s1 WHERE s1.run_id = r AND s1.step_index = idx FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    UPDATE execution.https_loop_steps
    SET status = 'unknown',
        refresh_outcome = CASE WHEN effect_kind = 'refresh' THEN reason ELSE refresh_outcome END,
        head_outcome = CASE WHEN effect_kind = 'refresh' THEN 'unknown_refresh' ELSE 'unknown' END,
        disposition = reason,
        protection_state = CASE WHEN protection_state = 'protected' THEN 'unknown' ELSE protection_state END,
        updated_at_ms = floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint
    WHERE run_id = r AND step_index = idx;
END $$;

CREATE OR REPLACE FUNCTION execution.close_https_loop_effect_session(r uuid, disposition text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs;
BEGIN
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF NOT FOUND OR x.request->>'version' IS DISTINCT FROM 'm1_https_loop_v1' THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    IF disposition NOT IN ('completed', 'blocked', 'stopped', 'unknown') THEN RAISE EXCEPTION 'run_terminal_unknown'; END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
END $$;

CREATE OR REPLACE FUNCTION execution.run_terminal(r uuid, reason text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE x execution.autonomous_runs; p jsonb; expected text;
BEGIN
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    IF x.request->>'version' = 'm1_https_loop_v1' THEN
        PERFORM execution.run_https_loop_terminal(r, 1, reason);
        RETURN;
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text || ':' || x.c::text, 0));
    UPDATE execution.session_fences SET epoch = epoch + 1 WHERE engagement_id = x.e AND campaign_id = x.c;
    PERFORM execution.run_current(r);
    SELECT payload->'proposal' INTO p FROM cognition.reasoning_decisions WHERE (e, c, d) = (x.e, x.c, x.decision_one);
    expected := CASE
        WHEN EXISTS (SELECT 1 FROM cognition.reasoning_decisions WHERE (e, c, d) = (x.e, x.c, x.decision_one) AND payload->>'disposition' = 'recorded_stale_premises') THEN 'deferred'
        WHEN p#>'{next_intent,view_request}' IS NOT NULL AND p#>'{next_intent,view_request}' <> 'null' THEN 'owner_view_requested'
        WHEN p#>>'{next_intent,category}' = 'retain' THEN 'retained'
        WHEN p#>>'{next_intent,category}' = 'defer' THEN 'deferred'
        WHEN p#>>'{next_intent,category}' = 'stop' THEN 'stopped'
        ELSE 'capability_gap' END;
    IF reason IS DISTINCT FROM expected OR NOT EXISTS (SELECT 1 FROM execution.autonomous_phases WHERE run_id = r AND phase = 'decision_one' AND status = 'completed') THEN
        RAISE EXCEPTION 'run_terminal_refused';
    END IF;
    UPDATE execution.autonomous_control SET status = reason, lease_until_ms = 0 WHERE run_id = r AND status = 'running';
END $$;

CREATE OR REPLACE FUNCTION execution.run_summary_without_diagnostics(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    x execution.autonomous_runs; y execution.autonomous_control; status text; summary jsonb;
    n bigint := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
BEGIN
    IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
    SELECT * INTO x FROM execution.autonomous_runs WHERE run_id = r;
    SELECT * INTO y FROM execution.autonomous_control WHERE run_id = r;
    status := CASE WHEN y.status = 'running' AND least(y.lease_until_ms, x.deadline_at_ms) <= n THEN 'unknown_supervisor_lost' ELSE y.status END;
    IF x.request->>'version' = 'm1_https_loop_v1' THEN
        RETURN jsonb_build_object(
            'kind', 'm1_https_loop_run_receipt_v1', 'run_id', r, 'run_profile', x.request->'run_profile',
            'execution_mode', x.request->'execution_mode', 'inference_profile', x.request->'inference_profile',
            'status', status, 'phase', y.phase, 'dispatch_granted', false,
            'phases', (SELECT coalesce(jsonb_agg(jsonb_build_object('phase', p.phase, 'status', p.status, 'recorded_at_ms', p.recorded_at_ms) ORDER BY p.recorded_at_ms), '[]') FROM execution.autonomous_phases p WHERE p.run_id = r),
            'steps', execution.https_loop_steps_for_run(r),
            'inference', (SELECT coalesce(jsonb_agg(jsonb_build_object('decision', ic.d, 'call', ic.call, 'binding', ic.binding, 'state', ic.state, 'reported', ic.usage) ORDER BY ic.d, ic.call), '[]')
                FROM execution.inference_calls ic JOIN execution.https_loop_steps st ON st.run_id = r AND st.decision_id = ic.d WHERE (ic.e, ic.c) = (x.e, x.c)),
            'dns_effect_count', (SELECT count(*) FROM execution.fixture_dns_results WHERE run_id = r),
            'head_effect_count', (SELECT count(*) FROM execution.fixture_https_results WHERE run_id = r),
            'decision_count', (SELECT count(*) FROM cognition.reasoning_decisions cd JOIN execution.https_loop_steps st ON st.run_id = r AND st.decision_id = cd.d WHERE (cd.e, cd.c) = (x.e, x.c)),
            'decision_history_count', (SELECT count(*) FROM trajectory.reasoning_history h JOIN execution.https_loop_steps st ON st.run_id = r AND st.decision_id = h.operation_id WHERE (h.engagement_id, h.campaign_id) = (x.e, x.c) AND h.status = 'accepted'),
            'terrain_history_count', (SELECT count(*) FROM trajectory.terrain_history th JOIN execution.https_loop_steps st ON st.run_id = r AND th.operation_id IN (st.terrain_operation_id, st.head_terrain_operation_id) WHERE th.status = 'accepted'),
            'recorded_decisions', (SELECT coalesce(jsonb_agg(jsonb_build_object('step_index', st.step_index, 'decision', cd.d, 'event', cd.event,
                'version', cd.payload->'version', 'disposition', cd.payload->'disposition', 'proposal', cd.payload->'proposal',
                'occurred_at_ms', cd.payload->'occurred_at_ms', 'recorded_at_ms', cd.payload->'recorded_at_ms', 'dispatch_granted', false,
                'history', CASE WHEN EXISTS (SELECT 1 FROM trajectory.reasoning_history h WHERE (h.engagement_id, h.campaign_id, h.operation_id, h.event_id, h.status, h.contract) = (cd.e, cd.c, cd.d, cd.event, 'accepted', cd.payload)) THEN 'completed' ELSE 'pending_unknown' END)
                ORDER BY st.step_index), '[]')
                FROM execution.https_loop_steps st JOIN cognition.reasoning_decisions cd ON (cd.e, cd.c, cd.d) = (x.e, x.c, st.decision_id) WHERE st.run_id = r),
            'https_results', (SELECT coalesce(jsonb_agg(jsonb_build_object('step_index', hr.step_index, 'attempt', hr.attempt, 'result', hr.result,
                'endpoint_ref', hr.result_payload->'endpoint_ref', 'outcome', hr.result_payload->'outcome', 'status_code', hr.result_payload->'status_code',
                'whole_host_proven', false, 'objective_completed', false) ORDER BY hr.step_index), '[]')
                FROM execution.fixture_https_results hr WHERE hr.run_id = r)
        );
    END IF;
    summary := jsonb_build_object(
        'kind', 'm1_autonomous_run_receipt_v1', 'run_id', r, 'run_profile', x.request->'run_profile',
        'execution_mode', x.request->'execution_mode', 'status', status, 'phase', y.phase, 'dispatch_granted', false,
        'live_qualified', x.request->>'version' = 'm1_autonomous_run_request_v4' AND coalesce((SELECT n >= z.valid_from_ms AND n < z.valid_until_ms AND z.run_profile = x.request->>'run_profile' AND z.release_sha256 = x.release_sha256 AND host(z.resolver_ipv4) = x.request#>>'{live_binding,resolver_ipv4}' AND z.purpose = CASE x.request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END FROM execution.live_qualifications z WHERE z.qualification_ref = x.qualification_ref), false),
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

REVOKE ALL ON FUNCTION
    execution.https_loop_step_record(uuid, integer),
    execution.https_loop_steps_for_run(uuid),
    execution.https_loop_step_predecessor(uuid, integer),
    execution.record_https_loop_dns_intent(uuid, integer, uuid, text),
    execution.record_https_loop_head_intent(uuid, integer, uuid, text, text, text, text, uuid, integer, text),
    execution.record_https_loop_d4_outcome(uuid, text),
    execution.run_https_loop_terminal(uuid, integer, text),
    execution.https_loop_refresh_check(uuid, integer),
    execution.reserve_https_loop_dns(uuid, integer, text, integer),
    execution.commit_https_loop_dns(uuid, integer, text, jsonb),
    execution.reserve_https_loop_head(uuid, integer),
    execution.commit_https_loop_head(uuid, integer, jsonb),
    execution.block_https_loop_head(uuid, integer, text),
    execution.mark_https_loop_effect_unknown(uuid, integer, text, text),
    execution.close_https_loop_effect_session(uuid, text)
FROM PUBLIC;

GRANT EXECUTE ON FUNCTION
    execution.https_loop_step_record(uuid, integer),
    execution.https_loop_steps_for_run(uuid),
    execution.https_loop_step_predecessor(uuid, integer),
    execution.record_https_loop_dns_intent(uuid, integer, uuid, text),
    execution.record_https_loop_head_intent(uuid, integer, uuid, text, text, text, text, uuid, integer, text),
    execution.record_https_loop_d4_outcome(uuid, text),
    execution.run_https_loop_terminal(uuid, integer, text),
    execution.https_loop_refresh_check(uuid, integer),
    execution.reserve_https_loop_dns(uuid, integer, text, integer),
    execution.commit_https_loop_dns(uuid, integer, text, jsonb),
    execution.reserve_https_loop_head(uuid, integer),
    execution.commit_https_loop_head(uuid, integer, jsonb),
    execution.block_https_loop_head(uuid, integer, text),
    execution.mark_https_loop_effect_unknown(uuid, integer, text, text),
    execution.close_https_loop_effect_session(uuid, text)
TO dw_m1_broker;

-- 13. Terrain functions for HTTPS observations, endpoint material, HTTPS context view, and multi-step DNS issuance.
CREATE OR REPLACE FUNCTION terrain.accept_https_observation(req jsonb, req_hash text, payload jsonb, reg_op uuid)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
DECLARE
    v_e uuid; v_c uuid; v_op uuid; existing terrain.https_outbox;
    ev_id uuid; obs_id uuid; mat_id uuid; eval_id uuid; clm_id uuid;
    reg mission.registration_outbox; clm jsonb; evl jsonb; max_rev integer;
    res_rec execution.fixture_https_results;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_terrain', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user, g, 'MEMBER')) THEN
        RAISE EXCEPTION 'unqualified_terrain_writer' USING ERRCODE = '42501';
    END IF;
    IF current_setting('transaction_isolation') <> 'serializable' OR current_setting('synchronous_commit') <> 'on' THEN
        RAISE EXCEPTION 'terrain_transaction_refused';
    END IF;
    v_e := (payload->>'engagement_id')::uuid;
    v_c := (payload->>'campaign_id')::uuid;
    v_op := (payload->>'operation_id')::uuid;
    SELECT * INTO reg FROM mission.registration_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = reg_op;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    SELECT * INTO res_rec FROM execution.fixture_https_results hr WHERE hr.result = (req->>'result_id')::uuid;
    IF NOT FOUND OR res_rec.result_payload IS DISTINCT FROM req THEN RAISE EXCEPTION 'issued_result_unavailable'; END IF;
    SELECT * INTO existing FROM terrain.https_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op FOR UPDATE;
    IF FOUND THEN
        IF existing.request_hash = req_hash AND existing.contract = payload THEN RETURN existing.contract; END IF;
        RAISE EXCEPTION 'terrain_identity_conflict';
    END IF;
    clm := payload->'claim'; evl := payload->'evaluation';
    ev_id := (payload->>'event_id')::uuid;
    obs_id := (evl->>'observation_id')::uuid;
    mat_id := (evl->>'material_id')::uuid;
    eval_id := (evl->>'evaluation_id')::uuid;
    clm_id := (evl->>'claim_id')::uuid;
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
        req->>'redirect', (clm->>'endpoint_reachable')::boolean, false, false,
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

CREATE OR REPLACE FUNCTION terrain.current_dns_endpoint_material(e uuid, c uuid, assets uuid[])
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') THEN
        RAISE EXCEPTION 'unqualified_terrain_reader' USING ERRCODE = '42501';
    END IF;
    RETURN coalesce((
        SELECT jsonb_agg(jsonb_build_object(
            'asset_ref', cc.asset_ref, 'hostname', cc.subject_name, 'claim_id', cc.claim_id,
            'claim_revision', cc.claim_revision, 'vantage_ref', cc.vantage_ref, 'resolver_ipv4', cc.resolver_ipv4,
            'perceived_at', cc.perceived_at, 'lowest_ttl_seconds', ev.lowest_ttl_seconds,
            'valid_until', cc.valid_until, 'effective_status', cc.claim_status, 'conflicted', cc.conflicted,
            'integrity_hold', EXISTS (
                SELECT 1 FROM terrain.integrity_conflicts ic
                WHERE ic.engagement_id = e AND ic.campaign_id = c AND ic.claim_id = cc.claim_id AND ic.blocks_use
            ),
            'answers', coalesce((SELECT jsonb_agg(a->>'ipv4') FROM jsonb_array_elements(cc.answers) a), '[]'::jsonb)
        ) ORDER BY cc.perceived_at, cc.claim_id)
        FROM terrain.current_claims cc
        JOIN terrain.evaluations ev USING (engagement_id, campaign_id, operation_id)
        WHERE cc.engagement_id = e AND cc.campaign_id = c AND cc.asset_ref = ANY(assets)
          AND cc.claim_kind = 'dns_hostname_ipv4_resolution'
    ), '[]'::jsonb);
END $$;

CREATE OR REPLACE FUNCTION terrain.current_https_context_view(e uuid, c uuid, assets uuid[], eval_at bigint)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
#variable_conflict use_variable
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER') THEN
        RAISE EXCEPTION 'unqualified_terrain_reader' USING ERRCODE = '42501';
    END IF;
    RETURN coalesce((
        SELECT jsonb_agg(jsonb_set(o.payload->'claim', '{evaluated_at}', to_jsonb(greatest(o.perceived_at, eval_at)))
            ORDER BY o.perceived_at, o.claim_id)
        FROM terrain.https_observations o
        WHERE o.engagement_id = e AND o.campaign_id = c AND o.asset_ref = ANY(assets)
          AND o.claim_revision = (
              SELECT max(o2.claim_revision) FROM terrain.https_observations o2
              WHERE o2.engagement_id = o.engagement_id AND o2.campaign_id = o.campaign_id
                AND o2.asset_ref = o.asset_ref AND o2.hostname = o.hostname
                AND o2.ipv4 = o.ipv4 AND o2.approved_path = o.approved_path
          )
    ), '[]'::jsonb);
END $$;

CREATE OR REPLACE FUNCTION terrain.issued_dns_request(q jsonb) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
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
        IF q->>'run_profile' NOT IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1')
           OR q->>'execution_mode' IS DISTINCT FROM 'live' THEN
            RAISE EXCEPTION 'issued_reference_refused';
        END IF;
        SELECT jsonb_build_object('request', execution.private_live_issued_request(e, c, result_id), 'operation', x.terrain_operation) INTO raw
        FROM execution.autonomous_runs x JOIN execution.live_dns_results b ON b.run_id = x.run_id
        WHERE b.result = result_id AND (x.e, x.c) = (e, c) AND b.result = x.result AND b.attempt = x.attempt
          AND x.request->>'run_profile' = q->>'run_profile' AND x.request->>'execution_mode' = 'live'
          AND b.publication_obligation = 'terrain.issued_dns.v2';
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
RETURNS jsonb LANGUAGE plpgsql SET search_path = pg_catalog AS $$
DECLARE
    r jsonb; k text; a jsonb; label text; rule jsonb; rules jsonb;
    m mission.missions%ROWTYPE; reg mission.registration_outbox%ROWTYPE;
    f jsonb; policy jsonb; matched boolean := false;
    now_at bigint := floor(extract(epoch FROM statement_timestamp()))::bigint;
    serialized text;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_terrain', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user, g, 'MEMBER'))
       OR EXISTS (SELECT 1 FROM pg_roles p WHERE pg_has_role(session_user, p.oid, 'SET') AND (p.rolsuper OR p.rolbypassrls OR p.rolcreatedb OR p.rolcreaterole)) THEN
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
        ) OR (r->>'version', r->>'source_kind') = ('m1_terrain_dns_issued_observation_v2', 'issued_dns_acquisition') AND EXISTS (
            SELECT 1 FROM execution.live_dns_results b JOIN execution.autonomous_runs x ON x.run_id = b.run_id
            WHERE b.result = (r->>'correlation_id')::uuid AND b.result = x.result AND b.attempt = x.attempt
              AND (x.e, x.c) = ((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid)
              AND x.request->>'execution_mode' = 'live'
              AND x.request->>'run_profile' IN ('autonomous-dns-genai-live-v1', 'autonomous-dns-genai-lab-v1')
              AND b.publication_obligation = 'terrain.issued_dns.v2'
              AND r = execution.private_live_issued_request((r->>'engagement_id')::uuid, (r->>'campaign_id')::uuid, (r->>'correlation_id')::uuid)
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

REVOKE ALL ON FUNCTION
    terrain.accept_https_observation(jsonb, text, jsonb, uuid),
    terrain.current_dns_endpoint_material(uuid, uuid, uuid[]),
    terrain.current_https_context_view(uuid, uuid, uuid[], bigint)
FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;

GRANT EXECUTE ON FUNCTION terrain.accept_https_observation(jsonb, text, jsonb, uuid) TO dw_m1_terrain;
GRANT EXECUTE ON FUNCTION
    terrain.current_dns_endpoint_material(uuid, uuid, uuid[]),
    terrain.current_https_context_view(uuid, uuid, uuid[], bigint)
TO dw_m1_broker;

-- 14. Trajectory terrain history support for typed HTTPS endpoint observations.
CREATE OR REPLACE FUNCTION trajectory.record_terrain_history(p_event jsonb)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    v_e uuid; v_c uuid; v_op uuid;
    v_source terrain.outbox%ROWTYPE; v_https terrain.https_outbox%ROWTYPE;
    v_hist trajectory.terrain_history%ROWTYPE;
    v_source_ok boolean; v_reason text;
    v_now bigint := floor(extract(epoch FROM statement_timestamp()))::bigint;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_trajectory', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_terrain']) g WHERE pg_has_role(session_user, g, 'MEMBER')) THEN
        RAISE EXCEPTION 'unqualified_trajectory_writer' USING ERRCODE = '42501';
    END IF;
    BEGIN
        v_e := (p_event->>'engagement_id')::uuid;
        v_c := (p_event->>'campaign_id')::uuid;
        v_op := (p_event->>'operation_id')::uuid;
    EXCEPTION WHEN invalid_text_representation THEN
        RETURN 'uncommitted';
    END;
    IF p_event#>>'{claim,claim_kind}' = 'https_endpoint_head_observation' THEN
        SELECT * INTO v_https FROM terrain.https_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op;
        IF NOT FOUND THEN RETURN 'uncommitted'; END IF;
        IF EXISTS (SELECT 1 FROM trajectory.terrain_history WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op AND status = 'anomaly') THEN
            RETURN 'anomaly';
        END IF;
        IF NOT EXISTS (
            SELECT 1 FROM mission.registration_outbox r JOIN trajectory.registration_history h
                ON h.engagement_id = r.engagement_id AND h.campaign_id = r.campaign_id
               AND h.operation_id = r.operation_id AND h.event_id = r.event_id AND h.status = 'accepted'
            WHERE r.engagement_id = v_e AND r.campaign_id = v_c
              AND r.operation_id = v_https.registration_operation_id AND r.event_id = v_https.registration_event_id
        ) OR EXISTS (SELECT 1 FROM trajectory.registration_history WHERE engagement_id = v_e AND campaign_id = v_c AND status = 'anomaly') THEN
            RETURN 'pending';
        END IF;
        SELECT EXISTS (
            SELECT 1 FROM terrain.https_observations ob
            WHERE ob.engagement_id = v_e AND ob.campaign_id = v_c AND ob.operation_id = v_op
              AND v_https.contract = p_event
              AND ob.observation_id = v_https.observation_id AND ob.request_hash = v_https.request_hash
              AND ob.material_id = v_https.material_id AND ob.evaluation_id = v_https.evaluation_id
              AND ob.claim_id = v_https.claim_id AND ob.claim_revision = v_https.claim_revision
              AND ob.payload = v_https.contract AND v_https.event_id = (v_https.contract->>'event_id')::uuid
        ) INTO v_source_ok;
        SELECT * INTO v_hist FROM trajectory.terrain_history
        WHERE ((engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op) OR event_id = v_https.event_id)
          AND status = 'accepted' LIMIT 1;
        IF FOUND THEN
            IF v_source_ok AND v_hist.contract = v_https.contract
               AND v_hist.engagement_id = v_e AND v_hist.campaign_id = v_c AND v_hist.operation_id = v_op
               AND v_hist.event_id = v_https.event_id AND v_hist.observation_id = v_https.observation_id
               AND v_hist.material_id = v_https.material_id AND v_hist.evaluation_id = v_https.evaluation_id
               AND v_hist.claim_id = v_https.claim_id THEN
                RETURN 'completed';
            END IF;
            v_reason := 'contract_conflict';
        ELSIF NOT v_source_ok THEN
            v_reason := 'source_conflict';
        END IF;
        IF v_reason IS NOT NULL THEN
            INSERT INTO trajectory.terrain_history
                (engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id, status, reason, contract, recorded_at)
            VALUES (v_e, v_c, v_op, v_https.event_id, v_https.observation_id, v_https.material_id, v_https.evaluation_id, v_https.claim_id,
                'anomaly', v_reason, jsonb_build_object('category', v_reason, 'affected_claim_id', v_https.claim_id), v_now);
            RETURN 'anomaly';
        END IF;
        INSERT INTO trajectory.terrain_history
            (engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id, status, reason, contract, recorded_at)
        VALUES (v_e, v_c, v_op, v_https.event_id, v_https.observation_id, v_https.material_id, v_https.evaluation_id, v_https.claim_id,
            'accepted', NULL, v_https.contract, v_now);
        RETURN 'completed';
    END IF;
    SELECT * INTO v_source FROM terrain.outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op;
    IF NOT FOUND THEN RETURN 'uncommitted'; END IF;
    IF EXISTS (SELECT 1 FROM trajectory.terrain_history WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op AND status = 'anomaly')
       OR EXISTS (SELECT 1 FROM terrain.integrity_conflicts WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op AND blocks_use) THEN
        RETURN 'anomaly';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM mission.registration_outbox r JOIN trajectory.registration_history h
            ON h.engagement_id = r.engagement_id AND h.campaign_id = r.campaign_id
           AND h.operation_id = r.operation_id AND h.event_id = r.event_id AND h.status = 'accepted'
        WHERE r.engagement_id = v_e AND r.campaign_id = v_c
          AND r.operation_id = (v_source.contract->>'registration_operation_id')::uuid
          AND r.event_id = (v_source.contract->>'registration_event_id')::uuid
    ) OR EXISTS (SELECT 1 FROM trajectory.registration_history WHERE engagement_id = v_e AND campaign_id = v_c AND status = 'anomaly') THEN
        RETURN 'pending';
    END IF;
    SELECT EXISTS (
        SELECT 1 FROM terrain.observations ob
        JOIN terrain.materials m USING (engagement_id, campaign_id, operation_id)
        JOIN terrain.evaluations ev USING (engagement_id, campaign_id, operation_id)
        JOIN terrain.claim_revisions cr USING (engagement_id, campaign_id, operation_id)
        WHERE ob.engagement_id = v_e AND ob.campaign_id = v_c AND ob.operation_id = v_op
          AND v_source.contract = p_event
          AND ob.observation_id = v_source.observation_id AND ob.request_hash = v_source.request_hash
          AND ob.payload = (v_source.contract->'observation')
          AND m.material_id = v_source.material_id AND m.observation_id = ob.observation_id
          AND m.payload = (v_source.contract->'material')
          AND ev.evaluation_id = v_source.evaluation_id AND ev.claim_id = v_source.claim_id
          AND ev.payload = (v_source.contract->'evaluation')
          AND cr.claim_id = v_source.claim_id AND cr.claim_revision = v_source.claim_revision
          AND cr.payload = (v_source.contract->'claim')
          AND v_source.event_id = (v_source.contract->>'event_id')::uuid
    ) INTO v_source_ok;
    SELECT * INTO v_hist FROM trajectory.terrain_history
    WHERE ((engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op) OR event_id = v_source.event_id) AND status = 'accepted' LIMIT 1;
    IF FOUND THEN
        IF v_source_ok AND v_hist.contract = v_source.contract
           AND v_hist.engagement_id = v_e AND v_hist.campaign_id = v_c AND v_hist.operation_id = v_op
           AND v_hist.event_id = v_source.event_id AND v_hist.observation_id = v_source.observation_id
           AND v_hist.material_id = v_source.material_id AND v_hist.evaluation_id = v_source.evaluation_id
           AND v_hist.claim_id = v_source.claim_id THEN
            RETURN 'completed';
        END IF;
        v_reason := 'contract_conflict';
    ELSIF NOT v_source_ok THEN
        v_reason := 'source_conflict';
    END IF;
    IF v_reason IS NOT NULL THEN
        INSERT INTO trajectory.terrain_history
            (engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id, status, reason, contract, recorded_at)
        VALUES (v_e, v_c, v_op, v_source.event_id, v_source.observation_id, v_source.material_id, v_source.evaluation_id, v_source.claim_id,
            'anomaly', v_reason, jsonb_build_object('category', v_reason, 'affected_claim_id', v_source.claim_id), v_now);
        RETURN 'anomaly';
    END IF;
    INSERT INTO trajectory.terrain_history
        (engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id, status, reason, contract, recorded_at)
    VALUES (v_e, v_c, v_op, v_source.event_id, v_source.observation_id, v_source.material_id, v_source.evaluation_id, v_source.claim_id,
        'accepted', NULL, v_source.contract, v_now);
    RETURN 'completed';
END $$;

COMMIT;
