-- Live inference permission v6: GenAI profile binding plus deployment live_binding.
CREATE OR REPLACE FUNCTION execution.inference_profile_matches(p jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path=pg_catalog AS $$
 SELECT coalesce(CASE WHEN p->>'version'='m1_inference_permission_v6' THEN
 (p->>'profile',p->>'recipient') IN (
 ('deepseek-genai-responses-low-v1','https://api.deepseek.com/responses'),
 ('fixture-genai-responses-v1','http://127.0.0.1:18083/responses'))
 AND jsonb_typeof(p#>'{operation_limits,request_bytes}')='number'
 AND p#>>'{operation_limits,request_bytes}'='16384'
 AND (p->>'run_profile',p->>'execution_mode') IN (('autonomous-dns-genai-live-v1','live'),('autonomous-dns-genai-lab-v1','live'))
 AND (p->>'profile'='deepseek-genai-responses-low-v1')=(p->>'run_profile'='autonomous-dns-genai-live-v1')
 AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback','admitted_repair_fields_codes']
 AND jsonb_typeof(p->'live_binding')='object'
 AND (p->'live_binding')-ARRAY['qualification_ref','release_sha256','resolver_ipv4']='{}'::jsonb
 AND (p#>>'{live_binding,qualification_ref}')::uuid<>'00000000-0000-0000-0000-000000000000'::uuid
 AND p#>>'{live_binding,release_sha256}' ~ '^[0-9a-f]{64}$'
 AND host((p#>>'{live_binding,resolver_ipv4}')::inet)=p#>>'{live_binding,resolver_ipv4}'
 AND family((p#>>'{live_binding,resolver_ipv4}')::inet)=4
 AND (p->>'run_profile'='autonomous-dns-genai-lab-v1')=(p#>>'{live_binding,resolver_ipv4}'='192.0.2.53')
 AND NOT EXISTS(SELECT 1 FROM (VALUES ('decision_wall_time_ms',129000),('per_call_wall_time_ms',62000),
 ('soft_completion_ms',60000),('shared_wall_time_ms',124000),('phase_wall_time_ms',134000),
 ('cancel_grace_ms',2000),('accounting_window_ms',3000)) b(k,v)
 WHERE jsonb_typeof(p->'operation_limits'->b.k) IS DISTINCT FROM 'number'
 OR p->'operation_limits'->>b.k IS DISTINCT FROM b.v::text)
 WHEN p->>'version'='m1_inference_permission_v5' THEN
 (p->>'profile',p->>'recipient') IN (
 ('deepseek-genai-responses-low-v1','https://api.deepseek.com/responses'),
 ('fixture-genai-responses-v1','http://127.0.0.1:18083/responses'))
 AND jsonb_typeof(p#>'{operation_limits,request_bytes}')='number'
 AND p#>>'{operation_limits,request_bytes}'='16384'
 AND (p->>'run_profile',p->>'execution_mode')=('autonomous-dns-fixture-v2','controlled_fixture')
 AND p->'field_classes' ?& ARRAY['admitted_campaign_decision_feedback','admitted_repair_fields_codes']
 AND NOT EXISTS(SELECT 1 FROM (VALUES ('decision_wall_time_ms',129000),('per_call_wall_time_ms',62000),
 ('soft_completion_ms',60000),('shared_wall_time_ms',124000),('phase_wall_time_ms',134000),
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
    -- Validate the raw SQL boundary too: nulls, unknown fields and numeric strings
    -- must not turn malformed consent into a reservation.
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
    -- Cardinality plus all required names below closes the numeric objects.
    -- One profile table validates each operation field's type and interval.
    IF EXISTS (SELECT 1 FROM (VALUES ('request_bytes',1,16384),('response_bytes',1,65536),
        ('decision_wall_time_ms',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN CASE WHEN consent->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 129000 ELSE 45000 END ELSE 20000 END),('context_bytes',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 8192 ELSE 4096 END),('candidates',1,8),
        ('configuration_bytes',1,16384),('connect_time_ms',1,5000),('proposal_bytes',1,8192),
        ('repair_bytes',1,2048),('text_field_bytes',1,1024),('repair_error_codes',1,8),
        ('json_depth',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 32 ELSE 8 END),('collection_entries',1,64),('calls_per_decision',2,2),
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
 IF p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN
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
 IF p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN
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
 RETURN least(CASE WHEN p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN CASE WHEN p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') THEN 124000 ELSE 40000 END ELSE 20000 END,(l->>'decision_wall_time_ms')::bigint,(s->'fields'->>'ends_at')::bigint*1000-n,(p->>'ends_at')::bigint*1000-n);
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
 IF perm->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5','m1_inference_permission_v6') AND remaining<62000 THEN RAISE EXCEPTION 'inference_call_window_refused'; END IF;
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
