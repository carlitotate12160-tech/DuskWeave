ALTER TABLE execution.session_fences DROP CONSTRAINT session_fences_phase_check;
ALTER TABLE execution.session_fences ADD CONSTRAINT session_fences_phase_check CHECK(phase IN ('idle','prepared_no_effects','fixture_effects'));
CREATE TABLE execution.fixture_session_closures(run_id uuid PRIMARY KEY,attempt uuid NOT NULL UNIQUE, disposition text NOT NULL);
REVOKE ALL ON execution.fixture_session_closures FROM PUBLIC;
GRANT SELECT ON execution.fixture_session_closures TO dw_m1_broker;
-- Application lifecycle and Broker-owned fixture records. No network deployment is admitted here.
CREATE TABLE execution.autonomous_runs (
 run_id uuid PRIMARY KEY, e uuid NOT NULL, c uuid NOT NULL, writer_oid oid NOT NULL,
 request jsonb NOT NULL, decision_one uuid NOT NULL UNIQUE, decision_two uuid NOT NULL UNIQUE,
 attempt uuid NOT NULL UNIQUE, result uuid NOT NULL UNIQUE, terrain_operation uuid NOT NULL UNIQUE,
 release_sha256 text NOT NULL, qualification_ref uuid NOT NULL UNIQUE, deadline_at_ms bigint NOT NULL,
 UNIQUE(e,c));
CREATE TABLE execution.autonomous_control (
 run_id uuid PRIMARY KEY REFERENCES execution.autonomous_runs, lease_until_ms bigint NOT NULL,
 status text NOT NULL CHECK(status IN ('running','completed','stopped','unknown','deferred','retained','capability_gap','owner_view_requested')),
 phase text, claimed boolean NOT NULL DEFAULT false);
CREATE TABLE execution.autonomous_phases (
 run_id uuid NOT NULL REFERENCES execution.autonomous_runs, phase text NOT NULL,
 status text NOT NULL CHECK(status IN ('started','claimed','completed','unknown')),
 recorded_at_ms bigint NOT NULL, PRIMARY KEY(run_id,phase,status));
CREATE TABLE execution.fixture_dns_attempts (
 run_id uuid PRIMARY KEY REFERENCES execution.autonomous_runs, attempt uuid NOT NULL UNIQUE,
 session_generation bigint NOT NULL, decision uuid NOT NULL, seed_index integer NOT NULL,
 profile text NOT NULL CHECK(profile='dns-routing-fixture-v1'), mode text NOT NULL CHECK(mode='controlled_fixture'),
 release_sha256 text NOT NULL, reserved_at_ms bigint NOT NULL,
 episode_count integer NOT NULL CHECK(episode_count=1), dns_questions integer NOT NULL CHECK(dns_questions=1));
CREATE TABLE execution.fixture_dns_results (
 result uuid PRIMARY KEY, run_id uuid NOT NULL UNIQUE REFERENCES execution.fixture_dns_attempts,
 attempt uuid NOT NULL UNIQUE, request jsonb NOT NULL, profile text NOT NULL CHECK(profile='dns-routing-fixture-v1'),
 mode text NOT NULL CHECK(mode='controlled_fixture'), release_sha256 text NOT NULL,
 received_at_ms bigint NOT NULL, publication_obligation text NOT NULL CHECK(publication_obligation='terrain.issued_dns.v1'));
REVOKE ALL ON execution.autonomous_runs,execution.autonomous_control,execution.autonomous_phases,
 execution.fixture_dns_attempts,execution.fixture_dns_results FROM PUBLIC;
GRANT SELECT ON execution.autonomous_runs,execution.autonomous_control,execution.autonomous_phases,
 execution.fixture_dns_attempts,execution.fixture_dns_results TO dw_m1_broker;
GRANT SELECT ON execution.fixture_dns_attempts,execution.fixture_dns_results,execution.autonomous_runs TO dw_m1_terrain;

CREATE FUNCTION execution.run_record(r uuid) RETURNS jsonb LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT jsonb_build_object('request',x.request,'decision_one',x.decision_one,'decision_two',x.decision_two,
 'attempt',x.attempt,'result',x.result,'terrain_operation',x.terrain_operation,'release_sha256',x.release_sha256,
 'qualification_ref',x.qualification_ref,'status',y.status,'deadline_at_ms',x.deadline_at_ms)
 FROM execution.autonomous_runs x JOIN execution.autonomous_control y USING(run_id)
 WHERE x.run_id=r AND x.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user)
 AND pg_has_role(session_user,'dw_m1_broker','MEMBER')
$$;

CREATE FUNCTION execution.run_current(r uuid) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
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
 OR (p->>'version',p->>'run_profile',p->>'execution_mode',p->>'profile') IS DISTINCT FROM
 ('m1_inference_permission_v3','autonomous-dns-fixture-v1','controlled_fixture','deterministic-autonomous-v1')
 OR p->>'permission_id' IS DISTINCT FROM x.request#>>'{context,mission,permission_id}'
 OR n>=(p->>'ends_at')::bigint*1000 OR n<(p->>'starts_at')::bigint*1000
 OR n>=(s->'fields'->>'ends_at')::bigint*1000 OR n<(s->'fields'->>'starts_at')::bigint*1000
 OR NOT EXISTS(SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id,h.operation_id,h.event_id,h.status,h.contract)
  =(x.e,x.c,(s->>'operation_id')::uuid,(s->>'event_id')::uuid,'accepted',s))
 OR EXISTS(SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id)=(x.e,x.c) AND h.status='anomaly')
 THEN RAISE EXCEPTION 'run_authority_unavailable'; END IF;
 RETURN s;
END $$;

CREATE FUNCTION execution.run_start(q jsonb,release text) RETURNS boolean LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; r uuid; e uuid; c uuid; n bigint; s jsonb;p jsonb; deadline bigint;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR release !~ '^[0-9a-f]{64}$'
 OR jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text)>16384
 OR q-ARRAY['version','run_id','run_profile','execution_mode','deadline_at_ms','context','seed_names']<>'{}'
 OR q->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v1'
 OR (q->>'run_profile',q->>'execution_mode') IS DISTINCT FROM ('autonomous-dns-fixture-v1','controlled_fixture')
 OR jsonb_typeof(q->'seed_names') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'seed_names') NOT BETWEEN 1 AND 8
 OR jsonb_array_length(q#>'{context,candidates}')<>jsonb_array_length(q->'seed_names')
 THEN RAISE EXCEPTION 'run_request_refused'; END IF;
 r:=(q->>'run_id')::uuid;e:=(q#>>'{context,mission,engagement_id}')::uuid;c:=(q#>>'{context,mission,campaign_id}')::uuid;
 IF '00000000-0000-0000-0000-000000000000'::uuid IN(r,e,c) THEN RAISE EXCEPTION 'run_request_refused';END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(e::text||':'||c::text,0));
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r OR (autonomous_runs.e,autonomous_runs.c)=(e,c);
 IF FOUND THEN
   IF x.run_id<>r OR x.request IS DISTINCT FROM q OR x.release_sha256<>release
   OR x.writer_oid<>(SELECT oid FROM pg_roles WHERE rolname=session_user) THEN RAISE EXCEPTION 'run_identity_conflict';END IF;
   RETURN false;
 END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint; deadline:=(q->>'deadline_at_ms')::bigint;
 IF jsonb_typeof(q->'deadline_at_ms') IS DISTINCT FROM 'number' OR q->>'deadline_at_ms' !~ '^[1-9][0-9]*$'
 OR deadline<=n OR deadline>n+150000 THEN RAISE EXCEPTION 'run_deadline_refused';END IF;
 s:=mission.current_inference_publication(e,c,r);p:=s->'fields'->'m1_inference_permission';
 IF s IS NULL OR NOT execution.inference_profile_matches(p) OR p->>'run_profile' IS DISTINCT FROM 'autonomous-dns-fixture-v1'
 OR q#>'{context,mission}' IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1',
 'engagement_id',e,'campaign_id',c,'operator_ref',s->'fields'->'operator_ref','expected_mission_revision',1,
 'permission_id',p->'permission_id','goal_ref',s->'fields'->'goal_ref','exercise_mode',s->'fields'->'exercise_mode')
 OR NOT p->'field_classes' ?& ARRAY['mission_question_constraints','scoped_opaque_references','owner_qualified_summaries',
 'admitted_operator_premises','manifest_availability','omissions_unknown','capacity','admitted_repair_fields_codes','admitted_campaign_decision_feedback']
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(q->'seed_names') WITH ORDINALITY a(v,i)
 WHERE v-ARRAY['asset_ref','name']<>'{}' OR v->'asset_ref' IS DISTINCT FROM q#>'{context,candidates}'->(i::int-1)->'asset_ref'
 OR NOT s->'fields'->'included_assets' @> jsonb_build_array(v->'asset_ref')
 OR s->'fields'->'excluded_assets' @> jsonb_build_array(v->'asset_ref'))

 OR EXISTS(SELECT 1 FROM (VALUES ('calls',4),('request_bytes',65536),('response_bytes',262144),('output_token_capacity',16384),('wall_time_ms',90000)) b(k,v)
 WHERE NOT coalesce((p->'campaign_limits'->>b.k)::numeric>=(execution.inference_capacity_snapshot(e,c)->'charged'->>b.k)::numeric+b.v,false))
 THEN RAISE EXCEPTION 'run_authority_unavailable'; END IF;
 deadline:=least(deadline,(s->'fields'->>'ends_at')::bigint*1000,(p->>'ends_at')::bigint*1000);
 INSERT INTO execution.autonomous_runs VALUES(r,e,c,(SELECT oid FROM pg_roles WHERE rolname=session_user),q,
 gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),gen_random_uuid(),release,gen_random_uuid(),deadline);
 INSERT INTO execution.autonomous_control VALUES(r,least(n+2000,deadline),'running',NULL,false);
 PERFORM execution.run_current(r);
 RETURN true;
END $$;

CREATE FUNCTION execution.run_renew(r uuid) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE n bigint;
BEGIN
 PERFORM execution.run_current(r);
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 UPDATE execution.autonomous_control SET lease_until_ms=least(n+2000,(SELECT deadline_at_ms FROM execution.autonomous_runs WHERE run_id=r)) WHERE run_id=r;
END $$;

CREATE FUNCTION execution.run_phase_start(r uuid,wanted text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE previous text;
BEGIN
 PERFORM execution.run_current(r);
 SELECT phase INTO previous FROM execution.autonomous_control WHERE run_id=r FOR UPDATE;
 IF NOT ((previous IS NULL AND wanted='decision_one') OR (previous='decision_one' AND wanted='acquisition')
 OR (previous='acquisition' AND wanted='decision_two'))
 OR (previous IS NOT NULL AND NOT EXISTS(SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase=previous AND status='completed'))
 THEN RAISE EXCEPTION 'run_phase_refused'; END IF;
 UPDATE execution.autonomous_control SET phase=wanted,claimed=false WHERE run_id=r;
 INSERT INTO execution.autonomous_phases VALUES(r,wanted,'started',floor(extract(epoch FROM clock_timestamp())*1000)::bigint);
END $$;
CREATE FUNCTION execution.run_phase_claim(r uuid,wanted text,release text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
 PERFORM execution.run_current(r);
 UPDATE execution.autonomous_control y SET claimed=true FROM execution.autonomous_runs x
 WHERE y.run_id=r AND x.run_id=r AND y.phase=wanted AND NOT y.claimed AND x.release_sha256=release;
 IF NOT FOUND THEN RAISE EXCEPTION 'run_child_refused';END IF;
 INSERT INTO execution.autonomous_phases VALUES(r,wanted,'claimed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint);
END $$;

CREATE FUNCTION execution.run_stop(r uuid) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable';END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 UPDATE execution.autonomous_control SET status='stopped',lease_until_ms=0 WHERE run_id=r AND status='running';
 PERFORM execution.close_fixture_session(r,'stopped');
END $$;

CREATE FUNCTION execution.run_phase_complete(r uuid,wanted text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
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
 ELSE
  IF NOT EXISTS(SELECT 1 FROM execution.fixture_dns_results b JOIN terrain.outbox t
   ON t.operation_id=x.terrain_operation JOIN trajectory.terrain_history h ON h.event_id=t.event_id
   WHERE b.run_id=r AND h.status='accepted' AND h.contract=t.contract)
  THEN RAISE EXCEPTION 'run_terrain_pending';END IF;
 END IF;
 INSERT INTO execution.autonomous_phases VALUES(r,wanted,'completed',floor(extract(epoch FROM clock_timestamp())*1000)::bigint);
 IF wanted='acquisition' THEN PERFORM execution.close_fixture_session(r,'completed');END IF;
 IF wanted='decision_two' THEN UPDATE execution.autonomous_control SET status='completed',lease_until_ms=0 WHERE run_id=r AND status='running';END IF;
END $$;

CREATE FUNCTION execution.run_unknown(r uuid,wanted text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable';END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 INSERT INTO execution.autonomous_phases VALUES(r,wanted,'unknown',floor(extract(epoch FROM clock_timestamp())*1000)::bigint) ON CONFLICT DO NOTHING;
 UPDATE execution.autonomous_control SET status='unknown',lease_until_ms=0 WHERE run_id=r AND status='running';
 PERFORM execution.close_fixture_session(r,'unknown');
END $$;

REVOKE ALL ON FUNCTION execution.run_record(uuid),execution.run_current(uuid),execution.run_start(jsonb,text),
 execution.run_renew(uuid),execution.run_phase_start(uuid,text),execution.run_phase_claim(uuid,text,text),
 execution.run_stop(uuid),execution.run_phase_complete(uuid,text),execution.run_unknown(uuid,text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.run_record(uuid),execution.run_current(uuid),execution.run_start(jsonb,text),
 execution.run_renew(uuid),execution.run_phase_start(uuid,text),execution.run_phase_claim(uuid,text,text),
 execution.run_stop(uuid),execution.run_phase_complete(uuid,text),execution.run_unknown(uuid,text) TO dw_m1_broker;

CREATE FUNCTION execution.run_terminal(r uuid,reason text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;p jsonb;expected text;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 PERFORM execution.run_current(r);
 SELECT payload->'proposal' INTO p FROM cognition.reasoning_decisions WHERE (e,c,d)=(x.e,x.c,x.decision_one);
 expected:=CASE WHEN p#>'{next_intent,view_request}' IS NOT NULL AND p#>'{next_intent,view_request}'<>'null' THEN 'owner_view_requested'
 WHEN p#>>'{next_intent,category}'='retain' THEN 'retained' WHEN p#>>'{next_intent,category}'='defer' THEN 'deferred'
 WHEN p#>>'{next_intent,category}'='stop' THEN 'stopped' ELSE 'capability_gap' END;
 IF reason IS DISTINCT FROM expected OR NOT EXISTS(SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase='decision_one' AND status='completed')
 THEN RAISE EXCEPTION 'run_terminal_refused'; END IF;
 UPDATE execution.autonomous_control SET status=reason,lease_until_ms=0 WHERE run_id=r AND status='running';
END $$;

CREATE FUNCTION execution.reserve_fixture_dns(r uuid,seed integer) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
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
 SELECT payload->'proposal' INTO proposal FROM cognition.reasoning_decisions WHERE (e,c,d)=(x.e,x.c,x.decision_one);
 SELECT call.p::jsonb INTO pack FROM execution.inference_calls call JOIN execution.inference_reservations h
 ON h.generation_call_id=call.call WHERE (call.e,call.c,call.d)=(x.e,x.c,x.decision_one) AND call.state='completed';
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

CREATE FUNCTION execution.commit_fixture_dns(r uuid,q jsonb) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;a execution.fixture_dns_attempts;s jsonb;f jsonb;p jsonb;chosen jsonb;expected jsonb;n bigint;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET epoch=epoch+1 WHERE engagement_id=x.e AND campaign_id=x.c;
 s:=execution.run_current(r);f:=s->'fields';p:=f->'m1_permission';
 SELECT * INTO a FROM execution.fixture_dns_attempts WHERE run_id=r;
 chosen:=x.request->'seed_names'->a.seed_index;n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF a.run_id IS NULL OR a.release_sha256<>x.release_sha256 OR a.mode<>'controlled_fixture'
 OR EXISTS(SELECT 1 FROM execution.fixture_dns_results WHERE run_id=r)
 OR q->>'perceived_at' !~ '^[1-9][0-9]*$' OR jsonb_typeof(q->'perceived_at') IS DISTINCT FROM 'number'
 OR (q->>'perceived_at')::bigint<a.reserved_at_ms/1000 OR (q->>'perceived_at')::bigint>n/1000
 OR n-a.reserved_at_ms>60000 THEN RAISE EXCEPTION 'fixture_result_window_refused';END IF;
 expected:=jsonb_build_object('version','m1_terrain_dns_issued_observation_v1','engagement_id',x.e,'campaign_id',x.c,
 'operator_ref',f->'operator_ref','expected_mission_revision',1,'goal_ref',f->'goal_ref','exercise_mode',f->'exercise_mode',
 'correlation_id',x.result,'source_kind','fixture_dns_acquisition','vantage_ref',p->'vantage_ref','resolver_ipv4',p->'resolver_ipv4',
 'subject_name',chosen->'name','asset_ref',chosen->'asset_ref','expected_claim_revision',0,'perceived_at',q->'perceived_at','uncertainty_seconds',0,
 'transport','udp','qtype','A','qclass','IN','rcode','noerror','truncated',false,'completeness','complete',
 'limitations',jsonb_build_array('single_vantage_only'),'answers',jsonb_build_array(jsonb_build_object('ipv4','192.0.2.80','ttl_seconds',60)));
 IF q::text IS DISTINCT FROM expected::text THEN RAISE EXCEPTION 'fixture_result_binding_refused';END IF;
 INSERT INTO execution.fixture_dns_results VALUES(x.result,r,x.attempt,q,'dns-routing-fixture-v1','controlled_fixture',x.release_sha256,n,'terrain.issued_dns.v1');
END $$;

CREATE FUNCTION execution.run_predecessor(r uuid) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;d jsonb;t terrain.outbox;b execution.fixture_dns_results;
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
 RETURN jsonb_build_object('availability','committed','decision_reference','previous_decision','selected_seed_reference','asset_'||(SELECT seed_index FROM execution.fixture_dns_attempts WHERE run_id=r)::text,
 'selected_seed_entity',(x.request->'seed_names'->(SELECT seed_index FROM execution.fixture_dns_attempts WHERE run_id=r))->'asset_ref',
 'selected_intent','observe_more','attempt_ref',x.attempt,'result_ref',x.result,'terrain_publication_ref',t.event_id,
 'execution_mode','controlled_fixture','outcome','owner_qualified_observed','hypothesis_summary_omission','not_required',
 'decision_entity',d->'decision','decision_event',d->'event','recorded_at',((d->>'recorded_at_ms')::bigint/1000));
END $$;
REVOKE ALL ON FUNCTION execution.run_terminal(uuid,text),execution.reserve_fixture_dns(uuid,integer),execution.commit_fixture_dns(uuid,jsonb),execution.run_predecessor(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.run_terminal(uuid,text),execution.reserve_fixture_dns(uuid,integer),execution.commit_fixture_dns(uuid,jsonb),execution.run_predecessor(uuid) TO dw_m1_broker;

CREATE FUNCTION execution.close_fixture_session(r uuid,disposition text) RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 IF x.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user) THEN RAISE EXCEPTION 'run_actor_refused';END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(x.e::text||':'||x.c::text,0));
 UPDATE execution.session_fences SET operation_id=NULL,writer_oid=NULL,phase='idle',epoch=epoch+1
 WHERE engagement_id=x.e AND campaign_id=x.c AND phase='fixture_effects' AND operation_id=x.attempt;
 IF FOUND THEN INSERT INTO execution.fixture_session_closures VALUES(r,x.attempt,disposition) ON CONFLICT DO NOTHING; END IF;
END $$;
REVOKE ALL ON FUNCTION execution.close_fixture_session(uuid,text) FROM PUBLIC;

-- Withdrawal orders against the same scope fence and stops fixture eligibility before Mission publication.
CREATE FUNCTION execution.stop_fixture_withdrawal() RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE r uuid;
BEGIN
 PERFORM pg_advisory_xact_lock(hashtextextended(NEW.engagement_id::text||':'||NEW.campaign_id::text,0));
 SELECT x.run_id INTO r FROM execution.autonomous_runs x JOIN execution.autonomous_control y USING(run_id)
 WHERE x.e=NEW.engagement_id AND x.c=NEW.campaign_id AND y.status='running';
 IF r IS NOT NULL THEN
  UPDATE execution.autonomous_control SET status='stopped',lease_until_ms=0 WHERE run_id=r;
  UPDATE execution.session_fences SET operation_id=NULL,writer_oid=NULL,phase='idle',epoch=epoch+1
  WHERE engagement_id=NEW.engagement_id AND campaign_id=NEW.campaign_id AND phase='fixture_effects';
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION execution.stop_fixture_withdrawal() FROM PUBLIC;
CREATE TRIGGER a_fixture_withdrawal BEFORE INSERT ON mission.withdrawals FOR EACH ROW EXECUTE FUNCTION execution.stop_fixture_withdrawal();

CREATE FUNCTION execution.run_summary(r uuid) RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs;y execution.autonomous_control;status text;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable';END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT * INTO y FROM execution.autonomous_control WHERE run_id=r;
 status:=CASE WHEN y.status='running' AND least(y.lease_until_ms,x.deadline_at_ms)<=floor(extract(epoch FROM clock_timestamp())*1000)::bigint
 THEN 'unknown_supervisor_lost' ELSE y.status END;
 RETURN jsonb_build_object('kind','m1_autonomous_run_receipt_v1','run_id',r,'run_profile','autonomous-dns-fixture-v1',
 'execution_mode','controlled_fixture','status',status,'phase',y.phase,'dispatch_granted',false,'live_qualified',false,
 'decision_one',x.decision_one,'decision_two',x.decision_two,'attempt',x.attempt,'result',x.result,
 'terrain_operation',x.terrain_operation,
 'phases',(SELECT coalesce(jsonb_agg(jsonb_build_object('phase',p.phase,'status',p.status,'recorded_at_ms',p.recorded_at_ms) ORDER BY p.recorded_at_ms),'[]') FROM execution.autonomous_phases p WHERE run_id=r),
 'acquisition',CASE WHEN EXISTS(SELECT 1 FROM execution.fixture_dns_results WHERE run_id=r) THEN 'committed_fixture_result'
   WHEN EXISTS(SELECT 1 FROM execution.fixture_dns_attempts WHERE run_id=r) THEN 'reserved_outcome_unknown' ELSE 'not_reserved' END,
 'terrain',CASE WHEN EXISTS(SELECT 1 FROM terrain.outbox WHERE operation_id=x.terrain_operation) THEN 'committed' ELSE 'pending_or_not_requested' END,
 'effect_count',(SELECT count(*) FROM execution.fixture_dns_results WHERE run_id=r),
 'decision_count',(SELECT count(*) FROM cognition.reasoning_decisions cd WHERE (cd.e,cd.c)=(x.e,x.c) AND cd.d IN(x.decision_one,x.decision_two)),
 'decision_history_count',(SELECT count(*) FROM trajectory.reasoning_history h WHERE (h.engagement_id,h.campaign_id)=(x.e,x.c) AND h.operation_id IN(x.decision_one,x.decision_two) AND h.status='accepted'),
 'terrain_history_count',(SELECT count(*) FROM trajectory.terrain_history h WHERE h.operation_id=x.terrain_operation AND h.status='accepted'));
END $$;
REVOKE ALL ON FUNCTION execution.run_summary(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.run_summary(uuid) TO dw_m1_broker;

-- Validate causal pack identifiers against committed owner records, not caller labels or a digest alone.
CREATE FUNCTION execution.assert_autonomous_pack(e uuid,c uuid,d uuid,p jsonb,cfg jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs;expected jsonb;prior jsonb;seed text;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs a WHERE (a.e,a.c)=(e,c) AND d IN(a.decision_one,a.decision_two);
 PERFORM execution.run_current(x.run_id);
 IF p->>'version' IS DISTINCT FROM 'm1_context_pack_autonomous_v1'
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM ('autonomous-dns-fixture-v1','controlled_fixture')
 OR (cfg->>'run_profile',cfg->>'execution_mode') IS DISTINCT FROM (p->>'run_profile',p->>'execution_mode')
 OR (p->>'engagement_id',p->>'campaign_id',p->>'decision_id') IS DISTINCT FROM (e::text,c::text,d::text)
 OR p->'manifests' IS DISTINCT FROM jsonb_build_array(jsonb_build_object('availability','qualified','execution_mode','controlled_fixture',
 'profile','dns-routing-fixture-v1','qualification_ref',x.qualification_ref,'reference','dns_udp','release_sha256',x.release_sha256,
 'schema','m1_dns_ipc_v1','transport','udp')) THEN RAISE EXCEPTION 'run_pack_binding_refused';END IF;
 IF d=x.decision_one THEN
  IF p->'predecessor' IS DISTINCT FROM '{"availability":"absent","reason":"first_decision"}'::jsonb
  THEN RAISE EXCEPTION 'run_predecessor_refused';END IF;
 ELSE
  expected:=execution.run_predecessor(x.run_id);prior:=p->'predecessor';
  SELECT ref->>'id' INTO seed FROM jsonb_array_elements(p->'references') ref
  WHERE ref->>'owner'='operator' AND ref->'entity'=expected->'selected_seed_entity';
  expected:=jsonb_set(expected,'{selected_seed_reference}',to_jsonb(seed));
  IF seed IS NULL OR prior IS DISTINCT FROM
   (expected-ARRAY['decision_entity','decision_event','recorded_at','selected_seed_entity'])
  OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'references') ref WHERE ref->>'id'='previous_decision'
    AND ref->'entity'=expected->'decision_entity' AND ref->'event'=expected->'decision_event'
    AND ref->>'status'='committed_historical_decision' AND ref->>'source'='cognition')
  THEN RAISE EXCEPTION 'run_predecessor_refused';END IF;
 END IF;
END $$;
REVOKE ALL ON FUNCTION execution.assert_autonomous_pack(uuid,uuid,uuid,jsonb,jsonb) FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.inference_may_send(e uuid,c uuid,d uuid,k uuid,p text,cfg text,b text,h text)
RETURNS void LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable

DECLARE r execution.inference_reservations;s jsonb;perm jsonb;now_s bigint;actual text;
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
 PERFORM execution.inference_window_ms(e,c,d);
 IF NOT coalesce(perm->'field_classes' ? 'admitted_repair_fields_codes',false) THEN RAISE EXCEPTION 'inference_consent_refused'; END IF;
 IF perm->>'version'='m1_inference_permission_v3' THEN PERFORM execution.assert_autonomous_pack(e,c,d,p::jsonb,cfg::jsonb);END IF;
 now_s:=floor(extract(epoch FROM clock_timestamp()))::bigint;
 IF s IS NULL OR perm->>'permission_id' IS DISTINCT FROM r.request->>'permission_id'
 OR now_s<(s->'fields'->>'starts_at')::bigint OR now_s>=(s->'fields'->>'ends_at')::bigint
 OR now_s<(perm->>'starts_at')::bigint OR now_s>=(perm->>'ends_at')::bigint
 OR octet_length(p)>(perm->'operation_limits'->>'context_bytes')::int
 OR octet_length(cfg)>(perm->'operation_limits'->>'configuration_bytes')::int
 OR octet_length(b)>(perm->'operation_limits'->>'request_bytes')::int
 OR cfg::jsonb->>'recipient' IS DISTINCT FROM perm->>'recipient'
 OR cfg::jsonb->>'profile' IS DISTINCT FROM perm->>'profile'
 OR b::jsonb->>'input' IS DISTINCT FROM p AND k=r.generation_call_id THEN
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
