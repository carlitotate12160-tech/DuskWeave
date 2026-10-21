-- Extend only the closed controlled-fixture tuple. Existing release/writer fences remain in place.
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
 OR p->>'version' IS DISTINCT FROM 'm1_inference_permission_v3'
 OR x.request->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1')
 OR (p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    (x.request->>'run_profile',x.request->>'execution_mode')
 OR p->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
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
 RETURN s;
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
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1')
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
 AND x.request->>'run_profile' IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1')
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

CREATE OR REPLACE FUNCTION execution.run_summary(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE x execution.autonomous_runs; y execution.autonomous_control; status text;
BEGIN
 IF execution.run_record(r) IS NULL THEN RAISE EXCEPTION 'run_identity_unavailable'; END IF;
 SELECT * INTO x FROM execution.autonomous_runs WHERE run_id=r;
 SELECT * INTO y FROM execution.autonomous_control WHERE run_id=r;
 status:=CASE WHEN y.status='running' AND least(y.lease_until_ms,x.deadline_at_ms)<=floor(extract(epoch FROM clock_timestamp())*1000)::bigint
 THEN 'unknown_supervisor_lost' ELSE y.status END;
 RETURN jsonb_build_object('kind','m1_autonomous_run_receipt_v1','run_id',r,'run_profile',x.request->'run_profile',
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
END $$;

CREATE OR REPLACE FUNCTION execution.run_start(q jsonb,release text) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; r uuid; e uuid; c uuid; n bigint; s jsonb; p jsonb; deadline bigint;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR release !~ '^[0-9a-f]{64}$'
 OR jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text)>16384
 OR q-ARRAY['version','run_id','run_profile','execution_mode','deadline_at_ms','context','seed_names']<>'{}'
 OR q->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v1'
 OR q->>'run_profile' IS NULL
 OR q->>'run_profile' NOT IN ('autonomous-dns-fixture-v1','autonomous-dns-provider-fixture-v1')
 OR q->>'execution_mode' IS DISTINCT FROM 'controlled_fixture'
 OR jsonb_typeof(q->'seed_names') IS DISTINCT FROM 'array' OR jsonb_array_length(q->'seed_names') NOT BETWEEN 1 AND 8
 OR jsonb_array_length(q#>'{context,candidates}')<>jsonb_array_length(q->'seed_names')
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
 OR deadline<=n OR deadline>n+150000 THEN RAISE EXCEPTION 'run_deadline_refused'; END IF;
 s:=mission.current_inference_publication(e,c,r); p:=s->'fields'->'m1_inference_permission';
 IF s IS NULL OR NOT execution.inference_profile_matches(p)
 OR (p->>'version',p->>'run_profile',p->>'execution_mode') IS DISTINCT FROM
    ('m1_inference_permission_v3',q->>'run_profile',q->>'execution_mode')
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

CREATE OR REPLACE FUNCTION execution.assert_autonomous_pack(e uuid,c uuid,d uuid,p jsonb,cfg jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE x execution.autonomous_runs; expected jsonb; prior jsonb; seed text; permission jsonb;
BEGIN
 SELECT * INTO x FROM execution.autonomous_runs a WHERE (a.e,a.c)=(e,c) AND d IN(a.decision_one,a.decision_two);
 permission:=execution.run_current(x.run_id)#>'{fields,m1_inference_permission}';
 IF p->>'version' IS DISTINCT FROM 'm1_context_pack_autonomous_v1'
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
  IF seed IS NULL OR prior IS DISTINCT FROM
   (expected-ARRAY['decision_entity','decision_event','recorded_at','selected_seed_entity'])
  OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'references') ref WHERE ref->>'id'='previous_decision'
    AND ref->'entity'=expected->'decision_entity' AND ref->'event'=expected->'decision_event'
    AND ref->>'status'='committed_historical_decision' AND ref->>'source'='cognition')
  THEN RAISE EXCEPTION 'run_predecessor_refused'; END IF;
 END IF;
END $$;
