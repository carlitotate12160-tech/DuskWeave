-- Successor inference lifecycle. Historical migrations and permissions stay unchanged.

-- One writer-bound origin survives repeated reads and generation/repair handoffs.
CREATE TABLE IF NOT EXISTS execution.inference_decision_windows (
 e uuid NOT NULL,c uuid NOT NULL,d uuid NOT NULL,writer_oid oid NOT NULL,
 started_at_ms bigint NOT NULL,active_until_ms bigint NOT NULL,decision_until_ms bigint NOT NULL,
 phase_until_ms bigint NOT NULL,PRIMARY KEY(e,c,d),
 FOREIGN KEY(e,c,d) REFERENCES execution.native_input_bindings(e,c,d),
 CHECK(started_at_ms<active_until_ms AND active_until_ms<decision_until_ms AND decision_until_ms<=phase_until_ms));
REVOKE ALL ON execution.inference_decision_windows FROM PUBLIC,dw_runtime,dw_m1_broker,dw_m1_cognition,dw_m1_trajectory,dw_m1_terrain;

CREATE OR REPLACE FUNCTION execution.inference_profile_matches(p jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path=pg_catalog AS $$
 SELECT coalesce(CASE WHEN p->>'version'='m1_inference_permission_v4' THEN
 (p->>'profile',p->>'recipient') IN (
 ('deepseek-responses-reasoning-v1','https://api.deepseek.com/responses'),
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
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE
    e uuid; c uuid;
    actor oid := (SELECT oid FROM pg_roles WHERE rolname=session_user);
    prior execution.inference_reservations;
    source jsonb; consent jsonb; limits jsonb; totals jsonb;
    cap jsonb; held jsonb;
    now_s bigint; next_generation bigint; claimed uuid;
BEGIN
    IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER')
       OR EXISTS (SELECT 1 FROM pg_roles WHERE oid=actor AND (rolsuper OR rolbypassrls))
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
    IF consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4') THEN
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
    -- Validate the raw SQL boundary too: nulls, unknown fields and numeric strings
    -- must not turn malformed consent into a reservation.
    IF (SELECT count(*) FROM jsonb_object_keys(consent))<>(CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN 21 ELSE 19 END)
       OR jsonb_typeof(consent->'provider_terms_ref') IS DISTINCT FROM 'string'
       OR (consent->>'provider_terms_ref')::uuid IS NULL
       OR jsonb_typeof(consent->'field_classes') IS DISTINCT FROM 'array'
       OR jsonb_array_length(consent->'field_classes') NOT BETWEEN 1 AND (CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN 9 ELSE 8 END)
       OR (SELECT count(DISTINCT v) FROM jsonb_array_elements(consent->'field_classes') v)
          <>jsonb_array_length(consent->'field_classes')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(consent->'field_classes') v
           WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(consent->'field_classes') v
           WHERE v NOT IN ('mission_question_constraints','scoped_opaque_references',
           'owner_qualified_summaries','admitted_operator_premises','manifest_availability',
           'omissions_unknown','capacity','admitted_repair_fields_codes') AND NOT (consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') AND v='admitted_campaign_decision_feedback'))
       OR (SELECT count(*) FROM jsonb_object_keys(limits))<>(CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN CASE WHEN consent->>'version'='m1_inference_permission_v4' THEN 22 ELSE 21 END ELSE 16 END)
       OR (SELECT count(*) FROM jsonb_object_keys(totals))<>5
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['starts_at','ends_at',
           'authority_revision','mission_revision']) k
           WHERE jsonb_typeof(consent->k) IS DISTINCT FROM 'number'
           OR consent->>k !~ '^-?[0-9]+$') THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_consent_refused';
    END IF;
    -- Cardinality plus all required names below closes the numeric objects.
    -- One profile table validates each operation field's type and interval.
    IF EXISTS (SELECT 1 FROM (VALUES ('request_bytes',1,16384),('response_bytes',1,65536),
        ('decision_wall_time_ms',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN CASE WHEN consent->>'version'='m1_inference_permission_v4' THEN 129000 ELSE 45000 END ELSE 20000 END),('context_bytes',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN 8192 ELSE 4096 END),('candidates',1,8),
        ('configuration_bytes',1,16384),('connect_time_ms',1,5000),('proposal_bytes',1,8192),
        ('repair_bytes',1,2048),('text_field_bytes',1,1024),('repair_error_codes',1,8),
        ('json_depth',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4') THEN 32 ELSE 8 END),('collection_entries',1,64),('calls_per_decision',2,2),
        ('output_tokens_per_call',4096,4096),('concurrency',1,1)) AS b(name,floor,ceiling)
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
        'response_bytes',2*(limits->>'response_bytes')::bigint,'output_token_capacity',8192,
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
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE s jsonb;p jsonb;n bigint;l jsonb;run uuid;phase text;started bigint;phase_end bigint;decision_end bigint;w execution.inference_decision_windows;
BEGIN
 s:=mission.current_inference_publication(e,c,d);p:=s->'fields'->'m1_inference_permission';l:=p->'operation_limits';
 IF EXISTS(SELECT 1 FROM execution.inference_closures z LEFT JOIN cognition.reasoning_decisions q
 ON (q.e,q.c,q.d)=(z.e,z.c,z.d) WHERE (z.e,z.c)=(e,c) AND (
 q.event IS NULL OR NOT EXISTS(SELECT 1 FROM trajectory.reasoning_history h
 WHERE (h.engagement_id,h.campaign_id,h.operation_id,h.event_id,h.status,h.contract)=(z.e,z.c,z.d,q.event,'accepted',q.payload))
 OR EXISTS(SELECT 1 FROM trajectory.reasoning_history h WHERE (h.engagement_id,h.campaign_id)=(z.e,z.c)
 AND (h.operation_id=z.d OR h.event_id=q.event) AND h.status='anomaly')))
 THEN RAISE EXCEPTION 'inference_prior_history_unqualified'; END IF;
 IF p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN
 SELECT x.run_id, CASE WHEN x.decision_one=d THEN 'decision_one' ELSE 'decision_two' END INTO run,phase
 FROM execution.autonomous_runs x WHERE (x.e,x.c)=(e,c) AND d IN(x.decision_one,x.decision_two);
 PERFORM execution.run_current(run);
 IF NOT EXISTS(SELECT 1 FROM execution.autonomous_control y WHERE y.run_id=run AND y.phase=phase AND y.claimed)
 THEN RAISE EXCEPTION 'run_phase_refused';END IF;
 END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF s IS NULL OR NOT execution.inference_profile_matches(p) OR p->>'version'='m1_inference_permission_v1'
 OR p->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb
 OR NOT coalesce(p->'field_classes' ? 'admitted_repair_fields_codes',false)
 OR n<(s->'fields'->>'starts_at')::bigint*1000 OR n<(p->>'starts_at')::bigint*1000
 OR EXISTS(SELECT 1 FROM (VALUES ('json_depth',32),('collection_entries',64),('response_bytes',65536),('proposal_bytes',8192),('text_field_bytes',1024),('repair_bytes',2048),('repair_error_codes',8),('connect_time_ms',5000),('calls_per_decision',2),('output_tokens_per_call',4096),('concurrency',1)) b(k,v) WHERE l->b.k IS DISTINCT FROM to_jsonb(b.v))
 THEN RAISE EXCEPTION 'inference_authority_unavailable'; END IF;
 IF p->>'version'='m1_inference_permission_v4' THEN
  SELECT * INTO w FROM execution.inference_decision_windows a WHERE (a.e,a.c,a.d)=(e,c,d);
  IF NOT FOUND THEN
   SELECT recorded_at_ms INTO started FROM execution.autonomous_phases WHERE run_id=run AND autonomous_phases.phase=phase AND status='started';
   IF started IS NULL OR n>=started+5000 THEN RAISE EXCEPTION 'inference_preparation_expired'; END IF;
   SELECT least(started+134000,x.deadline_at_ms,(p->>'ends_at')::bigint*1000,(s#>>'{fields,ends_at}')::bigint*1000)
    INTO phase_end FROM execution.autonomous_runs x WHERE x.run_id=run;
   decision_end:=least(n+129000,phase_end);
   INSERT INTO execution.inference_decision_windows VALUES(e,c,d,(SELECT oid FROM pg_roles WHERE rolname=session_user),
    n,least(n+124000,decision_end-5000),decision_end,phase_end) ON CONFLICT DO NOTHING;
   SELECT * INTO w FROM execution.inference_decision_windows a WHERE (a.e,a.c,a.d)=(e,c,d);
  END IF;
  IF w.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user) OR n>=w.active_until_ms
  THEN RAISE EXCEPTION 'inference_deadline'; END IF;
  RETURN least(w.active_until_ms-n,(s#>>'{fields,ends_at}')::bigint*1000-n,(p->>'ends_at')::bigint*1000-n);
 END IF;
 RETURN least(CASE WHEN p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN CASE WHEN p->>'version'='m1_inference_permission_v4' THEN 124000 ELSE 40000 END ELSE 20000 END,(l->>'decision_wall_time_ms')::bigint,(s->'fields'->>'ends_at')::bigint*1000-n,(p->>'ends_at')::bigint*1000-n);
END $$;

CREATE OR REPLACE FUNCTION execution.run_current(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs; y execution.autonomous_control; s jsonb; p jsonb; n bigint;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT * INTO y FROM execution.autonomous_control WHERE run_id=r;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF x.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user)
 OR NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR y.status IS DISTINCT FROM 'running'
 OR n>=least(x.deadline_at_ms,y.lease_until_ms) THEN RAISE EXCEPTION 'run_not_current'; END IF;
 s:=mission.current_inference_publication(x.e,x.c,x.decision_one); p:=s->'fields'->'m1_inference_permission';
 IF s IS NULL OR NOT execution.inference_profile_matches(p)
 OR NOT coalesce((p->>'version',x.request->>'version') IN (('m1_inference_permission_v3','m1_autonomous_run_request_v1'),('m1_inference_permission_v4','m1_autonomous_run_request_v2')),false)
 OR x.request->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2')
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (x.request->>'run_profile',x.request->>'execution_mode')
 OR p->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
 OR (p->>'version'='m1_inference_permission_v4' AND (p->>'profile' IS DISTINCT FROM x.request->>'inference_profile' OR x.request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v2'))
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
 IF p->>'version'='m1_inference_permission_v4' AND EXISTS(
  SELECT 1 FROM execution.autonomous_phases a WHERE a.run_id=r AND a.phase=y.phase AND a.status='started'
  AND n>=a.recorded_at_ms+CASE WHEN a.phase='acquisition' THEN 60000 ELSE 134000 END)
 THEN RAISE EXCEPTION 'run_phase_expired'; END IF;
 RETURN s;
END $$;

CREATE OR REPLACE FUNCTION execution.run_start(q jsonb,release text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; r uuid; e uuid; c uuid; n bigint; s jsonb; p jsonb; deadline bigint;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR release !~ '^[0-9a-f]{64}$'
 OR jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text)>16384
 OR q-ARRAY['version','run_id','run_profile','execution_mode','deadline_at_ms','context','seed_names','inference_profile']<>'{}'
 OR NOT coalesce((q->>'version',q->>'run_profile') IN (('m1_autonomous_run_request_v1','autonomous-dns-fixture-v1'),('m1_autonomous_run_request_v1','autonomous-dns-provider-fixture-v1'),('m1_autonomous_run_request_v2','autonomous-dns-fixture-v2')),false)
 OR (q->>'version'='m1_autonomous_run_request_v1' AND q ? 'inference_profile')
 OR (q->>'version'='m1_autonomous_run_request_v2' AND jsonb_typeof(q->'inference_profile') IS DISTINCT FROM 'string')
 OR q->>'run_profile' IS NULL
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2')
 OR q->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
 OR jsonb_typeof(q->'seed_names') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'seed_names') NOT BETWEEN 1 AND 8
 OR jsonb_array_length(q#>'{context,candidates}')<>jsonb_array_length(q->'seed_names')
 THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 IF q->>'version'='m1_autonomous_run_request_v2' AND (
  jsonb_typeof(q->'context') IS DISTINCT FROM 'object'
  OR (q->'context')-ARRAY['version','mission','candidates']<>'{}'
  OR q#>>'{context,version}' IS DISTINCT FROM 'm1_context_prepare_request_v1'
  OR jsonb_typeof(q#>'{context,candidates}') IS DISTINCT FROM 'array'
  OR EXISTS(SELECT 1 FROM jsonb_array_elements(q#>'{context,candidates}') v WHERE
   jsonb_typeof(v) IS DISTINCT FROM 'object' OR v-ARRAY['asset_ref','priority']<>'{}'
   OR jsonb_typeof(v->'asset_ref') IS DISTINCT FROM 'string'
   OR coalesce(v->>'priority','') NOT IN ('high','medium','low')))
 THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 r:=(q->>'run_id')::uuid; e:=(q#>>'{context,mission,engagement_id}')::uuid; c:=(q#>>'{context,mission,campaign_id}')::uuid;
 IF '00000000-0000-0000-0000-000000000000'::uuid IN(r,e,c) THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(e::text||':'||c::text,0));
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r OR (autonomous_runs.e,autonomous_runs.c)=(e,c);
 IF FOUND THEN
  IF x.run_id<>r OR x.request IS DISTINCT FROM q OR x.release_sha256<>release
  OR x.writer_oid<>(SELECT oid FROM pg_roles WHERE rolname=session_user) THEN RAISE EXCEPTION 'run_identity_conflict'; END IF;
  RETURN false;
 END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint; deadline:=(q->>'deadline_at_ms')::bigint;
 IF jsonb_typeof(q->'deadline_at_ms') IS DISTINCT FROM 'number' OR q->>'deadline_at_ms' !~ '^[1-9][0-9]*$'
 OR deadline<=n OR deadline>n+(CASE WHEN q->>'version'='m1_autonomous_run_request_v2' THEN 400000 ELSE 150000 END) THEN RAISE EXCEPTION 'run_deadline_refused'; END IF;
 s:=mission.current_inference_publication(e,c,r); p:=s->'fields'->'m1_inference_permission';
 IF s IS NULL OR NOT execution.inference_profile_matches(p)
 OR (p->>'version',p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (CASE WHEN q->>'version'='m1_autonomous_run_request_v2' THEN 'm1_inference_permission_v4' ELSE 'm1_inference_permission_v3' END,q->>'run_profile',q->>'execution_mode')
 OR (q->>'version'='m1_autonomous_run_request_v2' AND q->>'inference_profile' IS DISTINCT FROM p->>'profile')
 OR q#>'{context,mission}' IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1',
 'engagement_id',e,'campaign_id',c,'operator_ref',s->'fields'->'operator_ref','expected_mission_revision',1,
 'permission_id',p->'permission_id','goal_ref',s->'fields'->'goal_ref','exercise_mode',s->'fields'->'exercise_mode')
 OR NOT p->'field_classes' ?& ARRAY['mission_question_constraints','scoped_opaque_references','owner_qualified_summaries',
 'admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes','admitted_campaign_decision_feedback']
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(q->'seed_names') WITH ORDINALITY a(v,i)
 WHERE v-ARRAY['asset_ref','name']<>'{}' OR v->'asset_ref' IS DISTINCT FROM q#>'{context,candidates}'->(i::int-1)->'asset_ref'
 OR NOT s->'fields'->'included_assets' @> jsonb_build_array(v->'asset_ref')
 OR s->'fields'->'excluded_assets' @> jsonb_build_array(v->'asset_ref'))
 OR EXISTS(SELECT 1 FROM (VALUES ('calls',4),('request_bytes',65536),('response_bytes',262144),('output_token_capacity',16384),('wall_time_ms',CASE WHEN q->>'version'='m1_autonomous_run_request_v2' THEN 258000 ELSE 90000 END)) b(k,v)
 WHERE NOT coalesce((p->'campaign_limits'->>b.k)::numeric>=(execution.inference_capacity_snapshot(e,c)->'charged'->>b.k)::numeric+b.v,false))
 THEN RAISE EXCEPTION 'run_authority_unavailable'; END IF;
 deadline:=least(deadline,(s->'fields'->>'ends_at')::bigint*1000,(p->>'ends_at')::bigint*1000);
 INSERT INTO execution.autonomous_runs VALUES(r,e,c,(SELECT oid FROM pg_roles WHERE rolname=session_user),q,
 gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),release,gen_random_uuid(),deadline);
 INSERT INTO execution.autonomous_control VALUES(r,least(n+2000,deadline),'running',NULL,false);
 PERFORM execution.run_current(r);
 RETURN true;
END $$;


-- Independent SQL reconstruction of the closed successor wire contract.
CREATE OR REPLACE FUNCTION execution.assert_successor_pack(p jsonb,reg jsonb,request jsonb) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE ref jsonb; candidate jsonb; original jsonb; expected jsonb; ordinal integer;
 keys text[]:=ARRAY['version','engagement_id','campaign_id','decision_id','permission_id','mission_revision','goal_ref',
 'exercise_mode','scope','question','evaluated_at','owner_views','references','candidates','omissions','pending_unknown',
 'manifests','remaining_capacity','execution_mode','run_profile','inference_profile','predecessor'];
BEGIN
 IF jsonb_typeof(p) IS DISTINCT FROM 'object' OR NOT p ?& keys OR p-keys<>'{}'
 OR p->'permission_id' IS DISTINCT FROM reg#>'{fields,m1_inference_permission,permission_id}'
 OR p->>'mission_revision' IS DISTINCT FROM '1' OR jsonb_typeof(p->'mission_revision') IS DISTINCT FROM 'number'
 OR p->'goal_ref' IS DISTINCT FROM reg#>'{fields,goal_ref}'
 OR p->'exercise_mode' IS DISTINCT FROM reg#>'{fields,exercise_mode}'
 OR p->'scope' IS DISTINCT FROM jsonb_build_object('owner','mission','publication_ref',reg->'event_id')
 OR p->>'question' IS DISTINCT FROM 'Which already-authorized, mission-relevant HTTPS candidate is the best next subject for access-validation planning within the permitted contact envelope, and which routing or response premise still needs observation, a different candidate, or deferral?'
 OR jsonb_typeof(p->'evaluated_at') IS DISTINCT FROM 'number' OR coalesce(p->>'evaluated_at','') !~ '^[1-9][0-9]*$'
 OR jsonb_typeof(p->'references') IS DISTINCT FROM 'array' OR jsonb_typeof(p->'candidates') IS DISTINCT FROM 'array'
 OR jsonb_typeof(p->'owner_views') IS DISTINCT FROM 'array'
 THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 IF jsonb_array_length(p->'candidates') NOT BETWEEN 1 AND 8 OR jsonb_array_length(p->'owner_views')<>6
 OR (SELECT count(DISTINCT v->>'owner') FROM jsonb_array_elements(p->'owner_views') v)<>6
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(p->'owner_views') v WHERE coalesce(v->>'owner','') NOT IN
 ('mission','cyber_terrain','foothold_graph','attack_path_view','objective_state','campaign_trajectory'))
 OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'owner_views') v WHERE v=jsonb_build_object('owner','mission',
 'availability','available','revision',1,'frontier',reg->'event_id','evaluated_at',p->'evaluated_at'))
 OR (SELECT count(*) FROM jsonb_array_elements(p->'references') v WHERE v->>'owner'='mission')<>1
 OR (SELECT count(*) FROM jsonb_array_elements(p->'references') v WHERE v->>'owner'='operator')<>jsonb_array_length(p->'candidates')
 THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 FOR ref IN SELECT value FROM jsonb_array_elements(p->'references') LOOP
  CASE ref->>'owner'
   WHEN 'mission' THEN
    expected:=jsonb_build_object('id','mission','owner','mission','entity',reg->'campaign_id','revision',1,
      'lineage',jsonb_build_array(reg->'event_id',reg->'operation_id'),'perceived_at',reg->'occurred_at',
      'time_uncertainty','unspecified_by_owner','status','owner_published','tier',NULL,'freshness','as_of_snapshot','completeness','scope_reference');
    IF ref IS DISTINCT FROM expected THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   WHEN 'operator' THEN
    SELECT value INTO original FROM jsonb_array_elements(request#>'{context,candidates}') WHERE value->'asset_ref'=ref->'entity';
    IF original IS NULL OR NOT (reg#>'{fields,included_assets}') @> jsonb_build_array(ref->'entity')
    OR (reg#>'{fields,excluded_assets}') @> jsonb_build_array(ref->'entity') THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
    expected:=jsonb_build_object('id',ref->'id','owner','operator','entity',original->'asset_ref','revision',NULL,'lineage','[]'::jsonb,
     'source','admitted_operator_seed','perceived_at',NULL,'time_uncertainty','unknown','status','premise','tier',NULL,'freshness','unknown','completeness','declared_only');
    IF ref IS DISTINCT FROM expected THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   WHEN 'cyber_terrain' THEN
    IF NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'references') a WHERE a->>'owner'='operator' AND a->'entity'=ref->'asset_ref')
    OR ref-ARRAY['id','owner','source','entity','asset_ref','vantage_ref','revision','lineage','perceived_at','time_uncertainty',
     'evaluated_at','temporal_frontier','valid_until','ttl_seconds','status','tier','freshness','completeness','limitations','corroborated','conflicted','integrity_hold']<>'{}'
    THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   WHEN 'campaign_trajectory' THEN
    IF p#>>'{predecessor,availability}' IS DISTINCT FROM 'committed' OR ref->>'id' IS DISTINCT FROM 'previous_decision'
    OR ref-ARRAY['id','owner','source','entity','event','recorded_at','status']<>'{}'
    THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   ELSE RAISE EXCEPTION 'run_pack_source_refused';
  END CASE;
 END LOOP;
 FOR candidate,ordinal IN SELECT value,n::integer FROM jsonb_array_elements(p->'candidates') WITH ORDINALITY a(value,n) LOOP
  SELECT value INTO ref FROM jsonb_array_elements(p->'references') WHERE value->>'owner'='operator' AND value->'id'=candidate->'reference';
  SELECT value INTO original FROM jsonb_array_elements(request#>'{context,candidates}') WHERE value->'asset_ref'=ref->'entity';
  IF ref IS NULL OR original IS NULL OR candidate->>'reference' IS DISTINCT FROM 'asset_'||(ordinal-1)::text
  OR candidate->>'status' IS DISTINCT FROM 'operator_premise' OR candidate->'priority' IS DISTINCT FROM original->'priority'
  OR candidate-ARRAY['reference','priority','status','terrain_references']<>'{}'
  THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 END LOOP;
END $$;
REVOKE ALL ON FUNCTION execution.assert_successor_pack(jsonb,jsonb,jsonb) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.successor_configuration(p jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE cfg jsonb := $configuration${
 "adapter_release": "autonomous-libcurl-v1",
 "content_type": "application/json",
 "execution_mode": "live",
 "instructions": "DuskWeave autonomous inference v1: propose one objective-led hypothesis, alternative, expected and falsifying observation and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not playbooks or permission. Operator seeds are premises, observations are not facts, and historical decisions do not grant current authority. Use exact references[].id and qualified manifests[].reference values only. observe_more with a DNS manifest proposes one observation for one seed; observe_more with view_request requests only an owner view. Do not combine them. Do not invent endpoints, references, availability, access or scope. Missing, stale, conflicted and fixture sources retain their limits. Explain uncertainty and any retention/revision from predecessor feedback. Retain, defer and stop are valid choices. Synthesis/unsupported effects are explanations of a capability gap, never permission to execute. Return concise safe explanation, never private reasoning or raw client content. Output grants no dispatch. Fixture results cannot establish live target facts.",
 "limits": {
  "accounting_window_ms": 3000,
  "calls_per_decision": 2,
  "cancel_grace_ms": 2000,
  "collection_entries": 64,
  "concurrency": 1,
  "configuration_bytes": 16384,
  "connect_time_ms": 5000,
  "context_bytes": 8192,
  "decision_wall_time_ms": 45000,
  "json_depth": 8,
  "per_call_wall_time_ms": 20000,
  "phase_wall_time_ms": 50000,
  "proposal_bytes": 8192,
  "repair_bytes": 2048,
  "request_bytes": 16384,
  "response_bytes": 65536,
  "response_envelope_json_depth": 32,
  "shared_wall_time_ms": 40000,
  "text_field_bytes": 1024
 },
 "method": "POST",
 "omissions": [
  "temperature",
  "top_p"
 ],
 "profile": "deepseek-responses-autonomous-v1",
 "recipient": "https://api.deepseek.com/responses",
 "requested_settings": {
  "max_output_tokens": 4096,
  "model": "deepseek-flash",
  "reasoning": {
   "effort": "high"
  },
  "stream": false,
  "tool_choice": "none",
  "tools": []
 },
 "run_profile": "autonomous-dns-v1",
 "schema": {
  "additionalProperties": false,
  "properties": {
   "considered_alternative": {
    "maxLength": 1024,
    "minLength": 1,
    "type": "string"
   },
   "expected_observation": {
    "maxLength": 1024,
    "minLength": 1,
    "type": "string"
   },
   "falsifying_observation": {
    "maxLength": 1024,
    "minLength": 1,
    "type": "string"
   },
   "hypothesis": {
    "maxLength": 1024,
    "minLength": 1,
    "type": "string"
   },
   "next_intent": {
    "anyOf": [
     {
      "additionalProperties": false,
      "properties": {
       "category": {
        "enum": [
         "validate"
        ],
        "type": "string"
       },
       "manifest_reference": {
        "type": "null"
       },
       "references": {
        "items": {
         "enum": [
          "mission",
          "asset_0"
         ],
         "type": "string"
        },
        "maxItems": 64,
        "type": "array"
       }
      },
      "required": [
       "category",
       "manifest_reference",
       "references"
      ],
      "type": "object"
     },
     {
      "additionalProperties": false,
      "properties": {
       "category": {
        "enum": [
         "observe_more"
        ],
        "type": "string"
       },
       "manifest_reference": {
        "anyOf": [
         {
          "type": "null"
         },
         {
          "enum": [
           "dns_udp"
          ],
          "type": "string"
         }
        ]
       },
       "references": {
        "items": {
         "enum": [
          "mission",
          "asset_0"
         ],
         "type": "string"
        },
        "maxItems": 64,
        "type": "array"
       },
       "view_request": {
        "anyOf": [
         {
          "type": "null"
         },
         {
          "additionalProperties": false,
          "properties": {
           "owner": {
            "enum": [
             "cyber_terrain",
             "foothold_graph",
             "attack_path_view",
             "objective_state",
             "campaign_trajectory"
            ],
            "type": "string"
           },
           "selectors": {
            "items": {
             "enum": [
              "mission",
              "asset_0"
             ],
             "type": "string"
            },
            "maxItems": 64,
            "type": "array"
           }
          },
          "required": [
           "owner",
           "selectors"
          ],
          "type": "object"
         }
        ]
       }
      },
      "required": [
       "category",
       "manifest_reference",
       "references",
       "view_request"
      ],
      "type": "object"
     },
     {
      "additionalProperties": false,
      "properties": {
       "category": {
        "enum": [
         "synthesize_candidate_implementation"
        ],
        "type": "string"
       },
       "manifest_reference": {
        "type": "null"
       },
       "references": {
        "items": {
         "enum": [
          "mission",
          "asset_0"
         ],
         "type": "string"
        },
        "maxItems": 64,
        "type": "array"
       }
      },
      "required": [
       "category",
       "manifest_reference",
       "references"
      ],
      "type": "object"
     },
     {
      "additionalProperties": false,
      "properties": {
       "category": {
        "enum": [
         "retain"
        ],
        "type": "string"
       },
       "manifest_reference": {
        "type": "null"
       },
       "references": {
        "items": {
         "enum": [
          "mission",
          "asset_0"
         ],
         "type": "string"
        },
        "maxItems": 64,
        "type": "array"
       }
      },
      "required": [
       "category",
       "manifest_reference",
       "references"
      ],
      "type": "object"
     },
     {
      "additionalProperties": false,
      "properties": {
       "category": {
        "enum": [
         "defer"
        ],
        "type": "string"
       },
       "manifest_reference": {
        "type": "null"
       },
       "references": {
        "items": {
         "enum": [
          "mission",
          "asset_0"
         ],
         "type": "string"
        },
        "maxItems": 64,
        "type": "array"
       }
      },
      "required": [
       "category",
       "manifest_reference",
       "references"
      ],
      "type": "object"
     },
     {
      "additionalProperties": false,
      "properties": {
       "category": {
        "enum": [
         "stop"
        ],
        "type": "string"
       },
       "manifest_reference": {
        "type": "null"
       },
       "references": {
        "items": {
         "enum": [
          "mission",
          "asset_0"
         ],
         "type": "string"
        },
        "maxItems": 64,
        "type": "array"
       }
      },
      "required": [
       "category",
       "manifest_reference",
       "references"
      ],
      "type": "object"
     }
    ]
   },
   "rationale": {
    "maxLength": 1024,
    "minLength": 1,
    "type": "string"
   },
   "references": {
    "items": {
     "enum": [
      "mission",
      "asset_0"
     ],
     "type": "string"
    },
    "maxItems": 64,
    "type": "array"
   },
   "uncertainty": {
    "maxLength": 1024,
    "minLength": 1,
    "type": "string"
   },
   "version": {
    "enum": [
     "m1_inference_proposal_v1"
    ],
    "type": "string"
   }
  },
  "required": [
   "considered_alternative",
   "expected_observation",
   "falsifying_observation",
   "hypothesis",
   "next_intent",
   "rationale",
   "references",
   "uncertainty",
   "version"
  ],
  "type": "object"
 },
 "schema_version": "m1_inference_proposal_v1",
 "template_version": "m1-autonomous-inference-v1",
 "transport": {
  "compression": false,
  "http_response_policy": "bounded-http1.1-interim-then-final-200-no-upgrade-identity-only",
  "json_refusal_policy": "bounded-json-refusal-v2-envelope32-proposal8",
  "normal_certificate_hostname_validation": true,
  "proxy": false,
  "redirects": false,
  "retries": false
 }
}$configuration$::jsonb;
 ids jsonb; refs jsonb; i integer; profile text:=p->>'inference_profile'; chat boolean; external boolean;
BEGIN
 IF jsonb_typeof(p->'references') IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'inference_binding_refused'; END IF;
 SELECT jsonb_agg(value->'id' ORDER BY n) INTO ids FROM jsonb_array_elements(p->'references') WITH ORDINALITY a(value,n);
 IF ids IS NULL OR jsonb_array_length(ids)>64 OR EXISTS(SELECT 1 FROM jsonb_array_elements(ids) a WHERE jsonb_typeof(a) IS DISTINCT FROM 'string')
 OR (SELECT count(DISTINCT a) FROM jsonb_array_elements(ids) a)<>jsonb_array_length(ids) THEN RAISE EXCEPTION 'inference_binding_refused'; END IF;
 chat:=profile='fixture-chat-completions-v1'; external:=profile='deepseek-responses-reasoning-v1';
 IF profile IS NULL OR profile NOT IN ('fixture-responses-v1','fixture-chat-completions-v1','deepseek-responses-reasoning-v1')
 THEN RAISE EXCEPTION 'inference_binding_refused'; END IF;
 refs:=jsonb_build_object('type','array','items',jsonb_build_object('type','string','enum',ids),'maxItems',64);
 cfg:=jsonb_set(cfg,'{schema,properties,references}',refs);
 FOR i IN 0..5 LOOP
  cfg:=jsonb_set(cfg,ARRAY['schema','properties','next_intent','anyOf',i::text,'properties','references'],refs);
 END LOOP;
 cfg:=jsonb_set(cfg,'{schema,properties,next_intent,anyOf,1,properties,view_request,anyOf,1,properties,selectors}',refs);
 cfg:=jsonb_set(cfg,'{schema,properties,next_intent,anyOf,1,properties,manifest_reference}',
 '{"anyOf":[{"type":"null"},{"type":"string","enum":["dns_udp"]}]}'::jsonb);
 cfg:=cfg||jsonb_build_object('profile',profile,'recipient',CASE WHEN external THEN 'https://api.deepseek.com/responses'
 WHEN chat THEN 'urn:duskweave:fixture:chat-completions:v1' ELSE 'urn:duskweave:fixture:responses:v1' END,
 'protocol',CASE WHEN chat THEN 'chat-completions-v1' ELSE 'responses-v1' END,
 'credential_reference',CASE WHEN external THEN 'DEEPSEEK_API_KEY' ELSE NULL END,
 'execution_mode','controlled_fixture','run_profile','autonomous-dns-fixture-v2','timing_profile','reasoning-bounded-v1',
 'adapter_release','inference-lifecycle-v1','template_version','m1-autonomous-inference-v2',
 'requested_settings',jsonb_build_object('model',CASE WHEN external THEN 'deepseek-flash' ELSE 'protocol-fixture-v1' END,
 'stream',false,'tools','[]'::jsonb,'tool_choice','none')||
 CASE WHEN chat THEN '{"max_tokens":4096}'::jsonb ELSE '{"max_output_tokens":4096}'::jsonb END ||
 CASE WHEN external THEN '{"reasoning":{"effort":"high"}}'::jsonb ELSE '{}'::jsonb END);
 cfg:=jsonb_set(cfg,'{limits}',cfg->'limits'||'{"soft_completion_ms":60000,"per_call_wall_time_ms":62000,
 "shared_wall_time_ms":124000,"decision_wall_time_ms":129000,"phase_wall_time_ms":134000}'::jsonb);
 RETURN cfg;
END $$;
REVOKE ALL ON FUNCTION execution.successor_configuration(jsonb) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.assert_successor_wire(p text,cfg jsonb,b jsonb,repair boolean) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE expected jsonb; inputs jsonb; messages jsonb; chat boolean; content text; codes jsonb;
BEGIN
 expected:=execution.successor_configuration(p::jsonb); chat:=expected->>'protocol'='chat-completions-v1';
 IF cfg::text IS DISTINCT FROM expected::text THEN RAISE EXCEPTION 'inference_configuration_refused'; END IF;
 inputs:=to_jsonb(p);
 messages:=jsonb_build_array(jsonb_build_object('role','system','content',cfg->'instructions'),
 jsonb_build_object('role','user','content',p));
 IF repair THEN
  content:=CASE WHEN chat THEN b#>>'{messages,2,content}' ELSE b#>>'{input,1,content}' END;
  IF content IS NULL OR octet_length(content)>2048 THEN RAISE EXCEPTION 'inference_repair_refused'; END IF;
  codes:=content::jsonb;
  IF jsonb_typeof(codes) IS DISTINCT FROM 'object' OR codes-ARRAY['repair_error_codes']<>'{}'
  OR jsonb_typeof(codes->'repair_error_codes') IS DISTINCT FROM 'array' THEN RAISE EXCEPTION 'inference_repair_refused'; END IF;
  IF jsonb_array_length(codes->'repair_error_codes') NOT BETWEEN 1 AND 8
  OR EXISTS(SELECT 1 FROM jsonb_array_elements(codes->'repair_error_codes') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string'
  OR v#>>'{}' NOT IN ('json_size','json_structure','json_trailing','proposal_shape','proposal_version','unexpected_view_request','proposal_text'))
  THEN RAISE EXCEPTION 'inference_repair_refused'; END IF;
  IF content IS DISTINCT FROM '{"repair_error_codes":'||replace((codes->'repair_error_codes')::text,', ',',')||'}'
  THEN RAISE EXCEPTION 'inference_repair_refused'; END IF;
  inputs:=jsonb_build_array(jsonb_build_object('role','user','content',p),jsonb_build_object('role','user','content',content));
  messages:=messages||jsonb_build_array(jsonb_build_object('role','user','content',content));
 END IF;
 expected:=cfg->'requested_settings'||CASE WHEN chat THEN jsonb_build_object('messages',messages,
 'response_format',jsonb_build_object('type','json_schema','json_schema',jsonb_build_object('name','m1_inference_proposal_v1','schema',cfg->'schema')))
 ELSE jsonb_build_object('instructions',cfg->'instructions','input',inputs,
 'text',jsonb_build_object('format',jsonb_build_object('type','json_schema','name','m1_inference_proposal_v1','schema',cfg->'schema'))) END;
 IF b::text IS DISTINCT FROM expected::text THEN RAISE EXCEPTION 'inference_body_refused'; END IF;
END $$;
REVOKE ALL ON FUNCTION execution.assert_successor_wire(text,jsonb,jsonb,boolean) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.run_predecessor(r uuid) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;d jsonb;t terrain.outbox;b execution.fixture_dns_results; prior jsonb;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable';END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT cd.payload INTO d FROM cognition.reasoning_decisions cd WHERE (cd.e,cd.c,cd.d)=(x.e,x.c,x.decision_one);
 SELECT * INTO b FROM execution.fixture_dns_results WHERE run_id=r;
 SELECT * INTO t FROM terrain.outbox WHERE (engagement_id,campaign_id,operation_id)=(x.e,x.c,x.terrain_operation);
 IF d IS NULL OR b.result IS NULL OR t.event_id IS NULL
 OR t.contract#>>'{observation,source_kind}'<>'fixture_dns_acquisition'
 OR t.contract#>>'{claim,claim_status}'<>'observed'
 OR NOT EXISTS(SELECT 1 FROM trajectory.terrain_history WHERE event_id=t.event_id AND status='accepted' AND contract=t.contract)
 OR NOT EXISTS(SELECT 1 FROM execution.inference_closures WHERE (e,c,execution.inference_closures.d)=(x.e,x.c,x.decision_one))
 THEN RAISE EXCEPTION 'run_feedback_pending';END IF;
 prior:=jsonb_build_object('availability','committed','decision_reference','previous_decision','selected_seed_reference','asset_'||(SELECT seed_index FROM execution.fixture_dns_attempts WHERE run_id=r)::text,
 'selected_seed_entity',(x.request->'seed_names'->(SELECT seed_index FROM execution.fixture_dns_attempts WHERE run_id=r))->'asset_ref',
 'selected_intent','observe_more','attempt_ref',x.attempt,'result_ref',x.result,'terrain_publication_ref',t.event_id,
 'execution_mode','controlled_fixture','outcome','owner_qualified_observed','hypothesis_summary_omission','not_required',
 'decision_entity',d->'decision','decision_event',d->'event','recorded_at',((d->>'recorded_at_ms')::bigint/1000));
 IF x.request->>'version'='m1_autonomous_run_request_v2' THEN
  prior:=(prior-'hypothesis_summary_omission')||jsonb_build_object('hypothesis_summary',d#>'{proposal,hypothesis}');
 END IF;
 RETURN prior;
END $$;

CREATE OR REPLACE FUNCTION execution.assert_autonomous_pack(e uuid,c uuid,d uuid,p jsonb,cfg jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; expected jsonb; prior jsonb; seed text; permission jsonb; registration jsonb;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs a WHERE (a.e,a.c)=(e,c) AND d IN(a.decision_one,a.decision_two);
 registration:=execution.run_current(x.run_id); permission:=registration#>'{fields,m1_inference_permission}';
 IF x.request->>'version'='m1_autonomous_run_request_v2' THEN
  PERFORM execution.assert_successor_pack(p,registration,x.request);
  IF (p->>'evaluated_at')::bigint>floor(extract(epoch FROM clock_timestamp()))::bigint
  OR (p->>'evaluated_at')::bigint<(registration->>'occurred_at')::bigint THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 END IF;
 IF p->>'version' IS DISTINCT FROM (CASE WHEN x.request->>'version'='m1_autonomous_run_request_v2' THEN 'm1_context_pack_autonomous_v2' ELSE 'm1_context_pack_autonomous_v1' END)
 OR (x.request->>'version'='m1_autonomous_run_request_v2' AND (p->>'inference_profile' IS DISTINCT FROM permission->>'profile' OR cfg->>'timing_profile' IS DISTINCT FROM 'reasoning-bounded-v1'))
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (x.request->>'run_profile',x.request->>'execution_mode')
 OR (cfg->>'run_profile',cfg->>'execution_mode') IS DISTINCT FROM (p->>'run_profile',p->>'execution_mode')
 OR (cfg->>'profile',cfg->>'recipient') IS DISTINCT FROM (permission->>'profile',permission->>'recipient')
 OR (p->>'engagement_id',p->>'campaign_id',p->>'decision_id') IS DISTINCT FROM (e::text,c::text,d::text)
 OR p->'manifests' IS DISTINCT FROM jsonb_build_array(jsonb_build_object('availability','qualified','execution_mode','controlled_fixture',
 'profile','dns-routing-fixture-v1','qualification_ref',x.qualification_ref,'reference','dns_udp','release_sha256',x.release_sha256,
 'schema','m1_dns_ipc_v1','transport','udp')) THEN RAISE EXCEPTION 'run_pack_binding_refused'; END IF;
 IF d=x.decision_one THEN
  IF p->'predecessor' IS DISTINCT FROM '{"availability":"absent","reason":"first_decision"}'::jsonb
  THEN RAISE EXCEPTION 'run_predecessor_refused'; END IF;
 ELSE
  expected:=execution.run_predecessor(x.run_id); prior:=p->'predecessor';
  SELECT ref->>'id' INTO seed FROM jsonb_array_elements(p->'references') ref
  WHERE ref->>'owner'='operator' AND ref->'entity'=expected->'selected_seed_entity';
  expected:=jsonb_set(expected,'{selected_seed_reference}',to_jsonb(seed));
  IF x.request->>'version'='m1_autonomous_run_request_v2' AND prior->>'hypothesis_summary_omission'='byte_bound'
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

CREATE OR REPLACE FUNCTION execution.inference_may_send(e uuid,c uuid,d uuid,k uuid,p text,cfg text,b text,h text)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable

DECLARE r execution.inference_reservations;s jsonb;perm jsonb;now_s bigint;actual text;remaining bigint;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') THEN RAISE EXCEPTION 'inference_actor_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(e::text||':'||c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=e AND campaign_id=c;
 SELECT * INTO r FROM execution.inference_reservations x
 WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,d) AND x.record_revision=1;
 IF r.operation_id IS NULL OR r.writer_oid<>(SELECT oid FROM pg_roles WHERE rolname=session_user)
 OR NOT EXISTS(SELECT 1 FROM execution.native_input_bindings i WHERE (i.e,i.c,i.d,i.writer_oid)=(e,c,d,r.writer_oid))
 OR k NOT IN (r.generation_call_id,r.repair_call_id)
 OR EXISTS(SELECT 1 FROM execution.inference_reservations x
 WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,d) AND x.record_revision=2)
 THEN
 RAISE EXCEPTION 'inference_reservation_refused'; END IF;
 s:=mission.current_inference_publication(e,c,d);perm:=s->'fields'->'m1_inference_permission';
 remaining:=execution.inference_window_ms(e,c,d);
 IF perm->>'version'='m1_inference_permission_v4' AND remaining<62000 THEN RAISE EXCEPTION 'inference_call_window_refused'; END IF;
 IF NOT coalesce(perm->'field_classes' ? 'admitted_repair_fields_codes',false) THEN RAISE EXCEPTION 'inference_consent_refused'; END IF;
 IF perm->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4') THEN PERFORM execution.assert_autonomous_pack(e,c,d,p::jsonb,cfg::jsonb);END IF;
 IF perm->>'version'='m1_inference_permission_v4' THEN
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
 OR (CASE WHEN cfg::jsonb->>'protocol'='chat-completions-v1' THEN b::jsonb#>>'{messages,1,content}' ELSE b::jsonb->>'input' END) IS DISTINCT FROM p AND k=r.generation_call_id THEN
 RAISE EXCEPTION 'inference_authority_refused'; END IF;
 actual:=encode(sha256(convert_to('dw-inference-binding-v1','UTF8')||decode('00','hex')||
 int8send(octet_length(p)::bigint)||convert_to(p,'UTF8')||
 int8send(octet_length(cfg)::bigint)||convert_to(cfg,'UTF8')||
 int8send(octet_length(b)::bigint)||convert_to(b,'UTF8')),'hex');
 IF actual<>h THEN RAISE EXCEPTION 'inference_binding_refused'; END IF;
 IF k=r.repair_call_id AND NOT EXISTS(SELECT 1 FROM execution.inference_calls x
 WHERE x.call=r.generation_call_id AND x.state='completed') THEN
 RAISE EXCEPTION 'inference_repair_refused'; END IF;
 INSERT INTO execution.inference_calls VALUES(e,c,d,k,h,p,cfg,b,'may_send_uncertain',NULL);
END $$;

CREATE OR REPLACE FUNCTION terrain.issued_dns_request(q jsonb) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE raw jsonb; e uuid; c uuid; result_id uuid;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_terrain','MEMBER') THEN RAISE EXCEPTION 'unqualified_terrain_writer'; END IF;
 IF jsonb_typeof(q) IS DISTINCT FROM 'object' OR q-ARRAY['version','engagement_id','campaign_id','result_ref','run_profile','execution_mode']<>'{}'
 OR q->>'version' IS DISTINCT FROM 'm1_terrain_dns_issued_observation_v1'
 OR q->>'run_profile' IS NULL
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2')
 OR q->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
 THEN RAISE EXCEPTION 'issued_reference_refused'; END IF;
 e:=(q->>'engagement_id')::uuid; c:=(q->>'campaign_id')::uuid; result_id:=(q->>'result_ref')::uuid;
 SELECT jsonb_build_object('request',b.request,'operation',x.terrain_operation) INTO raw
 FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a USING(run_id)
 JOIN execution.autonomous_runs x USING(run_id) WHERE b.result=result_id AND (x.e,x.c)=(e,c)
 AND (x.request->>'run_profile',x.request->>'execution_mode')=(q->>'run_profile',q->>'execution_mode')
 AND b.result=x.result AND b.attempt=a.attempt AND b.attempt=x.attempt AND a.decision=x.decision_one
 AND a.session_generation>0 AND b.release_sha256=x.release_sha256 AND b.release_sha256=a.release_sha256
 AND b.mode=a.mode AND b.profile=a.profile AND b.mode='controlled_fixture' AND b.profile='dns-routing-fixture-v1'
 AND b.publication_obligation='terrain.issued_dns.v1'
 AND (b.request->>'engagement_id',b.request->>'campaign_id',b.request->>'correlation_id')=(e::text,c::text,result_id::text)
 AND (b.request->>'version',b.request->>'source_kind')=('m1_terrain_dns_issued_observation_v1','fixture_dns_acquisition');
 IF raw IS NULL THEN RAISE EXCEPTION 'issued_result_unavailable'; END IF;
 RETURN raw;
END $$;

CREATE OR REPLACE FUNCTION execution.run_summary_without_diagnostics(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs; y execution.autonomous_control; status text; summary jsonb;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT * INTO y FROM execution.autonomous_control WHERE run_id=r;
 status:=CASE WHEN y.status='running' AND least(y.lease_until_ms,x.deadline_at_ms)<=floor(extract(epoch FROM clock_timestamp())*1000)::bigint
 THEN 'unknown_supervisor_lost' ELSE y.status END;
 summary:=jsonb_build_object('kind','m1_autonomous_run_receipt_v1','run_id',r,'run_profile',x.request->'run_profile',
 'execution_mode',x.request->'execution_mode','status',status,'phase',y.phase,'dispatch_granted',false,'live_qualified',false,
 'decision_one',x.decision_one,'decision_two',x.decision_two,'attempt',x.attempt,'result',x.result,
 'terrain_operation',x.terrain_operation,
 'phases',(SELECT coalesce(jsonb_agg(jsonb_build_object('phase',p.phase,'status',p.status,'recorded_at_ms',p.recorded_at_ms) ORDER BY p.recorded_at_ms),'[]') FROM execution.autonomous_phases p WHERE run_id=r),
 'inference',(SELECT coalesce(jsonb_agg(jsonb_build_object('decision',ic.d,'call',ic.call,'binding',ic.binding,'state',ic.state,'reported',ic.usage) ORDER BY ic.d,ic.call),'[]') FROM execution.inference_calls ic WHERE (ic.e,ic.c)=(x.e,x.c) AND ic.d IN(x.decision_one,x.decision_two)),
 'acquisition',CASE WHEN EXISTS(SELECT 1 FROM execution.fixture_dns_results WHERE run_id=r) THEN 'committed_fixture_result'
   WHEN EXISTS(SELECT 1 FROM execution.fixture_dns_attempts WHERE run_id=r) THEN 'reserved_outcome_unknown' ELSE 'not_reserved' END,
 'terrain',CASE WHEN EXISTS(SELECT 1 FROM terrain.outbox WHERE operation_id=x.terrain_operation) THEN 'committed' ELSE 'pending_or_not_requested' END,
 'effect_count',(SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=r),
 'decision_count',(SELECT count(*) FROM cognition.reasoning_decisions cd WHERE (cd.e,cd.c)=(x.e,x.c) AND cd.d IN(x.decision_one,x.decision_two)),
 'decision_history_count',(SELECT count(*) FROM trajectory.reasoning_history h WHERE (h.engagement_id,h.campaign_id)=(x.e,x.c) AND h.operation_id IN(x.decision_one,x.decision_two) AND h.status='accepted'),
 'terrain_history_count',(SELECT count(*) FROM trajectory.terrain_history h WHERE h.operation_id=x.terrain_operation AND h.status='accepted'));
 IF x.request->>'version'='m1_autonomous_run_request_v2' THEN
  summary:=summary||jsonb_build_object('inference_profile',x.request->'inference_profile',
   'recorded_decisions',(SELECT coalesce(jsonb_agg(jsonb_build_object('decision',cd.d,'event',cd.event,
    'version',cd.payload->'version','disposition',cd.payload->'disposition','proposal',cd.payload->'proposal',
    'occurred_at_ms',cd.payload->'occurred_at_ms','recorded_at_ms',cd.payload->'recorded_at_ms','dispatch_granted',false,
    'current_use',CASE WHEN cd.payload->>'disposition'='recorded_stale_premises' THEN 'blocked_stale_premises'
     WHEN cd.payload->>'disposition'='validated_proposal' THEN 'requires_current_owner_admission' ELSE 'not_admitted' END,
    'history',CASE WHEN EXISTS(SELECT 1 FROM trajectory.reasoning_history h WHERE
     (h.engagement_id,h.campaign_id,h.operation_id,h.event_id,h.status,h.contract)=(cd.e,cd.c,cd.d,cd.event,'accepted',cd.payload))
     THEN 'completed' ELSE 'pending_unknown' END)
      ORDER BY CASE WHEN cd.d=x.decision_one THEN 0 ELSE 1 END),'[]') FROM cognition.reasoning_decisions cd
    WHERE (cd.e,cd.c)=(x.e,x.c) AND cd.d IN(x.decision_one,x.decision_two)));
 END IF;
 RETURN summary;
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
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user,g,'MEMBER'))
       OR EXISTS (SELECT 1 FROM pg_roles p WHERE pg_has_role(session_user,p.oid,'SET') AND (p.rolsuper OR p.rolbypassrls OR p.rolcreatedb OR p.rolcreaterole)) THEN
        RAISE EXCEPTION 'unqualified_terrain_writer' USING ERRCODE='42501';
    END IF;
    IF current_setting('transaction_isolation')<>'serializable'
       OR current_setting('synchronous_commit')<>'on' THEN
        RAISE EXCEPTION 'terrain_transaction_refused';
    END IF;
    IF jsonb_typeof(p_request) IS DISTINCT FROM 'object' OR octet_length(p_request::text)>16384 THEN
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
        IF k='asset_ref' AND r->k='null'::jsonb THEN CONTINUE; END IF;
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'string'
           OR (r->>k) !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           OR (r->>k)::uuid='00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    FOREACH k IN ARRAY ARRAY['expected_mission_revision','expected_claim_revision','perceived_at','uncertainty_seconds'] LOOP
        IF jsonb_typeof(r->k) IS DISTINCT FROM 'number' OR (r->>k) !~ '^(0|[1-9][0-9]*)$' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF NOT ((r->>'version',r->>'source_kind')=('m1_terrain_dns_observation_v1','operator_reviewed_dns') OR
 (r->>'version',r->>'source_kind')=('m1_terrain_dns_issued_observation_v1','fixture_dns_acquisition') AND EXISTS(
 SELECT 1 FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a USING(run_id)
 JOIN execution.autonomous_runs x USING(run_id) WHERE b.result=(r->>'correlation_id')::uuid
 AND b.request=r AND b.result=x.result AND b.attempt=a.attempt AND b.attempt=x.attempt
 AND a.decision=x.decision_one AND a.session_generation>0 AND b.profile=a.profile AND b.mode=a.mode
 AND b.profile='dns-routing-fixture-v1' AND b.mode='controlled_fixture'
 AND b.release_sha256=a.release_sha256 AND b.release_sha256=x.release_sha256
 AND (x.e,x.c)=((r->>'engagement_id')::uuid,(r->>'campaign_id')::uuid)
 AND x.request->>'execution_mode'=b.mode
 AND x.request->>'run_profile' IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2')
 AND b.publication_obligation='terrain.issued_dns.v1'))
       OR r->>'expected_mission_revision'<>'1' OR (r->>'expected_claim_revision')::bigint>10000
       OR (r->>'uncertainty_seconds')::bigint>300 OR (r->>'perceived_at')::bigint<=0
       OR (r->>'perceived_at')::bigint>now_at OR r->>'exercise_mode' NOT IN ('blind','defender_informed')
       OR r->>'transport' NOT IN ('udp','tcp') OR r->>'qtype'<>'A' OR r->>'qclass'<>'IN'
       OR r->>'rcode' NOT IN ('noerror','nxdomain','servfail','refused','formerr','notimp')
       OR r->>'completeness' NOT IN ('complete','incomplete','truncated','timeout')
       OR jsonb_typeof(r->'truncated') IS DISTINCT FROM 'boolean' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF length(r->>'subject_name')>253 OR position('.' in r->>'subject_name')=0
       OR r->>'subject_name' ~ '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$' THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
    FOREACH label IN ARRAY string_to_array(r->>'subject_name','.') LOOP
        IF length(label)>63 OR label !~ '^[a-z0-9]([a-z0-9-]*[a-z0-9])?$' OR label LIKE 'xn--%' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF family((r->>'resolver_ipv4')::inet)<>4 OR host((r->>'resolver_ipv4')::inet)<>r->>'resolver_ipv4' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF jsonb_typeof(r->'answers') IS DISTINCT FROM 'array' OR jsonb_typeof(r->'limitations') IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    IF jsonb_array_length(r->'answers')>4 OR jsonb_array_length(r->'limitations')>4 THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
    FOR a IN SELECT value FROM jsonb_array_elements(r->'answers') LOOP
        IF jsonb_typeof(a) IS DISTINCT FROM 'object' OR (a-ARRAY['ipv4','ttl_seconds'])<>'{}'::jsonb
           OR jsonb_typeof(a->'ipv4') IS DISTINCT FROM 'string' OR jsonb_typeof(a->'ttl_seconds') IS DISTINCT FROM 'number'
           OR (a->>'ttl_seconds') !~ '^(0|[1-9][0-9]*)$' THEN RAISE EXCEPTION 'terrain_request_refused'; END IF;
        IF (a->>'ttl_seconds')::bigint>86400 OR family((a->>'ipv4')::inet)<>4 OR host((a->>'ipv4')::inet)<>a->>'ipv4' THEN
            RAISE EXCEPTION 'terrain_request_refused';
        END IF;
    END LOOP;
    IF (SELECT count(DISTINCT value->>'ipv4') FROM jsonb_array_elements(r->'answers'))<>jsonb_array_length(r->'answers')
       OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(r->'limitations'))<>jsonb_array_length(r->'limitations')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(r->'limitations') x WHERE jsonb_typeof(x.value)<>'string'
          OR x.value#>>'{}' NOT IN ('cache_remaining_ttl','single_vantage_only','partial_alias_chain','resolver_recursion_variance')) THEN
        RAISE EXCEPTION 'terrain_request_refused';
    END IF;
    SELECT * INTO m FROM mission.missions WHERE engagement_id=(r->>'engagement_id')::uuid AND campaign_id=(r->>'campaign_id')::uuid FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    SELECT * INTO reg FROM mission.registration_outbox WHERE engagement_id=m.engagement_id AND campaign_id=m.campaign_id AND operation_id=m.operation_id;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    f := reg.contract->'fields'; policy := f->'m1_permission';
    IF ((reg.contract @> jsonb_build_object('engagement_id',m.engagement_id,'campaign_id',m.campaign_id,'operation_id',m.operation_id,
          'event_id',reg.event_id,'producer','mission','kind','mission_registered','owner_revision',m.revision)) IS NOT TRUE
       OR (f @> jsonb_build_object('operator_ref',m.operator_ref,'goal_ref',m.goal_ref,'authority_ref',m.authority_ref,
          'authority_revision',m.authority_revision,'included_assets',m.included_assets,'excluded_assets',m.excluded_assets,
          'exercise_mode',m.exercise_mode,'starts_at',m.starts_at,'ends_at',m.ends_at)) IS NOT TRUE
       OR m.revision<>1 OR r->>'operator_ref'<>m.operator_ref::text OR r->>'goal_ref'<>m.goal_ref::text
       OR r->>'exercise_mode'<>m.exercise_mode OR policy->>'policy_version' IS DISTINCT FROM '1'
       OR policy->>'vantage_ref' IS DISTINCT FROM r->>'vantage_ref' OR policy->>'resolver_ipv4' IS DISTINCT FROM r->>'resolver_ipv4'
       OR NOT (m.starts_at<=now_at AND now_at<m.ends_at AND m.starts_at<=(r->>'perceived_at')::bigint AND (r->>'perceived_at')::bigint<m.ends_at)
       OR NOT ((policy->>'starts_at')::bigint<=now_at AND now_at<(policy->>'ends_at')::bigint
          AND (policy->>'starts_at')::bigint<=(r->>'perceived_at')::bigint AND (r->>'perceived_at')::bigint<(policy->>'ends_at')::bigint)
       OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE engagement_id=m.engagement_id AND campaign_id=m.campaign_id)) IS NOT FALSE THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    IF r->'asset_ref'='null'::jsonb THEN rules:=policy->'discovery_rules';
    ELSE
        IF NOT (m.included_assets @> jsonb_build_array(r->'asset_ref')) OR m.excluded_assets @> jsonb_build_array(r->'asset_ref') THEN
            RAISE EXCEPTION 'terrain_authority_unavailable';
        END IF;
        SELECT coalesce(jsonb_agg(value->'rule'),'[]'::jsonb) INTO rules FROM jsonb_array_elements(policy->'contact_rules') WHERE value->'asset_ref'=r->'asset_ref';
    END IF;
    FOR rule IN SELECT value FROM jsonb_array_elements(rules) LOOP
        matched := matched OR rule->>'exact'=r->>'subject_name' OR rule->>'label_suffix'=r->>'subject_name'
            OR right(r->>'subject_name',length(rule->>'label_suffix')+1)='.'||(rule->>'label_suffix');
    END LOOP;
    IF matched IS NOT TRUE OR EXISTS (SELECT 1 FROM jsonb_array_elements(policy->'excluded_names') x
        WHERE x.value->>'exact'=r->>'subject_name' OR x.value->>'label_suffix'=r->>'subject_name'
           OR right(r->>'subject_name',length(x.value->>'label_suffix')+1)='.'||(x.value->>'label_suffix')) THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    serialized := replace(json_build_object('version',r->'version','engagement_id',r->'engagement_id','campaign_id',r->'campaign_id',
        'operator_ref',r->'operator_ref','expected_mission_revision',r->'expected_mission_revision','goal_ref',r->'goal_ref',
        'exercise_mode',r->'exercise_mode','correlation_id',r->'correlation_id','source_kind',r->'source_kind','vantage_ref',r->'vantage_ref',
        'resolver_ipv4',r->'resolver_ipv4','subject_name',r->'subject_name','asset_ref',r->'asset_ref','expected_claim_revision',r->'expected_claim_revision',
        'perceived_at',r->'perceived_at','uncertainty_seconds',r->'uncertainty_seconds','transport',r->'transport','qtype',r->'qtype','qclass',r->'qclass',
        'rcode',r->'rcode','truncated',r->'truncated','completeness',r->'completeness','limitations',r->'limitations','answers',r->'answers')::text,' ','');
    RETURN jsonb_build_object('request',r,'registration',reg.contract,'request_hash',encode(sha256(convert_to(serialized,'UTF8')),'hex'),'now',now_at);
EXCEPTION WHEN data_exception THEN RAISE EXCEPTION 'terrain_request_refused';
END; $$;



-- Additive compatible catalog changes, repeatable in the explicit fixture refresh.
ALTER TABLE trajectory.reasoning_history DROP CONSTRAINT IF EXISTS reasoning_history_version_check;
ALTER TABLE trajectory.reasoning_history ADD CONSTRAINT reasoning_history_version_check CHECK(version IN (1,2));

CREATE OR REPLACE FUNCTION cognition.successor_reasoning_identity(x jsonb) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT coalesce(EXISTS(
 SELECT 1 FROM execution.autonomous_runs r JOIN mission.registration_outbox m ON (m.engagement_id,m.campaign_id)=(r.e,r.c)
 JOIN execution.native_input_bindings i ON (i.e,i.c,i.d)=((x->>'engagement')::uuid,(x->>'campaign')::uuid,(x->>'decision')::uuid)
 WHERE (r.e,r.c)=((x->>'engagement')::uuid,(x->>'campaign')::uuid)
 AND (x->>'decision')::uuid IN(r.decision_one,r.decision_two) AND i.writer_oid=r.writer_oid
 AND r.request->>'version'='m1_autonomous_run_request_v2'
 AND r.request->>'run_profile'='autonomous-dns-fixture-v2'
 AND m.contract#>>'{fields,m1_inference_permission,version}'='m1_inference_permission_v4'
 AND (m.contract#>>'{fields,m1_inference_permission,profile}')=(r.request->>'inference_profile')
 AND (m.contract#>>'{fields,m1_inference_permission,permission_id}')=(x->>'permission')
 AND r.request#>>'{context,mission,permission_id}'=x->>'permission'
 AND r.request#>>'{context,mission,exercise_mode}'=x->>'source_mode'
 AND m.contract#>>'{fields,exercise_mode}'=x->>'source_mode'
 AND r.request#>>'{context,mission,expected_mission_revision}'=x->>'mission_revision'
 AND (m.contract->>'owner_revision')=(x->>'mission_revision')
 AND (x->'version')='2'::jsonb AND x->>'version'='2'
 AND jsonb_typeof(x->'provenance')='array'
 AND NOT EXISTS(SELECT 1 FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(i.e,i.c,i.d)
  AND ((a.p::jsonb->>'version') IS DISTINCT FROM 'm1_context_pack_autonomous_v2'
   OR (a.p::jsonb->>'inference_profile') IS DISTINCT FROM r.request->>'inference_profile'))
 ),false)
$$;
REVOKE ALL ON FUNCTION cognition.successor_reasoning_identity(jsonb) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.assert_current_pack_terrain(e uuid,c uuid,d uuid,p jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE source jsonb; ref jsonb; v jsonb; claim jsonb; q jsonb;
BEGIN
 source:=mission.current_inference_publication(e,c,d);
 IF source IS NULL THEN RAISE EXCEPTION 'fixture_authority_expired'; END IF;
 FOR ref IN SELECT value FROM jsonb_array_elements(p->'references') WHERE value->>'owner'='cyber_terrain' LOOP
  q:=jsonb_build_object('version','m1_terrain_context_view_query_v1','engagement_id',e,'campaign_id',c,
   'operator_ref',source#>'{fields,operator_ref}','expected_mission_revision',1,'goal_ref',source#>'{fields,goal_ref}',
   'exercise_mode',source#>'{fields,exercise_mode}','selected_assets',jsonb_build_array(ref->'asset_ref'),
   'claim_ids',jsonb_build_array(ref->'entity'));
  v:=terrain.current_dns_context_view(q);
  SELECT value INTO claim FROM jsonb_array_elements(v->'claims') WHERE value->'claim_id'=ref->'entity';
  IF claim IS NULL OR
   (claim->'claim_revision',claim->'asset_ref',claim->'event_id',claim->'operation_id',
    claim->'observation_id',claim->'material_id',claim->'evaluation_id',claim->'perceived_at',
    claim->'valid_until',claim->'temporal_frontier',claim->'source_kind') IS DISTINCT FROM
   (ref->'revision',ref->'asset_ref',ref->'lineage'->0,ref->'lineage'->1,
    ref->'lineage'->2,ref->'lineage'->3,ref->'lineage'->4,ref->'perceived_at',
    ref->'valid_until',ref->'temporal_frontier',ref->'source')
   OR claim->>'claim_status' IS DISTINCT FROM 'observed'
   OR claim->'conflicted' IS DISTINCT FROM 'false'::jsonb
   OR claim->'integrity_hold' IS DISTINCT FROM 'false'::jsonb
   OR claim->>'outcome' IS DISTINCT FROM 'qualified_dns_claim'
   OR (claim->'vantage_ref',claim->'uncertainty_seconds',claim->'lowest_ttl_seconds',claim->'completeness',claim->'limitations') IS DISTINCT FROM
      (ref->'vantage_ref',ref->'time_uncertainty',ref->'ttl_seconds',ref->'completeness',ref->'limitations')
   OR (ref->>'status',ref->>'tier',ref->>'freshness') IS DISTINCT FROM ('observed','tier1','fresh')
   OR ref->'conflicted' IS DISTINCT FROM 'false'::jsonb OR ref->'integrity_hold' IS DISTINCT FROM 'false'::jsonb
   OR ref->'corroborated' IS DISTINCT FROM 'false'::jsonb OR ref->'evaluated_at' IS DISTINCT FROM p->'evaluated_at'
   OR (claim->>'valid_until')::bigint <= (v->>'evaluated_at')::bigint
  THEN RAISE EXCEPTION 'fixture_stale_premises'; END IF;
 END LOOP;
END $$;
REVOKE ALL ON FUNCTION execution.assert_current_pack_terrain(uuid,uuid,uuid,jsonb) FROM PUBLIC;


CREATE OR REPLACE FUNCTION cognition.publish_reasoning(x jsonb,input_hash text) RETURNS void LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable

DECLARE existing jsonb;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_cognition','MEMBER') THEN RAISE EXCEPTION 'inference_actor_refused'; END IF;
 IF jsonb_typeof(x) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF NOT x ?& ARRAY['kind','occurred_at_ms','recorded_at_ms','source_mode','provenance','event',
 'engagement','campaign','decision','producer','version','mission_revision','permission',
 'call_ids','bindings','proposal','disposition','publication_obligation']
 OR (SELECT count(*) FROM jsonb_object_keys(x))<>18
 OR x->>'kind' IS DISTINCT FROM 'reasoning_decision'
 OR (x->>'source_mode' IN ('blind','defender_informed')) IS DISTINCT FROM true
 OR jsonb_typeof(x->'provenance') NOT IN ('array','null')
 OR EXISTS(SELECT 1 FROM unnest(ARRAY['occurred_at_ms','recorded_at_ms']) f
 WHERE jsonb_typeof(x->f) IS DISTINCT FROM 'number' OR x->>f !~ '^[0-9]+$'
 OR (x->>f)::numeric NOT BETWEEN 0 AND 18446744073709551615)
 THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF x->>'version'='2' OR x->>'disposition'='recorded_stale_premises' THEN
 IF NOT cognition.successor_reasoning_identity(x) THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 END IF;
 IF x->>'producer' IS DISTINCT FROM 'cognition'
 OR (x->'version' IN ('1'::jsonb,'2'::jsonb)) IS DISTINCT FROM true
 OR x->>'publication_obligation' IS DISTINCT FROM 'trajectory.reasoning_history.v1'
 OR jsonb_typeof(x->'producer') IS DISTINCT FROM 'string'
 OR jsonb_typeof(x->'publication_obligation') IS DISTINCT FROM 'string'
 OR jsonb_typeof(x->'mission_revision') IS DISTINCT FROM 'number'
 OR jsonb_typeof(x->'call_ids') IS DISTINCT FROM 'array'
 OR jsonb_typeof(x->'bindings') IS DISTINCT FROM 'array'
 OR jsonb_typeof(x->'disposition') IS DISTINCT FROM 'string'
 OR NOT (x ? 'proposal') THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF x->>'mission_revision' !~ '^[0-9]+$' OR (x->>'mission_revision')::numeric NOT BETWEEN 1 AND 9223372036854775807
 OR x->>'disposition' NOT IN (
  'validated_proposal','recorded_stale_premises','schema_repair_exhausted','unsafe_explanation','unsupported_premise',
  'reference_ineligible','manifest_ineligible','view_selector_ineligible','current_authority_unavailable',
  'repair_code_ineligible','repair_code_limit','pack_size','pack_utf8',
  'repair_encode','request_encode','request_size','inference_marker_commit_unknown',
  'inference_marker_refused','inference_marker_unknown','inference_sql_ambiguous','inference_sql_privilege',
  'inference_sql_function_missing','stopped_before_release','inference_stop_unknown','inference_authority_unavailable',
  'inference_reservation_unknown','inference_reservation_missing','inference_deadline','inference_guard_missing',
  'inference_platform_unqualified','inference_guard_busy','inference_fixture_endpoint_refused','live_key_missing',
  'live_key_invalid','inference_curl_missing','inference_pack_encode','inference_config_encode',
  'inference_body_encode','inference_usage_encode','inference_accounting_unknown','inference_database_config_missing',
  'inference_database_config_invalid','inference_database_unavailable','provider_incomplete','provider_protocol',
  'provider_unsafe_response','provider_response_limit','provider_transport_unknown','provider_cancelled_uncertain',
  'accounting_unknown','inference_release_unknown','inference_transport_unavailable',
  'inference_inspect_unavailable') THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF EXISTS(SELECT 1 FROM unnest(ARRAY['event','engagement','campaign','decision','permission']) f
 WHERE jsonb_typeof(x->f) IS DISTINCT FROM 'string'
 OR (x->>f) IS NULL OR (x->>f) !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(x->'call_ids') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string'
 OR v#>>'{}' !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(x->'bindings') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
 THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF x->>'disposition' IN ('validated_proposal','recorded_stale_premises') THEN
  IF jsonb_typeof(x->'proposal') IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 ELSE
  IF jsonb_typeof(x->'proposal') IS DISTINCT FROM 'null' THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 END IF;
 IF NOT EXISTS(SELECT 1 FROM execution.native_input_bindings i WHERE
 (i.e,i.c,i.d)=((x->>'engagement')::uuid,(x->>'campaign')::uuid,(x->>'decision')::uuid)
 AND i.input_hash=publish_reasoning.input_hash) THEN RAISE EXCEPTION 'inference_input_refused'; END IF;
 SELECT payload INTO existing FROM cognition.reasoning_decisions z WHERE
 (z.e,z.c,z.d)=((x->>'engagement')::uuid,(x->>'campaign')::uuid,(x->>'decision')::uuid);
 IF FOUND THEN
 IF existing IS DISTINCT FROM x THEN RAISE EXCEPTION 'inference_identity_conflict'; END IF;
 RETURN; END IF;
 INSERT INTO cognition.reasoning_decisions VALUES((x->>'engagement')::uuid,(x->>'campaign')::uuid,
 (x->>'decision')::uuid,(x->>'event')::uuid,input_hash,x);
 INSERT INTO cognition.reasoning_outbox VALUES((x->>'event')::uuid,'trajectory.reasoning_history.v1');
END $$;

CREATE OR REPLACE FUNCTION cognition.reasoning_source_matches(k uuid,x jsonb) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE existing jsonb;s jsonb;r execution.inference_reservations;e uuid;c uuid;d uuid;
BEGIN

 SELECT payload INTO s FROM cognition.reasoning_decisions z JOIN cognition.reasoning_outbox o USING(event)
 JOIN execution.native_input_bindings i ON (i.e,i.c,i.d,i.input_hash)=(z.e,z.c,z.d,z.input_hash) WHERE z.event=k;
 IF s IS NULL OR s IS DISTINCT FROM x THEN RAISE EXCEPTION 'inference_source_unavailable'; END IF;
 IF s->>'kind' IS DISTINCT FROM 'reasoning_decision' OR s->>'source_mode' NOT IN ('blind','defender_informed') OR jsonb_typeof(s->'occurred_at_ms') IS DISTINCT FROM 'number' OR jsonb_typeof(s->'recorded_at_ms') IS DISTINCT FROM 'number' OR jsonb_typeof(s->'provenance') NOT IN ('array','null') THEN RAISE EXCEPTION 'inference_binding_conflict';END IF;
 IF jsonb_typeof(s) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF s->>'version'='2' OR s->>'disposition'='recorded_stale_premises' THEN
 IF NOT cognition.successor_reasoning_identity(s) THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 END IF;
 IF s->>'producer' IS DISTINCT FROM 'cognition'
 OR (s->'version' IN ('1'::jsonb,'2'::jsonb)) IS DISTINCT FROM true
 OR s->>'publication_obligation' IS DISTINCT FROM 'trajectory.reasoning_history.v1'
 OR jsonb_typeof(s->'producer') IS DISTINCT FROM 'string'
 OR jsonb_typeof(s->'publication_obligation') IS DISTINCT FROM 'string'
 OR jsonb_typeof(s->'mission_revision') IS DISTINCT FROM 'number'
 OR jsonb_typeof(s->'call_ids') IS DISTINCT FROM 'array'
 OR jsonb_typeof(s->'bindings') IS DISTINCT FROM 'array'
 OR jsonb_typeof(s->'disposition') IS DISTINCT FROM 'string'
 OR NOT (s ? 'proposal') THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF s->>'mission_revision' !~ '^[0-9]+$' OR (s->>'mission_revision')::numeric NOT BETWEEN 1 AND 9223372036854775807
 OR s->>'disposition' NOT IN (
  'validated_proposal','recorded_stale_premises','schema_repair_exhausted','unsafe_explanation','unsupported_premise',
  'reference_ineligible','manifest_ineligible','view_selector_ineligible','current_authority_unavailable',
  'repair_code_ineligible','repair_code_limit','pack_size','pack_utf8',
  'repair_encode','request_encode','request_size','inference_marker_commit_unknown',
  'inference_marker_refused','inference_marker_unknown','inference_sql_ambiguous','inference_sql_privilege',
  'inference_sql_function_missing','stopped_before_release','inference_stop_unknown','inference_authority_unavailable',
  'inference_reservation_unknown','inference_reservation_missing','inference_deadline','inference_guard_missing',
  'inference_platform_unqualified','inference_guard_busy','inference_fixture_endpoint_refused','live_key_missing',
  'live_key_invalid','inference_curl_missing','inference_pack_encode','inference_config_encode',
  'inference_body_encode','inference_usage_encode','inference_accounting_unknown','inference_database_config_missing',
  'inference_database_config_invalid','inference_database_unavailable','provider_incomplete','provider_protocol',
  'provider_unsafe_response','provider_response_limit','provider_transport_unknown','provider_cancelled_uncertain',
  'accounting_unknown','inference_release_unknown','inference_transport_unavailable',
  'inference_inspect_unavailable') THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF EXISTS(SELECT 1 FROM unnest(ARRAY['event','engagement','campaign','decision','permission']) f
 WHERE jsonb_typeof(s->f) IS DISTINCT FROM 'string'
 OR (s->>f) IS NULL OR (s->>f) !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(s->'call_ids') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string'
 OR v#>>'{}' !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(s->'bindings') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF s->>'disposition' IN ('validated_proposal','recorded_stale_premises') THEN
  IF jsonb_typeof(s->'proposal') IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 ELSE
  IF jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 END IF;
 e:=(s->>'engagement')::uuid;c:=(s->>'campaign')::uuid;d:=(s->>'decision')::uuid;
 IF NOT EXISTS(SELECT 1 FROM mission.registration_outbox m JOIN trajectory.registration_history h
  ON (h.engagement_id,h.campaign_id,h.operation_id,h.event_id)=(m.engagement_id,m.campaign_id,m.operation_id,m.event_id)
  WHERE (m.engagement_id,m.campaign_id)=(e,c) AND h.status='accepted'
  AND (m.contract->>'owner_revision')::bigint=(s->>'mission_revision')::bigint
  AND m.contract->'fields'->'m1_inference_permission'->>'permission_id'=s->>'permission')
  OR EXISTS(SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id)=(e,c) AND h.status='anomaly')
 THEN RAISE EXCEPTION 'inference_predecessor_missing'; END IF;
 IF jsonb_typeof(s->'call_ids') IS DISTINCT FROM 'array' OR jsonb_typeof(s->'bindings') IS DISTINCT FROM 'array'
  OR (CASE WHEN s->>'disposition' IN ('validated_proposal','recorded_stale_premises') THEN jsonb_typeof(s->'proposal') IS DISTINCT FROM 'object' OR jsonb_array_length(s->'bindings') NOT BETWEEN 1 AND 2
   WHEN s->>'disposition'='schema_repair_exhausted' THEN jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' OR jsonb_array_length(s->'bindings')<>2
   ELSE jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' END)
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF jsonb_array_length(s->'call_ids')=0 THEN
  IF jsonb_array_length(s->'bindings')<>0 OR EXISTS(SELECT 1 FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d))
  THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 ELSE
  SELECT * INTO r FROM execution.inference_reservations y WHERE (y.engagement_id,y.campaign_id,y.operation_id)=(e,c,d) AND y.record_revision=1;
  IF r.operation_id IS NULL OR s->'call_ids'<>jsonb_build_array(r.generation_call_id,r.repair_call_id)
   OR (SELECT coalesce(jsonb_agg(a.binding ORDER BY CASE WHEN a.call=r.generation_call_id THEN 1 WHEN a.call=r.repair_call_id THEN 2 ELSE 3 END),'[]'::jsonb)
    FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d))<>s->'bindings'
   OR EXISTS(SELECT 1 FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d) AND a.call NOT IN (r.generation_call_id,r.repair_call_id))
  THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 END IF;
 IF EXISTS(SELECT 1 FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d)
 AND encode(sha256(convert_to('dw-inference-binding-v1','UTF8')||decode('00','hex')||
 int8send(octet_length(a.p)::bigint)||convert_to(a.p,'UTF8')||
 int8send(octet_length(a.configuration)::bigint)||convert_to(a.configuration,'UTF8')||
 int8send(octet_length(a.body)::bigint)||convert_to(a.body,'UTF8')),'hex')<>a.binding)
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 RETURN true;
END $$;

CREATE OR REPLACE FUNCTION execution.reserve_fixture_dns(r uuid,seed integer) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;s jsonb;f jsonb;p jsonb;chosen jsonb;proposal jsonb;g bigint;n bigint;pack jsonb;asset uuid;selected integer;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 s:=execution.run_current(r);f:=s->'fields';p:=f->'m1_permission';
 IF NOT EXISTS(SELECT 1 FROM execution.autonomous_control WHERE run_id=r AND phase='acquisition' AND claimed)
 OR NOT EXISTS(SELECT 1 FROM execution.inference_closures WHERE (e,c,d)=(x.e,x.c,x.decision_one))
 OR EXISTS(SELECT 1 FROM execution.fixture_dns_attempts WHERE run_id=r)
 OR EXISTS(SELECT 1 FROM execution.inference_reservations h WHERE (h.engagement_id,h.campaign_id)=(x.e,x.c) AND record_revision=1
   AND NOT EXISTS(SELECT 1 FROM execution.inference_closures z WHERE (z.e,z.c,z.d)=(x.e,x.c,h.operation_id)))
 THEN RAISE EXCEPTION 'fixture_release_refused';END IF;
 IF NOT EXISTS(SELECT 1 FROM cognition.reasoning_decisions WHERE (e,c,d)=(x.e,x.c,x.decision_one)
 AND payload->>'disposition'='validated_proposal') THEN RAISE EXCEPTION 'fixture_stale_or_unvalidated'; END IF;
 SELECT payload->'proposal' INTO proposal FROM cognition.reasoning_decisions WHERE (e,c,d)=(x.e,x.c,x.decision_one);
 SELECT call.p::jsonb INTO pack FROM execution.inference_calls call JOIN execution.inference_reservations h
 ON h.generation_call_id=call.call WHERE (call.e,call.c,call.d)=(x.e,x.c,x.decision_one) AND call.state='completed';
 PERFORM execution.assert_current_pack_terrain(x.e,x.c,x.decision_one,pack);
 SELECT (reference->>'entity')::uuid INTO asset FROM jsonb_array_elements(pack->'references') reference
 WHERE reference->>'id'='asset_'||seed::text AND reference->>'owner'='operator' AND reference->>'source'='admitted_operator_seed';
 SELECT v,i::integer-1 INTO chosen,selected FROM jsonb_array_elements(x.request->'seed_names') WITH ORDINALITY names(v,i)
 WHERE v->>'asset_ref'=asset::text;
 IF seed<0 OR chosen IS NULL OR proposal#>>'{next_intent,category}' IS DISTINCT FROM 'observe_more'
 OR proposal#>>'{next_intent,manifest_reference}' IS DISTINCT FROM 'dns_udp'
 OR proposal#>'{next_intent,references}' IS DISTINCT FROM jsonb_build_array('asset_'||seed::text)
 OR coalesce(proposal#>'{next_intent,view_request}','null'::jsonb)<>'null'::jsonb
 OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'contact_rules') rule WHERE rule->'asset_ref'=chosen->'asset_ref'
 AND (rule->'rule'->>'exact'=chosen->>'name' OR rule->'rule'->>'label_suffix'=chosen->>'name'
 OR right(chosen->>'name',length(rule->'rule'->>'label_suffix')+1)='.'||(rule->'rule'->>'label_suffix')))
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(p->'excluded_names') rule WHERE rule->>'exact'=chosen->>'name'
 OR rule->>'label_suffix'=chosen->>'name' OR right(chosen->>'name',length(rule->>'label_suffix')+1)='.'||(rule->>'label_suffix'))
 OR (p#>>'{campaign_limits,episodes}')::bigint<1 OR (p#>>'{campaign_limits,dns_questions}')::bigint<1
 THEN RAISE EXCEPTION 'fixture_scope_refused';END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF NOT ((p->>'starts_at')::bigint*1000<=n AND n<(p->>'ends_at')::bigint*1000) THEN RAISE EXCEPTION 'fixture_authority_expired';END IF;
 -- This is a separately bound fixture session, never reinterpretation of prepared_no_effects.
 UPDATE execution.session_fences SET generation=generation+1,epoch=epoch+1,operation_id=x.attempt,writer_oid=x.writer_oid,phase='fixture_effects'
 WHERE engagement_id=x.e AND campaign_id=x.c AND operation_id IS NULL RETURNING generation INTO g;
 IF g IS NULL THEN RAISE EXCEPTION 'fixture_session_busy';END IF;
 INSERT INTO execution.fixture_dns_attempts VALUES(r,x.attempt,g,x.decision_one,selected,'dns-routing-fixture-v1','controlled_fixture',x.release_sha256,n,1,1);
 RETURN jsonb_build_object('version','m1_terrain_dns_issued_observation_v1','engagement_id',x.e,'campaign_id',x.c,
 'operator_ref',f->'operator_ref','expected_mission_revision',1,'goal_ref',f->'goal_ref','exercise_mode',f->'exercise_mode',
 'correlation_id',x.result,'source_kind','fixture_dns_acquisition','vantage_ref',p->'vantage_ref','resolver_ipv4',p->'resolver_ipv4',
 'subject_name',chosen->'name','asset_ref',chosen->'asset_ref','expected_claim_revision',0,'perceived_at',n/1000,'uncertainty_seconds',0,
 'transport','udp','qtype','A','qclass','IN','rcode','noerror','truncated',false,'completeness','complete',
 'limitations',jsonb_build_array('single_vantage_only'),'answers','[]'::jsonb);
END $$;

CREATE OR REPLACE FUNCTION execution.run_terminal(r uuid,reason text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;p jsonb;expected text;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 PERFORM execution.run_current(r);
 SELECT payload->'proposal' INTO p FROM cognition.reasoning_decisions WHERE (e,c,d)=(x.e,x.c,x.decision_one);
 expected:=CASE WHEN EXISTS(SELECT 1 FROM cognition.reasoning_decisions WHERE (e,c,d)=(x.e,x.c,x.decision_one)
 AND payload->>'disposition'='recorded_stale_premises') THEN 'deferred' WHEN p#>'{next_intent,view_request}' IS NOT NULL AND p#>'{next_intent,view_request}'<>'null' THEN 'owner_view_requested'
 WHEN p#>>'{next_intent,category}'='retain' THEN 'retained' WHEN p#>>'{next_intent,category}'='defer' THEN 'deferred'
 WHEN p#>>'{next_intent,category}'='stop' THEN 'stopped' ELSE 'capability_gap' END;
 IF reason IS DISTINCT FROM expected OR NOT EXISTS(SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase='decision_one' AND status='completed')
 THEN RAISE EXCEPTION 'run_terminal_refused'; END IF;
 UPDATE execution.autonomous_control SET status=reason,lease_until_ms=0 WHERE run_id=r AND status='running';
END $$;

-- Successor guard reports remain observations separate from the parent's cause.
CREATE OR REPLACE FUNCTION execution.run_phase_diagnostic(q jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE r uuid; p text; old jsonb;
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
 IF q->'elapsed_ms'<>'null'::jsonb THEN
  IF jsonb_typeof(q->'elapsed_ms') IS DISTINCT FROM 'number' OR q->>'elapsed_ms' !~ '^(0|[1-9][0-9]*)$'
  THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
  IF (q->>'elapsed_ms')::numeric NOT BETWEEN 0 AND 150000
  THEN RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
 END IF;
 r:=(q->>'run_id')::uuid; p:=q->>'phase';
 IF execution.run_record(r) IS NULL OR (q->>'version'='m1_run_phase_diagnostic_v2' AND NOT EXISTS(SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id=r AND x.request->>'version'='m1_autonomous_run_request_v2')) OR NOT EXISTS(
  SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase=p AND status='started')
 THEN RAISE EXCEPTION 'run_diagnostic_writer_refused'; END IF;
 INSERT INTO execution.run_phase_diagnostics VALUES(r,p,q) ON CONFLICT DO NOTHING;
 SELECT payload INTO old FROM execution.run_phase_diagnostics WHERE run_id=r AND phase=p;
 IF old IS DISTINCT FROM q THEN RAISE EXCEPTION 'run_diagnostic_conflict'; END IF;
END $$;
