-- DW-M1-HTTPS-PROVIDER-FIXTURE-R1: admit the closed actual Pro-v2 profile on the
-- controlled-fixture m1_https_loop_v1 tuple. The two functions below are replaced
-- verbatim from 0038 except for the scoped profile admission; LAB stays fixture-only.
BEGIN;

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
        jsonb_typeof(p#>'{operation_limits,request_bytes}') = 'number'
        AND p#>>'{operation_limits,request_bytes}' = '16384'
        AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback', 'admitted_repair_fields_codes']
        AND NOT (p ? 'qualification_purpose')
        AND (
            ((p->>'run_profile', p->>'execution_mode') = ('m1_https_loop_v1', 'controlled_fixture')
                AND (p->>'profile', p->>'recipient') IN (
                    ('fixture-genai-v4-pro-low-v2', 'http://127.0.0.1:18083/responses'),
                    ('deepseek-genai-v4-pro-low-v2', 'https://api.deepseek.com/responses')
                )
                AND NOT (p ? 'live_binding'))
            OR ((p->>'run_profile', p->>'execution_mode') = ('autonomous-https-genai-lab-v1', 'live')
                AND (p->>'profile', p->>'recipient') = ('fixture-genai-v4-pro-low-v2', 'http://127.0.0.1:18083/responses')
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

REVOKE ALL ON FUNCTION execution.inference_profile_matches(jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.inference_profile_matches(jsonb) TO dw_m1_broker;

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
       OR (hl AND NOT (
           (NOT hl_live AND q->>'inference_profile' IN ('fixture-genai-v4-pro-low-v2', 'deepseek-genai-v4-pro-low-v2'))
           OR (hl_live AND q->>'inference_profile' = 'fixture-genai-v4-pro-low-v2')
       )) THEN
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

REVOKE ALL ON FUNCTION execution.run_start(jsonb, text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.run_start(jsonb, text) TO dw_m1_broker;

COMMIT;
