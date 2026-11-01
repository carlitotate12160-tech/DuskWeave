-- Live intent admission: v4 packs carry live_binding and qualification_purpose.
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
 'profile',p->'inference_profile','recipient',recipient,'model','deepseek-flash',
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

CREATE OR REPLACE FUNCTION execution.genai_binding_valid(e uuid,c uuid,d uuid,k uuid,p text,i text,b text,h text) RETURNS boolean
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE r execution.inference_reservations; request jsonb; permission jsonb; expected jsonb;
BEGIN
 SELECT * INTO r FROM execution.inference_reservations a WHERE (a.engagement_id,a.campaign_id,a.operation_id)=(e,c,d) AND a.record_revision=1;
 SELECT x.request,m.contract#>'{fields,m1_inference_permission}' INTO request,permission
 FROM execution.autonomous_runs x JOIN mission.registration_outbox m ON (m.engagement_id,m.campaign_id)=(x.e,x.c)
 WHERE (x.e,x.c)=(e,c) AND d IN(x.decision_one,x.decision_two);
 IF r.operation_id IS NULL OR request->>'version' NOT IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4')
 OR permission->>'version' IS DISTINCT FROM (CASE request->>'version' WHEN 'm1_autonomous_run_request_v4' THEN 'm1_inference_permission_v6' ELSE 'm1_inference_permission_v5' END)
 OR NOT execution.inference_profile_matches(permission)
 OR request->>'inference_profile' IS DISTINCT FROM permission->>'profile'
 OR p::jsonb->>'inference_profile' IS DISTINCT FROM permission->>'profile'
 OR (request->>'version'='m1_autonomous_run_request_v3' AND (request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v2' OR request->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'))
 OR (request->>'version'='m1_autonomous_run_request_v4' AND (
   request->>'run_profile' NOT IN ('autonomous-dns-genai-live-v1','autonomous-dns-genai-lab-v1')
   OR request->>'execution_mode' IS DISTINCT FROM 'live'
   OR request->'live_binding' IS DISTINCT FROM permission->'live_binding'
   OR p::jsonb->'live_binding' IS DISTINCT FROM permission->'live_binding'
   OR p::jsonb->>'qualification_purpose' IS DISTINCT FROM CASE request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END))
 OR p::jsonb->>'run_profile' IS DISTINCT FROM request->>'run_profile'
 OR p::jsonb->>'execution_mode' IS DISTINCT FROM request->>'execution_mode'
 OR p::jsonb->'permission_id' IS DISTINCT FROM permission->'permission_id'
 OR octet_length(p)>8192 OR octet_length(i)>16384 OR b<>'' THEN RETURN false; END IF;
 expected:=execution.genai_expected_intent(p::jsonb,r,k,i::jsonb->'repair_error_codes');
 RETURN coalesce(i=execution.genai_canonical(expected) AND h=execution.genai_binding_digest(p,i),false);
EXCEPTION WHEN OTHERS THEN RETURN false;
END $$;

CREATE OR REPLACE FUNCTION cognition.successor_reasoning_identity(x jsonb) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT coalesce(EXISTS(
 SELECT 1 FROM execution.autonomous_runs r JOIN mission.registration_outbox m ON (m.engagement_id,m.campaign_id)=(r.e,r.c)
 JOIN execution.native_input_bindings i ON (i.e,i.c,i.d)=((x->>'engagement')::uuid,(x->>'campaign')::uuid,(x->>'decision')::uuid)
 WHERE (r.e,r.c)=((x->>'engagement')::uuid,(x->>'campaign')::uuid)
 AND (x->>'decision')::uuid IN(r.decision_one,r.decision_two) AND i.writer_oid=r.writer_oid
 AND r.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4')
 AND r.request->>'run_profile' IN ('autonomous-dns-fixture-v2','autonomous-dns-genai-live-v1','autonomous-dns-genai-lab-v1')
 AND (r.request->>'version'='m1_autonomous_run_request_v4')=(r.request->>'run_profile'<>'autonomous-dns-fixture-v2')
 AND r.request->'live_binding' IS NOT DISTINCT FROM m.contract#>'{fields,m1_inference_permission,live_binding}'
 AND (m.contract#>>'{fields,m1_inference_permission,version}',r.request->>'version') IN
 (('m1_inference_permission_v4','m1_autonomous_run_request_v2'),('m1_inference_permission_v5','m1_autonomous_run_request_v3'),('m1_inference_permission_v6','m1_autonomous_run_request_v4'))
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
  AND ((a.p::jsonb->>'version') IS DISTINCT FROM (CASE WHEN r.request->>'version'='m1_autonomous_run_request_v4' THEN 'm1_context_pack_autonomous_v4' WHEN r.request->>'version'='m1_autonomous_run_request_v3' THEN 'm1_context_pack_autonomous_v3' ELSE 'm1_context_pack_autonomous_v2' END)
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
 AND CASE WHEN EXISTS(SELECT 1 FROM execution.autonomous_runs z WHERE (z.e,z.c)=(e,c) AND d IN(z.decision_one,z.decision_two) AND z.request->>'version' IN ('m1_autonomous_run_request_v3','m1_autonomous_run_request_v4'))
 THEN NOT execution.genai_binding_valid(e,c,d,a.call,a.p,a.configuration,a.body,a.binding)
 ELSE encode(sha256(convert_to('dw-inference-binding-v1','UTF8')||decode('00','hex')||
 int8send(octet_length(a.p)::bigint)||convert_to(a.p,'UTF8')||
 int8send(octet_length(a.configuration)::bigint)||convert_to(a.configuration,'UTF8')||
 int8send(octet_length(a.body)::bigint)||convert_to(a.body,'UTF8')),'hex')<>a.binding END)
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 RETURN true;
END $$;
