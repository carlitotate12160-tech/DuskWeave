-- Closed Pro extension; CREATE OR REPLACE preserves installed ownership and ACLs.
BEGIN;
-- Live inference permission v6: GenAI profile binding plus deployment live_binding.
CREATE OR REPLACE FUNCTION execution.inference_profile_matches(p jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path=pg_catalog,pg_temp AS $$
 SELECT coalesce(CASE WHEN p->>'version'='m1_inference_permission_v6' THEN
 (p->>'profile',p->>'recipient') IN (
 ('deepseek-genai-responses-low-v1','https://api.deepseek.com/responses'),
 ('deepseek-genai-v4-pro-low-v1','https://api.deepseek.com/responses'),
 ('fixture-genai-responses-v1','http://127.0.0.1:18083/responses'))
 AND jsonb_typeof(p#>'{operation_limits,request_bytes}')='number'
 AND p#>>'{operation_limits,request_bytes}'='16384'
 AND (p->>'run_profile',p->>'execution_mode') IN (('autonomous-dns-genai-live-v1','live'),('autonomous-dns-genai-lab-v1','live'))
 AND (p->>'profile' IN ('deepseek-genai-responses-low-v1','deepseek-genai-v4-pro-low-v1'))=(p->>'run_profile'='autonomous-dns-genai-live-v1')
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
 ('deepseek-genai-v4-pro-low-v1','https://api.deepseek.com/responses'),
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

-- Live intent admission: v4 packs carry live_binding and qualification_purpose.
CREATE OR REPLACE FUNCTION execution.genai_expected_intent(p jsonb,r execution.inference_reservations,k uuid,codes jsonb) RETURNS jsonb
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog,pg_temp AS $$
DECLARE i jsonb; recipient text;
BEGIN
 recipient:=CASE p->>'inference_profile'
 WHEN 'fixture-genai-responses-v1' THEN 'http://127.0.0.1:18083/responses'
 WHEN 'deepseek-genai-responses-low-v1' THEN 'https://api.deepseek.com/responses'
 WHEN 'deepseek-genai-v4-pro-low-v1' THEN 'https://api.deepseek.com/responses'
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
 i:=jsonb_build_object('version','dw-inference-intent-v1','permission_version',CASE WHEN p->>'version'='m1_context_pack_autonomous_v4' THEN 'm1_inference_permission_v6' ELSE 'm1_inference_permission_v5' END,
 'run_version',CASE WHEN p->>'version'='m1_context_pack_autonomous_v4' THEN 'm1_autonomous_run_request_v4' ELSE 'm1_autonomous_run_request_v3' END,'engagement_id',p->'engagement_id','campaign_id',p->'campaign_id',
 'decision_id',p->'decision_id','permission_id',p->'permission_id','mission_revision',p->'mission_revision',
 'generation',r.generation,'generation_call_id',r.generation_call_id,'repair_call_id',r.repair_call_id,'call_id',k,
 'call_role',CASE WHEN k=r.generation_call_id THEN 'generation' ELSE 'repair' END,
 'run_profile',p->'run_profile','execution_mode',p->'execution_mode','protocol','openai-resp-v1',
 'profile',p->'inference_profile','recipient',recipient,'model',CASE WHEN p->>'inference_profile'='deepseek-genai-v4-pro-low-v1' THEN 'deepseek-v4-pro' ELSE 'deepseek-flash' END,
 'adapter_release','genai-a9504f1-dw1','template_version','m1-autonomous-inference-v3',
 'schema_version','m1_inference_proposal_v1','timing_profile','reasoning-bounded-v1',
 'instructions','DuskWeave autonomous inference v1: propose one objective-led hypothesis, alternative, expected and falsifying observation and next intent over this immutable pack. APT41, Lazarus and Volt Typhoon are behavioral lenses, not playbooks or permission. Operator seeds are premises, observations are not facts, and historical decisions do not grant current authority. Use exact references[].id and qualified manifests[].reference values only. observe_more with a DNS manifest proposes one observation for one seed; observe_more with view_request requests only an owner view. Do not combine them. Do not invent endpoints, references, availability, access or scope. Missing, stale, conflicted and fixture sources retain their limits. Explain uncertainty and any retention/revision from predecessor feedback. Retain, defer and stop are valid choices. Synthesis/unsupported effects are explanations of a capability gap, never permission to execute. Return concise safe explanation, never private reasoning or raw client content. Output grants no dispatch. Fixture results cannot establish live target facts.','schema',execution.genai_proposal_schema(p),'repair_error_codes',codes,
 'settings','{"reasoning_effort":"low","max_output_tokens":4096,"stream":false,"store":false,"tools":[]}'::jsonb,
 'limits','{"request_bytes":16384,"response_bytes":65536,"context_bytes":8192,"configuration_bytes":16384,"connect_time_ms":5000,"soft_completion_ms":60000,"per_call_wall_time_ms":62000,"shared_wall_time_ms":124000,"decision_wall_time_ms":129000,"phase_wall_time_ms":134000,"cancel_grace_ms":2000,"accounting_window_ms":3000,"output_tokens_per_call":4096,"calls_per_decision":2,"concurrency":1}'::jsonb);
 IF p->>'version'='m1_context_pack_autonomous_v4' THEN
  i:=i||jsonb_build_object('live_binding',p->'live_binding','qualification_purpose',p->'qualification_purpose');
 END IF;
 RETURN i;
END $$;

-- Bounded proposal admission evidence; legacy absent metadata is unchanged.
CREATE OR REPLACE FUNCTION execution.inference_call_state(k uuid,wanted text,u jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
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
 -- Every usage-bearing SDK settlement binds to the immutable admitted profile,
 -- including ordinary completion without either optional diagnostic field.
 sdk:=coalesce(owner_call.configuration::jsonb->>'version'='dw-inference-intent-v1',false);
 IF sdk AND u IS NOT NULL AND u<>'null'::jsonb THEN
  expected_model:=CASE owner_call.configuration::jsonb->>'profile'
   WHEN 'deepseek-genai-v4-pro-low-v1' THEN 'deepseek-v4-pro'
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
 -- Content-free diagnostic evidence is valid only on a completed response.
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

COMMIT;
