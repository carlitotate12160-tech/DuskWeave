-- Safe terminal observations are accountable metadata, never effect authority.
CREATE TABLE execution.run_phase_diagnostics (
 run_id uuid NOT NULL REFERENCES execution.autonomous_runs,
 phase text NOT NULL CHECK(phase IN ('decision_one','acquisition','decision_two')),
 payload jsonb NOT NULL CHECK(octet_length(payload::text)<=1024),
 PRIMARY KEY(run_id,phase));
REVOKE ALL ON execution.run_phase_diagnostics FROM PUBLIC,dw_runtime,dw_m1_broker,dw_m1_cognition,dw_m1_trajectory,dw_m1_terrain;

CREATE FUNCTION execution.run_phase_diagnostic(q jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE r uuid; p text; old jsonb;
BEGIN
 IF jsonb_typeof(q) IS DISTINCT FROM 'object' OR octet_length(q::text)>1024 THEN
  RAISE EXCEPTION 'run_diagnostic_refused'; END IF;
 IF NOT q ?& ARRAY['version','run_id','phase','cause','cause_source','child_error','exit_code','readiness_observed','elapsed_ms']
 OR q-ARRAY['version','run_id','phase','cause','cause_source','child_error','exit_code','readiness_observed','elapsed_ms']<>'{}'
 OR q->>'version' IS DISTINCT FROM 'm1_run_phase_diagnostic_v1'
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
 IF execution.run_record(r) IS NULL OR NOT EXISTS(
  SELECT 1 FROM execution.autonomous_phases WHERE run_id=r AND phase=p AND status='started')
 THEN RAISE EXCEPTION 'run_diagnostic_writer_refused'; END IF;
 INSERT INTO execution.run_phase_diagnostics VALUES(r,p,q) ON CONFLICT DO NOTHING;
 SELECT payload INTO old FROM execution.run_phase_diagnostics WHERE run_id=r AND phase=p;
 IF old IS DISTINCT FROM q THEN RAISE EXCEPTION 'run_diagnostic_conflict'; END IF;
END $$;
REVOKE ALL ON FUNCTION execution.run_phase_diagnostic(jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.run_phase_diagnostic(jsonb) TO dw_m1_broker;

-- Keep the existing receipt producer; add only the diagnostic projection.
-- Refreshing 0019 before 0020 must not rename/replace the preserved producer again.
DO $$ BEGIN
 IF to_regprocedure('execution.run_summary_without_diagnostics(uuid)') IS NULL THEN
  ALTER FUNCTION execution.run_summary(uuid) RENAME TO run_summary_without_diagnostics;
 END IF;
END $$;
CREATE OR REPLACE FUNCTION execution.run_summary(r uuid) RETURNS jsonb
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE summary jsonb; diagnostics jsonb;
BEGIN
 summary:=execution.run_summary_without_diagnostics(r);
 SELECT coalesce(jsonb_agg(payload ORDER BY phase),'[]') INTO diagnostics
 FROM execution.run_phase_diagnostics WHERE run_id=r;
 RETURN summary || jsonb_build_object('diagnostics',diagnostics,'diagnostic_publication',
  CASE WHEN diagnostics='[]'::jsonb OR EXISTS(
   SELECT 1 FROM execution.autonomous_phases p WHERE p.run_id=r AND p.status='started'
   AND NOT EXISTS(SELECT 1 FROM execution.run_phase_diagnostics d WHERE (d.run_id,d.phase)=(p.run_id,p.phase)))
   THEN 'unavailable' ELSE 'committed' END);
END $$;
REVOKE ALL ON FUNCTION execution.run_summary(uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.run_summary(uuid) TO dw_m1_broker;
