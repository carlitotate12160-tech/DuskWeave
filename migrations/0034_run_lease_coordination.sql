-- DW-M1-FIX-HEARTBEAT-FENCE: positive liveness-only renewal of an already-running,
-- unexpired autonomous run is not an authority mutation. Replace private_live_mutation_guard
-- to validate run lease renewals before campaign advisory locking, and replace run_renew
-- to update lease monotonically without holding the campaign fence.
CREATE OR REPLACE FUNCTION execution.private_live_mutation_guard() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
#variable_conflict use_variable
DECLARE payload jsonb; prior_payload jsonb; e uuid; c uuid; old_e uuid; old_c uuid; lock_key bigint;
 s execution.live_effect_sessions;
 x execution.live_effect_control; allowed boolean; control_op text;
 r_run execution.autonomous_runs; now_ms bigint;
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

 -- Classify only positive liveness renewal before the generic campaign advisory lock loop.
 IF TG_TABLE_SCHEMA='execution' AND TG_TABLE_NAME='autonomous_control' AND TG_OP='UPDATE'
 AND (payload->>'lease_until_ms')::bigint > 0
 AND payload - 'lease_until_ms' = prior_payload - 'lease_until_ms' THEN
  SELECT a.* INTO r_run FROM execution.autonomous_runs a WHERE a.run_id=(payload->>'run_id')::uuid;
  IF r_run.run_id IS NULL OR (r_run.e, r_run.c) IS DISTINCT FROM (e, c)
  OR r_run.writer_oid IS DISTINCT FROM (SELECT oid FROM pg_catalog.pg_roles WHERE rolname = session_user)
  OR NOT execution.private_live_actor('dw_m1_broker')
  OR prior_payload->>'status' IS DISTINCT FROM 'running'
  OR payload->>'status' IS DISTINCT FROM 'running' THEN
   RAISE EXCEPTION 'run_not_current';
  END IF;
  now_ms := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
  IF (prior_payload->>'lease_until_ms')::bigint <= now_ms
  OR r_run.deadline_at_ms <= now_ms
  OR (payload->>'lease_until_ms')::bigint < (prior_payload->>'lease_until_ms')::bigint
  OR (payload->>'lease_until_ms')::bigint > least(now_ms + 2000, r_run.deadline_at_ms) THEN
   RAISE EXCEPTION 'run_not_current';
  END IF;
  RETURN NEW;
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
 AND w.actor_oid=(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user);
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
 AND s.writer_oid=(SELECT oid FROM pg_catalog.pg_roles WHERE rolname=session_user)
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
 IF NOT coalesce(allowed,false) THEN RAISE EXCEPTION 'active_live_authority_fence'; END IF;
 RETURN CASE WHEN TG_OP='DELETE' THEN OLD ELSE NEW END;
END $$;

CREATE OR REPLACE FUNCTION execution.run_renew(r uuid) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog, pg_temp AS $$
DECLARE n bigint; d bigint; updated_count int;
BEGIN
 PERFORM execution.run_current(r);
 SELECT a.deadline_at_ms INTO d FROM execution.autonomous_runs a WHERE a.run_id=r;
 IF d IS NULL THEN RAISE EXCEPTION 'run_not_current'; END IF;
 n:=floor(extract(epoch FROM clock_timestamp())*1000)::bigint;
 IF n >= d THEN RAISE EXCEPTION 'run_not_current'; END IF;
 UPDATE execution.autonomous_control
  SET lease_until_ms=greatest(lease_until_ms, least(n+2000, d))
  WHERE run_id=r AND status='running' AND lease_until_ms > n;
 GET DIAGNOSTICS updated_count = ROW_COUNT;
 IF updated_count = 0 THEN RAISE EXCEPTION 'run_not_current'; END IF;
END $$;
