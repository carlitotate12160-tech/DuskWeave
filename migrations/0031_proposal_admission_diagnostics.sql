-- Bounded proposal admission evidence; legacy absent metadata is unchanged.
CREATE OR REPLACE FUNCTION execution.inference_call_state(k uuid,wanted text,u jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
#variable_conflict use_variable

DECLARE owner_call execution.inference_calls; field text; metadata jsonb; available integer; diag jsonb;
BEGIN
 SELECT * INTO owner_call FROM execution.inference_calls x WHERE x.call=k;
 IF NOT EXISTS(SELECT 1 FROM execution.inference_reservations r WHERE
 (r.engagement_id,r.campaign_id,r.operation_id)=(owner_call.e,owner_call.c,owner_call.d)
 AND r.record_revision=1 AND r.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user))
 THEN RAISE EXCEPTION 'inference_state_refused'; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended(owner_call.e::text||':'||owner_call.c::text,0));
 IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER') OR wanted NOT IN
 ('sent_awaiting_result','completed','cancelled_uncertain','failed_never_released','usage_unknown')
 THEN RAISE EXCEPTION 'inference_state_refused'; END IF;
 -- Re-read under the campaign fence so concurrent reports cannot replace each other.
 SELECT * INTO owner_call FROM execution.inference_calls x WHERE x.call=k FOR UPDATE;
 -- Content-free diagnostic evidence is valid only on a completed response.
 IF u ? 'proposal_diagnostic' THEN
  IF wanted <> 'completed' OR jsonb_typeof(u) IS DISTINCT FROM 'object'
   OR u-ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms','requested_model','reported_model','proposal_diagnostic']<>'{}'::jsonb
   OR NOT u ?& ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms','requested_model','reported_model','proposal_diagnostic']
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  diag:=u->'proposal_diagnostic';
  IF jsonb_typeof(diag) IS DISTINCT FROM 'object' OR octet_length(diag::text)>512
   OR NOT diag ?& ARRAY['version','reason','shape'] OR diag-ARRAY['version','reason','shape']<>'{}'::jsonb
   OR diag->>'version' IS DISTINCT FROM 'proposal_admission_diagnostic_v1'
   OR jsonb_typeof(diag->'reason') IS DISTINCT FROM 'string'
   OR NOT coalesce(diag->>'reason' IN ('json_size','json_structure','json_trailing','proposal_shape','proposal_version','proposal_text','unexpected_view_request'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF diag->>'reason'='proposal_shape' THEN
   IF jsonb_typeof(diag->'shape') IS DISTINCT FROM 'object'
    OR NOT diag->'shape' ?& ARRAY['kind','field'] OR (diag->'shape')-ARRAY['kind','field']<>'{}'::jsonb
    OR jsonb_typeof(diag#>'{shape,kind}') IS DISTINCT FROM 'string'
    OR jsonb_typeof(diag#>'{shape,field}') IS DISTINCT FROM 'string'
    OR NOT coalesce(diag#>>'{shape,kind}' IN ('missing_field','unknown_field','invalid_type','invalid_value','invalid_enum','other'),false)
    OR NOT coalesce(diag#>>'{shape,field}' IN ('unclassified','version','hypothesis','considered_alternative','references','expected_observation','falsifying_observation','uncertainty','rationale','next_intent','category','manifest_reference','view_request','owner','selectors'),false)
    OR (diag#>>'{shape,kind}'<>'missing_field' AND diag#>>'{shape,field}'<>'unclassified')
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  ELSIF diag->'shape' IS DISTINCT FROM 'null'::jsonb THEN RAISE EXCEPTION 'inference_usage_refused';
  END IF;
  FOREACH field IN ARRAY ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms'] LOOP
   IF u->field='null'::jsonb AND field IN ('input_tokens','output_tokens','reasoning_tokens') THEN CONTINUE; END IF;
   IF jsonb_typeof(u->field) IS DISTINCT FROM 'number' OR (u->>field) !~ '^[0-9]+$' THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
   IF (u->>field)::numeric>9223372036854775807 OR (field='response_bytes' AND (u->>field)::numeric>65536) THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  END LOOP;
  IF u->>'requested_model' IS DISTINCT FROM 'deepseek-flash' OR NOT coalesce(u->'reported_model'='null'::jsonb OR u->>'reported_model' IN ('deepseek-flash','probe-fixture','protocol-fixture-v1'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
 END IF;
 IF u IS NOT NULL AND u <> 'null'::jsonb AND
    (wanted IN ('usage_unknown','cancelled_uncertain') OR u ? 'provider_response') THEN
  IF u ? 'provider_response' AND wanted NOT IN ('usage_unknown','cancelled_uncertain')
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF jsonb_typeof(u) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF (u - ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes',
      'wall_time_ms','requested_model','reported_model','provider_response']) <> '{}'::jsonb
     OR NOT (u ?& ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes',
      'wall_time_ms','requested_model','reported_model','provider_response'])
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  metadata := u->'provider_response';
  IF jsonb_typeof(metadata) IS DISTINCT FROM 'object' THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF (metadata - ARRAY['status','incomplete_reason','usage_availability']) <> '{}'::jsonb
     OR NOT (metadata ?& ARRAY['status','incomplete_reason','usage_availability'])
     OR NOT coalesce(metadata->>'status' IN ('completed','incomplete','failed','in_progress','unrecognized','missing'),false)
     OR NOT coalesce(metadata->>'usage_availability' IN ('reported','partial','unavailable'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  IF metadata->>'status'='incomplete' THEN
   IF NOT coalesce(metadata->>'incomplete_reason' IN ('max_output_tokens','content_filter','unknown'),false)
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  ELSIF metadata->'incomplete_reason' IS DISTINCT FROM 'null'::jsonb
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
  available := 0;
  FOREACH field IN ARRAY ARRAY['input_tokens','output_tokens','reasoning_tokens','response_bytes','wall_time_ms'] LOOP
   IF u->field='null'::jsonb AND field IN ('input_tokens','output_tokens','reasoning_tokens') THEN CONTINUE; END IF;
   IF jsonb_typeof(u->field) IS DISTINCT FROM 'number' OR (u->>field) !~ '^[0-9]+$'
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
   IF (u->>field)::numeric > 9223372036854775807 OR
      (field='response_bytes' AND (u->>field)::numeric > 65536)
   THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
   IF field IN ('input_tokens','output_tokens','reasoning_tokens') THEN available := available+1; END IF;
  END LOOP;
  IF metadata->>'usage_availability' IS DISTINCT FROM
       (CASE WHEN available=3 THEN 'reported' WHEN available=0 THEN 'unavailable' ELSE 'partial' END)
     OR u->>'requested_model' IS DISTINCT FROM 'deepseek-flash'
     OR NOT coalesce(u->'reported_model'='null'::jsonb OR
           u->>'reported_model' IN ('deepseek-flash','probe-fixture','protocol-fixture-v1'),false)
  THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
 END IF;
 IF wanted IN ('usage_unknown','cancelled_uncertain') AND
    owner_call.usage IS NOT NULL AND owner_call.usage <> 'null'::jsonb AND
    u IS NOT NULL AND u <> 'null'::jsonb AND owner_call.usage IS DISTINCT FROM u
 THEN RAISE EXCEPTION 'inference_usage_conflict'; END IF;
 IF wanted IN ('usage_unknown','cancelled_uncertain') AND EXISTS(
  SELECT 1 FROM execution.inference_calls WHERE call=k AND state IN ('cancelled_uncertain','usage_unknown')
 ) THEN
  UPDATE execution.inference_calls SET
   state=CASE WHEN state='cancelled_uncertain' THEN state ELSE wanted END,
   usage=CASE WHEN u IS NULL OR u='null'::jsonb THEN usage ELSE u END WHERE call=k;
  RETURN;
 END IF;
 UPDATE execution.inference_calls SET state=wanted,usage=u WHERE call=k
 AND ((wanted='failed_never_released' AND state='may_send_uncertain')
  OR (wanted<>'failed_never_released' AND state IN ('may_send_uncertain','sent_awaiting_result')));
 IF NOT FOUND THEN RAISE EXCEPTION 'inference_state_conflict'; END IF;
END $$;

-- A distinct completion port makes diagnostic writes fail closed on older schemas.
CREATE FUNCTION execution.complete_inference_proposal(k uuid,u jsonb) RETURNS void
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
 IF jsonb_typeof(u) IS DISTINCT FROM 'object' OR NOT u ? 'proposal_diagnostic'
 THEN RAISE EXCEPTION 'inference_usage_refused'; END IF;
 PERFORM execution.inference_call_state(k,'completed',u);
END $$;
REVOKE ALL ON FUNCTION execution.complete_inference_proposal(uuid,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.complete_inference_proposal(uuid,jsonb) TO dw_m1_broker;
