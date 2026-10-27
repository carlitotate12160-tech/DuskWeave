-- 0022: live boundary DNS acquisition lane (ADR-015/ADR-016/ADR-017).
-- Adds the ('autonomous-dns-live-v1','boundary_dns_acquisition') pair beside the
-- frozen successor fixture pair. Fixture rows keep their original values; every
-- binding remains an explicit enumeration, never an open mode string.

ALTER TABLE execution.fixture_dns_attempts
 DROP CONSTRAINT fixture_dns_attempts_profile_check,
 DROP CONSTRAINT fixture_dns_attempts_mode_check,
 ADD CONSTRAINT fixture_dns_attempts_profile_check CHECK(profile IN ('dns-routing-fixture-v1','dns-routing-boundary-v1')),
 ADD CONSTRAINT fixture_dns_attempts_mode_check CHECK(mode IN ('controlled_fixture','boundary_dns_acquisition'));
ALTER TABLE execution.fixture_dns_results
 DROP CONSTRAINT fixture_dns_results_profile_check,
 DROP CONSTRAINT fixture_dns_results_mode_check,
 ADD CONSTRAINT fixture_dns_results_profile_check CHECK(profile IN ('dns-routing-fixture-v1','dns-routing-boundary-v1')),
 ADD CONSTRAINT fixture_dns_results_mode_check CHECK(mode IN ('controlled_fixture','boundary_dns_acquisition'));

CREATE OR REPLACE FUNCTION execution.inference_profile_matches(p jsonb) RETURNS boolean
LANGUAGE sql IMMUTABLE SET search_path=pg_catalog AS $$
 SELECT coalesce(CASE WHEN p->>'version'='m1_inference_permission_v4' THEN
 (p->>'profile',p->>'recipient') IN (
 ('deepseek-responses-reasoning-v1','https://api.deepseek.com/responses'),
 ('fixture-responses-v1','urn:duskweave:fixture:responses:v1'),
 ('fixture-chat-completions-v1','urn:duskweave:fixture:chat-completions:v1'))
 AND (p->>'run_profile',p->>'execution_mode') IN (
 ('autonomous-dns-fixture-v2','controlled_fixture'),
 ('autonomous-dns-live-v1','boundary_dns_acquisition'))
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
 OR x.request->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2','autonomous-dns-live-v1')
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (x.request->>'run_profile',x.request->>'execution_mode')
 OR p->>'execution_mode' IS DISTINCT FROM (CASE WHEN p->>'run_profile'='autonomous-dns-live-v1' THEN 'boundary_dns_acquisition' ELSE 'controlled_fixture' END)
 OR (p->>'version'='m1_inference_permission_v4' AND (p->>'profile' IS DISTINCT FROM x.request->>'inference_profile' OR x.request->>'run_profile' NOT IN ('autonomous-dns-fixture-v2','autonomous-dns-live-v1')))
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
 OR NOT coalesce((q->>'version',q->>'run_profile') IN (('m1_autonomous_run_request_v1','autonomous-dns-fixture-v1'),('m1_autonomous_run_request_v1','autonomous-dns-provider-fixture-v1'),('m1_autonomous_run_request_v2','autonomous-dns-fixture-v2'),('m1_autonomous_run_request_v2','autonomous-dns-live-v1')),false)
 OR (q->>'version'='m1_autonomous_run_request_v1' AND q ? 'inference_profile')
 OR (q->>'version'='m1_autonomous_run_request_v2' AND jsonb_typeof(q->'inference_profile') IS DISTINCT FROM 'string')
 OR q->>'run_profile' IS NULL
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2','autonomous-dns-live-v1')
 OR q->>'execution_mode' IS DISTINCT FROM (CASE WHEN q->>'run_profile'='autonomous-dns-live-v1' THEN 'boundary_dns_acquisition' ELSE 'controlled_fixture' END)
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
 OR deadline<=n OR deadline>n+(CASE WHEN q->>'version'='m1_autonomous_run_request_v2' THEN 400000 ELSE 150000 END) THEN RAISE EXCEPTION 'run_deadline_refused';END IF;
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
 OR p->'manifests' IS DISTINCT FROM jsonb_build_array(jsonb_build_object('availability','qualified',
 'execution_mode',x.request->>'execution_mode',
 'profile',CASE WHEN x.request->>'execution_mode'='boundary_dns_acquisition' THEN 'dns-routing-boundary-v1' ELSE 'dns-routing-fixture-v1' END,
 'qualification_ref',x.qualification_ref,'reference','dns_udp','release_sha256',x.release_sha256,
 'schema','m1_dns_ipc_v1','transport','udp')) THEN RAISE EXCEPTION 'run_pack_binding_refused';END IF;
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

CREATE OR REPLACE FUNCTION execution.run_predecessor(r uuid) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;d jsonb;t terrain.outbox;b execution.fixture_dns_results; prior jsonb;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable';END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT cd.payload INTO d FROM cognition.reasoning_decisions cd WHERE (cd.e,cd.c,cd.d)=(x.e,x.c,x.decision_one);
 SELECT * INTO b FROM execution.fixture_dns_results WHERE run_id=r;
 SELECT * INTO t FROM terrain.outbox WHERE (engagement_id,campaign_id,operation_id)=(x.e,x.c,x.terrain_operation);
 IF d IS NULL OR b.result IS NULL OR t.event_id IS NULL
 OR t.contract#>>'{observation,source_kind}' IS DISTINCT FROM
    (CASE WHEN x.request->>'execution_mode'='boundary_dns_acquisition' THEN 'boundary_dns_acquisition' ELSE 'fixture_dns_acquisition' END)
 OR t.contract#>>'{claim,claim_status}'<>'observed'
 OR NOT EXISTS(SELECT 1 FROM trajectory.terrain_history WHERE event_id=t.event_id AND status='accepted' AND contract=t.contract)
 OR NOT EXISTS(SELECT 1 FROM execution.inference_closures WHERE (e,c,execution.inference_closures.d)=(x.e,x.c,x.decision_one))
 THEN RAISE EXCEPTION 'run_feedback_pending';END IF;
 prior:=jsonb_build_object('availability','committed','decision_reference','previous_decision','selected_seed_reference','asset_'||(SELECT seed_index FROM execution.fixture_dns_attempts WHERE run_id=r)::text,
 'selected_seed_entity',(x.request->'seed_names'->(SELECT seed_index FROM execution.fixture_dns_attempts WHERE run_id=r))->'asset_ref',
 'selected_intent','observe_more','attempt_ref',x.attempt,'result_ref',x.result,'terrain_publication_ref',t.event_id,
 'execution_mode',x.request->>'execution_mode','outcome','owner_qualified_observed','hypothesis_summary_omission','not_required',
 'decision_entity',d->'decision','decision_event',d->'event','recorded_at',((d->>'recorded_at_ms')::bigint/1000));
 IF x.request->>'version'='m1_autonomous_run_request_v2' THEN
  prior:=(prior-'hypothesis_summary_omission')||jsonb_build_object('hypothesis_summary',d#>'{proposal,hypothesis}');
 END IF;
 RETURN prior;
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
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2','autonomous-dns-live-v1')
 OR q->>'execution_mode' IS DISTINCT FROM (CASE WHEN q->>'run_profile'='autonomous-dns-live-v1' THEN 'boundary_dns_acquisition' ELSE 'controlled_fixture' END)
 THEN RAISE EXCEPTION 'issued_reference_refused'; END IF;
 e:=(q->>'engagement_id')::uuid; c:=(q->>'campaign_id')::uuid; result_id:=(q->>'result_ref')::uuid;
 SELECT jsonb_build_object('request',b.request,'operation',x.terrain_operation) INTO raw
 FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a USING(run_id)
 JOIN execution.autonomous_runs x USING(run_id) WHERE b.result=result_id AND (x.e,x.c)=(e,c)
 AND (x.request->>'run_profile',x.request->>'execution_mode')=(q->>'run_profile',q->>'execution_mode')
 AND b.result=x.result AND b.attempt=a.attempt AND b.attempt=x.attempt AND a.decision=x.decision_one
 AND a.session_generation>0 AND b.release_sha256=x.release_sha256 AND b.release_sha256=a.release_sha256
 AND b.mode=a.mode AND b.profile=a.profile AND x.request->>'execution_mode'=b.mode
 AND (b.mode,b.profile) IN (('controlled_fixture','dns-routing-fixture-v1'),('boundary_dns_acquisition','dns-routing-boundary-v1'))
 AND b.publication_obligation='terrain.issued_dns.v1'
 AND (b.request->>'engagement_id',b.request->>'campaign_id',b.request->>'correlation_id')=(e::text,c::text,result_id::text)
 AND b.request->>'version'='m1_terrain_dns_issued_observation_v1'
 AND b.request->>'source_kind'=CASE WHEN b.mode='boundary_dns_acquisition' THEN 'boundary_dns_acquisition' ELSE 'fixture_dns_acquisition' END;
 IF raw IS NULL THEN RAISE EXCEPTION 'issued_result_unavailable'; END IF;
 RETURN raw;
END $$;

CREATE OR REPLACE FUNCTION execution.reserve_boundary_dns(r uuid,seed integer) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;s jsonb;f jsonb;p jsonb;chosen jsonb;proposal jsonb;g bigint;n bigint;pack jsonb;asset uuid;selected integer;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 s:=execution.run_current(r);f:=s->'fields';p:=f->'m1_permission';
 IF x.request->>'run_profile' IS DISTINCT FROM 'autonomous-dns-live-v1'
 OR x.request->>'execution_mode' IS DISTINCT FROM 'boundary_dns_acquisition'
 OR NOT EXISTS(SELECT 1 FROM execution.autonomous_control WHERE run_id=r AND phase='acquisition' AND claimed)
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
 IF seed<0 OR chosen IS NULL
 OR proposal#>>'{next_intent,category}' IS DISTINCT FROM 'observe_more'
 OR proposal#>>'{next_intent,manifest_reference}' IS DISTINCT FROM 'dns_udp'
 OR jsonb_typeof(proposal->'references') IS DISTINCT FROM 'array'
 OR jsonb_array_length(proposal->'references') NOT BETWEEN 1 AND 64
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(proposal->'references') ref_id
  WHERE jsonb_typeof(ref_id) IS DISTINCT FROM 'string'
  OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(pack->'references') pk WHERE pk->'id'=ref_id))
 OR jsonb_typeof(proposal#>'{next_intent,references}') IS DISTINCT FROM 'array'
 OR jsonb_array_length(proposal#>'{next_intent,references}') NOT BETWEEN 1 AND 64
 OR NOT (proposal#>'{next_intent,references}' @> jsonb_build_array('asset_'||seed::text))
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(proposal#>'{next_intent,references}') ref_id
  WHERE jsonb_typeof(ref_id) IS DISTINCT FROM 'string'
  OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(pack->'references') pk WHERE pk->'id'=ref_id
   AND ((pk->>'owner'='operator' AND pk->>'source'='admitted_operator_seed' AND pk->>'id'='asset_'||seed::text AND (pk->>'entity')::uuid=asset)
    OR (pk->>'owner'='mission' AND pk->>'id'='mission')
    OR (pk->>'owner'='campaign_trajectory' AND pk->>'id'='previous_decision')
    OR (pk->>'owner'='cyber_terrain' AND pk->>'id' ~ '^terrain_(0|[1-9][0-9]*)$'))))
 OR coalesce(proposal#>'{next_intent,view_request}','null'::jsonb)<>'null'::jsonb
 OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'contact_rules') rule WHERE rule->'asset_ref'=chosen->'asset_ref'
 AND (rule->'rule'->>'exact'=chosen->>'name' OR rule->'rule'->>'label_suffix'=chosen->>'name'
 OR right(chosen->>'name',length(rule->'rule'->>'label_suffix')+1)='.'||(rule->'rule'->>'label_suffix')))
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(p->'excluded_names') rule WHERE rule->>'exact'=chosen->>'name'
 OR rule->>'label_suffix'=chosen->>'name' OR right(chosen->>'name',length(rule->>'label_suffix')+1)='.'||(rule->>'label_suffix'))
 OR (p#>>'{campaign_limits,episodes}')::bigint<1 OR (p#>>'{campaign_limits,dns_questions}')::bigint<1
 THEN RAISE EXCEPTION 'fixture_scope_refused';END IF;
 IF jsonb_typeof(p->'resolver_ipv4') IS DISTINCT FROM 'string'
 OR EXISTS(SELECT 1 FROM (VALUES ('0.0.0.0/8'),('10.0.0.0/8'),('100.64.0.0/10'),('127.0.0.0/8'),
  ('169.254.0.0/16'),('172.16.0.0/12'),('192.0.0.0/24'),('192.0.2.0/24'),('192.168.0.0/16'),
  ('198.18.0.0/15'),('198.51.100.0/24'),('203.0.113.0/24'),('224.0.0.0/4'),('240.0.0.0/4')) nets(cidr)
  WHERE (p->>'resolver_ipv4')::inet <<= nets.cidr::inet)
 THEN RAISE EXCEPTION 'boundary_resolver_refused';END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF NOT ((p->>'starts_at')::bigint*1000<=n AND n<(p->>'ends_at')::bigint*1000) THEN RAISE EXCEPTION 'fixture_authority_expired';END IF;
 -- Same separately bound effect session; the fence phase name is historical.
 UPDATE execution.session_fences SET generation=generation+1,epoch=epoch+1,operation_id=x.attempt,writer_oid=x.writer_oid,phase='fixture_effects'
 WHERE engagement_id=x.e AND campaign_id=x.c AND operation_id IS NULL RETURNING generation INTO g;
 IF g IS NULL THEN RAISE EXCEPTION 'fixture_session_busy';END IF;
 INSERT INTO execution.fixture_dns_attempts VALUES(r,x.attempt,g,x.decision_one,selected,'dns-routing-boundary-v1','boundary_dns_acquisition',x.release_sha256,n,1,1);
 RETURN jsonb_build_object('version','m1_terrain_dns_issued_observation_v1','engagement_id',x.e,'campaign_id',x.c,
 'operator_ref',f->'operator_ref','expected_mission_revision',1,'goal_ref',f->'goal_ref','exercise_mode',f->'exercise_mode',
 'correlation_id',x.result,'source_kind','boundary_dns_acquisition','vantage_ref',p->'vantage_ref','resolver_ipv4',p->'resolver_ipv4',
 'subject_name',chosen->>'name','asset_ref',chosen->>'asset_ref','expected_claim_revision',0,'perceived_at',n/1000,'uncertainty_seconds',0,
 'transport','udp','qtype','A','qclass','IN','rcode','noerror','truncated',false,'completeness','complete',
 'limitations',jsonb_build_array('single_vantage_only'),'answers','[]'::jsonb);
END $$;

CREATE OR REPLACE FUNCTION execution.commit_boundary_dns(r uuid,q jsonb) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;a execution.fixture_dns_attempts;s jsonb;f jsonb;p jsonb;chosen jsonb;expected jsonb;n bigint;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 s:=execution.run_current(r);f:=s->'fields';p:=f->'m1_permission';
 SELECT * INTO a FROM execution.fixture_dns_attempts WHERE run_id=r;
 chosen:=x.request->'seed_names'->a.seed_index;n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF a.run_id IS NULL OR a.release_sha256<>x.release_sha256 OR a.mode<>'boundary_dns_acquisition'
 OR EXISTS(SELECT 1 FROM execution.fixture_dns_results WHERE run_id=r)
 OR q->>'perceived_at' !~ '^[1-9][0-9]*$' OR jsonb_typeof(q->'perceived_at') IS DISTINCT FROM 'number'
 OR (q->>'perceived_at')::bigint<a.reserved_at_ms/1000 OR (q->>'perceived_at')::bigint>n/1000
 OR n-a.reserved_at_ms>60000 THEN RAISE EXCEPTION 'fixture_result_window_refused';END IF;
 IF q->>'rcode' NOT IN ('noerror','nxdomain','servfail','refused','formerr','notimp')
 OR q->>'completeness' NOT IN ('complete','incomplete','truncated','timeout')
 OR jsonb_typeof(q->'truncated') IS DISTINCT FROM 'boolean'
 OR jsonb_typeof(q->'uncertainty_seconds') IS DISTINCT FROM 'number' OR (q->>'uncertainty_seconds') !~ '^(0|[1-9][0-9]*)$'
 OR (q->>'uncertainty_seconds')::bigint>300
 OR jsonb_typeof(q->'limitations') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'limitations')>4
 OR NOT q->'limitations' @> '["single_vantage_only"]'::jsonb
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(q->'limitations') l WHERE l#>>'{}' NOT IN
  ('cache_remaining_ttl','single_vantage_only','partial_alias_chain','resolver_recursion_variance'))
 OR jsonb_typeof(q->'answers') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'answers')>4
 OR (SELECT count(DISTINCT a2->>'ipv4') FROM jsonb_array_elements(q->'answers') a2)<>jsonb_array_length(q->'answers')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(q->'answers') a2 WHERE a2-ARRAY['ipv4','ttl_seconds']<>'{}'
  OR jsonb_typeof(a2->'ipv4') IS DISTINCT FROM 'string' OR (a2->>'ipv4') !~ '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$'
  OR family((a2->>'ipv4')::inet)<>4 OR host((a2->>'ipv4')::inet)<>(a2->>'ipv4')
  OR jsonb_typeof(a2->'ttl_seconds') IS DISTINCT FROM 'number' OR (a2->>'ttl_seconds') !~ '^(0|[1-9][0-9]*)$'
  OR (a2->>'ttl_seconds')::bigint>86400)
 THEN RAISE EXCEPTION 'boundary_result_field_refused';END IF;
 expected:=jsonb_build_object('version','m1_terrain_dns_issued_observation_v1','engagement_id',x.e,'campaign_id',x.c,
 'operator_ref',f->'operator_ref','expected_mission_revision',1,'goal_ref',f->'goal_ref','exercise_mode',f->'exercise_mode',
 'correlation_id',x.result,'source_kind','boundary_dns_acquisition','vantage_ref',p->'vantage_ref','resolver_ipv4',p->'resolver_ipv4',
 'subject_name',chosen->>'name','asset_ref',chosen->>'asset_ref','expected_claim_revision',0,'perceived_at',q->'perceived_at',
 'uncertainty_seconds',q->'uncertainty_seconds','transport','udp','qtype','A','qclass','IN','rcode',q->'rcode',
 'truncated',q->'truncated','completeness',q->'completeness','limitations',q->'limitations','answers',q->'answers');
 IF q::text IS DISTINCT FROM expected::text THEN RAISE EXCEPTION 'fixture_result_binding_refused';END IF;
 INSERT INTO execution.fixture_dns_results VALUES(x.result,r,x.attempt,q,'dns-routing-boundary-v1','boundary_dns_acquisition',x.release_sha256,n,'terrain.issued_dns.v1');
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
 (r->>'version'='m1_terrain_dns_issued_observation_v1' AND r->>'source_kind' IN ('fixture_dns_acquisition','boundary_dns_acquisition') AND EXISTS(
 SELECT 1 FROM execution.fixture_dns_results b JOIN execution.fixture_dns_attempts a USING(run_id)
 JOIN execution.autonomous_runs x USING(run_id) WHERE b.result=(r->>'correlation_id')::uuid
 AND b.request=r AND b.result=x.result AND b.attempt=a.attempt AND b.attempt=x.attempt
 AND a.decision=x.decision_one AND a.session_generation>0 AND b.profile=a.profile AND b.mode=a.mode
 AND (b.mode,b.profile) IN (('controlled_fixture','dns-routing-fixture-v1'),('boundary_dns_acquisition','dns-routing-boundary-v1'))
 AND b.release_sha256=a.release_sha256 AND b.release_sha256=x.release_sha256
 AND (x.e,x.c)=((r->>'engagement_id')::uuid,(r->>'campaign_id')::uuid)
 AND x.request->>'execution_mode'=b.mode AND b.request->>'source_kind'=CASE WHEN b.mode='boundary_dns_acquisition' THEN 'boundary_dns_acquisition' ELSE 'fixture_dns_acquisition' END
 AND x.request->>'run_profile' IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1','autonomous-dns-fixture-v2','autonomous-dns-live-v1')
 AND b.publication_obligation='terrain.issued_dns.v1')))
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

CREATE OR REPLACE FUNCTION cognition.successor_reasoning_identity(x jsonb) RETURNS boolean
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT coalesce(EXISTS(
 SELECT 1 FROM execution.autonomous_runs r JOIN mission.registration_outbox m ON (m.engagement_id,m.campaign_id)=(r.e,r.c)
 JOIN execution.native_input_bindings i ON (i.e,i.c,i.d)=((x->>'engagement')::uuid,(x->>'campaign')::uuid,(x->>'decision')::uuid)
 WHERE (r.e,r.c)=((x->>'engagement')::uuid,(x->>'campaign')::uuid)
 AND (x->>'decision')::uuid IN(r.decision_one,r.decision_two) AND i.writer_oid=r.writer_oid
 AND r.request->>'version'='m1_autonomous_run_request_v2'
 AND r.request->>'run_profile' IN ('autonomous-dns-fixture-v2','autonomous-dns-live-v1')
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

-- successor_configuration emits the fixture pair as its base template; the
-- authoritative lane pair is pack-derived and patched onto the expectation.
CREATE OR REPLACE FUNCTION execution.assert_successor_wire(p text,cfg jsonb,b jsonb,repair boolean) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE expected jsonb; inputs jsonb; messages jsonb; chat boolean; content text; codes jsonb;
BEGIN
 expected:=execution.successor_configuration(p::jsonb);
 expected:=expected||jsonb_build_object('execution_mode',(p::jsonb)->>'execution_mode','run_profile',(p::jsonb)->>'run_profile');
 chat:=expected->>'protocol'='chat-completions-v1';
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
  messages:=messages||jsonb_build_object('role','user','content',content);
 END IF;
 expected:=cfg->'requested_settings'||CASE WHEN chat THEN jsonb_build_object('messages',messages,
 'response_format',jsonb_build_object('type','json_schema','json_schema',jsonb_build_object('name','m1_inference_proposal_v1','schema',cfg->'schema')))
 ELSE jsonb_build_object('instructions',cfg->'instructions','input',inputs,
 'text',jsonb_build_object('format',jsonb_build_object('type','json_schema','name','m1_inference_proposal_v1','schema',cfg->'schema'))) END;
 IF b::text IS DISTINCT FROM expected::text THEN RAISE EXCEPTION 'inference_body_refused'; END IF;
END $$;
REVOKE ALL ON FUNCTION execution.assert_successor_wire(text,jsonb,jsonb,boolean) FROM PUBLIC;

REVOKE ALL ON FUNCTION execution.reserve_boundary_dns(uuid,integer),execution.commit_boundary_dns(uuid,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.reserve_boundary_dns(uuid,integer),execution.commit_boundary_dns(uuid,jsonb) TO dw_m1_broker;


-- The issued-source lineage check above accepts both enumerated lanes; the
-- observations table carries the same enumeration.
ALTER TABLE terrain.observations DROP CONSTRAINT observations_source_kind_check;
ALTER TABLE terrain.observations ADD CONSTRAINT observations_source_kind_check
 CHECK(source_kind IN ('operator_reviewed_dns','fixture_dns_acquisition','boundary_dns_acquisition'));

CREATE OR REPLACE FUNCTION terrain.current_dns_context_view(p_query jsonb)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    q jsonb; k text; elem jsonb;
    v_e uuid; v_c uuid; v_vantage uuid; v_resolver text;
    v_assets uuid[] := ARRAY[]::uuid[];
    v_claim_ids uuid[] := ARRAY[]::uuid[];
    v_has_claim_filter boolean := false;
    m mission.missions%ROWTYPE; reg mission.registration_outbox%ROWTYPE;
    f jsonb; policy jsonb; infer_perm jsonb;
    v_now bigint := floor(extract(epoch FROM transaction_timestamp()))::bigint;
    v_total integer := 0; v_omitted integer := 0;
    v_claims jsonb := '[]'::jsonb;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_m1_cognition','dw_m1_trajectory','dw_m1_terrain']) g WHERE pg_has_role(session_user, g, 'MEMBER'))
       OR current_setting('fsync') <> 'on' OR current_setting('full_page_writes') <> 'on' OR current_setting('synchronous_commit') <> 'on'
       OR EXISTS (SELECT 1 FROM pg_roles a WHERE pg_has_role(session_user, a.oid, 'SET') AND (
            a.rolsuper OR a.rolbypassrls OR a.rolcreaterole OR a.rolcreatedb
            OR EXISTS (SELECT 1 FROM pg_namespace n WHERE n.nspname IN ('mission','trajectory','execution','cognition','terrain')
                       AND (has_schema_privilege(a.oid, n.oid, 'CREATE') OR pg_has_role(a.oid, n.nspowner, 'MEMBER')))
            OR EXISTS (SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid = t.relnamespace
                       WHERE n.nspname IN ('mission','trajectory','execution','cognition','terrain') AND t.relkind = 'r'
                       AND (pg_has_role(a.oid, t.relowner, 'MEMBER')
                            OR has_table_privilege(a.oid, t.oid, 'UPDATE,DELETE,TRUNCATE,TRIGGER')
                            OR has_any_column_privilege(a.oid, t.oid, 'UPDATE')
                            OR (n.nspname = 'terrain' AND (has_table_privilege(a.oid, t.oid, 'SELECT,INSERT') OR has_any_column_privilege(a.oid, t.oid, 'SELECT,INSERT')))
                            OR (n.nspname IN ('cognition','execution') AND (has_table_privilege(a.oid, t.oid, 'INSERT') OR has_any_column_privilege(a.oid, t.oid, 'INSERT'))))))) THEN
        RAISE EXCEPTION 'unqualified_terrain_reader' USING ERRCODE = '42501';
    END IF;
    IF jsonb_typeof(p_query) IS DISTINCT FROM 'object' OR octet_length(p_query::text) > 4096 THEN
        RAISE EXCEPTION 'terrain_view_query_refused';
    END IF;
    q := '{"claim_ids":null}'::jsonb || p_query;
    IF (q - ARRAY['version','engagement_id','campaign_id','operator_ref','expected_mission_revision','goal_ref','exercise_mode','selected_assets','claim_ids']) <> '{}'::jsonb THEN
        RAISE EXCEPTION 'terrain_view_query_refused';
    END IF;
    FOREACH k IN ARRAY ARRAY['version','engagement_id','campaign_id','operator_ref','goal_ref','exercise_mode'] LOOP
        IF jsonb_typeof(q->k) IS DISTINCT FROM 'string' THEN RAISE EXCEPTION 'terrain_view_query_refused'; END IF;
    END LOOP;
    FOREACH k IN ARRAY ARRAY['engagement_id','campaign_id','operator_ref','goal_ref'] LOOP
        IF (q->>k) !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           OR (q->>k)::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'terrain_view_query_refused';
        END IF;
    END LOOP;
    IF q->>'version' <> 'm1_terrain_context_view_query_v1'
       OR jsonb_typeof(q->'expected_mission_revision') IS DISTINCT FROM 'number'
       OR (q->>'expected_mission_revision') <> '1'
       OR q->>'exercise_mode' NOT IN ('blind','defender_informed')
       OR jsonb_typeof(q->'selected_assets') IS DISTINCT FROM 'array'
       OR jsonb_array_length(q->'selected_assets') NOT BETWEEN 1 AND 8
       OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(q->'selected_assets')) <> jsonb_array_length(q->'selected_assets') THEN
        RAISE EXCEPTION 'terrain_view_query_refused';
    END IF;
    FOR elem IN SELECT value FROM jsonb_array_elements(q->'selected_assets') LOOP
        IF jsonb_typeof(elem) IS DISTINCT FROM 'string'
           OR (elem#>>'{}') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           OR (elem#>>'{}')::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'terrain_view_query_refused';
        END IF;
        v_assets := array_append(v_assets, (elem#>>'{}')::uuid);
    END LOOP;
    IF q->'claim_ids' <> 'null'::jsonb THEN
        IF jsonb_typeof(q->'claim_ids') IS DISTINCT FROM 'array'
           OR jsonb_array_length(q->'claim_ids') NOT BETWEEN 1 AND 8
           OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(q->'claim_ids')) <> jsonb_array_length(q->'claim_ids') THEN
            RAISE EXCEPTION 'terrain_view_query_refused';
        END IF;
        v_has_claim_filter := true;
        FOR elem IN SELECT value FROM jsonb_array_elements(q->'claim_ids') LOOP
            IF jsonb_typeof(elem) IS DISTINCT FROM 'string'
               OR (elem#>>'{}') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
               OR (elem#>>'{}')::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
                RAISE EXCEPTION 'terrain_view_query_refused';
            END IF;
            v_claim_ids := array_append(v_claim_ids, (elem#>>'{}')::uuid);
        END LOOP;
    END IF;
    v_e := (q->>'engagement_id')::uuid; v_c := (q->>'campaign_id')::uuid;
    SELECT * INTO m FROM mission.missions WHERE engagement_id = v_e AND campaign_id = v_c;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    SELECT * INTO reg FROM mission.registration_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = m.operation_id;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    f := reg.contract->'fields'; policy := f->'m1_permission'; infer_perm := f->'m1_inference_permission';
    IF ((reg.contract @> jsonb_build_object('engagement_id',m.engagement_id,'campaign_id',m.campaign_id,'operation_id',m.operation_id,
          'event_id',reg.event_id,'producer','mission','kind','mission_registered','owner_revision',m.revision)) IS NOT TRUE
       OR (f @> jsonb_build_object('operator_ref',m.operator_ref,'goal_ref',m.goal_ref,'authority_ref',m.authority_ref,
          'authority_revision',m.authority_revision,'included_assets',m.included_assets,'excluded_assets',m.excluded_assets,
          'exercise_mode',m.exercise_mode,'starts_at',m.starts_at,'ends_at',m.ends_at)) IS NOT TRUE
       OR m.revision <> 1 OR q->>'operator_ref' <> m.operator_ref::text OR q->>'goal_ref' <> m.goal_ref::text
       OR q->>'exercise_mode' <> m.exercise_mode OR policy->>'policy_version' IS DISTINCT FROM '1'
       OR NOT (m.starts_at <= v_now AND v_now < m.ends_at)
       OR NOT ((policy->>'starts_at')::bigint <= v_now AND v_now < (policy->>'ends_at')::bigint)
       OR NOT ((infer_perm->>'starts_at')::bigint <= v_now AND v_now < (infer_perm->>'ends_at')::bigint)
       OR NOT coalesce(infer_perm->'field_classes' ? 'owner_qualified_summaries', false)
       OR NOT EXISTS (SELECT 1 FROM trajectory.registration_history h
                      WHERE h.engagement_id = v_e AND h.campaign_id = v_c
                        AND h.operation_id = m.operation_id AND h.event_id = reg.event_id AND h.status = 'accepted')
       OR EXISTS (SELECT 1 FROM trajectory.registration_history h
                  WHERE h.engagement_id = v_e AND h.campaign_id = v_c AND h.status = 'anomaly')
       OR EXISTS (SELECT 1 FROM mission.withdrawals w WHERE w.engagement_id = v_e AND w.campaign_id = v_c)) IS NOT FALSE THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    IF EXISTS (SELECT 1 FROM unnest(v_assets) a
               WHERE NOT (m.included_assets @> to_jsonb(a)) OR (m.excluded_assets @> to_jsonb(a))) THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    BEGIN
    v_vantage := (policy->>'vantage_ref')::uuid; v_resolver := policy->>'resolver_ipv4';
    IF EXISTS (
        SELECT 1 FROM terrain.current_claims cc
        LEFT JOIN terrain.outbox o ON o.engagement_id = cc.engagement_id AND o.campaign_id = cc.campaign_id AND o.operation_id = cc.operation_id
        LEFT JOIN terrain.observations ob ON ob.engagement_id = cc.engagement_id AND ob.campaign_id = cc.campaign_id AND ob.operation_id = cc.operation_id
        LEFT JOIN terrain.materials mat ON mat.engagement_id = cc.engagement_id AND mat.campaign_id = cc.campaign_id AND mat.operation_id = cc.operation_id
        LEFT JOIN terrain.evaluations ev ON ev.engagement_id = cc.engagement_id AND ev.campaign_id = cc.campaign_id AND ev.operation_id = cc.operation_id
        LEFT JOIN terrain.claim_revisions cr ON cr.engagement_id = cc.engagement_id AND cr.campaign_id = cc.campaign_id AND cr.operation_id = cc.operation_id
        WHERE cc.engagement_id = v_e AND cc.campaign_id = v_c
          AND (
              cc.asset_ref = ANY(v_assets)
              OR cr.asset_ref = ANY(v_assets)
              OR ob.asset_ref = ANY(v_assets)
              OR (cc.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a)
              OR (cr.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a)
              OR (ob.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a)
              OR (o.contract#>>'{claim,asset_ref}') = ANY(SELECT a::text FROM unnest(v_assets) a)
          )
          AND (
              NOT v_has_claim_filter
              OR cc.claim_id = ANY(v_claim_ids)
              OR cr.claim_id = ANY(v_claim_ids)
              OR o.claim_id = ANY(v_claim_ids)
              OR (cc.payload->>'claim_id') = ANY(SELECT cid::text FROM unnest(v_claim_ids) cid)
              OR (o.contract#>>'{claim,claim_id}') = ANY(SELECT cid::text FROM unnest(v_claim_ids) cid)
          )
          AND (
              o.operation_id IS NULL OR ob.operation_id IS NULL OR mat.operation_id IS NULL OR ev.operation_id IS NULL OR cr.operation_id IS NULL
              OR o.event_id IS DISTINCT FROM cc.event_id OR o.event_id IS DISTINCT FROM (o.contract->>'event_id')::uuid
              OR o.observation_id IS DISTINCT FROM cc.observation_id OR o.observation_id IS DISTINCT FROM ob.observation_id
              OR o.material_id IS DISTINCT FROM cc.material_id OR o.material_id IS DISTINCT FROM mat.material_id OR mat.observation_id IS DISTINCT FROM ob.observation_id
              OR o.evaluation_id IS DISTINCT FROM cc.evaluation_id OR o.evaluation_id IS DISTINCT FROM ev.evaluation_id OR ev.claim_id IS DISTINCT FROM cc.claim_id
              OR o.claim_id IS DISTINCT FROM cc.claim_id OR o.claim_revision IS DISTINCT FROM cc.claim_revision
              OR cr.claim_id IS DISTINCT FROM cc.claim_id OR cr.claim_revision IS DISTINCT FROM cc.claim_revision
              OR ob.request_hash IS DISTINCT FROM o.request_hash OR o.request_hash IS DISTINCT FROM (o.contract->>'request_hash')
              OR (o.contract->>'kind') IS DISTINCT FROM 'terrain_changed'
              OR (o.contract->'version') IS DISTINCT FROM '1'::jsonb
              OR (o.contract->>'producer') IS DISTINCT FROM 'terrain'
              OR (o.contract->>'publication_obligation') IS DISTINCT FROM 'trajectory.terrain_history.v1'
              OR (o.contract->>'engagement_id')::uuid IS DISTINCT FROM cc.engagement_id
              OR (o.contract->>'campaign_id')::uuid IS DISTINCT FROM cc.campaign_id
              OR (o.contract->>'operation_id')::uuid IS DISTINCT FROM cc.operation_id
              OR ob.payload IS DISTINCT FROM (o.contract->'observation')
              OR mat.payload IS DISTINCT FROM (o.contract->'material') OR ev.payload IS DISTINCT FROM (o.contract->'evaluation')
              OR cr.payload IS DISTINCT FROM (o.contract->'claim') OR cc.payload IS DISTINCT FROM cr.payload
              OR to_jsonb(cr) IS DISTINCT FROM (to_jsonb(cc) - 'event_id')
              OR cc.payload IS DISTINCT FROM ((to_jsonb(cc) - ARRAY['engagement_id','campaign_id','operation_id','event_id','payload']) || jsonb_build_object('temporal_frontier', cc.payload->'temporal_frontier'))
              OR ob.payload IS DISTINCT FROM ((to_jsonb(ob) - ARRAY['request_hash','payload']) || jsonb_build_object('uncertainty_seconds', ob.payload->'uncertainty_seconds'))
              OR mat.payload IS DISTINCT FROM ((to_jsonb(mat) - ARRAY['engagement_id','campaign_id','operation_id','payload']) || jsonb_build_object('qclass', 'IN', 'limitations', mat.payload->'limitations'))
              OR ev.payload IS DISTINCT FROM (to_jsonb(ev) - ARRAY['engagement_id','campaign_id','operation_id','payload'])
              OR ob.correlation_id IS DISTINCT FROM (o.contract->>'correlation_id')::uuid
              OR ob.vantage_ref IS DISTINCT FROM cc.vantage_ref OR ob.resolver_ipv4 IS DISTINCT FROM cc.resolver_ipv4
              OR ob.subject_name IS DISTINCT FROM cc.subject_name OR ob.asset_ref IS DISTINCT FROM cc.asset_ref
              OR ob.perceived_at IS DISTINCT FROM cc.perceived_at OR ob.recorded_at < ob.perceived_at
              OR jsonb_typeof(ob.payload->'uncertainty_seconds') IS DISTINCT FROM 'number'
              OR (ob.payload->>'uncertainty_seconds')::integer NOT BETWEEN 0 AND 300
              OR mat.answers IS DISTINCT FROM cc.answers
              OR jsonb_typeof(mat.payload->'limitations') IS DISTINCT FROM 'array'
              OR jsonb_array_length(mat.payload->'limitations') > 4
              OR jsonb_typeof(cc.answers) IS DISTINCT FROM 'array'
              OR jsonb_array_length(cc.answers) > 4
              OR ev.observation_id IS DISTINCT FROM cc.observation_id OR ev.material_id IS DISTINCT FROM cc.material_id
              OR ev.valid_until IS DISTINCT FROM cc.valid_until
              OR ev.usable_routing_premise IS DISTINCT FROM (ev.outcome = 'qualified_dns_claim' AND cc.claim_status = 'observed' AND NOT cc.conflicted)
              OR cc.previous_claim_revision IS DISTINCT FROM (cc.claim_revision - 1)
              OR jsonb_typeof(cc.payload->'temporal_frontier') IS DISTINCT FROM 'number'
              OR (cc.payload->>'temporal_frontier')::bigint < cc.perceived_at
              OR (o.contract->>'registration_operation_id')::uuid IS DISTINCT FROM m.operation_id
              OR (o.contract->>'registration_event_id')::uuid IS DISTINCT FROM reg.event_id
              OR (o.contract->>'mission_revision')::integer IS DISTINCT FROM 1
              OR NOT (ob.source_kind=CASE infer_perm->>'execution_mode' WHEN 'controlled_fixture' THEN 'fixture_dns_acquisition' WHEN 'boundary_dns_acquisition' THEN 'boundary_dns_acquisition' ELSE 'operator_reviewed_dns' END)
              OR (ob.source_kind IN ('fixture_dns_acquisition','boundary_dns_acquisition') AND NOT EXISTS(SELECT 1 FROM execution.fixture_dns_results b
                  JOIN execution.autonomous_runs x USING(run_id) WHERE b.result=ob.correlation_id
                  AND (b.mode,b.profile) IN (('controlled_fixture','dns-routing-fixture-v1'),('boundary_dns_acquisition','dns-routing-boundary-v1'))
                  AND x.request->>'execution_mode'=b.mode AND b.release_sha256=x.release_sha256
                  AND (x.e,x.c)=(v_e,v_c) AND b.request->'asset_ref'=to_jsonb(cc.asset_ref)
                  AND b.request->'perceived_at'=to_jsonb(ob.perceived_at) AND b.request->>'source_kind'=ob.source_kind))
              OR cc.claim_kind IS DISTINCT FROM 'dns_hostname_ipv4_resolution'
              OR cc.trust_tier IS DISTINCT FROM 'tier_1' OR cc.corroborated IS DISTINCT FROM false
              OR cc.vantage_ref IS DISTINCT FROM v_vantage OR cc.resolver_ipv4 IS DISTINCT FROM v_resolver
              OR NOT EXISTS (
                  SELECT 1 FROM jsonb_array_elements(policy->'contact_rules') cr_rule
                  WHERE (cr_rule->>'asset_ref')::uuid = cc.asset_ref
                    AND (cr_rule->'rule'->>'exact' = cc.subject_name
                         OR cr_rule->'rule'->>'label_suffix' = cc.subject_name
                         OR right(cc.subject_name, length(cr_rule->'rule'->>'label_suffix') + 1) = '.' || (cr_rule->'rule'->>'label_suffix')))
              OR EXISTS (
                  SELECT 1 FROM jsonb_array_elements(policy->'excluded_names') ex_rule
                  WHERE ex_rule->>'exact' = cc.subject_name
                     OR ex_rule->>'label_suffix' = cc.subject_name
                     OR right(cc.subject_name, length(ex_rule->>'label_suffix') + 1) = '.' || (ex_rule->>'label_suffix'))
          )
    ) OR EXISTS (
        SELECT 1 FROM terrain.claim_revisions cr
        WHERE cr.engagement_id = v_e AND cr.campaign_id = v_c
          AND (cr.asset_ref = ANY(v_assets) OR (cr.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a))
          AND (NOT v_has_claim_filter OR cr.claim_id = ANY(v_claim_ids) OR (cr.payload->>'claim_id') = ANY(SELECT cid::text FROM unnest(v_claim_ids) cid))
          AND NOT EXISTS (
              SELECT 1 FROM terrain.current_claims cc
              WHERE cc.engagement_id = cr.engagement_id AND cc.campaign_id = cr.campaign_id
                AND cc.subject_name = cr.subject_name AND cc.claim_kind = cr.claim_kind
                AND cc.vantage_ref = cr.vantage_ref AND cc.resolver_ipv4 = cr.resolver_ipv4
                AND cc.claim_revision >= cr.claim_revision
          )
    ) THEN
        RAISE EXCEPTION 'terrain_lineage_unavailable';
    END IF;
    SELECT count(*)::integer INTO v_total
    FROM terrain.current_claims cc
    WHERE cc.engagement_id = v_e AND cc.campaign_id = v_c
      AND cc.asset_ref = ANY(v_assets)
      AND (NOT v_has_claim_filter OR cc.claim_id = ANY(v_claim_ids));
    v_omitted := GREATEST(v_total - 8, 0);
    SELECT coalesce(jsonb_agg(item ORDER BY sort_id), '[]'::jsonb) INTO v_claims
    FROM (
        SELECT cc.claim_id AS sort_id,
               jsonb_build_object(
                   'claim_id', cc.claim_id,
                   'claim_revision', cc.claim_revision,
                   'previous_claim_revision', cc.previous_claim_revision,
                   'operation_id', cc.operation_id,
                   'event_id', cc.event_id,
                   'observation_id', cc.observation_id,
                   'material_id', cc.material_id,
                   'evaluation_id', cc.evaluation_id,
                   'asset_ref', cc.asset_ref,
                   'vantage_ref', cc.vantage_ref,
                   'source_kind', ob.source_kind,
                   'claim_kind', cc.claim_kind,
                   'trust_tier', cc.trust_tier,
                   'claim_status', cc.claim_status,
                   'corroborated', cc.corroborated,
                   'conflicted', cc.conflicted,
                   'integrity_hold', held.frontier IS NOT NULL,
                   'hold_frontier', held.frontier,
                   'temporal_frontier', GREATEST((cc.payload->>'temporal_frontier')::bigint, COALESCE(held.frontier, (cc.payload->>'temporal_frontier')::bigint)),
                   'perceived_at', cc.perceived_at,
                   'uncertainty_seconds', (ob.payload->>'uncertainty_seconds')::integer,
                   'valid_until', cc.valid_until,
                   'lowest_ttl_seconds', ev.lowest_ttl_seconds,
                   'answer_count', jsonb_array_length(cc.answers),
                   'completeness', mat.completeness,
                   'limitations', mat.payload->'limitations',
                   'outcome', ev.outcome
               ) AS item
        FROM terrain.current_claims cc
        JOIN terrain.observations ob ON ob.engagement_id = cc.engagement_id AND ob.campaign_id = cc.campaign_id AND ob.operation_id = cc.operation_id
        JOIN terrain.materials mat ON mat.engagement_id = cc.engagement_id AND mat.campaign_id = cc.campaign_id AND mat.operation_id = cc.operation_id
        JOIN terrain.evaluations ev ON ev.engagement_id = cc.engagement_id AND ev.campaign_id = cc.campaign_id AND ev.operation_id = cc.operation_id
        LEFT JOIN LATERAL (
            SELECT max(h.temporal_frontier) AS frontier
            FROM terrain.claim_integrity_holds h
            WHERE h.engagement_id = cc.engagement_id AND h.campaign_id = cc.campaign_id
              AND h.subject_name = cc.subject_name AND h.claim_kind = cc.claim_kind
              AND h.vantage_ref = cc.vantage_ref AND h.resolver_ipv4 = cc.resolver_ipv4
              AND h.temporal_frontier >= (cc.payload->>'temporal_frontier')::bigint
        ) held ON true
        WHERE cc.engagement_id = v_e AND cc.campaign_id = v_c
          AND cc.asset_ref = ANY(v_assets)
          AND (NOT v_has_claim_filter OR cc.claim_id = ANY(v_claim_ids))
        ORDER BY cc.claim_id ASC
        LIMIT 8
    ) s;
    RETURN jsonb_build_object(
        'version', 'm1_terrain_dns_context_view_v1',
        'engagement_id', v_e,
        'campaign_id', v_c,
        'mission_revision', 1,
        'registration_operation_id', m.operation_id,
        'registration_event_id', reg.event_id,
        'vantage_ref', v_vantage,
        'evaluated_at', v_now,
        'selected_assets', q->'selected_assets',
        'claims', v_claims,
        'omitted_by_claim_limit', v_omitted
    );
    EXCEPTION
        WHEN data_exception THEN RAISE EXCEPTION 'terrain_lineage_unavailable';
    END;
EXCEPTION
    WHEN data_exception THEN RAISE EXCEPTION 'terrain_view_query_refused';
END; $$;
