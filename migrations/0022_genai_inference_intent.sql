-- SDK semantic intent admission; legacy P/C/B meanings and ACLs remain intact.

CREATE OR REPLACE FUNCTION execution.genai_canonical(v jsonb) RETURNS text
LANGUAGE plpgsql IMMUTABLE STRICT SET search_path=pg_catalog AS $$
DECLARE result text;
BEGIN
 CASE jsonb_typeof(v)
 WHEN 'object' THEN
  SELECT '{'||coalesce(string_agg(to_jsonb(key)::text||':'||execution.genai_canonical(value),',' ORDER BY key COLLATE "C"),'')||'}'
  INTO result FROM jsonb_each(v);
 WHEN 'array' THEN
  SELECT '['||coalesce(string_agg(execution.genai_canonical(value),',' ORDER BY n),'')||']'
  INTO result FROM jsonb_array_elements(v) WITH ORDINALITY a(value,n);
 ELSE result:=v::text;
 END CASE;
 RETURN result;
END $$;

CREATE OR REPLACE FUNCTION execution.genai_binding_digest(p text,i text) RETURNS text
LANGUAGE sql IMMUTABLE STRICT SET search_path=pg_catalog AS $$
 SELECT encode(sha256(convert_to('dw-inference-intent-v1','UTF8')||decode('00','hex')||
 int8send(octet_length(p)::bigint)||convert_to(p,'UTF8')||
 int8send(octet_length(i)::bigint)||convert_to(i,'UTF8')),'hex')
$$;

CREATE OR REPLACE FUNCTION execution.genai_proposal_schema(p jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE ids jsonb; refs jsonb; manifests jsonb; properties jsonb; variants jsonb:='[]';
 fields jsonb; view_schema jsonb; required jsonb; category text; name text;
BEGIN
 SELECT coalesce(jsonb_agg(v->'id' ORDER BY n),'[]') INTO ids
 FROM jsonb_array_elements(p->'references') WITH ORDINALITY a(v,n);
 refs:=jsonb_build_object('type','array','items',CASE WHEN jsonb_array_length(ids)=0
 THEN '{"type":"string"}'::jsonb ELSE jsonb_build_object('type','string','enum',ids) END,
 'maxItems',CASE WHEN jsonb_array_length(ids)=0 THEN 0 ELSE 64 END);
 SELECT coalesce(jsonb_agg(v->'reference' ORDER BY n),'[]') INTO manifests
 FROM jsonb_array_elements(p->'manifests') WITH ORDINALITY a(v,n);
 view_schema:=jsonb_build_object('type','object','additionalProperties',false,'required',jsonb_build_array('owner','selectors'),
 'properties',jsonb_build_object('owner','{"type":"string","enum":["cyber_terrain","foothold_graph","attack_path_view","objective_state","campaign_trajectory"]}'::jsonb,'selectors',refs));
 FOREACH category IN ARRAY ARRAY['validate','observe_more','synthesize_candidate_implementation','retain','defer','stop'] LOOP
  fields:=jsonb_build_object('category',jsonb_build_object('type','string','enum',jsonb_build_array(category)),
  'references',refs,'manifest_reference','{"type":"null"}'::jsonb);
  required:='["category","manifest_reference","references"]'::jsonb;
  IF category='observe_more' THEN
   fields:=fields||jsonb_build_object('view_request',jsonb_build_object('anyOf',jsonb_build_array('{"type":"null"}'::jsonb,view_schema)));
   required:=required||'"view_request"'::jsonb;
   IF jsonb_array_length(manifests)>0 THEN
    fields:=jsonb_set(fields,'{manifest_reference}',jsonb_build_object('anyOf',jsonb_build_array('{"type":"null"}'::jsonb,jsonb_build_object('type','string','enum',manifests))));
   END IF;
  END IF;
  variants:=variants||jsonb_build_array(jsonb_build_object('type','object','additionalProperties',false,'required',required,'properties',fields));
 END LOOP;
 properties:=jsonb_build_object('version','{"type":"string","enum":["m1_inference_proposal_v1"]}'::jsonb,
 'references',refs,'next_intent',jsonb_build_object('anyOf',variants));
 FOREACH name IN ARRAY ARRAY['hypothesis','considered_alternative','expected_observation','falsifying_observation','uncertainty','rationale'] LOOP
  properties:=properties||jsonb_build_object(name,'{"type":"string","minLength":1,"maxLength":1024}'::jsonb);
 END LOOP;
 SELECT jsonb_agg(key ORDER BY key COLLATE "C") INTO required FROM jsonb_object_keys(properties) a(key);
 RETURN jsonb_build_object('type','object','additionalProperties',false,'required',required,'properties',properties);
END $$;

CREATE OR REPLACE FUNCTION execution.genai_expected_intent(p jsonb,r execution.inference_reservations,k uuid,codes jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE i jsonb; recipient text;
BEGIN
 recipient:=CASE p->>'inference_profile'
 WHEN 'fixture-genai-responses-v1' THEN 'http://127.0.0.1:18083/responses'
 WHEN 'deepseek-genai-responses-low-v1' THEN 'https://api.deepseek.com/responses'
 ELSE NULL END;
 IF recipient IS NULL OR k NOT IN(r.generation_call_id,r.repair_call_id) OR r.generation<1
 OR r.generation_call_id=r.repair_call_id OR r.generation_call_id='00000000-0000-0000-0000-000000000000'::uuid
 OR r.repair_call_id='00000000-0000-0000-0000-000000000000'::uuid
 OR p->>'version' IS DISTINCT FROM 'm1_context_pack_autonomous_v3'
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
 i:=jsonb_build_object('version','dw-inference-intent-v1','permission_version','m1_inference_permission_v5',
 'run_version','m1_autonomous_run_request_v3','engagement_id',p->'engagement_id','campaign_id',p->'campaign_id',
 'decision_id',p->'decision_id','permission_id',p->'permission_id','mission_revision',p->'mission_revision',
 'generation',r.generation,'generation_call_id',r.generation_call_id,'repair_call_id',r.repair_call_id,'call_id',k,
 'call_role',CASE WHEN k=r.generation_call_id THEN 'generation' ELSE 'repair' END,
 'run_profile','autonomous-dns-fixture-v2','execution_mode','controlled_fixture','protocol','openai-resp-v1',
 'profile',p->'inference_profile','recipient',recipient,'model','deepseek-flash',
 'adapter_release','genai-a9504f1-dw1','template_version','m1-autonomous-inference-v3',
 'schema_version','m1_inference_proposal_v1','timing_profile','reasoning-bounded-v1',
 'instructions','DuskWeave autonomous inference v1: propose one objective-led hypothesis, alternative, expected and falsifying observation and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not playbooks or permission. Operator seeds are premises, observations are not facts, and historical decisions do not grant current authority. Use exact references[].id and qualified manifests[].reference values only. observe_more with a DNS manifest proposes one observation for one seed; observe_more with view_request requests only an owner view. Do not combine them. Do not invent endpoints, references, availability, access or scope. Missing, stale, conflicted and fixture sources retain their limits. Explain uncertainty and any retention/revision from predecessor feedback. Retain, defer and stop are valid choices. Synthesis/unsupported effects are explanations of a capability gap, never permission to execute. Return concise safe explanation, never private reasoning or raw client content. Output grants no dispatch. Fixture results cannot establish live target facts.','schema',execution.genai_proposal_schema(p),'repair_error_codes',codes,
 'settings','{"reasoning_effort":"low","max_output_tokens":4096,"stream":false,"store":false,"tools":[]}'::jsonb,
 'limits','{"request_bytes":16384,"response_bytes":65536,"context_bytes":8192,"configuration_bytes":16384,"connect_time_ms":5000,"soft_completion_ms":60000,"per_call_wall_time_ms":62000,"shared_wall_time_ms":124000,"decision_wall_time_ms":129000,"phase_wall_time_ms":134000,"cancel_grace_ms":2000,"accounting_window_ms":3000,"output_tokens_per_call":4096,"calls_per_decision":2,"concurrency":1}'::jsonb);
 RETURN i;
END $$;

CREATE OR REPLACE FUNCTION execution.genai_binding_valid(e uuid,c uuid,d uuid,k uuid,p text,i text,b text,h text) RETURNS boolean
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE r execution.inference_reservations; request jsonb; permission jsonb; expected jsonb;
BEGIN
 SELECT * INTO r FROM execution.inference_reservations a WHERE (a.engagement_id,a.campaign_id,a.operation_id)=(e,c,d) AND a.record_revision=1;
 SELECT x.request,m.contract#>'{fields,m1_inference_permission}' INTO request,permission
 FROM execution.autonomous_runs x JOIN mission.registration_outbox m ON (m.engagement_id,m.campaign_id)=(x.e,x.c)
 WHERE (x.e,x.c)=(e,c) AND d IN(x.decision_one,x.decision_two);
 IF r.operation_id IS NULL OR request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v3'
 OR permission->>'version' IS DISTINCT FROM 'm1_inference_permission_v5'
 OR NOT execution.inference_profile_matches(permission)
 OR request->>'inference_profile' IS DISTINCT FROM permission->>'profile'
 OR p::jsonb->>'inference_profile' IS DISTINCT FROM permission->>'profile'
 OR request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v2'
 OR request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
 OR p::jsonb->>'run_profile' IS DISTINCT FROM request->>'run_profile'
 OR p::jsonb->>'execution_mode' IS DISTINCT FROM request->>'execution_mode'
 OR p::jsonb->'permission_id' IS DISTINCT FROM permission->'permission_id'
 OR octet_length(p)>8192 OR octet_length(i)>16384 OR b<>'' THEN RETURN false; END IF;
 expected:=execution.genai_expected_intent(p::jsonb,r,k,i::jsonb->'repair_error_codes');
 RETURN coalesce(i=execution.genai_canonical(expected) AND h=execution.genai_binding_digest(p,i),false);
EXCEPTION WHEN OTHERS THEN RETURN false;
END $$;

CREATE OR REPLACE FUNCTION execution.inference_profile_matches(p jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path=pg_catalog AS $$
 SELECT coalesce(CASE WHEN p->>'version'='m1_inference_permission_v5' THEN
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
    IF consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN
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
    IF (SELECT count(*) FROM jsonb_object_keys(consent))<>(CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN 21 ELSE 19 END)
       OR jsonb_typeof(consent->'provider_terms_ref') IS DISTINCT FROM 'string'
       OR (consent->>'provider_terms_ref')::uuid IS NULL
       OR jsonb_typeof(consent->'field_classes') IS DISTINCT FROM 'array'
       OR jsonb_array_length(consent->'field_classes') NOT BETWEEN 1 AND (CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN 9 ELSE 8 END)
       OR (SELECT count(DISTINCT v) FROM jsonb_array_elements(consent->'field_classes') v)
          <>jsonb_array_length(consent->'field_classes')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(consent->'field_classes') v
           WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(consent->'field_classes') v
           WHERE v NOT IN ('mission_question_constraints','scoped_opaque_references',
           'owner_qualified_summaries','admitted_operator_premises','manifest_availability',
           'omissions_unknown','capacity','admitted_repair_fields_codes') AND NOT (consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') AND v='admitted_campaign_decision_feedback'))
       OR (SELECT count(*) FROM jsonb_object_keys(limits))<>(CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN CASE WHEN consent->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') THEN 22 ELSE 21 END ELSE 16 END)
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
        ('decision_wall_time_ms',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN CASE WHEN consent->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') THEN 129000 ELSE 45000 END ELSE 20000 END),('context_bytes',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN 8192 ELSE 4096 END),('candidates',1,8),
        ('configuration_bytes',1,16384),('connect_time_ms',1,5000),('proposal_bytes',1,8192),
        ('repair_bytes',1,2048),('text_field_bytes',1,1024),('repair_error_codes',1,8),
        ('json_depth',1,CASE WHEN consent->>'version' IN ('m1_inference_permission_v2','m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN 32 ELSE 8 END),('collection_entries',1,64),('calls_per_decision',2,2),
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
 IF p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN
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
 IF p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') THEN
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
 RETURN least(CASE WHEN p->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN CASE WHEN p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') THEN 124000 ELSE 40000 END ELSE 20000 END,(l->>'decision_wall_time_ms')::bigint,(s->'fields'->>'ends_at')::bigint*1000-n,(p->>'ends_at')::bigint*1000-n);
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
 OR NOT coalesce((p->>'version',x.request->>'version') IN (('m1_inference_permission_v3','m1_autonomous_run_request_v1'),('m1_inference_permission_v4','m1_autonomous_run_request_v2'),('m1_inference_permission_v5','m1_autonomous_run_request_v3')),false)
 OR x.request->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2')
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (x.request->>'run_profile',x.request->>'execution_mode')
 OR p->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
 OR (p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') AND (p->>'profile' IS DISTINCT FROM x.request->>'inference_profile' OR x.request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v2'))
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
 IF p->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') AND EXISTS(
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
 OR NOT coalesce((q->>'version',q->>'run_profile') IN (('m1_autonomous_run_request_v1','autonomous-dns-fixture-v1'),('m1_autonomous_run_request_v1','autonomous-dns-provider-fixture-v1'),('m1_autonomous_run_request_v2','autonomous-dns-fixture-v2'),('m1_autonomous_run_request_v3','autonomous-dns-fixture-v2')),false)
 OR (q->>'version'='m1_autonomous_run_request_v1' AND q ? 'inference_profile')
 OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') AND jsonb_typeof(q->'inference_profile') IS DISTINCT FROM 'string')
 OR q->>'run_profile' IS NULL
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2')
 OR q->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
 OR jsonb_typeof(q->'seed_names') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'seed_names') NOT BETWEEN 1 AND 8
 OR jsonb_array_length(q#>'{context,candidates}')<>jsonb_array_length(q->'seed_names')
 THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 IF q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') AND (
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
 OR deadline<=n OR deadline>n+(CASE WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') THEN 400000 ELSE 150000 END) THEN RAISE EXCEPTION 'run_deadline_refused'; END IF;
 s:=mission.current_inference_publication(e,c,r); p:=s->'fields'->'m1_inference_permission';
 IF s IS NULL OR NOT execution.inference_profile_matches(p)
 OR (p->>'version',p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (CASE WHEN q->>'version'='m1_autonomous_run_request_v3' THEN 'm1_inference_permission_v5' WHEN q->>'version'='m1_autonomous_run_request_v2' THEN 'm1_inference_permission_v4' ELSE 'm1_inference_permission_v3' END,q->>'run_profile',q->>'execution_mode')
 OR (q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') AND q->>'inference_profile' IS DISTINCT FROM p->>'profile')
 OR q#>'{context,mission}' IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1',
 'engagement_id',e,'campaign_id',c,'operator_ref',s->'fields'->'operator_ref','expected_mission_revision',1,
 'permission_id',p->'permission_id','goal_ref',s->'fields'->'goal_ref','exercise_mode',s->'fields'->'exercise_mode')
 OR NOT p->'field_classes' ?& ARRAY['mission_question_constraints','scoped_opaque_references','owner_qualified_summaries',
 'admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes','admitted_campaign_decision_feedback']
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(q->'seed_names') WITH ORDINALITY a(v,i)
 WHERE v-ARRAY['asset_ref','name']<>'{}' OR v->'asset_ref' IS DISTINCT FROM q#>'{context,candidates}'->(i::int-1)->'asset_ref'
 OR NOT s->'fields'->'included_assets' @> jsonb_build_array(v->'asset_ref')
 OR s->'fields'->'excluded_assets' @> jsonb_build_array(v->'asset_ref'))
 OR EXISTS(SELECT 1 FROM (VALUES ('calls',4),('request_bytes',65536),('response_bytes',262144),('output_token_capacity',16384),('wall_time_ms',CASE WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') THEN 258000 ELSE 90000 END)) b(k,v)
 WHERE NOT coalesce((p->'campaign_limits'->>b.k)::numeric>=(execution.inference_capacity_snapshot(e,c)->'charged'->>b.k)::numeric+b.v,false))
 THEN RAISE EXCEPTION 'run_authority_unavailable'; END IF;
 deadline:=least(deadline,(s->'fields'->>'ends_at')::bigint*1000,(p->>'ends_at')::bigint*1000);
 INSERT INTO execution.autonomous_runs VALUES(r,e,c,(SELECT oid FROM pg_roles WHERE rolname=session_user),q,
 gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),release,gen_random_uuid(),deadline);
 INSERT INTO execution.autonomous_control VALUES(r,least(n+2000,deadline),'running',NULL,false);
 PERFORM execution.run_current(r);
 RETURN true;
END $$;

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
 IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') THEN
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
 IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') THEN
  PERFORM execution.assert_successor_pack(p,registration,x.request);
  IF (p->>'evaluated_at')::bigint>floor(extract(epoch FROM clock_timestamp()))::bigint
  OR (p->>'evaluated_at')::bigint<(registration->>'occurred_at')::bigint THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 END IF;
 IF p->>'version' IS DISTINCT FROM (CASE WHEN x.request->>'version'='m1_autonomous_run_request_v3' THEN 'm1_context_pack_autonomous_v3' WHEN x.request->>'version'='m1_autonomous_run_request_v2' THEN 'm1_context_pack_autonomous_v2' ELSE 'm1_context_pack_autonomous_v1' END)
 OR (x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') AND (p->>'inference_profile' IS DISTINCT FROM permission->>'profile' OR cfg->>'timing_profile' IS DISTINCT FROM 'reasoning-bounded-v1'))
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
  IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') AND prior->>'hypothesis_summary_omission'='byte_bound'
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
 IF perm->>'version' IN ('m1_inference_permission_v4','m1_inference_permission_v5') AND remaining<62000 THEN RAISE EXCEPTION 'inference_call_window_refused'; END IF;
 IF NOT coalesce(perm->'field_classes' ? 'admitted_repair_fields_codes',false) THEN RAISE EXCEPTION 'inference_consent_refused'; END IF;
 IF perm->>'version' IN ('m1_inference_permission_v3','m1_inference_permission_v4','m1_inference_permission_v5') THEN PERFORM execution.assert_autonomous_pack(e,c,d,p::jsonb,cfg::jsonb);END IF;
 IF perm->>'version'='m1_inference_permission_v5' THEN
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
 OR (CASE WHEN perm->>'version'='m1_inference_permission_v5' THEN false ELSE (CASE WHEN cfg::jsonb->>'protocol'='chat-completions-v1' THEN b::jsonb#>>'{messages,1,content}' ELSE b::jsonb->>'input' END) IS DISTINCT FROM p AND k=r.generation_call_id END) THEN
 RAISE EXCEPTION 'inference_authority_refused'; END IF;
 IF perm->>'version'='m1_inference_permission_v5' THEN actual:=execution.genai_binding_digest(p,cfg);
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
 IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3') THEN
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

CREATE OR REPLACE FUNCTION cognition.successor_reasoning_identity(x jsonb) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT coalesce(EXISTS(
 SELECT 1 FROM execution.autonomous_runs r JOIN mission.registration_outbox m ON (m.engagement_id,m.campaign_id)=(r.e,r.c)
 JOIN execution.native_input_bindings i ON (i.e,i.c,i.d)=((x->>'engagement')::uuid,(x->>'campaign')::uuid,(x->>'decision')::uuid)
 WHERE (r.e,r.c)=((x->>'engagement')::uuid,(x->>'campaign')::uuid)
 AND (x->>'decision')::uuid IN(r.decision_one,r.decision_two) AND i.writer_oid=r.writer_oid
 AND r.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3')
 AND r.request->>'run_profile'='autonomous-dns-fixture-v2'
 AND (m.contract#>>'{fields,m1_inference_permission,version}',r.request->>'version') IN
 (('m1_inference_permission_v4','m1_autonomous_run_request_v2'),('m1_inference_permission_v5','m1_autonomous_run_request_v3'))
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
  AND ((a.p::jsonb->>'version') IS DISTINCT FROM (CASE WHEN r.request->>'version'='m1_autonomous_run_request_v3' THEN 'm1_context_pack_autonomous_v3' ELSE 'm1_context_pack_autonomous_v2' END)
   OR (a.p::jsonb->>'inference_profile') IS DISTINCT FROM r.request->>'inference_profile'))
 ),false)
$$;

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
 AND CASE WHEN EXISTS(SELECT 1 FROM execution.autonomous_runs z WHERE (z.e,z.c)=(e,c) AND d IN(z.decision_one,z.decision_two) AND z.request->>'version'='m1_autonomous_run_request_v3')
 THEN NOT execution.genai_binding_valid(e,c,d,a.call,a.p,a.configuration,a.body,a.binding)
 ELSE encode(sha256(convert_to('dw-inference-binding-v1','UTF8')||decode('00','hex')||
 int8send(octet_length(a.p)::bigint)||convert_to(a.p,'UTF8')||
 int8send(octet_length(a.configuration)::bigint)||convert_to(a.configuration,'UTF8')||
 int8send(octet_length(a.body)::bigint)||convert_to(a.body,'UTF8')),'hex')<>a.binding END)
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 RETURN true;
END $$;

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
 IF execution.run_record(r) IS NULL OR (q->>'version'='m1_run_phase_diagnostic_v2' AND NOT EXISTS(SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id=r AND x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3'))) OR NOT EXISTS(
  SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase=p AND status='started')
 THEN RAISE EXCEPTION 'run_diagnostic_writer_refused'; END IF;
 INSERT INTO execution.run_phase_diagnostics VALUES(r,p,q) ON CONFLICT DO NOTHING;
 SELECT payload INTO old FROM execution.run_phase_diagnostics WHERE run_id=r AND phase=p;
 IF old IS DISTINCT FROM q THEN RAISE EXCEPTION 'run_diagnostic_conflict'; END IF;
END $$;

REVOKE ALL ON FUNCTION execution.genai_canonical(jsonb) FROM PUBLIC;
REVOKE ALL ON FUNCTION execution.genai_binding_digest(text,text) FROM PUBLIC;
REVOKE ALL ON FUNCTION execution.genai_proposal_schema(jsonb) FROM PUBLIC;
REVOKE ALL ON FUNCTION execution.genai_expected_intent(jsonb,execution.inference_reservations,uuid,jsonb) FROM PUBLIC;
REVOKE ALL ON FUNCTION execution.genai_binding_valid(uuid,uuid,uuid,uuid,text,text,text,text) FROM PUBLIC;
