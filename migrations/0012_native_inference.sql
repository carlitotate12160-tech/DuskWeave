-- Native ownership; no fixture, probe account, synthetic target or provider key.
CREATE SCHEMA cognition;
DO $$ BEGIN
 IF NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='dw_m1_cognition') THEN
  CREATE ROLE dw_m1_cognition NOLOGIN NOSUPERUSER NOBYPASSRLS;
 END IF;
 IF NOT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='dw_m1_trajectory') THEN
  CREATE ROLE dw_m1_trajectory NOLOGIN NOSUPERUSER NOBYPASSRLS;
 END IF;
END $$;
CREATE TABLE execution.inference_calls (
 e uuid NOT NULL,c uuid NOT NULL,d uuid NOT NULL,call uuid PRIMARY KEY,
 binding text NOT NULL,p text NOT NULL,configuration text NOT NULL,body text NOT NULL,
 state text NOT NULL CHECK(state IN ('may_send_uncertain','sent_awaiting_result','completed','cancelled_uncertain','failed_never_released','usage_unknown')),
 usage jsonb,UNIQUE(e,c,d,binding)
);
CREATE TABLE execution.inference_closures(e uuid,c uuid,d uuid,PRIMARY KEY(e,c,d),recorded_at timestamptz NOT NULL DEFAULT clock_timestamp());
CREATE TABLE cognition.reasoning_decisions (
 e uuid NOT NULL,c uuid NOT NULL,d uuid NOT NULL,event uuid NOT NULL UNIQUE,
 input_hash text NOT NULL,payload jsonb NOT NULL,PRIMARY KEY(e,c,d),recorded_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE cognition.reasoning_outbox(event uuid PRIMARY KEY REFERENCES cognition.reasoning_decisions(event),obligation text NOT NULL CHECK(obligation='trajectory.reasoning_history.v1'));
CREATE TABLE trajectory.reasoning_history (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL,
    producer text NOT NULL DEFAULT 'cognition' CHECK (producer = 'cognition'),
    kind text NOT NULL DEFAULT 'reasoning_decision' CHECK (kind = 'reasoning_decision'),
    version integer NOT NULL DEFAULT 1 CHECK (version = 1),
    obligation text NOT NULL DEFAULT 'trajectory.reasoning_history.v1'
        CHECK (obligation = 'trajectory.reasoning_history.v1'),
    status text NOT NULL CHECK (status IN ('accepted', 'anomaly')),
    contract jsonb,
    anomaly_category text,
    recorded_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
    completed_at timestamptz,
    CHECK ((status = 'accepted' AND jsonb_typeof(contract) = 'object'
            AND contract IS NOT NULL AND completed_at IS NOT NULL AND anomaly_category IS NULL)
        OR (status = 'anomaly' AND contract IS NULL AND completed_at IS NULL
            AND anomaly_category = 'conflicting_identity' AND anomaly_category IS NOT NULL)),
    PRIMARY KEY (engagement_id, campaign_id, event_id, operation_id, status)
);

-- Accepted rows deduplicate each scoped identity axis. Anomaly markers
-- deduplicate only on the exact conflicting pair, so a distinct pair
-- sharing one axis is durably retained and still observed by either axis.
CREATE UNIQUE INDEX reasoning_history_accepted_event
    ON trajectory.reasoning_history (engagement_id, campaign_id, event_id)
    WHERE status = 'accepted';
CREATE UNIQUE INDEX reasoning_history_accepted_operation
    ON trajectory.reasoning_history (engagement_id, campaign_id, operation_id)
    WHERE status = 'accepted';


GRANT USAGE ON SCHEMA cognition TO dw_m1_broker,dw_m1_cognition,dw_runtime,dw_m1_trajectory;
GRANT USAGE ON SCHEMA trajectory TO dw_m1_trajectory;
GRANT SELECT ON cognition.reasoning_decisions,cognition.reasoning_outbox TO dw_m1_trajectory;
GRANT SELECT ON cognition.reasoning_decisions,cognition.reasoning_outbox,execution.inference_calls,execution.inference_closures TO dw_m1_broker,dw_m1_cognition,dw_runtime;
GRANT SELECT,INSERT ON trajectory.reasoning_history TO dw_m1_trajectory;
CREATE FUNCTION execution.inference_may_send(e uuid,c uuid,d uuid,k uuid,p text,cfg text,b text,h text)
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
 OR k NOT IN (r.generation_call_id,r.repair_call_id)
 OR EXISTS(SELECT 1 FROM execution.inference_reservations x
 WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,d) AND x.record_revision=2)
 THEN
 RAISE EXCEPTION 'inference_reservation_refused'; END IF;
 s:=mission.current_inference_publication(e,c,d);perm:=s->'fields'->'m1_inference_permission';
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
REVOKE ALL ON FUNCTION execution.inference_may_send(uuid,uuid,uuid,uuid,text,text,text,text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.inference_may_send(uuid,uuid,uuid,uuid,text,text,text,text) TO dw_m1_broker;

CREATE FUNCTION execution.inference_call_state(k uuid,wanted text,u jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable

BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR wanted NOT IN
 ('sent_awaiting_result','completed','cancelled_uncertain','failed_never_released','usage_unknown')
 THEN RAISE EXCEPTION 'inference_state_refused'; END IF;
 IF wanted IN ('usage_unknown','cancelled_uncertain') AND EXISTS(
  SELECT 1 FROM execution.inference_calls WHERE call=k AND state IN ('cancelled_uncertain','usage_unknown')
 ) THEN
  IF wanted='cancelled_uncertain' THEN
   UPDATE execution.inference_calls SET state='cancelled_uncertain',usage=NULL WHERE call=k;
  END IF;
  RETURN;
 END IF;
 UPDATE execution.inference_calls SET state=wanted,usage=u WHERE call=k
 AND ((wanted='failed_never_released' AND state='may_send_uncertain')
  OR (wanted<>'failed_never_released' AND state IN ('may_send_uncertain','sent_awaiting_result')));
 IF NOT FOUND THEN RAISE EXCEPTION 'inference_state_conflict'; END IF;
END $$;
REVOKE ALL ON FUNCTION execution.inference_call_state(uuid,text,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.inference_call_state(uuid,text,jsonb) TO dw_m1_broker;

-- A legacy prepared-only cancel must not refund a possibly sent probe call.
CREATE FUNCTION execution.inference_no_false_refund() RETURNS trigger LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
 BEGIN
 IF NEW.record_revision=2 AND EXISTS(SELECT 1 FROM execution.inference_calls x
 WHERE (x.e,x.c,x.d)=(NEW.engagement_id,NEW.campaign_id,NEW.operation_id)
 AND x.state<>'failed_never_released') THEN RAISE EXCEPTION 'inference_possible_send_no_refund'; END IF;
 RETURN NEW; END $$;
CREATE TRIGGER inference_no_false_refund BEFORE INSERT ON execution.inference_reservations
FOR EACH ROW EXECUTE FUNCTION execution.inference_no_false_refund();


-- Cognition publication and Trajectory committed-source port.
CREATE FUNCTION cognition.publish_reasoning(x jsonb,input_hash text) RETURNS void LANGUAGE plpgsql
SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable

DECLARE existing jsonb;
BEGIN
 IF NOT pg_has_role(session_user,'dw_m1_cognition','MEMBER') THEN RAISE EXCEPTION 'inference_actor_refused'; END IF;
 IF jsonb_typeof(x) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF NOT x ?& ARRAY['kind','occurred_at_ms','recorded_at_ms','source_mode','provenance','event',
 'engagement','campaign','decision','producer','version','mission_revision','permission',
 'call_ids','bindings','proposal','disposition','publication_obligation']
 OR (SELECT count(*) FROM jsonb_object_keys(x))<>18
 OR x->>'kind' IS DISTINCT FROM 'reasoning_decision'
 OR (x->>'source_mode' IN ('blind','defender_informed')) IS DISTINCT FROM true
 OR jsonb_typeof(x->'provenance') NOT IN ('array','null')
 OR EXISTS(SELECT 1 FROM unnest(ARRAY['occurred_at_ms','recorded_at_ms']) f
 WHERE jsonb_typeof(x->f) IS DISTINCT FROM 'number' OR x->>f !~ '^[0-9]+$'
 OR (x->>f)::numeric NOT BETWEEN 0 AND 18446744073709551615)
 THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF x->>'producer' IS DISTINCT FROM 'cognition'
 OR x->'version' IS DISTINCT FROM '1'::jsonb
 OR x->>'publication_obligation' IS DISTINCT FROM 'trajectory.reasoning_history.v1'
 OR jsonb_typeof(x->'producer') IS DISTINCT FROM 'string'
 OR jsonb_typeof(x->'publication_obligation') IS DISTINCT FROM 'string'
 OR jsonb_typeof(x->'mission_revision') IS DISTINCT FROM 'number'
 OR jsonb_typeof(x->'call_ids') IS DISTINCT FROM 'array'
 OR jsonb_typeof(x->'bindings') IS DISTINCT FROM 'array'
 OR jsonb_typeof(x->'disposition') IS DISTINCT FROM 'string'
 OR NOT (x ? 'proposal') THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF x->>'mission_revision' !~ '^[0-9]+$' OR (x->>'mission_revision')::numeric NOT BETWEEN 1 AND 9223372036854775807
 OR x->>'disposition' NOT IN (
  'validated_proposal','schema_repair_exhausted','unsafe_explanation','unsupported_premise',
  'reference_ineligible','manifest_ineligible','view_selector_ineligible','current_authority_unavailable',
  'repair_code_ineligible','repair_code_limit','pack_size','pack_utf8',
  'repair_encode','request_encode','request_size','inference_marker_commit_unknown',
  'inference_marker_refused','inference_marker_unknown','inference_sql_ambiguous','inference_sql_privilege',
  'inference_sql_function_missing','stopped_before_release','inference_stop_unknown','inference_authority_unavailable',
  'inference_reservation_unknown','inference_reservation_missing','inference_deadline','inference_guard_missing',
  'inference_platform_unqualified','inference_guard_busy','inference_fixture_endpoint_refused','live_key_missing',
  'live_key_invalid','inference_curl_missing','inference_pack_encode','inference_config_encode',
  'inference_body_encode','inference_usage_encode','inference_accounting_unknown','inference_database_config_missing',
  'inference_database_config_invalid','inference_database_unavailable','provider_incomplete','provider_protocol',
  'provider_unsafe_response','provider_response_limit','provider_transport_unknown','provider_cancelled_uncertain',
  'accounting_unknown','inference_release_unknown','inference_transport_unavailable',
  'inference_inspect_unavailable') THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF EXISTS(SELECT 1 FROM unnest(ARRAY['event','engagement','campaign','decision','permission']) f
 WHERE jsonb_typeof(x->f) IS DISTINCT FROM 'string'
 OR (x->>f) IS NULL OR (x->>f) !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(x->'call_ids') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string'
 OR v#>>'{}' !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(x->'bindings') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
 THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 IF x->>'disposition'='validated_proposal' THEN
  IF jsonb_typeof(x->'proposal') IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 ELSE
  IF jsonb_typeof(x->'proposal') IS DISTINCT FROM 'null' THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 END IF;
 SELECT payload INTO existing FROM cognition.reasoning_decisions z WHERE
 (z.e,z.c,z.d)=((x->>'engagement')::uuid,(x->>'campaign')::uuid,(x->>'decision')::uuid);
 IF FOUND THEN
 IF existing IS DISTINCT FROM x THEN RAISE EXCEPTION 'inference_identity_conflict'; END IF;
 RETURN; END IF;
 INSERT INTO cognition.reasoning_decisions VALUES((x->>'engagement')::uuid,(x->>'campaign')::uuid,
 (x->>'decision')::uuid,(x->>'event')::uuid,input_hash,x);
 INSERT INTO cognition.reasoning_outbox VALUES((x->>'event')::uuid,'trajectory.reasoning_history.v1');
END $$;
REVOKE ALL ON FUNCTION cognition.publish_reasoning(jsonb,text) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION cognition.publish_reasoning(jsonb,text) TO dw_m1_cognition;


CREATE FUNCTION cognition.reasoning_source_matches(k uuid,x jsonb) RETURNS boolean
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable
DECLARE existing jsonb;s jsonb;r execution.inference_reservations;e uuid;c uuid;d uuid;
BEGIN

 SELECT payload INTO s FROM cognition.reasoning_decisions z JOIN cognition.reasoning_outbox o USING(event) WHERE z.event=k;
 IF s IS NULL OR s IS DISTINCT FROM x THEN RAISE EXCEPTION 'inference_source_unavailable'; END IF;
 IF s->>'kind' IS DISTINCT FROM 'reasoning_decision' OR s->>'source_mode' NOT IN ('blind','defender_informed') OR jsonb_typeof(s->'occurred_at_ms') IS DISTINCT FROM 'number' OR jsonb_typeof(s->'recorded_at_ms') IS DISTINCT FROM 'number' OR jsonb_typeof(s->'provenance') NOT IN ('array','null') THEN RAISE EXCEPTION 'inference_binding_conflict';END IF;
 IF jsonb_typeof(s) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF s->>'producer' IS DISTINCT FROM 'cognition'
 OR s->'version' IS DISTINCT FROM '1'::jsonb
 OR s->>'publication_obligation' IS DISTINCT FROM 'trajectory.reasoning_history.v1'
 OR jsonb_typeof(s->'producer') IS DISTINCT FROM 'string'
 OR jsonb_typeof(s->'publication_obligation') IS DISTINCT FROM 'string'
 OR jsonb_typeof(s->'mission_revision') IS DISTINCT FROM 'number'
 OR jsonb_typeof(s->'call_ids') IS DISTINCT FROM 'array'
 OR jsonb_typeof(s->'bindings') IS DISTINCT FROM 'array'
 OR jsonb_typeof(s->'disposition') IS DISTINCT FROM 'string'
 OR NOT (s ? 'proposal') THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF s->>'mission_revision' !~ '^[0-9]+$' OR (s->>'mission_revision')::numeric NOT BETWEEN 1 AND 9223372036854775807
 OR s->>'disposition' NOT IN (
  'validated_proposal','schema_repair_exhausted','unsafe_explanation','unsupported_premise',
  'reference_ineligible','manifest_ineligible','view_selector_ineligible','current_authority_unavailable',
  'repair_code_ineligible','repair_code_limit','pack_size','pack_utf8',
  'repair_encode','request_encode','request_size','inference_marker_commit_unknown',
  'inference_marker_refused','inference_marker_unknown','inference_sql_ambiguous','inference_sql_privilege',
  'inference_sql_function_missing','stopped_before_release','inference_stop_unknown','inference_authority_unavailable',
  'inference_reservation_unknown','inference_reservation_missing','inference_deadline','inference_guard_missing',
  'inference_platform_unqualified','inference_guard_busy','inference_fixture_endpoint_refused','live_key_missing',
  'live_key_invalid','inference_curl_missing','inference_pack_encode','inference_config_encode',
  'inference_body_encode','inference_usage_encode','inference_accounting_unknown','inference_database_config_missing',
  'inference_database_config_invalid','inference_database_unavailable','provider_incomplete','provider_protocol',
  'provider_unsafe_response','provider_response_limit','provider_transport_unknown','provider_cancelled_uncertain',
  'accounting_unknown','inference_release_unknown','inference_transport_unavailable',
  'inference_inspect_unavailable') THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF EXISTS(SELECT 1 FROM unnest(ARRAY['event','engagement','campaign','decision','permission']) f
 WHERE jsonb_typeof(s->f) IS DISTINCT FROM 'string'
 OR (s->>f) IS NULL OR (s->>f) !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(s->'call_ids') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string'
 OR v#>>'{}' !~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$')
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(s->'bindings') v WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF s->>'disposition'='validated_proposal' THEN
  IF jsonb_typeof(s->'proposal') IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 ELSE
  IF jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 END IF;
 e:=(s->>'engagement')::uuid;c:=(s->>'campaign')::uuid;d:=(s->>'decision')::uuid;
 IF NOT EXISTS(SELECT 1 FROM mission.registration_outbox m JOIN trajectory.registration_history h
  ON (h.engagement_id,h.campaign_id,h.operation_id,h.event_id)=(m.engagement_id,m.campaign_id,m.operation_id,m.event_id)
  WHERE (m.engagement_id,m.campaign_id)=(e,c) AND h.status='accepted'
  AND (m.contract->>'owner_revision')::bigint=(s->>'mission_revision')::bigint
  AND m.contract->'fields'->'m1_inference_permission'->>'permission_id'=s->>'permission')
  OR EXISTS(SELECT 1 FROM trajectory.registration_history h WHERE (h.engagement_id,h.campaign_id)=(e,c) AND h.status='anomaly')
 THEN RAISE EXCEPTION 'inference_predecessor_missing'; END IF;
 IF jsonb_typeof(s->'call_ids') IS DISTINCT FROM 'array' OR jsonb_typeof(s->'bindings') IS DISTINCT FROM 'array'
  OR (CASE WHEN s->>'disposition'='validated_proposal' THEN jsonb_typeof(s->'proposal') IS DISTINCT FROM 'object' OR jsonb_array_length(s->'bindings') NOT BETWEEN 1 AND 2
   WHEN s->>'disposition'='schema_repair_exhausted' THEN jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' OR jsonb_array_length(s->'bindings')<>2
   ELSE jsonb_typeof(s->'proposal') IS DISTINCT FROM 'null' END)
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 IF jsonb_array_length(s->'call_ids')=0 THEN
  IF jsonb_array_length(s->'bindings')<>0 OR EXISTS(SELECT 1 FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d))
  THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 ELSE
  SELECT * INTO r FROM execution.inference_reservations y WHERE (y.engagement_id,y.campaign_id,y.operation_id)=(e,c,d) AND y.record_revision=1;
  IF r.operation_id IS NULL OR s->'call_ids'<>jsonb_build_array(r.generation_call_id,r.repair_call_id)
   OR (SELECT coalesce(jsonb_agg(a.binding ORDER BY CASE WHEN a.call=r.generation_call_id THEN 1 WHEN a.call=r.repair_call_id THEN 2 ELSE 3 END),'[]'::jsonb)
    FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d))<>s->'bindings'
   OR EXISTS(SELECT 1 FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d) AND a.call NOT IN (r.generation_call_id,r.repair_call_id))
  THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 END IF;
 IF EXISTS(SELECT 1 FROM execution.inference_calls a WHERE (a.e,a.c,a.d)=(e,c,d)
 AND encode(sha256(convert_to('dw-inference-binding-v1','UTF8')||decode('00','hex')||
 int8send(octet_length(a.p)::bigint)||convert_to(a.p,'UTF8')||
 int8send(octet_length(a.configuration)::bigint)||convert_to(a.configuration,'UTF8')||
 int8send(octet_length(a.body)::bigint)||convert_to(a.body,'UTF8')),'hex')<>a.binding)
 THEN RAISE EXCEPTION 'inference_binding_conflict'; END IF;
 RETURN true;
END $$;
REVOKE ALL ON FUNCTION cognition.reasoning_source_matches(uuid,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION cognition.reasoning_source_matches(uuid,jsonb) TO dw_m1_trajectory;

-- The history login cannot bypass the committed-source consumer with raw SQL.
CREATE FUNCTION trajectory.guard_reasoning_source() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
 IF NEW.status='accepted' THEN
  IF (NEW.contract->>'engagement',NEW.contract->>'campaign',NEW.contract->>'decision',NEW.contract->>'event')
   IS DISTINCT FROM (NEW.engagement_id::text,NEW.campaign_id::text,NEW.operation_id::text,NEW.event_id::text)
   OR NOT cognition.reasoning_source_matches(NEW.event_id,NEW.contract)
  THEN RAISE EXCEPTION 'inference_source_refused'; END IF;
 END IF;
 RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION trajectory.guard_reasoning_source() FROM PUBLIC;
CREATE TRIGGER reasoning_source_guard BEFORE INSERT ON trajectory.reasoning_history
FOR EACH ROW EXECUTE FUNCTION trajectory.guard_reasoning_source();
