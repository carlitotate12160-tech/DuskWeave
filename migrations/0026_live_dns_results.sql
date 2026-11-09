-- Safe exporter projection only. No raw packet, digest, quarantine or diagnostic text.
CREATE TABLE execution.live_dns_results (
 result uuid PRIMARY KEY,
 run_id uuid NOT NULL UNIQUE REFERENCES execution.live_effect_sessions,
 attempt uuid NOT NULL UNIQUE,
 session_generation bigint NOT NULL,
 projection jsonb NOT NULL CHECK(jsonb_typeof(projection)='object'),
 received_at_ms bigint NOT NULL,
 publication_obligation text NOT NULL CHECK(publication_obligation='terrain.issued_dns.v2'),
 FOREIGN KEY(run_id,session_generation) REFERENCES execution.live_effect_sessions(run_id,generation)
);
REVOKE ALL ON execution.live_dns_results FROM PUBLIC,dw_runtime,dw_m1_broker,dw_m1_cognition,
 dw_m1_trajectory,dw_m1_terrain;
GRANT SELECT ON execution.live_dns_results TO dw_m1_broker;

CREATE OR REPLACE FUNCTION execution.live_effect_record(r uuid) RETURNS jsonb
LANGUAGE sql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
 SELECT jsonb_build_object('run_id',s.run_id,'request',s.request,'result_ref',s.result_ref,
 'phase',x.phase,'possibly_started',x.possibly_started,
 'attempt_deadline_at_ms',x.attempt_deadline_at_ms,'result',b.projection,'disposal_confirmed',x.disposal_confirmed)
 FROM execution.live_effect_sessions s JOIN execution.live_effect_control x USING(run_id)
 LEFT JOIN execution.live_dns_results b USING(run_id)
 WHERE s.run_id=r AND s.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user)
 AND execution.private_live_actor('dw_m1_broker')
$$;

CREATE FUNCTION execution.private_live_result_shape(v jsonb,q jsonb,deadline bigint,started bigint) RETURNS boolean
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE answer jsonb;name jsonb;received boolean;
BEGIN
 IF jsonb_typeof(v) IS DISTINCT FROM 'object' OR octet_length(v::text)>16384
 OR NOT v ?& ARRAY['schema','kind','attempt','session_generation','sequence','status','observed_at_ms',
 'rcode','truncated','complete','withheld_aliases','answers','aliases']
 OR v-ARRAY['schema','kind','attempt','session_generation','sequence','status','observed_at_ms',
 'rcode','truncated','complete','withheld_aliases','answers','aliases']<>'{}'
 OR (v->>'schema',v->>'kind',v->>'attempt',v->'session_generation',v->>'sequence') IS DISTINCT FROM
 ('m1_dns_ipc_v1','RESULT',q->>'attempt',q->'session_generation','3')
 OR jsonb_typeof(v->'status') IS DISTINCT FROM 'string'
 OR NOT coalesce(v->>'status' IN ('completed','limited','timeout','limit','process_failure','unsupported_response','ipc_failure'),false)
 OR EXISTS(SELECT 1 FROM unnest(ARRAY['session_generation','sequence','observed_at_ms','rcode','withheld_aliases']) k
 WHERE jsonb_typeof(v->k) IS DISTINCT FROM 'number' OR v->>k !~ '^(0|[1-9][0-9]*)$')
 OR (v->>'rcode')::numeric>5 OR (v->>'withheld_aliases')::numeric>4
 OR jsonb_typeof(v->'complete') IS DISTINCT FROM 'boolean'
 OR jsonb_typeof(v->'truncated') IS DISTINCT FROM 'boolean'
 OR jsonb_typeof(v->'answers') IS DISTINCT FROM 'array'
 OR jsonb_typeof(v->'aliases') IS DISTINCT FROM 'array' THEN RETURN false; END IF;
 IF jsonb_array_length(v->'answers')>4 OR jsonb_array_length(v->'aliases')>4 THEN RETURN false; END IF;
 received:=v->>'status' IN ('completed','limited');
 IF received THEN
  IF (v->>'observed_at_ms')::numeric<started OR (v->>'observed_at_ms')::numeric>deadline
  OR v->'complete' IS DISTINCT FROM to_jsonb(v->>'status'='completed')
  OR (v->'truncated'='true' AND v->'complete'='true')
  OR (v->>'rcode'<>'0' AND v->'answers'<>'[]')
  OR (v->>'withheld_aliases'<>'0' AND v->'complete'='true') THEN RETURN false; END IF;
 ELSE
  IF (v->>'observed_at_ms',v->>'rcode',v->>'withheld_aliases') IS DISTINCT FROM ('0','0','0')
  OR (v->'complete',v->'truncated',v->'answers',v->'aliases') IS DISTINCT FROM
  ('false'::jsonb,'false'::jsonb,'[]'::jsonb,'[]'::jsonb) THEN RETURN false; END IF;
 END IF;
 FOR answer IN SELECT value FROM jsonb_array_elements(v->'answers') LOOP
  IF jsonb_typeof(answer) IS DISTINCT FROM 'object' OR NOT answer ?& ARRAY['ipv4','ttl_seconds']
  OR answer-ARRAY['ipv4','ttl_seconds']<>'{}'
  OR jsonb_typeof(answer->'ipv4') IS DISTINCT FROM 'string'
  OR answer->>'ipv4' !~ '^[0-9]{1,3}(\.[0-9]{1,3}){3}$'
  OR jsonb_typeof(answer->'ttl_seconds') IS DISTINCT FROM 'number'
  OR answer->>'ttl_seconds' !~ '^(0|[1-9][0-9]*)$'
  OR (answer->>'ttl_seconds')::numeric>86400 THEN RETURN false; END IF;
  IF host((answer->>'ipv4')::inet) IS DISTINCT FROM answer->>'ipv4' THEN RETURN false; END IF;
 END LOOP;
 IF (SELECT count(DISTINCT value->>'ipv4') FROM jsonb_array_elements(v->'answers'))<>jsonb_array_length(v->'answers')
 THEN RETURN false; END IF;
 FOR name IN SELECT value FROM jsonb_array_elements(v->'aliases') LOOP
  IF jsonb_typeof(name) IS DISTINCT FROM 'string' OR NOT q->'allowed_names' @> jsonb_build_array(name)
  OR name=q->'name' THEN RETURN false; END IF;
 END LOOP;
 IF (SELECT count(DISTINCT value) FROM jsonb_array_elements(v->'aliases'))<>jsonb_array_length(v->'aliases')
 THEN RETURN false; END IF;
 RETURN true;
EXCEPTION WHEN invalid_text_representation OR numeric_value_out_of_range THEN RETURN false;
END $$;
REVOKE ALL ON FUNCTION execution.private_live_result_shape(jsonb,jsonb,bigint,bigint) FROM PUBLIC;

CREATE FUNCTION execution.live_dns_result(r uuid,g bigint,v jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE s execution.live_effect_sessions;x execution.live_effect_control;b execution.live_dns_results;
 n bigint;started bigint;
BEGIN
 SELECT * INTO s FROM execution.live_effect_sessions WHERE run_id=r;
 IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
 OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user)
 THEN RAISE EXCEPTION 'live_result_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text||':'||s.c::text,0));
 SELECT * INTO b FROM execution.live_dns_results WHERE run_id=r;
 IF FOUND THEN
  IF b.projection IS DISTINCT FROM v THEN RAISE EXCEPTION 'live_result_conflict'; END IF;
  RETURN;
 END IF;
 SELECT * INTO x FROM execution.live_effect_control WHERE run_id=r;
 SELECT occurred_at_ms INTO started FROM execution.live_effect_events WHERE run_id=r AND phase='may_start';
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF x.phase NOT IN ('may_start','stopped') OR NOT x.possibly_started OR x.disposal_confirmed OR started IS NULL
 OR NOT execution.private_live_result_shape(v,s.request,least(n,x.attempt_deadline_at_ms),started)
 THEN RAISE EXCEPTION 'live_result_refused'; END IF;
 INSERT INTO execution.live_dns_results VALUES(s.result_ref,r,s.attempt,g,v,n,'terrain.issued_dns.v2');
 INSERT INTO execution.live_effect_events VALUES(r,g,'result_recorded',n);
 UPDATE execution.live_effect_control SET phase='result_recorded' WHERE run_id=r AND phase='may_start';
END $$;
REVOKE ALL ON FUNCTION execution.live_dns_result(uuid,bigint,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.live_dns_result(uuid,bigint,jsonb) TO dw_m1_broker;

CREATE FUNCTION execution.live_effect_stop(r uuid,g bigint,disposed boolean) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE s execution.live_effect_sessions;x execution.live_effect_control;n bigint;target text;
BEGIN
 SELECT * INTO s FROM execution.live_effect_sessions WHERE run_id=r;
 IF NOT execution.private_live_actor('dw_m1_broker') OR g IS NULL OR s.generation IS DISTINCT FROM g
 OR s.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user) OR disposed IS NULL
 THEN RAISE EXCEPTION 'live_stop_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text||':'||s.c::text,0));
 SELECT * INTO x FROM execution.live_effect_control WHERE run_id=r;
 IF x.phase='closed' THEN RETURN; END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 target:=CASE WHEN disposed THEN 'closed' ELSE 'stopped' END;
 UPDATE execution.live_effect_control SET phase=target,disposal_confirmed=disposed WHERE run_id=r;
 INSERT INTO execution.live_effect_events VALUES(r,g,target,n) ON CONFLICT DO NOTHING;
 -- possibly_started is never reset. Closed+unknown remains charged and fenced.
END $$;
REVOKE ALL ON FUNCTION execution.live_effect_stop(uuid,bigint,boolean) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.live_effect_stop(uuid,bigint,boolean) TO dw_m1_broker;
