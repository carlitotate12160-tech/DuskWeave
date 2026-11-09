-- Every existing authoritative writer participates, including direct ordinary SQL.
CREATE FUNCTION execution.private_live_mutation_guard() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE payload jsonb; prior_payload jsonb; e uuid; c uuid; old_e uuid; old_c uuid; lock_key bigint;
 s execution.live_effect_sessions;
 x execution.live_effect_control; allowed boolean; control_op text;
BEGIN
 IF TG_OP='DELETE' THEN payload:=to_jsonb(OLD); ELSE payload:=to_jsonb(NEW); END IF;
 IF TG_OP='UPDATE' THEN prior_payload:=to_jsonb(OLD); END IF;
 IF payload ? 'run_id' THEN
  SELECT a.e,a.c INTO e,c FROM execution.autonomous_runs a WHERE a.run_id=(payload->>'run_id')::uuid;
 ELSE
  e:=coalesce(payload->>'engagement_id',payload->>'e')::uuid;
  c:=coalesce(payload->>'campaign_id',payload->>'c')::uuid;
 END IF;
 IF e IS NULL OR c IS NULL THEN RAISE EXCEPTION 'live_mutation_scope_refused'; END IF;
 IF prior_payload IS NOT NULL THEN
  IF prior_payload ? 'run_id' THEN
   SELECT a.e,a.c INTO old_e,old_c FROM execution.autonomous_runs a WHERE a.run_id=(prior_payload->>'run_id')::uuid;
  ELSE
   old_e:=coalesce(prior_payload->>'engagement_id',prior_payload->>'e')::uuid;
   old_c:=coalesce(prior_payload->>'campaign_id',prior_payload->>'c')::uuid;
  END IF;
 END IF;
 FOR lock_key IN SELECT DISTINCT k FROM unnest(ARRAY[
  hashtextextended(e::text||':'||c::text,0),hashtextextended(old_e::text||':'||old_c::text,0)]) k
  WHERE k IS NOT NULL ORDER BY k LOOP PERFORM pg_advisory_xact_lock(lock_key); END LOOP;
 IF (old_e,old_c) IS DISTINCT FROM (e,c) AND EXISTS(
  SELECT 1 FROM execution.live_effect_sessions a JOIN execution.live_effect_control b USING(run_id)
  WHERE (a.e,a.c)=(old_e,old_c) AND (b.phase<>'closed' OR (b.possibly_started AND NOT EXISTS(
   SELECT 1 FROM execution.live_effect_events z WHERE z.run_id=a.run_id AND z.phase='result_recorded'))))
 THEN RAISE EXCEPTION 'live_mutation_scope_refused'; END IF;
 SELECT a.* INTO s FROM execution.live_effect_sessions a
 WHERE (a.e,a.c)=(e,c) ORDER BY a.generation DESC LIMIT 1;
 IF s.run_id IS NULL THEN RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END; END IF;
 SELECT * INTO STRICT x FROM execution.live_effect_control b WHERE b.run_id=s.run_id;
 -- Historical uncertainty remains charged; the old prepared path cannot clear it.
 IF x.phase='closed' AND NOT (x.possibly_started AND NOT EXISTS(
  SELECT 1 FROM execution.live_effect_events z WHERE z.run_id=s.run_id AND z.phase='result_recorded'))
 THEN RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END; END IF;
 IF prior_payload IS NOT NULL AND (
 coalesce(prior_payload->>'engagement_id',prior_payload->>'e') IS DISTINCT FROM coalesce(payload->>'engagement_id',payload->>'e')
 OR coalesce(prior_payload->>'campaign_id',prior_payload->>'c') IS DISTINCT FROM coalesce(payload->>'campaign_id',payload->>'c')
 OR prior_payload->'run_id' IS DISTINCT FROM payload->'run_id')
 THEN RAISE EXCEPTION 'live_mutation_scope_refused'; END IF;
 SELECT w.operation INTO control_op FROM execution.private_live_writer_context w
 WHERE w.backend_pid=pg_backend_pid() AND w.transaction_id=pg_current_xact_id()
 AND (w.e,w.c,w.generation)=(s.e,s.c,s.generation)
 AND w.actor_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user);
 allowed:=control_op='withdraw';
 -- Existing Mission triggers stop the run and touch the legacy idle fence.
 -- The context is private to this withdrawal and admits only those restrictive writes.
 allowed:=allowed AND (
 (TG_TABLE_SCHEMA='mission' AND TG_TABLE_NAME='withdrawals' AND TG_OP='INSERT')
 OR (TG_TABLE_SCHEMA='execution' AND TG_TABLE_NAME='autonomous_control' AND TG_OP='UPDATE'
 AND payload-ARRAY['status','lease_until_ms']=prior_payload-ARRAY['status','lease_until_ms']
 AND payload->>'status'='stopped' AND payload->>'lease_until_ms'='0')
 OR (TG_TABLE_SCHEMA='execution' AND TG_TABLE_NAME='session_fences' AND (
 (TG_OP='INSERT' AND payload->>'operation_id' IS NULL AND payload->>'phase'='idle')
 OR (TG_OP='UPDATE' AND payload-'epoch'=prior_payload-'epoch'
 AND (payload->>'epoch')::bigint=(prior_payload->>'epoch')::bigint+1))));
 -- Existing run STOP/terminal-unknown functions may only revoke their own run.
 IF control_op IN ('stop_run','run_unknown')
 AND s.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user)
 AND execution.private_live_actor('dw_m1_broker') THEN
  allowed:=(
   TG_TABLE_SCHEMA='execution' AND TG_TABLE_NAME='autonomous_control' AND TG_OP='UPDATE'
   AND payload-ARRAY['status','lease_until_ms']=prior_payload-ARRAY['status','lease_until_ms']
   AND payload->>'status'=CASE control_op WHEN 'stop_run' THEN 'stopped' ELSE 'unknown' END
   AND payload->>'lease_until_ms'='0'
  ) OR (
   TG_TABLE_SCHEMA='execution' AND TG_TABLE_NAME='session_fences' AND TG_OP='UPDATE'
   AND payload-'epoch'=prior_payload-'epoch'
   AND (payload->>'epoch')::bigint=(prior_payload->>'epoch')::bigint+1
  );
 END IF;
 -- A committed result owns one immutable Terrain publication obligation.
 -- Publishing that exact result does not settle disposal or admit another effect.
 IF TG_TABLE_SCHEMA='terrain' AND TG_TABLE_NAME IN ('claim_revisions','current_claims')
 AND TG_OP IN ('INSERT','UPDATE') AND execution.private_live_actor('dw_m1_terrain')
 AND EXISTS(SELECT 1 FROM execution.autonomous_runs a JOIN execution.live_dns_results b USING(run_id)
  WHERE a.run_id=s.run_id AND b.result=s.result_ref AND b.attempt=s.attempt
  AND b.publication_obligation='terrain.issued_dns.v2'
  AND payload->>'operation_id'=a.terrain_operation::text)
 THEN allowed:=true; END IF;
 -- Conflicting replay persists an integrity hold through the validated command.
 -- Only the qualified Terrain owner may INSERT; the SECURITY DEFINER function
 -- enforces claim lineage, affected-projection locking and admission validation.
 IF TG_TABLE_SCHEMA='terrain' AND TG_TABLE_NAME='integrity_conflicts'
 AND TG_OP='INSERT' AND execution.private_live_actor('dw_m1_terrain')
 AND EXISTS(SELECT 1 FROM execution.autonomous_runs a JOIN execution.live_dns_results b USING(run_id)
  WHERE a.run_id=s.run_id AND b.result=s.result_ref AND b.attempt=s.attempt
  AND b.publication_obligation='terrain.issued_dns.v2'
  AND payload->>'operation_id'=a.terrain_operation::text)
 THEN allowed:=true; END IF;
 -- The parent may renew only its existing run lease, never any authority or phase.
 IF TG_TABLE_SCHEMA='execution' AND TG_TABLE_NAME='autonomous_control' AND TG_OP='UPDATE'
 AND s.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user)
 AND execution.private_live_actor('dw_m1_broker')
 AND payload-'lease_until_ms'=prior_payload-'lease_until_ms'
 AND (payload->>'lease_until_ms')::bigint<=(SELECT deadline_at_ms FROM execution.autonomous_runs WHERE run_id=s.run_id)
 THEN allowed:=true; END IF;
 IF NOT coalesce(allowed,false) THEN RAISE EXCEPTION 'active_live_authority_fence'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;
REVOKE ALL ON FUNCTION execution.private_live_mutation_guard() FROM PUBLIC;

DO $$
DECLARE target text;
BEGIN
 FOREACH target IN ARRAY ARRAY['mission.missions','mission.registration_outbox','mission.planning_assessments',
 'mission.withdrawals','execution.session_fences','execution.inference_reservations',
 'execution.inference_calls','execution.native_input_bindings',
 'execution.autonomous_control','terrain.current_claims','terrain.claim_revisions',
 'terrain.integrity_conflicts'] LOOP
  EXECUTE format('CREATE TRIGGER live_authority_fence BEFORE INSERT OR UPDATE OR DELETE ON %s FOR EACH ROW EXECUTE FUNCTION execution.private_live_mutation_guard()',target);
 END LOOP;
END $$;

CREATE OR REPLACE FUNCTION mission.live_stop_withdrawal(q jsonb,op uuid,g bigint,recover boolean) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE e uuid;c uuid;s execution.live_effect_sessions;m mission.missions;prior mission.withdrawals;
 event uuid;n bigint;contract jsonb;
BEGIN
 IF NOT execution.private_live_actor('dw_m1_stop_writer') OR (g IS NOT NULL AND g<1) OR op IS NULL
 OR op='00000000-0000-0000-0000-000000000000'::uuid OR jsonb_typeof(q) IS DISTINCT FROM 'object'
 OR q-ARRAY['engagement_id','campaign_id','operator_ref','expected_mission_revision','reason']<>'{}'
 OR NOT q ?& ARRAY['engagement_id','campaign_id','operator_ref','expected_mission_revision','reason']
 OR jsonb_typeof(q->'expected_mission_revision') IS DISTINCT FROM 'number'
 OR q->>'expected_mission_revision' IS DISTINCT FROM '1'
 OR jsonb_typeof(q->'reason') IS DISTINCT FROM 'string'
 OR NOT coalesce(q->>'reason' IN ('operator_requested','authorization_ended','scope_concern'),false)
 OR EXISTS(SELECT 1 FROM unnest(ARRAY['engagement_id','campaign_id','operator_ref']) k
 WHERE jsonb_typeof(q->k) IS DISTINCT FROM 'string'
 OR (q->>k)!~'^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
 OR q->>k='00000000-0000-0000-0000-000000000000')
 THEN RAISE EXCEPTION 'live_withdrawal_refused'; END IF;
 e:=(q->>'engagement_id')::uuid;c:=(q->>'campaign_id')::uuid;
 PERFORM pg_advisory_xact_lock(hashtextextended(e::text||':'||c::text,0));
 -- NULL g resolves the current session under this fence; an explicit
 -- generation must still exist. No session is ever manufactured.
 IF g IS NULL THEN
  SELECT * INTO s FROM execution.live_effect_sessions a WHERE (a.e,a.c)=(e,c)
  ORDER BY a.generation DESC LIMIT 1;
 ELSE
  SELECT * INTO s FROM execution.live_effect_sessions a WHERE (a.e,a.c,a.generation)=(e,c,g);
 END IF;
 IF s.run_id IS NULL AND g IS NOT NULL THEN RAISE EXCEPTION 'live_withdrawal_refused'; END IF;
 SELECT * INTO prior FROM mission.withdrawals a WHERE (a.engagement_id,a.campaign_id)=(e,c);
 IF FOUND THEN
  IF prior.operation_id<>op OR prior.contract->'request' IS DISTINCT FROM q THEN RAISE EXCEPTION 'live_withdrawal_conflict'; END IF;
  RETURN prior.contract;
 END IF;
 IF recover THEN RETURN NULL; END IF;
 SELECT * INTO m FROM mission.missions a WHERE (a.engagement_id,a.campaign_id)=(e,c);
 IF m.operation_id IS NULL OR m.revision<>1 OR m.operator_ref::text IS DISTINCT FROM q->>'operator_ref'
 OR EXISTS(SELECT 1 FROM mission.registration_outbox a WHERE (a.engagement_id,a.campaign_id,a.operation_id)=(e,c,op))
 OR EXISTS(SELECT 1 FROM mission.planning_assessments a WHERE (a.engagement_id,a.campaign_id,a.operation_id)=(e,c,op))
 OR NOT EXISTS(SELECT 1 FROM mission.registration_outbox a WHERE (a.engagement_id,a.campaign_id,a.operation_id)=(e,c,m.operation_id)
  AND jsonb_typeof(a.contract#>'{fields,m1_inference_permission,live_binding}')='object')
 THEN RAISE EXCEPTION 'live_withdrawal_refused'; END IF;
 event:=gen_random_uuid();n:=floor(extract(epoch FROM transaction_timestamp()))::bigint;
 contract:=jsonb_build_object('event_id',event,'operation_id',op,'request',q,'registration_operation_id',m.operation_id,
 'producer','mission','affected_entity',c,'owner_revision',2,'kind','mission_authority_withdrawn','version',1,
 'causation_id',op,'correlation_id',op,'occurred_at',n,'recorded_at',n);
 -- The writer context exists only to pass the active-session fence; with no
 -- session there is no fence to pass, so no context or session is invented.
 IF s.run_id IS NOT NULL THEN
  INSERT INTO execution.private_live_writer_context VALUES(pg_backend_pid(),pg_current_xact_id(),e,c,s.generation,
  (SELECT oid FROM pg_roles WHERE rolname=session_user),'withdraw');
 END IF;
 INSERT INTO mission.withdrawals(engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract)
 VALUES(e,c,op,event,m.operation_id,contract);
 DELETE FROM execution.private_live_writer_context w WHERE w.backend_pid=pg_backend_pid()
 AND w.transaction_id=pg_current_xact_id() AND (w.e,w.c)=(e,c);
 RETURN contract;
END $$;
REVOKE ALL ON FUNCTION mission.live_stop_withdrawal(jsonb,uuid,bigint,boolean) FROM PUBLIC;
GRANT USAGE ON SCHEMA mission TO dw_m1_stop_writer;
GRANT EXECUTE ON FUNCTION mission.live_stop_withdrawal(jsonb,uuid,bigint,boolean) TO dw_m1_stop_writer;

-- Stop-only metadata; absence is not permission to create or release a session.
CREATE FUNCTION execution.live_stop_generation(e uuid,c uuid) RETURNS bigint
LANGUAGE plpgsql STABLE SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
BEGIN
 IF NOT execution.private_live_actor('dw_m1_stop_writer')
 OR e IS NULL OR c IS NULL
 OR e='00000000-0000-0000-0000-000000000000'::uuid
 OR c='00000000-0000-0000-0000-000000000000'::uuid
 THEN RAISE EXCEPTION 'live_stop_metadata_refused'; END IF;
 RETURN (SELECT max(s.generation) FROM execution.live_effect_sessions s WHERE (s.e,s.c)=(e,c));
END $$;
REVOKE ALL ON FUNCTION execution.live_stop_generation(uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.live_stop_generation(uuid,uuid) TO dw_m1_stop_writer;

GRANT USAGE ON SCHEMA execution TO dw_m1_stop_writer;
