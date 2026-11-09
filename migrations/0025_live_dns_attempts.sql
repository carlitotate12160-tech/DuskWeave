-- Broker admission is fresh after D1; no database transaction spans host I/O.
CREATE FUNCTION execution.private_live_deadline(r uuid) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE a execution.autonomous_runs;q execution.live_qualifications;s jsonb;p jsonb;pack jsonb;
 n bigint;bound bigint;
BEGIN
 SELECT * INTO a FROM execution.autonomous_runs WHERE run_id=r;
 IF NOT execution.private_live_actor('dw_m1_broker')
 OR a.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user)
 OR a.request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4'
 THEN RAISE EXCEPTION 'live_effect_refused'; END IF;
 s:=execution.run_current(r);p:=s#>'{fields,m1_permission}';
 SELECT * INTO q FROM execution.live_qualifications WHERE qualification_ref=a.qualification_ref;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF q.qualification_ref IS NULL OR n<q.valid_from_ms OR n>=q.valid_until_ms
 OR (q.run_profile,q.release_sha256,host(q.resolver_ipv4)) IS DISTINCT FROM
 (a.request->>'run_profile',a.release_sha256,a.request#>>'{live_binding,resolver_ipv4}')
 OR a.request->'live_binding' IS DISTINCT FROM s#>'{fields,m1_inference_permission,live_binding}'
 OR p->>'resolver_ipv4' IS DISTINCT FROM host(q.resolver_ipv4)
 OR n<(p->>'starts_at')::bigint*1000 OR n>=(p->>'ends_at')::bigint*1000
 THEN RAISE EXCEPTION 'live_qualification_unavailable'; END IF;
 SELECT call.p::jsonb INTO pack FROM execution.inference_calls call
 JOIN execution.inference_reservations h ON h.generation_call_id=call.call
 WHERE (call.e,call.c,call.d)=(a.e,a.c,a.decision_one) AND call.state='completed';
 IF pack IS NULL THEN RAISE EXCEPTION 'live_decision_source_unavailable'; END IF;
 PERFORM execution.assert_current_pack_terrain(a.e,a.c,a.decision_one,pack);
 SELECT min((v->>'valid_until')::bigint*1000) INTO bound
 FROM jsonb_array_elements(pack->'references') v WHERE v->>'owner'='cyber_terrain';
 bound:=least(bound,a.deadline_at_ms,q.valid_until_ms,(p->>'ends_at')::bigint*1000,
 (s#>>'{fields,ends_at}')::bigint*1000,(s#>>'{fields,m1_inference_permission,ends_at}')::bigint*1000);
 IF bound IS NULL OR bound<=n THEN RAISE EXCEPTION 'live_effect_expired'; END IF;
 RETURN bound;
END $$;
REVOKE ALL ON FUNCTION execution.private_live_deadline(uuid) FROM PUBLIC;

CREATE FUNCTION execution.live_effect_reserve(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE a execution.autonomous_runs;prior execution.live_effect_sessions;
 s jsonb;p jsonb;proposal jsonb;pack jsonb;seed jsonb;g bigint;n bigint;bound bigint;request jsonb;
BEGIN
 SELECT * INTO a FROM execution.autonomous_runs WHERE run_id=r;
 IF a.run_id IS NULL THEN RAISE EXCEPTION 'live_effect_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(a.e::text||':'||a.c::text,0));
 SELECT * INTO prior FROM execution.live_effect_sessions WHERE run_id=r;
 IF FOUND THEN
  IF prior.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_roles WHERE rolname=session_user)
  OR NOT execution.private_live_actor('dw_m1_broker') THEN RAISE EXCEPTION 'live_effect_refused'; END IF;
  RETURN execution.live_effect_record(r);
 END IF;
 bound:=execution.private_live_deadline(r);
 s:=execution.run_current(r);p:=s#>'{fields,m1_permission}';seed:=a.request->'seed_names'->0;
 IF NOT EXISTS(SELECT 1 FROM execution.autonomous_control WHERE run_id=r AND phase='acquisition' AND claimed)
 OR NOT EXISTS(SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase='decision_one' AND status='completed')
 OR NOT EXISTS(SELECT 1 FROM execution.inference_closures WHERE (e,c,d)=(a.e,a.c,a.decision_one))
 OR EXISTS(SELECT 1 FROM execution.inference_reservations h WHERE (h.engagement_id,h.campaign_id)=(a.e,a.c)
 AND h.record_revision=1 AND NOT EXISTS(SELECT 1 FROM execution.inference_closures z
 WHERE (z.e,z.c,z.d)=(a.e,a.c,h.operation_id)))
 OR EXISTS(SELECT 1 FROM execution.fixture_dns_attempts WHERE run_id=r)
 OR EXISTS(SELECT 1 FROM execution.live_effect_sessions WHERE (e,c)=(a.e,a.c))
 THEN RAISE EXCEPTION 'live_sender_not_closed'; END IF;
 SELECT payload->'proposal' INTO proposal FROM cognition.reasoning_decisions d
 WHERE (d.e,d.c,d.d)=(a.e,a.c,a.decision_one) AND payload->>'disposition'='validated_proposal'
 AND EXISTS(SELECT 1 FROM trajectory.reasoning_history h WHERE
 (h.engagement_id,h.campaign_id,h.operation_id,h.event_id,h.status,h.contract)=
 (a.e,a.c,a.decision_one,d.event,'accepted',d.payload));
 SELECT call.p::jsonb INTO pack FROM execution.inference_calls call
 JOIN execution.inference_reservations h ON h.generation_call_id=call.call
 WHERE (call.e,call.c,call.d)=(a.e,a.c,a.decision_one) AND call.state='completed';
 IF proposal IS NULL OR proposal#>>'{next_intent,category}' IS DISTINCT FROM 'observe_more'
 OR proposal#>>'{next_intent,manifest_reference}' IS DISTINCT FROM 'dns_udp'
 OR NOT coalesce(proposal#>'{next_intent,references}' @> '["asset_0"]'::jsonb,false)
 OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(pack->'references') v
 WHERE v->>'id'='asset_0' AND v->>'owner'='operator' AND v->>'source'='admitted_operator_seed'
 AND v->'entity'=seed->'asset_ref')
 OR coalesce(proposal#>'{next_intent,view_request}','null'::jsonb)<>'null'::jsonb
 OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'contact_rules') rule WHERE rule->'asset_ref'=seed->'asset_ref'
 AND (rule->'rule'->>'exact'=seed->>'name' OR rule->'rule'->>'label_suffix'=seed->>'name'
 OR right(seed->>'name',length(rule->'rule'->>'label_suffix')+1)='.'||(rule->'rule'->>'label_suffix')))
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(p->'excluded_names') rule WHERE rule->>'exact'=seed->>'name'
 OR rule->>'label_suffix'=seed->>'name' OR right(seed->>'name',length(rule->>'label_suffix')+1)='.'||(rule->>'label_suffix'))
 OR NOT coalesce((p#>>'{campaign_limits,episodes}')::bigint>=1,false)
 OR NOT coalesce((p#>>'{campaign_limits,dns_questions}')::bigint>=1,false)
 THEN RAISE EXCEPTION 'live_effect_scope_refused'; END IF;
 -- The old fence remains idle: its generation advances but never becomes an effects permit.
 UPDATE execution.session_fences SET generation=generation+1,epoch=epoch+1
 WHERE engagement_id=a.e AND campaign_id=a.c AND operation_id IS NULL AND phase='idle'
 RETURNING generation INTO g;
 IF g IS NULL THEN RAISE EXCEPTION 'live_effect_session_busy'; END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 request:=jsonb_build_object('schema','m1_dns_ipc_v1','kind','REQUEST','attempt',a.attempt,
 'session_generation',g,'sequence',0,'engagement_id',a.e,'campaign_id',a.c,'decision_id',a.decision_one,
 'profile','dns-routing-v2','execution_mode','live','run_profile',a.request->'run_profile',
 'release_sha256',a.release_sha256,'qualification_ref',a.qualification_ref,'name',seed->'name',
 'resolver_ipv4',p->'resolver_ipv4','transport','udp','query_id',mod(get_byte(uuid_send(a.attempt),0)*256+
 get_byte(uuid_send(a.attempt),1),65536),'allowed_names',jsonb_build_array(seed->'name'),
 'deadline_at_ms',bound,'max_duration_ms',2000);
 INSERT INTO execution.live_effect_sessions VALUES(r,a.e,a.c,g,a.writer_oid,1,a.decision_one,
 a.attempt,a.result,request,a.qualification_ref,bound);
 INSERT INTO execution.live_effect_control VALUES(r,'reserved',false,bound,false);
 INSERT INTO execution.live_effect_events VALUES(r,g,'reserved',n);
 RETURN execution.live_effect_record(r);
END $$;
REVOKE ALL ON FUNCTION execution.live_effect_reserve(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.live_effect_reserve(uuid) TO dw_m1_broker;

CREATE FUNCTION execution.live_effect_current(r uuid,g bigint) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE s execution.live_effect_sessions;x execution.live_effect_control;bound bigint;
BEGIN
 SELECT * INTO s FROM execution.live_effect_sessions WHERE run_id=r;
 IF g IS NULL OR s.generation IS DISTINCT FROM g THEN RAISE EXCEPTION 'live_effect_identity_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(s.e::text||':'||s.c::text,0));
 SELECT * INTO x FROM execution.live_effect_control WHERE run_id=r;
 bound:=execution.private_live_deadline(r);
 IF x.phase NOT IN ('reserved','may_start') OR x.disposal_confirmed
 OR floor(extract(epoch FROM clock_timestamp())*1000)::bigint>=least(bound,x.attempt_deadline_at_ms)
 THEN RAISE EXCEPTION 'live_effect_stopped'; END IF;
 RETURN least(bound,x.attempt_deadline_at_ms);
END $$;
REVOKE ALL ON FUNCTION execution.live_effect_current(uuid,bigint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.live_effect_current(uuid,bigint) TO dw_m1_broker;

CREATE FUNCTION execution.live_effect_may_start(r uuid,g bigint) RETURNS bigint
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE bound bigint;n bigint;
BEGIN
 bound:=execution.live_effect_current(r,g);
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 -- Only the first marker selects the clock; retries/recovery cannot restart it.
 UPDATE execution.live_effect_control SET phase='may_start',possibly_started=true,
 attempt_deadline_at_ms=least(bound,n+2000)
 WHERE run_id=r AND phase='reserved' AND NOT possibly_started AND NOT disposal_confirmed;
 IF NOT FOUND THEN RAISE EXCEPTION 'live_effect_already_charged'; END IF;
 INSERT INTO execution.live_effect_events VALUES(r,g,'may_start',n);
 RETURN (SELECT attempt_deadline_at_ms FROM execution.live_effect_control WHERE run_id=r);
END $$;
REVOKE ALL ON FUNCTION execution.live_effect_may_start(uuid,bigint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.live_effect_may_start(uuid,bigint) TO dw_m1_broker;
