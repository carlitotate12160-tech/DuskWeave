-- Closed Pro-v2 resource policy (ADR-016) and companion loopback mirror.
-- CREATE OR REPLACE FUNCTION preserves installed OIDs, owners and ACLs.
BEGIN;

CREATE OR REPLACE FUNCTION execution.inference_profile_matches(p jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path=pg_catalog, pg_temp AS $$
 SELECT coalesce(CASE WHEN p->>'version'='m1_inference_permission_v6' THEN
 (p->>'profile',p->>'recipient') IN (
  ('deepseek-genai-responses-low-v1','https://api.deepseek.com/responses'),
  ('deepseek-genai-v4-pro-low-v1','https://api.deepseek.com/responses'),
  ('deepseek-genai-v4-pro-low-v2','https://api.deepseek.com/responses'),
  ('fixture-genai-responses-v1','http://127.0.0.1:18083/responses'))
 AND jsonb_typeof(p#>'{operation_limits,request_bytes}')='number'
 AND p#>>'{operation_limits,request_bytes}'='16384'
 AND (p->>'run_profile',p->>'execution_mode') IN (('autonomous-dns-genai-live-v1','live'),('autonomous-dns-genai-lab-v1','live'))
 AND (p->>'profile' IN ('deepseek-genai-responses-low-v1','deepseek-genai-v4-pro-low-v1','deepseek-genai-v4-pro-low-v2'))=(p->>'run_profile'='autonomous-dns-genai-live-v1')
 AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback','admitted_repair_fields_codes']
 AND jsonb_typeof(p->'live_binding')='object'
 AND (p->'live_binding')-ARRAY['qualification_ref','release_sha256','resolver_ipv4']='{}'::jsonb
 AND (p#>>'{live_binding,qualification_ref}')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
 AND p#>>'{live_binding,release_sha256}' ~ '^[0-9a-f]{64}$'
 AND host((p#>>'{live_binding,resolver_ipv4}')::inet)=p#>>'{live_binding,resolver_ipv4}'
 AND family((p#>>'{live_binding,resolver_ipv4}')::inet)=4
 AND (p->>'run_profile'='autonomous-dns-genai-lab-v1')=(p#>>'{live_binding,resolver_ipv4}'='192.0.2.53')
 AND NOT EXISTS(SELECT 1 FROM (VALUES
  ('output_tokens_per_call',CASE WHEN p->>'profile'='deepseek-genai-v4-pro-low-v2' THEN 8192 ELSE 4096 END),
  ('decision_wall_time_ms',CASE WHEN p->>'profile'='deepseek-genai-v4-pro-low-v2' THEN 249000 ELSE 129000 END),
  ('per_call_wall_time_ms',CASE WHEN p->>'profile'='deepseek-genai-v4-pro-low-v2' THEN 122000 ELSE 62000 END),
  ('soft_completion_ms',CASE WHEN p->>'profile'='deepseek-genai-v4-pro-low-v2' THEN 120000 ELSE 60000 END),
  ('shared_wall_time_ms',CASE WHEN p->>'profile'='deepseek-genai-v4-pro-low-v2' THEN 244000 ELSE 124000 END),
  ('phase_wall_time_ms',CASE WHEN p->>'profile'='deepseek-genai-v4-pro-low-v2' THEN 254000 ELSE 134000 END),
  ('cancel_grace_ms',2000),('accounting_window_ms',3000)) b(k,v)
 WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
 OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text)
 WHEN p->>'version'='m1_inference_permission_v5' THEN
 (p->>'profile',p->>'recipient') IN (
  ('deepseek-genai-responses-low-v1','https://api.deepseek.com/responses'),
  ('deepseek-genai-v4-pro-low-v1','https://api.deepseek.com/responses'),
  ('deepseek-genai-v4-pro-low-v2','https://api.deepseek.com/responses'),
  ('fixture-genai-responses-v1','http://127.0.0.1:18083/responses'),
  ('fixture-genai-v4-pro-low-v2','http://127.0.0.1:18083/responses'))
 AND jsonb_typeof(p#>'{operation_limits,request_bytes}')='number'
 AND p#>>'{operation_limits,request_bytes}'='16384'
 AND (p->>'run_profile',p->>'execution_mode')=('autonomous-dns-fixture-v2','controlled_fixture')
 AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback','admitted_repair_fields_codes']
 AND NOT EXISTS(SELECT 1 FROM (VALUES
  ('output_tokens_per_call',CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 8192 ELSE 4096 END),
  ('decision_wall_time_ms',CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 249000 ELSE 129000 END),
  ('per_call_wall_time_ms',CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 122000 ELSE 62000 END),
  ('soft_completion_ms',CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 120000 ELSE 60000 END),
  ('shared_wall_time_ms',CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 244000 ELSE 124000 END),
  ('phase_wall_time_ms',CASE WHEN p->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 254000 ELSE 134000 END),
  ('cancel_grace_ms',2000),('accounting_window_ms',3000)) b(k,v)
 WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
 OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text)
 WHEN p->>'version'='m1_inference_permission_v4' THEN
 (p->>'profile',p->>'recipient') IN (
  ('deepseek-responses-reasoning-v1','https://api.deepseek.com/responses'),
  ('deepseek-responses-low-v1','https://api.deepseek.com/responses'),
  ('fixture-responses-v1','urn:duskweave:fixture:responses:v1'),
  ('fixture-chat-completions-v1','urn:duskweave:fixture:chat-completions:v1'))
 AND (p->>'run_profile',p->>'execution_mode')=('autonomous-dns-fixture-v2','controlled_fixture')
 AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback','admitted_repair_fields_codes']
 AND NOT EXISTS(SELECT 1 FROM (VALUES ('decision_wall_time_ms',129000),('per_call_wall_time_ms',62000),
  ('soft_completion_ms',60000),('shared_wall_time_ms',124000),('phase_wall_time_ms',134000),
  ('cancel_grace_ms',2000),('accounting_window_ms',3000)) b(k,v)
  WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
  OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text)
 WHEN p->>'version'='m1_inference_permission_v3' THEN
  (p->>'run_profile',p->>'execution_mode',p->>'profile',p->>'recipient') IN (
   ('autonomous-dns-fixture-v1','controlled_fixture','deterministic-autonomous-v1','urn:duskweave:fixture:inference:v1'),
   ('autonomous-dns-provider-fixture-v1','controlled_fixture','deepseek-responses-autonomous-v1','https://api.deepseek.com/responses'),
   ('autonomous-dns-v1','live','deepseek-responses-autonomous-v1','https://api.deepseek.com/responses'))
  AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback','admitted_repair_fields_codes']
  AND NOT EXISTS(SELECT 1 FROM (VALUES ('decision_wall_time_ms',45000),('per_call_wall_time_ms',20000),
   ('shared_wall_time_ms',40000),('phase_wall_time_ms',50000),('cancel_grace_ms',2000),('accounting_window_ms',3000)) b(k,v)
   WHERE p->'operation_limits'->b.k IS DISTINCT FROM to_jsonb(b.v))
 ELSE (p->>'version',p->>'profile') IN (('m1_inference_permission_v1','deepseek-responses-v1'),
   ('m1_inference_permission_v2','deepseek-responses-envelope32-v1'))
  AND p->>'recipient'='https://api.deepseek.com/responses'
  AND NOT p ?| ARRAY['run_profile','execution_mode'] END,false)
$$;

CREATE OR REPLACE FUNCTION execution.inference_reservation(
    action text,op uuid,q jsonb,expected_source jsonb
) RETURNS execution.inference_reservations
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE
    e uuid; c uuid;
    actor oid := (SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user);
    prior execution.inference_reservations;
    source jsonb; consent jsonb; limits jsonb; totals jsonb;
    cap jsonb; held jsonb;
    now_s bigint; next_generation bigint; claimed uuid; pro_v2 boolean;
BEGIN
    IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER')
       OR EXISTS (SELECT 1 FROM pg_catalog.pg_roles WHERE oid=actor AND (rolsuper OR rolbypassrls))
       OR current_setting('fsync')<>'on' OR current_setting('full_page_writes')<>'on'
       OR current_setting('synchronous_commit')<>'on'
       OR action NOT IN ('reserve','recover','cancel') OR action IS NULL
       OR op IS NULL OR op='00000000-0000-0000-0000-000000000000'
       OR jsonb_typeof(q) IS DISTINCT FROM 'object'
       OR q->'version' IS DISTINCT FROM '"m1_inference_reservation_request_v1"'::jsonb
       OR q->'expected_mission_revision' IS DISTINCT FROM '1'::jsonb
       OR q->>'expected_mission_revision' IS DISTINCT FROM '1'
       OR coalesce(q->>'exercise_mode','') NOT IN ('blind','defender_informed')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['engagement_id','campaign_id',
           'operator_ref','permission_id','goal_ref']) k
           WHERE jsonb_typeof(q->k) IS DISTINCT FROM 'string'
           OR coalesce(q->>k,'') !~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
           OR q->>k='00000000-0000-0000-0000-000000000000')
       OR q-ARRAY['version','engagement_id','campaign_id','operator_ref',
                   'expected_mission_revision','permission_id','goal_ref','exercise_mode']<>'{}' THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_request_refused';
    END IF;
    e := (q->>'engagement_id')::uuid; c := (q->>'campaign_id')::uuid;
    -- Recover is historical/read-only. Mutating actions order before their one
    -- identity lookup; duplicate receipts require no epoch or ledger mutation.
    IF action<>'recover' THEN
        PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text,0));
    END IF;
    SELECT * INTO prior FROM execution.inference_reservations
    WHERE engagement_id=e AND campaign_id=c AND operation_id=op
    ORDER BY record_revision DESC LIMIT 1;
    IF FOUND THEN
        IF prior.writer_oid<>actor OR prior.request IS DISTINCT FROM q THEN
            RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_identity_conflict';
        END IF;
        IF action<>'cancel' OR prior.kind='cancelled_never_sent' THEN RETURN prior; END IF;
    ELSIF action='recover' THEN RETURN NULL;
    ELSIF action='cancel' THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_missing_reservation';
    END IF;
    UPDATE execution.session_fences SET epoch=epoch+1
    WHERE engagement_id=e AND campaign_id=c RETURNING operation_id INTO claimed;
    IF NOT FOUND THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_mission_missing';
    END IF;
    IF prior.operation_id IS NOT NULL THEN
        IF prior.kind<>'reserved_not_sent' THEN
            RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_cancellation_refused';
        END IF;
        prior.kind := 'cancelled_never_sent'; prior.record_revision := 2;
        prior.recorded_at := clock_timestamp();
        INSERT INTO execution.inference_reservations SELECT prior.* RETURNING * INTO prior;
        RETURN prior;
    END IF;
    IF claimed IS NOT NULL THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_busy';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.session_history
        WHERE engagement_id=e AND campaign_id=c AND operation_id=op)
       OR EXISTS (SELECT 1 FROM execution.prepared_withdrawals
        WHERE engagement_id=e AND campaign_id=c AND operation_id=op) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_identity_conflict';
    END IF;
    source := mission.current_inference_publication(e,c,op);
    IF source IS NULL OR source IS DISTINCT FROM expected_source THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_authority_unavailable';
    END IF;
    consent := source->'fields'->'m1_inference_permission';
    IF consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN
        IF NOT coalesce(consent->'field_classes' ? 'admitted_repair_fields_codes',false)
        OR NOT EXISTS(SELECT 1 FROM execution.native_input_bindings i WHERE (i.e,i.c,i.d,i.writer_oid)=(e,c,op,actor))
        THEN RAISE EXCEPTION 'inference_input_or_consent_refused'; END IF;
        PERFORM execution.inference_window_ms(e,c,op);
    END IF;
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    IF q IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1',
       'engagement_id',e,'campaign_id',c,'operator_ref',source->'fields'->'operator_ref',
       'expected_mission_revision',1,'permission_id',consent->'permission_id',
       'goal_ref',source->'fields'->'goal_ref','exercise_mode',source->'fields'->'exercise_mode')
       OR (source->'fields'->>'starts_at')::bigint>now_s
       OR now_s>=(source->'fields'->>'ends_at')::bigint
       OR (consent->>'starts_at')::bigint>now_s OR now_s>=(consent->>'ends_at')::bigint
       OR NOT execution.inference_profile_matches(consent)
       OR consent->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_consent_refused';
    END IF;
    IF NOT execution.inference_profile_matches(consent)
       OR (consent->>'provider_terms_ref')::uuid='00000000-0000-0000-0000-000000000000'
       OR (consent->'engagement_id',consent->'campaign_id',consent->'operator_ref',
           consent->'authority_ref',consent->'authority_revision',consent->'goal_ref',
           consent->'exercise_mode',consent->'mission_revision') IS DISTINCT FROM
          (source->'engagement_id',source->'campaign_id',source->'fields'->'operator_ref',
           source->'fields'->'authority_ref',source->'fields'->'authority_revision',
           source->'fields'->'goal_ref',source->'fields'->'exercise_mode','1'::jsonb)
       OR (consent->>'starts_at')::bigint<(source->'fields'->>'starts_at')::bigint
       OR (consent->>'ends_at')::bigint>(source->'fields'->>'ends_at')::bigint THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_consent_refused';
    END IF;
    limits := consent->'operation_limits'; totals := consent->'campaign_limits';
    pro_v2 := coalesce(consent->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6')
       AND consent->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2'),false);
    IF (SELECT count(*) FROM jsonb_object_keys(consent))<>(CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN CASE WHEN consent->>'version'='m1_inference_permission_v6' THEN 22 ELSE 21 END ELSE 19 END)
       OR jsonb_typeof(consent->'provider_terms_ref') IS DISTINCT FROM 'string'
       OR (consent->>'provider_terms_ref')::uuid IS NULL
       OR jsonb_typeof(consent->'field_classes') IS DISTINCT FROM 'array'
       OR jsonb_array_length(consent->'field_classes') NOT BETWEEN 1 AND (CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 9 ELSE 8 END)
       OR (SELECT count(DISTINCT v) FROM jsonb_array_elements(consent->'field_classes') v)
          <>jsonb_array_length(consent->'field_classes')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(consent->'field_classes') v
           WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(consent->'field_classes') v
           WHERE v NOT IN ('mission_question_constraints','scoped_opaque_references',
           'owner_qualified_summaries','admitted_operator_premises','manifest_availability',
           'omissions_unknown','capacity','admitted_repair_fields_codes') AND NOT (consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') AND v='admitted_campaign_decision_feedback'))
       OR (SELECT count(*) FROM jsonb_object_keys(limits))<>(CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 22 ELSE 16 END)
       OR (SELECT count(*) FROM jsonb_object_keys(totals))<>5
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['starts_at','ends_at',
           'authority_revision','mission_revision']) k
           WHERE jsonb_typeof(consent->k) IS DISTINCT FROM 'number'
           OR consent->>k !~ '^-?[0-9]+$') THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_consent_refused';
    END IF;
    IF EXISTS (SELECT 1 FROM (VALUES ('request_bytes',1,16384),('response_bytes',1,65536),
        ('decision_wall_time_ms',1,CASE WHEN pro_v2 THEN 249000 WHEN consent->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 129000 WHEN consent->>'version'='m1_inference_permission_v3' THEN 45000 ELSE 20000 END),
        ('context_bytes',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 8192 ELSE 4096 END),
        ('candidates',1,8),('configuration_bytes',1,16384),('connect_time_ms',1,5000),('proposal_bytes',1,8192),
        ('repair_bytes',1,2048),('text_field_bytes',1,1024),('repair_error_codes',1,8),
        ('json_depth',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 32 ELSE 8 END),
        ('collection_entries',1,64),('calls_per_decision',2,2),
        ('output_tokens_per_call',CASE WHEN pro_v2 THEN 8192 ELSE 4096 END,CASE WHEN pro_v2 THEN 8192 ELSE 4096 END),('concurrency',1,1)) AS b(name,floor,ceiling)
        WHERE CASE WHEN jsonb_typeof(limits->b.name)='number'
            AND limits->>b.name ~ '^[0-9]+$'
            THEN (limits->>b.name)::numeric<b.floor
                OR (limits->>b.name)::numeric>b.ceiling
            ELSE true END) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_limits_refused';
    END IF;
    IF (limits->>'connect_time_ms')::bigint>(limits->>'decision_wall_time_ms')::bigint THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_limits_refused';
    END IF;
    cap := jsonb_build_object('calls',2,'request_bytes',2*(limits->>'request_bytes')::bigint,
        'response_bytes',2*(limits->>'response_bytes')::bigint,
        'output_token_capacity',2*(limits->>'output_tokens_per_call')::bigint,
        'wall_time_ms',(limits->>'decision_wall_time_ms')::bigint);
    SELECT jsonb_object_agg(k,used) INTO held FROM (
        SELECT v.key AS k,coalesce(sum((h.capacity->>v.key)::numeric),0) AS used
        FROM jsonb_each(cap) v LEFT JOIN execution.inference_reservations h
        ON h.engagement_id=e AND h.campaign_id=c AND h.record_revision=1
        AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x
            WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,h.operation_id)
            AND x.kind='cancelled_never_sent') GROUP BY v.key) s;
    IF EXISTS (SELECT 1 FROM jsonb_each(cap) v
        WHERE jsonb_typeof(totals->v.key) IS DISTINCT FROM 'number'
        OR totals->>v.key !~ '^[0-9]+$' OR (totals->>v.key)::numeric<=0
        OR (totals->>v.key)::numeric>9223372036854775807
        OR (held->>v.key)::numeric+(v.value::text)::numeric>(totals->>v.key)::numeric) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_capacity_exhausted';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.inference_reservations h
        WHERE h.engagement_id=e AND h.campaign_id=c AND h.record_revision=1
        AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x
            WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,h.operation_id)
            AND x.kind='cancelled_never_sent')
        AND NOT EXISTS(SELECT 1 FROM execution.inference_closures z WHERE (z.e,z.c,z.d)=(e,c,h.operation_id))) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_busy';
    END IF;
    SELECT coalesce(max(generation),0)+1 INTO next_generation
    FROM execution.inference_reservations WHERE engagement_id=e AND campaign_id=c;
    INSERT INTO execution.inference_reservations
        (engagement_id,campaign_id,operation_id,generation,record_revision,writer_oid,
         request,registration_operation_id,registration_event_id,generation_call_id,
         repair_call_id,capacity,kind)
    VALUES (e,c,op,next_generation,1,actor,q,(source->>'operation_id')::uuid,
        (source->>'event_id')::uuid,gen_random_uuid(),gen_random_uuid(),cap,'reserved_not_sent')
    RETURNING * INTO prior;
    RETURN prior;
END $$;

CREATE OR REPLACE FUNCTION execution.inference_window_ms(e uuid,c uuid,d uuid) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE s jsonb;p jsonb;n bigint;l jsonb;run uuid;phase text;started bigint;phase_end bigint;decision_end bigint;w execution.inference_decision_windows;pro_v2 boolean;
BEGIN
 s:=mission.current_inference_publication(e,c,d);p:=s->'fields'->'m1_inference_permission';l:=p->'operation_limits';
 IF EXISTS(SELECT 1 FROM execution.inference_closures z LEFT JOIN cognition.reasoning_decisions q
 ON (q.e,q.c,q.d)=(z.e,z.c,z.d) WHERE (z.e,z.c)=(e,c) AND (
 q.event IS NULL OR NOT EXISTS(SELECT 1 FROM trajectory.reasoning_history h
 WHERE (h.engagement_id,h.campaign_id,h.operation_id,h.event_id,h.status,h.contract)=(z.e,z.c,z.d,q.event,'accepted',q.payload))
 OR EXISTS(SELECT 1 FROM trajectory.reasoning_history h WHERE (h.engagement_id,h.campaign_id)=(z.e,z.c)
 AND (h.operation_id=z.d OR h.event_id=q.event) AND h.status='anomaly')))
 THEN RAISE EXCEPTION 'inference_prior_history_unqualified'; END IF;
 IF p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN
  SELECT x.run_id, CASE WHEN x.decision_one=d THEN 'decision_one' ELSE 'decision_two' END INTO run,phase
  FROM execution.autonomous_runs x WHERE (x.e,x.c)=(e,c) AND d IN(x.decision_one,x.decision_two);
  PERFORM execution.run_current(run);
  IF NOT EXISTS(SELECT 1 FROM execution.autonomous_control y WHERE y.run_id=run AND y.phase=phase AND y.claimed)
  THEN RAISE EXCEPTION 'run_phase_refused';END IF;
 END IF;
 pro_v2:=coalesce(p->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6')
  AND p->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2'),false);
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF s IS NULL OR NOT execution.inference_profile_matches(p) OR p->>'version'='m1_inference_permission_v1'
 OR p->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb
 OR NOT coalesce(p->'field_classes' ? 'admitted_repair_fields_codes',false)
 OR n<(s->'fields'->>'starts_at')::bigint*1000 OR n<(p->>'starts_at')::bigint*1000
 OR EXISTS(SELECT 1 FROM (VALUES ('json_depth',32),('collection_entries',64),('response_bytes',65536),('proposal_bytes',8192),('text_field_bytes',1024),('repair_bytes',2048),('repair_error_codes',8),('connect_time_ms',5000),('calls_per_decision',2),('output_tokens_per_call',CASE WHEN pro_v2 THEN 8192 ELSE 4096 END),('concurrency',1)) b(k,v) WHERE l->b.k IS DISTINCT FROM to_jsonb(b.v))
 THEN RAISE EXCEPTION 'inference_authority_unavailable'; END IF;
 IF p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN
  SELECT * INTO w FROM execution.inference_decision_windows a WHERE (a.e,a.c,a.d)=(e,c,d);
  IF NOT FOUND THEN
   SELECT recorded_at_ms INTO started FROM execution.autonomous_phases WHERE run_id=run AND autonomous_phases.phase=phase AND status='started';
   IF started IS NULL OR n>=started+5000 THEN RAISE EXCEPTION 'inference_preparation_expired'; END IF;
   SELECT least(started+CASE WHEN pro_v2 THEN 254000 ELSE 134000 END,x.deadline_at_ms,(p->>'ends_at')::bigint*1000,(s#>>'{fields,ends_at}')::bigint*1000)
    INTO phase_end FROM execution.autonomous_runs x WHERE x.run_id=run;
   decision_end:=least(n+CASE WHEN pro_v2 THEN 249000 ELSE 129000 END,phase_end);
   INSERT INTO execution.inference_decision_windows VALUES(e,c,d,(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user),
    n,least(n+CASE WHEN pro_v2 THEN 244000 ELSE 124000 END,decision_end-5000),decision_end,phase_end) ON CONFLICT DO NOTHING;
   SELECT * INTO w FROM execution.inference_decision_windows a WHERE (a.e,a.c,a.d)=(e,c,d);
  END IF;
  IF w.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user) OR n>=w.active_until_ms
  THEN RAISE EXCEPTION 'inference_deadline'; END IF;
  RETURN least(w.active_until_ms-n,(s#>>'{fields,ends_at}')::bigint*1000-n,(p->>'ends_at')::bigint*1000-n);
 END IF;
 RETURN least(CASE WHEN p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN CASE WHEN pro_v2 THEN 244000 WHEN p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 124000 ELSE 40000 END ELSE 20000 END,(l->>'decision_wall_time_ms')::bigint,(s->'fields'->>'ends_at')::bigint*1000-n,(p->>'ends_at')::bigint*1000-n);
END $$;

CREATE OR REPLACE FUNCTION execution.inference_may_send(e uuid,c uuid,d uuid,k uuid,p text,cfg text,b text,h text)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE r execution.inference_reservations;s jsonb;perm jsonb;now_s bigint;actual text;remaining bigint;min_call_ms bigint;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') THEN RAISE EXCEPTION 'inference_actor_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(e::text||':'||c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=e AND campaign_id=c;
 SELECT * INTO r FROM execution.inference_reservations x
 WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,d) AND x.record_revision=1;
 IF r.operation_id IS NULL OR r.writer_oid<>(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user)
 OR NOT EXISTS(SELECT 1 FROM execution.native_input_bindings i WHERE (i.e,i.c,i.d,i.writer_oid)=(e,c,d,r.writer_oid))
 OR k NOT IN (r.generation_call_id,r.repair_call_id)
 OR EXISTS(SELECT 1 FROM execution.inference_reservations x
 WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,d) AND x.record_revision=2)
 THEN
 RAISE EXCEPTION 'inference_reservation_refused'; END IF;
 s:=mission.current_inference_publication(e,c,d);perm:=s->'fields'->'m1_inference_permission';
 remaining:=execution.inference_window_ms(e,c,d);
 min_call_ms:=CASE WHEN perm->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6')
  AND perm->>'profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 122000 ELSE 62000 END;
 IF perm->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') AND remaining<min_call_ms THEN RAISE EXCEPTION 'inference_call_window_refused'; END IF;
 IF NOT coalesce(perm->'field_classes' ? 'admitted_repair_fields_codes',false) THEN RAISE EXCEPTION 'inference_consent_refused'; END IF;
 IF perm->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN PERFORM execution.assert_autonomous_pack(e,c,d,p::jsonb,cfg::jsonb);END IF;
 IF perm->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6') THEN
  IF NOT execution.genai_binding_valid(e,c,d,k,p,cfg,b,h) THEN RAISE EXCEPTION 'inference_intent_refused'; END IF;
  PERFORM execution.assert_current_pack_terrain(e,c,d,p::jsonb);
  IF k=r.repair_call_id AND NOT EXISTS(SELECT 1 FROM execution.inference_calls a WHERE a.call=r.generation_call_id AND a.p=p AND a.state='completed') THEN RAISE EXCEPTION 'inference_repair_refused'; END IF;
 ELSIF perm->>'version'='m1_inference_permission_v4' THEN
  PERFORM execution.assert_successor_wire(p,cfg::jsonb,b::jsonb,k=r.repair_call_id);
  PERFORM execution.assert_current_pack_terrain(e,c,d,p::jsonb);
  IF k=r.repair_call_id AND NOT EXISTS(SELECT 1 FROM execution.inference_calls a
    WHERE a.call=r.generation_call_id AND a.p=p AND a.configuration=cfg AND a.state='completed')
  THEN RAISE EXCEPTION 'inference_repair_refused'; END IF;
 END IF;
 now_s:=floor(extract(epoch FROM clock_timestamp()))::bigint;
 IF s IS NULL OR perm->>'permission_id' IS DISTINCT FROM r.request->>'permission_id'
 OR now_s<(s->'fields'->>'starts_at')::bigint OR now_s>=(s->'fields'->>'ends_at')::bigint
 OR now_s<(perm->>'starts_at')::bigint OR now_s>=(perm->>'ends_at')::bigint
 OR octet_length(p)>(perm->'operation_limits'->>'context_bytes')::int
 OR octet_length(cfg)>(perm->'operation_limits'->>'configuration_bytes')::int
 OR octet_length(b)>(perm->'operation_limits'->>'request_bytes')::int
 OR cfg::jsonb->>'recipient' IS DISTINCT FROM perm->>'recipient'
 OR cfg::jsonb->>'profile' IS DISTINCT FROM perm->>'profile'
 OR (CASE WHEN perm->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6') THEN false ELSE (CASE WHEN cfg::jsonb->>'protocol'='chat-completions-v1' THEN b::jsonb#>>'{messages,1,content}' ELSE b::jsonb->>'input' END) IS DISTINCT FROM p AND k=r.generation_call_id END) THEN
 RAISE EXCEPTION 'inference_authority_refused'; END IF;
 IF perm->>'version' IN ('m1_inference_permission_v5','m1_inference_permission_v6') THEN actual:=execution.genai_binding_digest(p,cfg);
 ELSE actual:=encode(sha256(convert_to('dw-inference-binding-v1','UTF8')||decode('00','hex')||
 int8send(octet_length(p)::bigint)||convert_to(p,'UTF8')||
 int8send(octet_length(cfg)::bigint)||convert_to(cfg,'UTF8')||
 int8send(octet_length(b)::bigint)||convert_to(b,'UTF8')),'hex');
 END IF;
 IF actual<>h THEN RAISE EXCEPTION 'inference_binding_refused'; END IF;
 IF k=r.repair_call_id AND NOT EXISTS(SELECT 1 FROM execution.inference_calls x
 WHERE x.call=r.generation_call_id AND x.state='completed') THEN
 RAISE EXCEPTION 'inference_repair_refused'; END IF;
 INSERT INTO execution.inference_calls VALUES(e,c,d,k,h,p,cfg,b,'may_send_uncertain',NULL);
END $$;

CREATE OR REPLACE FUNCTION execution.genai_expected_intent(p jsonb,r execution.inference_reservations,k uuid,codes jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog, pg_temp AS $$
DECLARE i jsonb; recipient text; pro_v2 boolean; out_tokens integer; soft_ms bigint; hard_ms bigint; shared_ms bigint; decision_ms bigint; phase_ms bigint;
BEGIN
 recipient:=CASE p->>'inference_profile'
 WHEN 'fixture-genai-responses-v1' THEN 'http://127.0.0.1:18083/responses'
 WHEN 'fixture-genai-v4-pro-low-v2' THEN 'http://127.0.0.1:18083/responses'
 WHEN 'deepseek-genai-responses-low-v1' THEN 'https://api.deepseek.com/responses'
 WHEN 'deepseek-genai-v4-pro-low-v1' THEN 'https://api.deepseek.com/responses'
 WHEN 'deepseek-genai-v4-pro-low-v2' THEN 'https://api.deepseek.com/responses'
 ELSE NULL END;
 IF recipient IS NULL OR k NOT IN(r.generation_call_id,r.repair_call_id) OR r.generation<1
 OR r.generation_call_id=r.repair_call_id OR r.generation_call_id='00000000-0000-0000-0000-000000000000'::uuid
 OR r.repair_call_id='00000000-0000-0000-0000-000000000000'::uuid
 OR p->>'version' NOT IN ('m1_context_pack_autonomous_v3','m1_context_pack_autonomous_v4')
 OR p->>'engagement_id' IS DISTINCT FROM r.engagement_id::text
 OR p->>'campaign_id' IS DISTINCT FROM r.campaign_id::text
 OR p->>'decision_id' IS DISTINCT FROM r.operation_id::text
 OR p->'permission_id' IS DISTINCT FROM r.request->'permission_id'
 OR p->'mission_revision' IS DISTINCT FROM r.request->'expected_mission_revision'
 OR jsonb_typeof(codes) IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'inference_intent_refused'; END IF;
 IF (k=r.generation_call_id AND codes<>'[]') OR (k=r.repair_call_id AND jsonb_array_length(codes) NOT BETWEEN 1 AND 8)
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(codes) v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string'
 OR v#>>'{}' NOT IN ('json_size','json_structure','json_trailing','proposal_shape','proposal_version','unexpected_view_request','proposal_text'))
 THEN RAISE EXCEPTION 'inference_intent_refused'; END IF;
 pro_v2:=p->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2');
 out_tokens:=CASE WHEN pro_v2 THEN 8192 ELSE 4096 END;
 soft_ms:=CASE WHEN pro_v2 THEN 120000 ELSE 60000 END;
 hard_ms:=CASE WHEN pro_v2 THEN 122000 ELSE 62000 END;
 shared_ms:=CASE WHEN pro_v2 THEN 244000 ELSE 124000 END;
 decision_ms:=CASE WHEN pro_v2 THEN 249000 ELSE 129000 END;
 phase_ms:=CASE WHEN pro_v2 THEN 254000 ELSE 134000 END;
 i:=jsonb_build_object('version','dw-inference-intent-v1','permission_version',CASE WHEN p->>'version'='m1_context_pack_autonomous_v4' THEN 'm1_inference_permission_v6' ELSE 'm1_inference_permission_v5' END,
 'run_version',CASE WHEN p->>'version'='m1_context_pack_autonomous_v4' THEN 'm1_autonomous_run_request_v4' ELSE 'm1_autonomous_run_request_v3' END,'engagement_id',p->'engagement_id','campaign_id',p->'campaign_id',
 'decision_id',p->'decision_id','permission_id',p->'permission_id','mission_revision',p->'mission_revision',
 'generation',r.generation,'generation_call_id',r.generation_call_id,'repair_call_id',r.repair_call_id,'call_id',k,
 'call_role',CASE WHEN k=r.generation_call_id THEN 'generation' ELSE 'repair' END,
 'run_profile',p->'run_profile','execution_mode',p->'execution_mode','protocol','openai-resp-v1',
 'profile',p->'inference_profile','recipient',recipient,'model',CASE WHEN p->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v1','deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 'deepseek-v4-pro' ELSE 'deepseek-flash' END,
 'adapter_release','genai-a9504f1-dw1','template_version','m1-autonomous-inference-v3',
 'schema_version','m1_inference_proposal_v1','timing_profile',CASE WHEN pro_v2 THEN 'reasoning-bounded-pro-v2' ELSE 'reasoning-bounded-v1' END,
 'instructions','DuskWeave autonomous inference v1: propose one objective-led hypothesis, alternative, expected and falsifying observation and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not playbooks or permission. Operator seeds are premises, observations are not facts, and historical decisions do not grant current authority. Use exact references[].id and qualified manifests[].reference values only. observe_more with a DNS manifest proposes one observation for one seed; observe_more with view_request requests only an owner view. Do not combine them. Do not invent endpoints, references, availability, access or scope. Missing, stale, conflicted and fixture sources retain their limits. Explain uncertainty and any retention/revision from predecessor feedback. Retain, defer and stop are valid choices. Synthesis/unsupported effects are explanations of a capability gap, never permission to execute. Return concise safe explanation, never private reasoning or raw client content. Output grants no dispatch. Fixture results cannot establish live target facts.','schema',execution.genai_proposal_schema(p),'repair_error_codes',codes,
 'settings',jsonb_build_object('reasoning_effort','low','max_output_tokens',out_tokens,'stream',false,'store',false,'tools','[]'::jsonb),
 'limits',jsonb_build_object('request_bytes',16384,'response_bytes',65536,'context_bytes',8192,'configuration_bytes',16384,
  'connect_time_ms',5000,'soft_completion_ms',soft_ms,'per_call_wall_time_ms',hard_ms,'shared_wall_time_ms',shared_ms,
  'decision_wall_time_ms',decision_ms,'phase_wall_time_ms',phase_ms,'cancel_grace_ms',2000,'accounting_window_ms',3000,
  'output_tokens_per_call',out_tokens,'calls_per_decision',2,'concurrency',1));
 IF p->>'version'='m1_context_pack_autonomous_v4' THEN
  i:=i||jsonb_build_object('live_binding',p->'live_binding','qualification_purpose',p->'qualification_purpose');
 END IF;
 RETURN i;
END $$;

CREATE OR REPLACE FUNCTION execution.inference_call_state(k uuid,wanted text,u jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE owner_call execution.inference_calls; field text; metadata jsonb; available integer; diag jsonb; sdk boolean; expected_model text;
BEGIN
 SELECT * INTO owner_call FROM execution.inference_calls x WHERE x.call=k;
 IF NOT EXISTS(SELECT 1 FROM execution.inference_reservations r WHERE
 (r.engagement_id,r.campaign_id,r.operation_id)=(owner_call.e,owner_call.c,owner_call.d)
 AND r.record_revision=1 AND r.writer_oid=(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user))
 THEN RAISE EXCEPTION 'inference_state_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(owner_call.e::text||':'||owner_call.c::text,0));
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR wanted NOT IN
 ('sent_awaiting_result','completed','cancelled_uncertain','failed_never_released','usage_unknown')
 THEN RAISE EXCEPTION 'inference_state_refused'; END IF;
 -- Re-read under the campaign fence so concurrent reports cannot replace each other.
 SELECT * INTO owner_call FROM execution.inference_calls x WHERE x.call=k FOR UPDATE;
 sdk:=coalesce(owner_call.configuration::jsonb->>'version'='dw-inference-intent-v1',false);
 IF sdk AND u IS NOT NULL AND u<>'null'::jsonb THEN
  expected_model:=CASE owner_call.configuration::jsonb->>'profile'
   WHEN 'deepseek-genai-v4-pro-low-v1' THEN 'deepseek-v4-pro'
   WHEN 'deepseek-genai-v4-pro-low-v2' THEN 'deepseek-v4-pro'
   WHEN 'fixture-genai-v4-pro-low-v2' THEN 'deepseek-v4-pro'
   WHEN 'deepseek-genai-responses-low-v1' THEN 'deepseek-flash'
   WHEN 'fixture-genai-responses-v1' THEN 'deepseek-flash' ELSE NULL END;
  IF expected_model IS NULL OR owner_call.configuration::jsonb->>'model' IS DISTINCT FROM expected_model
   OR jsonb_typeof(u) IS DISTINCT FROM 'object'
   OR u->>'requested_model' IS DISTINCT FROM expected_model
   OR u-ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms','requested_model','reported_model','proposal_diagnostic','provider_response']<>'{}'::jsonb
   OR NOT u ?& ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms','requested_model','reported_model']
   OR NOT coalesce(u->'reported_model'='null'::jsonb OR u->>'reported_model' IN ('deepseek-flash','deepseek-v4-pro','probe-fixture','protocol-fixture-v1'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  FOREACH field IN ARRAY ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms'] LOOP
   IF u->field='null'::jsonb AND field IN ('input_tokens','output_tokens','reasoning_tokens') THEN CONTINUE; END IF;
   IF jsonb_typeof(u->field) IS DISTINCT FROM 'number' OR (u->>field) !~ '^[0-9]+$'
    OR (u->>field)::numeric>9223372036854775807
    OR (field='response_bytes' AND (u->>field)::numeric>65536)
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  END LOOP;
 END IF;
 IF u ? 'proposal_diagnostic' THEN
  IF wanted <> 'completed' OR jsonb_typeof(u) IS DISTINCT FROM 'object'
   OR u-ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms','requested_model','reported_model','proposal_diagnostic']<>'{}'::jsonb
   OR NOT u ?& ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms','requested_model','reported_model','proposal_diagnostic']
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  diag:=u->'proposal_diagnostic';
  IF jsonb_typeof(diag) IS DISTINCT FROM 'object' OR octet_length(diag::text)>512
   OR NOT diag ?& ARRAY['version','reason','shape'] OR diag-ARRAY['version','reason','shape']<>'{}'::jsonb
   OR diag->>'version' IS DISTINCT FROM 'proposal_admission_diagnostic_v1'
   OR jsonb_typeof(diag->'reason') IS DISTINCT FROM 'string'
   OR NOT coalesce(diag->>'reason' IN ('json_size','json_structure','json_trailing','proposal_shape','proposal_version','proposal_text','unexpected_view_request'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF diag->>'reason'='proposal_shape' THEN
   IF jsonb_typeof(diag->'shape') IS DISTINCT FROM 'object'
    OR NOT diag->'shape' ?& ARRAY['kind','field'] OR (diag->'shape')-ARRAY['kind','field']<>'{}'::jsonb
    OR jsonb_typeof(diag#>'{shape,kind}') IS DISTINCT FROM 'string'
    OR jsonb_typeof(diag#>'{shape,field}') IS DISTINCT FROM 'string'
    OR NOT coalesce(diag#>>'{shape,kind}' IN ('missing_field','unknown_field','invalid_type','invalid_value','invalid_enum','other'),false)
    OR NOT coalesce(diag#>>'{shape,field}' IN ('unclassified','version','hypothesis','considered_alternative','references','expected_observation','falsifying_observation','uncertainty','rationale','next_intent','category','manifest_reference','view_request','owner','selectors'),false)
    OR (diag#>>'{shape,kind}'<>'missing_field' AND diag#>>'{shape,field}'<>'unclassified')
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  ELSIF diag->'shape' IS DISTINCT FROM 'null'::jsonb THEN RAISE EXCEPTION 'inference_usage_refused';
  END IF;
  FOREACH field IN ARRAY ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms'] LOOP
   IF u->field='null'::jsonb AND field IN ('input_tokens','output_tokens','reasoning_tokens') THEN CONTINUE; END IF;
   IF jsonb_typeof(u->field) IS DISTINCT FROM 'number' OR (u->>field) !~ '^[0-9]+$' THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
   IF (u->>field)::numeric>9223372036854775807 OR (field='response_bytes' AND (u->>field)::numeric>65536) THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  END LOOP;
  IF (NOT sdk AND NOT coalesce(u->>'requested_model' IN ('deepseek-flash','protocol-fixture-v1'),false)) OR NOT coalesce(u->'reported_model'='null'::jsonb OR u->>'reported_model' IN ('deepseek-flash','probe-fixture','protocol-fixture-v1') OR (sdk AND u->>'reported_model'='deepseek-v4-pro'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
 END IF;
 IF u IS NOT NULL AND u <> 'null'::jsonb AND
    (wanted IN ('usage_unknown','cancelled_uncertain') OR u ? 'provider_response') THEN
  IF u ? 'provider_response' AND wanted NOT IN ('usage_unknown','cancelled_uncertain')
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF jsonb_typeof(u) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF (u - ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes',
      'wall_time_ms','requested_model','reported_model','provider_response']) <> '{}'::jsonb
     OR NOT (u ?& ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes',
      'wall_time_ms','requested_model','reported_model','provider_response'])
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  metadata := u->'provider_response';
  IF jsonb_typeof(metadata) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF (metadata - ARRAY['status','incomplete_reason','usage_availability']) <> '{}'::jsonb
     OR NOT (metadata ?& ARRAY['status','incomplete_reason','usage_availability'])
     OR NOT coalesce(metadata->>'status' IN ('completed','incomplete','failed','in_progress','unrecognized','missing'),false)
     OR NOT coalesce(metadata->>'usage_availability' IN ('reported','partial','unavailable'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF metadata->>'status'='incomplete' THEN
   IF NOT coalesce(metadata->>'incomplete_reason' IN ('max_output_tokens','content_filter','unknown'),false)
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  ELSIF metadata->'incomplete_reason' IS DISTINCT FROM 'null'::jsonb
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  available := 0;
  FOREACH field IN ARRAY ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms'] LOOP
   IF u->field='null'::jsonb AND field IN ('input_tokens','output_tokens','reasoning_tokens') THEN CONTINUE; END IF;
   IF jsonb_typeof(u->field) IS DISTINCT FROM 'number' OR (u->>field) !~ '^[0-9]+$'
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
   IF (u->>field)::numeric > 9223372036854775807 OR
      (field='response_bytes' AND (u->>field)::numeric > 65536)
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
   IF field IN ('input_tokens','output_tokens','reasoning_tokens') THEN available := available+1; END IF;
  END LOOP;
  IF metadata->>'usage_availability' IS DISTINCT FROM
       (CASE WHEN available=3 THEN 'reported' WHEN available=0 THEN 'unavailable' ELSE 'partial' END)
     OR (NOT sdk AND u->>'requested_model' IS DISTINCT FROM 'deepseek-flash')
     OR NOT coalesce(u->'reported_model'='null'::jsonb OR
           u->>'reported_model' IN ('deepseek-flash','probe-fixture','protocol-fixture-v1') OR (sdk AND u->>'reported_model'='deepseek-v4-pro'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
 END IF;
 IF wanted IN ('usage_unknown','cancelled_uncertain') AND
    owner_call.usage IS NOT NULL AND owner_call.usage <> 'null'::jsonb AND
    u IS NOT NULL AND u <> 'null'::jsonb AND owner_call.usage IS DISTINCT FROM u
 THEN RAISE EXCEPTION 'inference_usage_conflict'; END IF;
 IF wanted IN ('usage_unknown','cancelled_uncertain') AND EXISTS(
  SELECT 1 FROM execution.inference_calls WHERE call=k AND state IN ('cancelled_uncertain','usage_unknown')
 ) THEN
  UPDATE execution.inference_calls SET
   state=CASE WHEN state='cancelled_uncertain' THEN state ELSE wanted END,
   usage=CASE WHEN u IS NULL OR u='null'::jsonb THEN usage ELSE u END WHERE call=k;
  RETURN;
 END IF;
 UPDATE execution.inference_calls SET state=wanted,usage=u WHERE call=k
 AND ((wanted='failed_never_released' AND state='may_send_uncertain')
  OR (wanted<>'failed_never_released' AND state IN ('may_send_uncertain','sent_awaiting_result')));
 IF NOT FOUND THEN RAISE EXCEPTION 'inference_state_conflict'; END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.run_start(q jsonb,release text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; r uuid; e uuid; c uuid; n bigint; s jsonb; p jsonb; deadline bigint; pro_v2 boolean;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR release !~ '^[0-9a-f]{64}$'
 OR jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text)>16384
 OR q-ARRAY['version','run_id','run_profile','execution_mode','deadline_at_ms','context','seed_names','inference_profile','live_binding']<>'{}'
 OR NOT coalesce((q->>'version',q->>'run_profile') IN (('m1_autonomous_run_request_v1','autonomous-dns-fixture-v1'),('m1_autonomous_run_request_v1','autonomous-dns-provider-fixture-v1'),('m1_autonomous_run_request_v2','autonomous-dns-fixture-v2'),('m1_autonomous_run_request_v3','autonomous-dns-fixture-v2'),('m1_autonomous_run_request_v4','autonomous-dns-genai-live-v1'),('m1_autonomous_run_request_v4','autonomous-dns-genai-lab-v1')),false)
 OR (q->>'version'='m1_autonomous_run_request_v1' AND q ? 'inference_profile')
 OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') AND jsonb_typeof(q->'inference_profile') IS DISTINCT FROM 'string')
 OR (q->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4' AND q ? 'live_binding')
 OR (q->>'version'='m1_autonomous_run_request_v4' AND (jsonb_typeof(q->'live_binding') IS DISTINCT FROM 'object'
  OR NOT q->'live_binding' ?& ARRAY['qualification_ref','release_sha256','resolver_ipv4']
  OR (q->'live_binding')-ARRAY['qualification_ref','release_sha256','resolver_ipv4']<>'{}'))
 OR q->>'run_profile' IS NULL
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2','autonomous-dns-genai-live-v1','autonomous-dns-genai-lab-v1')
 OR q->>'execution_mode' IS DISTINCT FROM (CASE WHEN q->>'version'='m1_autonomous_run_request_v4' THEN 'live' ELSE 'controlled_fixture' END)
 OR jsonb_typeof(q->'seed_names') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'seed_names') NOT BETWEEN 1 AND 8
 OR jsonb_array_length(q#>'{context,candidates}')<>jsonb_array_length(q->'seed_names')
 THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 IF q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') AND (
  jsonb_typeof(q->'context') IS DISTINCT FROM 'object'
  OR (q->'context')-ARRAY['version','mission','candidates']<>'{}'
  OR q#>>'{context,version}' IS DISTINCT FROM 'm1_context_prepare_request_v1'
  OR jsonb_typeof(q#>'{context,candidates}') IS DISTINCT FROM 'array'
  OR EXISTS(SELECT 1 FROM jsonb_array_elements(q#>'{context,candidates}') v WHERE
   jsonb_typeof(v) IS DISTINCT FROM 'object' OR v-ARRAY['asset_ref','priority']<>'{}'
   OR jsonb_typeof(v->'asset_ref') IS DISTINCT FROM 'string'
   OR coalesce(v->>'priority','') NOT IN ('high','medium','low')))
 THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 IF q->>'version'='m1_autonomous_run_request_v4' AND (
  jsonb_array_length(q->'seed_names')<>1 OR jsonb_array_length(q#>'{context,candidates}')<>1
  OR (q->>'run_profile'='autonomous-dns-genai-lab-v1'
   AND (q#>>'{seed_names,0,name}' IS NULL
    OR q#>>'{seed_names,0,name}' !~ '^.+\.duskweave\.test$')))
 THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 r:=(q->>'run_id')::uuid; e:=(q#>>'{context,mission,engagement_id}')::uuid; c:=(q#>>'{context,mission,campaign_id}')::uuid;
 IF '00000000-0000-0000-0000-000000000000'::uuid IN(r,e,c) THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(e::text||':'||c::text,0));
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r OR (autonomous_runs.e,autonomous_runs.c)=(e,c);
 IF FOUND THEN
  IF x.run_id<>r OR x.request IS DISTINCT FROM q OR x.release_sha256<>release
  OR x.writer_oid<>(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user) THEN RAISE EXCEPTION 'run_identity_conflict'; END IF;
  RETURN false;
 END IF;
 pro_v2:=q->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4')
  AND q->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2');
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint; deadline:=(q->>'deadline_at_ms')::bigint;
 IF jsonb_typeof(q->'deadline_at_ms') IS DISTINCT FROM 'number' OR q->>'deadline_at_ms' !~ '^[1-9][0-9]*$'
 OR deadline<=n OR deadline>n+(CASE WHEN pro_v2 THEN 640000 WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN 400000 ELSE 150000 END) THEN RAISE EXCEPTION 'run_deadline_refused'; END IF;
 s:=mission.current_inference_publication(e,c,r); p:=s->'fields'->'m1_inference_permission';
 IF s IS NULL OR NOT execution.inference_profile_matches(p)
 OR (p->>'version',p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (CASE WHEN q->>'version'='m1_autonomous_run_request_v4' THEN 'm1_inference_permission_v6' WHEN q->>'version'='m1_autonomous_run_request_v3' THEN 'm1_inference_permission_v5' WHEN q->>'version'='m1_autonomous_run_request_v2' THEN 'm1_inference_permission_v4' ELSE 'm1_inference_permission_v3' END,q->>'run_profile',q->>'execution_mode')
 OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') AND q->>'inference_profile' IS DISTINCT FROM p->>'profile')
 OR (q->>'version'='m1_autonomous_run_request_v4' AND (q->'live_binding' IS DISTINCT FROM p->'live_binding'
  OR NOT EXISTS(SELECT 1 FROM execution.live_qualifications z WHERE z.qualification_ref=(q#>>'{live_binding,qualification_ref}')::uuid
   AND z.run_profile=q->>'run_profile' AND z.release_sha256=release AND host(z.resolver_ipv4)=q#>>'{live_binding,resolver_ipv4}'
   AND n>=z.valid_from_ms AND n<z.valid_until_ms AND z.purpose=CASE q->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END)))
 OR q#>'{context,mission}' IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1',
 'engagement_id',e,'campaign_id',c,'operator_ref',s->'fields'->'operator_ref','expected_mission_revision',1,
 'permission_id',p->'permission_id','goal_ref',s->'fields'->'goal_ref','exercise_mode',s->'fields'->'exercise_mode')
 OR NOT p->'field_classes' ?& ARRAY['mission_question_constraints','scoped_opaque_references','owner_qualified_summaries',
 'admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes','admitted_campaign_decision_feedback']
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(q->'seed_names') WITH ORDINALITY a(v,i)
 WHERE v-ARRAY['asset_ref','name']<>'{}' OR v->'asset_ref' IS DISTINCT FROM q#>'{context,candidates}'->(i::int-1)->'asset_ref'
 OR NOT s->'fields'->'included_assets' @> jsonb_build_array(v->'asset_ref')
 OR s->'fields'->'excluded_assets' @> jsonb_build_array(v->'asset_ref'))
 OR EXISTS(SELECT 1 FROM (VALUES ('calls',4),('request_bytes',65536),('response_bytes',262144),
  ('output_token_capacity',CASE WHEN pro_v2 THEN 32768 ELSE 16384 END),
  ('wall_time_ms',CASE WHEN pro_v2 THEN 498000 WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN 258000 ELSE 90000 END)) b(k,v)
 WHERE NOT coalesce((p->'campaign_limits'->>b.k)::numeric>=(execution.inference_capacity_snapshot(e,c)->'charged'->>b.k)::numeric+b.v,false))
 THEN RAISE EXCEPTION 'run_authority_unavailable'; END IF;
 deadline:=least(deadline,(s->'fields'->>'ends_at')::bigint*1000,(p->>'ends_at')::bigint*1000);
 INSERT INTO execution.autonomous_runs VALUES(r,e,c,(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user),q,
 gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),release,CASE WHEN q->>'version'='m1_autonomous_run_request_v4' THEN (q#>>'{live_binding,qualification_ref}')::uuid ELSE gen_random_uuid() END,deadline);
 INSERT INTO execution.autonomous_control VALUES(r,least(n+2000,deadline),'running',NULL,false);
 PERFORM execution.run_current(r);
 RETURN true;
END $$;

CREATE OR REPLACE FUNCTION execution.run_current(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
DECLARE x execution.autonomous_runs; y execution.autonomous_control; s jsonb; p jsonb; n bigint;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT * INTO y FROM execution.autonomous_control WHERE run_id=r;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF x.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user)
 OR NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR y.status IS DISTINCT FROM 'running'
 OR n>=least(x.deadline_at_ms,y.lease_until_ms) THEN RAISE EXCEPTION 'run_not_current'; END IF;
 s:=mission.current_inference_publication(x.e,x.c,x.decision_one); p:=s->'fields'->'m1_inference_permission';
 IF s IS NULL OR NOT execution.inference_profile_matches(p)
 OR NOT coalesce((p->>'version',x.request->>'version') IN (('m1_inference_permission_v3','m1_autonomous_run_request_v1'),('m1_inference_permission_v4','m1_autonomous_run_request_v2'),('m1_inference_permission_v5','m1_autonomous_run_request_v3'),('m1_inference_permission_v6','m1_autonomous_run_request_v4')),false)
 OR x.request->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2','autonomous-dns-genai-live-v1','autonomous-dns-genai-lab-v1')
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (x.request->>'run_profile',x.request->>'execution_mode')
 OR p->>'execution_mode' IS DISTINCT FROM x.request->>'execution_mode'
 OR (p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') AND (p->>'profile' IS DISTINCT FROM x.request->>'inference_profile' OR x.request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v2'))
 OR (p->>'version'='m1_inference_permission_v6' AND (p->>'profile' IS DISTINCT FROM x.request->>'inference_profile'
  OR x.request->>'run_profile' NOT IN ('autonomous-dns-genai-live-v1','autonomous-dns-genai-lab-v1')
  OR x.request->>'execution_mode' IS DISTINCT FROM 'live'
  OR x.request->'live_binding' IS DISTINCT FROM p->'live_binding'))
 OR p->>'permission_id' IS DISTINCT FROM x.request#>>'{context,mission,permission_id}'
 OR p->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb
 OR jsonb_typeof(p->'provider_terms_ref') IS DISTINCT FROM 'string'
 OR (p->>'provider_terms_ref')::uuid IS NULL
 OR (p->>'provider_terms_ref')::uuid='00000000-0000-0000-0000-000000000000'::uuid
 OR jsonb_typeof(p->'field_classes') IS DISTINCT FROM 'array'
 OR jsonb_array_length(p->'field_classes')<>9
 OR NOT p->'field_classes' ?& ARRAY['mission_question_constraints','scoped_opaque_references','owner_qualified_summaries',
 'admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes','admitted_campaign_decision_feedback']
 OR (p->'engagement_id',p->'campaign_id',p->'operator_ref',p->'authority_ref',p->'authority_revision',
     p->'goal_ref',p->'exercise_mode',p->'mission_revision') IS DISTINCT FROM
    (s->'engagement_id',s->'campaign_id',s->'fields'->'operator_ref',s->'fields'->'authority_ref',
     s->'fields'->'authority_revision',s->'fields'->'goal_ref',s->'fields'->'exercise_mode','1'::jsonb)
 OR n>=(p->>'ends_at')::bigint*1000 OR n<(p->>'starts_at')::bigint*1000
 OR n>=(s->'fields'->>'ends_at')::bigint*1000 OR n<(s->'fields'->>'starts_at')::bigint*1000
 OR NOT EXISTS(SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id,h.operation_id,h.event_id,h.status,h.contract)
  =(x.e,x.c,(s->>'operation_id')::uuid,(s->>'event_id')::uuid,'accepted',s))
 OR EXISTS(SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id)=(x.e,x.c) AND h.status='anomaly')
 THEN RAISE EXCEPTION 'run_authority_unavailable'; END IF;
 IF p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') AND EXISTS(
  SELECT 1 FROM execution.autonomous_phases a WHERE a.run_id=r AND a.phase=y.phase AND a.status='started'
  AND n>=a.recorded_at_ms+CASE WHEN a.phase='acquisition' THEN 60000
   WHEN x.request->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4')
    AND x.request->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2') THEN 254000
   ELSE 134000 END)
 THEN RAISE EXCEPTION 'run_phase_expired'; END IF;
 RETURN s;
END $$;

CREATE OR REPLACE FUNCTION execution.assert_autonomous_pack(e uuid,c uuid,d uuid,p jsonb,cfg jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; expected jsonb; prior jsonb; seed text; permission jsonb; registration jsonb; expected_timing text;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs a WHERE (a.e,a.c)=(e,c) AND d IN(a.decision_one,a.decision_two);
 registration:=execution.run_current(x.run_id); permission:=registration#>'{fields,m1_inference_permission}';
 IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN
  PERFORM execution.assert_successor_pack(p,registration,x.request);
  IF (p->>'evaluated_at')::bigint>floor(extract(epoch FROM clock_timestamp()))::bigint
  OR (p->>'evaluated_at')::bigint<(registration->>'occurred_at')::bigint THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 END IF;
 expected_timing:=CASE WHEN x.request->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4')
  AND x.request->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2')
  THEN 'reasoning-bounded-pro-v2' ELSE 'reasoning-bounded-v1' END;
 IF p->>'version' IS DISTINCT FROM (CASE WHEN x.request->>'version'='m1_autonomous_run_request_v4' THEN 'm1_context_pack_autonomous_v4' WHEN x.request->>'version'='m1_autonomous_run_request_v3' THEN 'm1_context_pack_autonomous_v3' WHEN x.request->>'version'='m1_autonomous_run_request_v2' THEN 'm1_context_pack_autonomous_v2' ELSE 'm1_context_pack_autonomous_v1' END)
 OR (x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') AND (p->>'inference_profile' IS DISTINCT FROM permission->>'profile' OR cfg->>'timing_profile' IS DISTINCT FROM expected_timing))
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (x.request->>'run_profile',x.request->>'execution_mode')
 OR (cfg->>'run_profile',cfg->>'execution_mode') IS DISTINCT FROM (p->>'run_profile',p->>'execution_mode')
 OR (cfg->>'profile',cfg->>'recipient') IS DISTINCT FROM (permission->>'profile',permission->>'recipient')
 OR (p->>'engagement_id',p->>'campaign_id',p->>'decision_id') IS DISTINCT FROM (e::text,c::text,d::text)
 OR p->'manifests' IS DISTINCT FROM jsonb_build_array(CASE WHEN x.request->>'version'='m1_autonomous_run_request_v4'
 THEN jsonb_build_object('availability','qualified','execution_mode','live','profile','dns-routing-v2','qualification_ref',x.qualification_ref,'reference','dns_udp','release_sha256',x.release_sha256,'schema','m1_dns_ipc_v1','transport','udp')
 ELSE jsonb_build_object('availability','qualified','execution_mode','controlled_fixture','profile','dns-routing-fixture-v1','qualification_ref',x.qualification_ref,'reference','dns_udp','release_sha256',x.release_sha256,'schema','m1_dns_ipc_v1','transport','udp') END)
 OR (x.request->>'version'='m1_autonomous_run_request_v4' AND (p->'live_binding' IS DISTINCT FROM permission->'live_binding'
  OR p->>'qualification_purpose' IS DISTINCT FROM CASE x.request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END)) THEN RAISE EXCEPTION 'run_pack_binding_refused'; END IF;
 IF d=x.decision_one THEN
  IF p->'predecessor' IS DISTINCT FROM '{"availability":"absent","reason":"first_decision"}'::jsonb
  THEN RAISE EXCEPTION 'run_predecessor_refused'; END IF;
 ELSE
  expected:=execution.run_predecessor(x.run_id); prior:=p->'predecessor';
  SELECT ref->>'id' INTO seed FROM jsonb_array_elements(p->'references') ref
  WHERE ref->>'owner'='operator' AND ref->'entity'=expected->'selected_seed_entity';
  expected:=jsonb_set(expected,'{selected_seed_reference}',to_jsonb(seed));
  IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') AND prior->>'hypothesis_summary_omission'='byte_bound'
  AND NOT prior ? 'hypothesis_summary' THEN
   expected:=(expected-'hypothesis_summary')||'{"hypothesis_summary_omission":"byte_bound"}'::jsonb;
  END IF;
  IF seed IS NULL OR prior IS DISTINCT FROM
   (expected-ARRAY['decision_entity','decision_event','recorded_at','selected_seed_entity'])
  OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'references') ref WHERE ref->>'id'='previous_decision'
    AND ref->'entity'=expected->'decision_entity' AND ref->'event'=expected->'decision_event'
    AND ref->>'status'='committed_historical_decision' AND ref->>'source'='cognition' AND ref->'recorded_at'=expected->'recorded_at')
  THEN RAISE EXCEPTION 'run_predecessor_refused'; END IF;
 END IF;
END $$;

CREATE OR REPLACE FUNCTION execution.run_phase_diagnostic(q jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
DECLARE r uuid; p text; old jsonb; elapsed_max numeric;
BEGIN
 IF jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text)>1024 THEN
  RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
 IF NOT q ?& ARRAY['version','run_id','phase','cause','cause_source','child_error','exit_code','readiness_observed','elapsed_ms']
 OR q-ARRAY['version','run_id','phase','cause','cause_source','child_error','exit_code','readiness_observed','elapsed_ms','reported_guard']<>'{}'
 OR coalesce(q->>'version','') NOT IN ('m1_run_phase_diagnostic_v1','m1_run_phase_diagnostic_v2') OR (q->>'version'='m1_run_phase_diagnostic_v1' AND q ? 'reported_guard') OR (q ? 'reported_guard' AND (jsonb_typeof(q->'reported_guard') IS DISTINCT FROM 'string' OR q->>'reported_guard' NOT IN ('transport_hard_deadline','decision_envelope','permit_expiry','command_watchdog')))
 OR jsonb_typeof(q->'run_id') IS DISTINCT FROM 'string'
 OR jsonb_typeof(q->'phase') IS DISTINCT FROM 'string'
 OR q->>'phase' NOT IN ('decision_one','acquisition','decision_two')
 OR jsonb_typeof(q->'cause') IS DISTINCT FROM 'string'
 OR q->>'cause' NOT IN ('child_error','child_exit','readiness_refused','readiness_deadline','phase_deadline','local_stop','supervisor_unavailable','unavailable')
 OR q->>'cause_source' IS DISTINCT FROM (CASE q->>'cause'
     WHEN 'child_error' THEN 'child_report' WHEN 'unavailable' THEN 'unavailable' ELSE 'parent_observed' END)
 OR jsonb_typeof(q->'readiness_observed') IS DISTINCT FROM 'boolean'
 THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
 IF q->'child_error'<>'null'::jsonb AND (
   jsonb_typeof(q->'child_error') IS DISTINCT FROM 'string' OR q->>'child_error' NOT IN (
    'missing_env','invalid_database_config_mode','database_config_conflict','missing_database_url_file',
    'database_file_platform_unqualified','invalid_dsn','non_loopback_host','missing_credentials','connect_failed',
    'unqualified_database_waits','run_control_unqualified','run_control_stopped','run_child_refused',
    'inference_authority_unavailable','run_inference_pending','child_failed'))
 OR (q->>'cause'='child_error' AND q->'child_error'='null'::jsonb)
 THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
 IF q->'exit_code'<>'null'::jsonb THEN
  IF jsonb_typeof(q->'exit_code') IS DISTINCT FROM 'number' OR q->>'exit_code' !~ '^-?(0|[1-9][0-9]*)$'
  THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
  IF (q->>'exit_code')::numeric NOT BETWEEN -2147483648 AND 2147483647
  THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
 END IF;
 r:=(q->>'run_id')::uuid; p:=q->>'phase';
 elapsed_max:=CASE WHEN q->>'version'='m1_run_phase_diagnostic_v2'
  AND EXISTS(SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id=r
   AND x.request->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4')
   AND x.request->>'inference_profile' IN ('deepseek-genai-v4-pro-low-v2','fixture-genai-v4-pro-low-v2'))
  THEN 650000 ELSE 150000 END;
 IF q->'elapsed_ms'<>'null'::jsonb THEN
  IF jsonb_typeof(q->'elapsed_ms') IS DISTINCT FROM 'number' OR q->>'elapsed_ms' !~ '^(0|[1-9][0-9]*)$'
  THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
  IF (q->>'elapsed_ms')::numeric NOT BETWEEN 0 AND elapsed_max
  THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
 END IF;
 IF execution.run_record(r) IS NULL OR (q->>'version'='m1_run_phase_diagnostic_v2' AND NOT EXISTS(SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id=r AND x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4'))) OR NOT EXISTS(
  SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase=p AND status='started')
 THEN RAISE EXCEPTION 'run_diagnostic_writer_refused'; END IF;
 INSERT INTO execution.run_phase_diagnostics VALUES(r,p,q) ON CONFLICT DO NOTHING;
 SELECT payload INTO old FROM execution.run_phase_diagnostics WHERE run_id=r AND phase=p;
 IF old IS DISTINCT FROM q THEN RAISE EXCEPTION 'run_diagnostic_conflict'; END IF;
END $$;

COMMIT;
