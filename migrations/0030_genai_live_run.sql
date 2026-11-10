-- Live run admission: v4 run request binds deployment qualification and live_binding.
ALTER TABLE execution.autonomous_runs DROP CONSTRAINT IF EXISTS autonomous_runs_qualification_ref_key;
CREATE OR REPLACE FUNCTION execution.run_start(q jsonb,release text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; r uuid; e uuid; c uuid; n bigint; s jsonb; p jsonb; deadline bigint;
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
 -- v4 requires exactly one seed/candidate; reject before identity/session/lease writes.
 -- LAB names must match Rust's canonical nonempty-prefix .duskweave.test rule.
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
  OR x.writer_oid<>(SELECT oid FROM pg_roles WHERE rolname=session_user) THEN RAISE EXCEPTION 'run_identity_conflict'; END IF;
  RETURN false;
 END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint; deadline:=(q->>'deadline_at_ms')::bigint;
 IF jsonb_typeof(q->'deadline_at_ms') IS DISTINCT FROM 'number' OR q->>'deadline_at_ms' !~ '^[1-9][0-9]*$'
 OR deadline<=n OR deadline>n+(CASE WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN 400000 ELSE 150000 END) THEN RAISE EXCEPTION 'run_deadline_refused'; END IF;
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
 OR EXISTS(SELECT 1 FROM (VALUES ('calls',4),('request_bytes',65536),('response_bytes',262144),('output_token_capacity',16384),('wall_time_ms',CASE WHEN q->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN 258000 ELSE 90000 END)) b(k,v)
 WHERE NOT coalesce((p->'campaign_limits'->>b.k)::numeric>=(execution.inference_capacity_snapshot(e,c)->'charged'->>b.k)::numeric+b.v,false))
 THEN RAISE EXCEPTION 'run_authority_unavailable'; END IF;
 deadline:=least(deadline,(s->'fields'->>'ends_at')::bigint*1000,(p->>'ends_at')::bigint*1000);
 INSERT INTO execution.autonomous_runs VALUES(r,e,c,(SELECT oid FROM pg_roles WHERE rolname=session_user),q,
 gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),release,CASE WHEN q->>'version'='m1_autonomous_run_request_v4' THEN (q#>>'{live_binding,qualification_ref}')::uuid ELSE gen_random_uuid() END,deadline);
 INSERT INTO execution.autonomous_control VALUES(r,least(n+2000,deadline),'running',NULL,false);
 PERFORM execution.run_current(r);
 RETURN true;
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
  AND n>=a.recorded_at_ms+CASE WHEN a.phase='acquisition' THEN 60000 ELSE 134000 END)
 THEN RAISE EXCEPTION 'run_phase_expired'; END IF;
 RETURN s;
END $$;

CREATE OR REPLACE FUNCTION execution.assert_autonomous_pack(e uuid,c uuid,d uuid,p jsonb,cfg jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; expected jsonb; prior jsonb; seed text; permission jsonb; registration jsonb;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs a WHERE (a.e,a.c)=(e,c) AND d IN(a.decision_one,a.decision_two);
 registration:=execution.run_current(x.run_id); permission:=registration#>'{fields,m1_inference_permission}';
 IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN
  PERFORM execution.assert_successor_pack(p,registration,x.request);
  IF (p->>'evaluated_at')::bigint>floor(extract(epoch FROM clock_timestamp()))::bigint
  OR (p->>'evaluated_at')::bigint<(registration->>'occurred_at')::bigint THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 END IF;
 IF p->>'version' IS DISTINCT FROM (CASE WHEN x.request->>'version'='m1_autonomous_run_request_v4' THEN 'm1_context_pack_autonomous_v4' WHEN x.request->>'version'='m1_autonomous_run_request_v3' THEN 'm1_context_pack_autonomous_v3' WHEN x.request->>'version'='m1_autonomous_run_request_v2' THEN 'm1_context_pack_autonomous_v2' ELSE 'm1_context_pack_autonomous_v1' END)
 OR (x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') AND (p->>'inference_profile' IS DISTINCT FROM permission->>'profile' OR cfg->>'timing_profile' IS DISTINCT FROM 'reasoning-bounded-v1'))
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

CREATE OR REPLACE FUNCTION execution.assert_successor_pack(p jsonb,reg jsonb,request jsonb) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE ref jsonb; candidate jsonb; original jsonb; expected jsonb; ordinal integer;
 keys text[]:=ARRAY['version','engagement_id','campaign_id','decision_id','permission_id','mission_revision','goal_ref',
 'exercise_mode','scope','question','evaluated_at','owner_views','references','candidates','omissions','pending_unknown',
 'manifests','remaining_capacity','execution_mode','run_profile','inference_profile','predecessor']
 ||CASE WHEN request->>'version'='m1_autonomous_run_request_v4' THEN ARRAY['live_binding','qualification_purpose'] ELSE '{}'::text[] END;
BEGIN
 IF request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4' AND (p ?& ARRAY['live_binding','qualification_purpose'])
 THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 IF request->>'version'='m1_autonomous_run_request_v4' AND (
  p->'live_binding' IS DISTINCT FROM reg#>'{fields,m1_inference_permission,live_binding}'
  OR p->>'qualification_purpose' IS DISTINCT FROM CASE request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END
 ) THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
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

CREATE OR REPLACE FUNCTION execution.run_summary_without_diagnostics(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs; y execution.autonomous_control; status text; summary jsonb; n bigint:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT * INTO y FROM execution.autonomous_control WHERE run_id=r;
 status:=CASE WHEN y.status='running' AND least(y.lease_until_ms,x.deadline_at_ms)<=floor(extract(epoch FROM clock_timestamp())*1000)::bigint
 THEN 'unknown_supervisor_lost' ELSE y.status END;
 summary:=jsonb_build_object('kind','m1_autonomous_run_receipt_v1','run_id',r,'run_profile',x.request->'run_profile',
 'execution_mode',x.request->'execution_mode','status',status,'phase',y.phase,'dispatch_granted',false,'live_qualified',x.request->>'version'='m1_autonomous_run_request_v4' AND coalesce((SELECT n>=z.valid_from_ms AND n<z.valid_until_ms AND z.run_profile=x.request->>'run_profile' AND z.release_sha256=x.release_sha256 AND host(z.resolver_ipv4)=x.request#>>'{live_binding,resolver_ipv4}' AND z.purpose=CASE x.request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END FROM execution.live_qualifications z WHERE z.qualification_ref=x.qualification_ref),false),
 'decision_one',x.decision_one,'decision_two',x.decision_two,'attempt',x.attempt,'result',x.result,
 'terrain_operation',x.terrain_operation,
 'phases',(SELECT coalesce(jsonb_agg(jsonb_build_object('phase',p.phase,'status',p.status,'recorded_at_ms',p.recorded_at_ms) ORDER BY p.recorded_at_ms),'[]') FROM execution.autonomous_phases p WHERE run_id=r),
 'inference',(SELECT coalesce(jsonb_agg(jsonb_build_object('decision',ic.d,'call',ic.call,'binding',ic.binding,'state',ic.state,'reported',ic.usage) ORDER BY ic.d,ic.call),'[]') FROM execution.inference_calls ic WHERE (ic.e,ic.c)=(x.e,x.c) AND ic.d IN(x.decision_one,x.decision_two)),
 'acquisition',CASE WHEN x.request->>'version'='m1_autonomous_run_request_v4' THEN CASE WHEN EXISTS(SELECT 1 FROM execution.live_dns_results WHERE run_id=r) THEN 'committed_live_result'
    WHEN EXISTS(SELECT 1 FROM execution.live_effect_sessions WHERE run_id=r) THEN 'reserved_outcome_unknown' ELSE 'not_reserved' END
   WHEN EXISTS(SELECT 1 FROM execution.fixture_dns_results WHERE run_id=r) THEN 'committed_fixture_result'
   WHEN EXISTS(SELECT 1 FROM execution.fixture_dns_attempts WHERE run_id=r) THEN 'reserved_outcome_unknown' ELSE 'not_reserved' END,
 'terrain',CASE WHEN EXISTS(SELECT 1 FROM terrain.outbox WHERE operation_id=x.terrain_operation) THEN 'committed' ELSE 'pending_or_not_requested' END,
 'effect_count',(SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=r)+(SELECT count(*) FROM execution.live_dns_results WHERE run_id=r),
 'decision_count',(SELECT count(*) FROM cognition.reasoning_decisions cd WHERE (cd.e,cd.c)=(x.e,x.c) AND cd.d IN(x.decision_one,x.decision_two)),
 'decision_history_count',(SELECT count(*) FROM trajectory.reasoning_history h WHERE (h.engagement_id,h.campaign_id)=(x.e,x.c) AND h.operation_id IN(x.decision_one,x.decision_two) AND h.status='accepted'),
 'terrain_history_count',(SELECT count(*) FROM trajectory.terrain_history h WHERE h.operation_id=x.terrain_operation AND h.status='accepted'));
 IF x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4') THEN
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
 IF execution.run_record(r) IS NULL OR (q->>'version'='m1_run_phase_diagnostic_v2' AND NOT EXISTS(SELECT 1 FROM execution.autonomous_runs x WHERE x.run_id=r AND x.request->>'version' IN ('m1_autonomous_run_request_v2','m1_autonomous_run_request_v3','m1_autonomous_run_request_v4'))) OR NOT EXISTS(
  SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase=p AND status='started')
 THEN RAISE EXCEPTION 'run_diagnostic_writer_refused'; END IF;
 INSERT INTO execution.run_phase_diagnostics VALUES(r,p,q) ON CONFLICT DO NOTHING;
 SELECT payload INTO old FROM execution.run_phase_diagnostics WHERE run_id=r AND phase=p;
 IF old IS DISTINCT FROM q THEN RAISE EXCEPTION 'run_diagnostic_conflict'; END IF;
END $$;

-- Live result/disposal and owner publication are the acquisition-to-D2 barrier.
CREATE OR REPLACE FUNCTION execution.run_phase_complete(r uuid,wanted text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; d uuid;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 PERFORM execution.run_current(r);
 IF NOT EXISTS(SELECT 1 FROM execution.autonomous_control WHERE run_id=r AND phase=wanted AND claimed)
 THEN RAISE EXCEPTION 'run_phase_refused';END IF;
 IF wanted IN ('decision_one','decision_two') THEN
  d:=CASE WHEN wanted='decision_one' THEN x.decision_one ELSE x.decision_two END;
  IF NOT EXISTS(SELECT 1 FROM execution.inference_closures z WHERE (z.e,z.c,z.d)=(x.e,x.c,d))
  THEN RAISE EXCEPTION 'run_inference_pending';END IF;
 ELSIF x.request->>'version'='m1_autonomous_run_request_v4' THEN
  IF NOT EXISTS(SELECT 1 FROM execution.live_dns_results b
   JOIN execution.live_effect_control xc USING(run_id)
   JOIN terrain.outbox t ON (t.engagement_id,t.campaign_id,t.operation_id)=(x.e,x.c,x.terrain_operation)
   JOIN trajectory.terrain_history h ON h.event_id=t.event_id
   WHERE b.run_id=r AND b.result=x.result AND b.attempt=x.attempt
   AND xc.phase='closed' AND xc.possibly_started AND xc.disposal_confirmed
   AND t.contract#>>'{observation,source_kind}'='issued_dns_acquisition'
   AND t.contract#>>'{observation,correlation_id}'=b.result::text
   AND h.status='accepted' AND h.contract=t.contract)
  THEN RAISE EXCEPTION 'run_terrain_pending'; END IF;
 ELSE
  IF NOT EXISTS(SELECT 1 FROM execution.fixture_dns_results b JOIN terrain.outbox t
   ON t.operation_id=x.terrain_operation JOIN trajectory.terrain_history h ON h.event_id=t.event_id
   WHERE b.run_id=r AND h.status='accepted' AND h.contract=t.contract)
  THEN RAISE EXCEPTION 'run_terrain_pending';END IF;
 END IF;
 INSERT INTO execution.autonomous_phases VALUES(r,wanted,'completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint);
 IF wanted='acquisition' AND x.request->>'version'<>'m1_autonomous_run_request_v4' THEN PERFORM execution.close_fixture_session(r,'completed');END IF;
 IF wanted='decision_two' THEN UPDATE execution.autonomous_control SET status='completed',lease_until_ms=0 WHERE run_id=r AND status='running';END IF;
END $$;


-- Private context for existing restrictive run-control consumers, never a caller permit.
CREATE FUNCTION execution.private_live_control_scope(r uuid,operation text) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs;s execution.live_effect_sessions;
BEGIN
 IF operation NOT IN ('stop_run','run_unknown') OR operation IS NULL
 OR NOT execution.private_live_actor('dw_m1_broker') THEN RAISE EXCEPTION 'live_control_refused'; END IF;
 SELECT * INTO x FROM execution.autonomous_runs a WHERE a.run_id=r;
 IF x.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user)
 THEN RAISE EXCEPTION 'live_control_refused'; END IF;
 IF x.request->>'version'<>'m1_autonomous_run_request_v4' THEN RETURN; END IF;
 SELECT * INTO s FROM execution.live_effect_sessions a WHERE (a.e,a.c)=(x.e,x.c)
 ORDER BY a.generation DESC LIMIT 1;
 IF s.run_id IS NULL THEN RETURN; END IF;
 IF s.run_id<>r THEN RAISE EXCEPTION 'live_control_refused'; END IF;
 INSERT INTO execution.private_live_writer_context VALUES(pg_backend_pid(),pg_current_xact_id(),x.e,x.c,s.generation,
 (SELECT oid FROM pg_roles WHERE rolname=session_user),operation);
END $$;
REVOKE ALL ON FUNCTION execution.private_live_control_scope(uuid,text) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.run_stop(r uuid) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable';END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 PERFORM execution.private_live_control_scope(r,'stop_run');
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 UPDATE execution.autonomous_control SET status='stopped',lease_until_ms=0 WHERE run_id=r AND status='running';
 IF x.request->>'version'<>'m1_autonomous_run_request_v4' THEN PERFORM execution.close_fixture_session(r,'stopped'); END IF;
 DELETE FROM execution.private_live_writer_context w WHERE w.backend_pid=pg_backend_pid()
 AND w.transaction_id=pg_current_xact_id() AND (w.e,w.c)=(x.e,x.c);
END $$;

CREATE OR REPLACE FUNCTION execution.run_unknown(r uuid,wanted text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable';END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 PERFORM execution.private_live_control_scope(r,'run_unknown');
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 INSERT INTO execution.autonomous_phases VALUES(r,wanted,'unknown',floor(extract(epoch FROM clock_timestamp())*1000)::bigint) ON CONFLICT DO NOTHING;
 UPDATE execution.autonomous_control SET status='unknown',lease_until_ms=0 WHERE run_id=r AND status='running';
 IF x.request->>'version'<>'m1_autonomous_run_request_v4' THEN PERFORM execution.close_fixture_session(r,'unknown'); END IF;
 DELETE FROM execution.private_live_writer_context w WHERE w.backend_pid=pg_backend_pid()
 AND w.transaction_id=pg_current_xact_id() AND (w.e,w.c)=(x.e,x.c);
END $$;
