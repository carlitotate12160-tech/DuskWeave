-- DW-AUD-007: a SECURITY DEFINER SET search_path=pg_catalog function still
-- resolves unqualified relation/type identifiers through the caller's pg_temp
-- first; a TEMP-eligible restricted caller could shadow pg_roles and defeat
-- original-writer isolation and role qualification. Listing pg_temp
-- explicitly after pg_catalog restores safe catalog resolution for the exact
-- inspected set. Bodies, OIDs, owners, ACLs, security mode, volatility and
-- every other proconfig entry are unchanged.
BEGIN;
ALTER FUNCTION execution.bind_inference_input(uuid,uuid,uuid,text) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.close_fixture_session(uuid,text) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.close_inference(uuid,uuid,uuid) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.guard_mission_writer() SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.inference_call_state(uuid,text,jsonb) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.inference_may_send(uuid,uuid,uuid,uuid,text,text,text,text) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.inference_reservation(text,uuid,jsonb,jsonb) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.inference_window_ms(uuid,uuid,uuid) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.live_dns_result(uuid,bigint,jsonb) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.live_effect_record(uuid) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.live_effect_reserve(uuid) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.live_effect_stop(uuid,bigint,boolean) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.prepare_session(uuid,uuid,uuid,uuid,bigint,jsonb) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.private_live_actor(text) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.private_live_control_scope(uuid,text) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.private_live_deadline(uuid) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.private_live_mutation_guard() SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.release_prepared_session(uuid,uuid,uuid,uuid,bigint) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.run_current(uuid) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.run_record(uuid) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.run_start(jsonb,text) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION execution.withdraw_prepared_session(uuid,uuid,uuid,uuid,bigint,uuid,bigint,text,jsonb) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION mission.live_stop_withdrawal(jsonb,uuid,bigint,boolean) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION terrain.admit_dns_request(jsonb) SET search_path = pg_catalog, pg_temp;
ALTER FUNCTION terrain.current_dns_context_view(jsonb) SET search_path = pg_catalog, pg_temp;
COMMIT;
