-- Explain current v4 LIVE/LAB routing work without revealing hostnames.
-- Preserve the original question for retained artifacts and every existing
-- identity, scope, disclosure and structural check. Arbitrary text is refused.
BEGIN;
CREATE OR REPLACE FUNCTION execution.assert_successor_pack(p jsonb,reg jsonb,request jsonb) RETURNS void
LANGUAGE plpgsql IMMUTABLE SET search_path=pg_catalog AS $$
DECLARE ref jsonb; candidate jsonb; original jsonb; expected jsonb; ordinal integer;
 keys text[]:=ARRAY['version','engagement_id','campaign_id','decision_id','permission_id','mission_revision','goal_ref',
 'exercise_mode','scope','question','evaluated_at','owner_views','references','candidates','omissions','pending_unknown',
 'manifests','remaining_capacity','execution_mode','run_profile','inference_profile','predecessor']
 ||CASE WHEN request->>'version'='m1_autonomous_run_request_v4' THEN ARRAY['live_binding','qualification_purpose'] ELSE '{}'::text[] END;
BEGIN
 IF request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4' AND (p ?& ARRAY['live_binding','qualification_purpose'])
 THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 IF request->>'version'='m1_autonomous_run_request_v4' AND (
  p->'live_binding' IS DISTINCT FROM reg#>'{fields,m1_inference_permission,live_binding}'
  OR p->>'qualification_purpose' IS DISTINCT FROM CASE request->>'run_profile' WHEN 'autonomous-dns-genai-live-v1' THEN 'LIVE' ELSE 'LAB' END
 ) THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 IF jsonb_typeof(p) IS DISTINCT FROM 'object' OR NOT p ?& keys OR p-keys<>'{}'
 OR p->'permission_id' IS DISTINCT FROM reg#>'{fields,m1_inference_permission,permission_id}'
 OR p->>'mission_revision' IS DISTINCT FROM '1' OR jsonb_typeof(p->'mission_revision') IS DISTINCT FROM 'number'
 OR p->'goal_ref' IS DISTINCT FROM reg#>'{fields,goal_ref}'
 OR p->'exercise_mode' IS DISTINCT FROM reg#>'{fields,exercise_mode}'
 OR p->'scope' IS DISTINCT FROM jsonb_build_object('owner','mission','publication_ref',reg->'event_id')
 OR (p->>'question' IS DISTINCT FROM 'Which already-authorized, mission-relevant HTTPS candidate is the best next subject for access-validation planning within the permitted contact envelope, and which routing or response premise still needs observation, a different candidate, or deferral?'
  AND (request->>'version' IS DISTINCT FROM 'm1_autonomous_run_request_v4'
   OR p->>'version' IS DISTINCT FROM 'm1_context_pack_autonomous_v4'
   OR p->>'question' IS DISTINCT FROM 'Which mission-relevant routing uncertainty warrants a bounded DNS observation, or is retain, defer or stop better? An admitted_operator_seed reference identifies a hostname seed held by the Broker; select that asset reference with dns_udp without inventing or disclosing the name. A seed is a premise, not observed DNS and not an execution grant; the Broker rechecks current input binding, scope and permission. Only DNS observation is executable in this run; HTTPS contact is unavailable. Read Terrain availability and omissions together: shown_claims=0 means no eligible claims are displayed, not never queried or not loaded. A view_request records owner_view_requested and ends this run without fetching a view, executing DNS or another decision; it does not refresh an already loaded empty view. With committed predecessor feedback, explain whether evidence supports or revises the hypothesis; further effects remain proposals. DNS is optional.'))
 OR jsonb_typeof(p->'evaluated_at') IS DISTINCT FROM 'number' OR coalesce(p->>'evaluated_at','') !~ '^[1-9][0-9]*$'
 OR jsonb_typeof(p->'references') IS DISTINCT FROM 'array' OR jsonb_typeof(p->'candidates') IS DISTINCT FROM 'array'
 OR jsonb_typeof(p->'owner_views') IS DISTINCT FROM 'array'
 THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 IF jsonb_array_length(p->'candidates') NOT BETWEEN 1 AND 8 OR jsonb_array_length(p->'owner_views')<>6
 OR (SELECT count(DISTINCT v->>'owner') FROM jsonb_array_elements(p->'owner_views') v)<>6
 OR EXISTS(SELECT 1 FROM jsonb_array_elements(p->'owner_views') v WHERE coalesce(v->>'owner','') NOT IN
 ('mission','cyber_terrain','foothold_graph','attack_path_view','objective_state','campaign_trajectory'))
 OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'owner_views') v WHERE v=jsonb_build_object('owner','mission',
 'availability','available','revision',1,'frontier',reg->'event_id','evaluated_at',p->'evaluated_at'))
 OR (SELECT count(*) FROM jsonb_array_elements(p->'references') v WHERE v->>'owner'='mission')<>1
 OR (SELECT count(*) FROM jsonb_array_elements(p->'references') v WHERE v->>'owner'='operator')<>jsonb_array_length(p->'candidates')
 THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 FOR ref IN SELECT value FROM jsonb_array_elements(p->'references') LOOP
  CASE ref->>'owner'
   WHEN 'mission' THEN
    expected:=jsonb_build_object('id','mission','owner','mission','entity',reg->'campaign_id','revision',1,
      'lineage',jsonb_build_array(reg->'event_id',reg->'operation_id'),'perceived_at',reg->'occurred_at',
      'time_uncertainty','unspecified_by_owner','status','owner_published','tier',NULL,'freshness','as_of_snapshot','completeness','scope_reference');
    IF ref IS DISTINCT FROM expected THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   WHEN 'operator' THEN
    SELECT value INTO original FROM jsonb_array_elements(request#>'{context,candidates}') WHERE value->'asset_ref'=ref->'entity';
    IF original IS NULL OR NOT (reg#>'{fields,included_assets}') @> jsonb_build_array(ref->'entity')
    OR (reg#>'{fields,excluded_assets}') @> jsonb_build_array(ref->'entity') THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
    expected:=jsonb_build_object('id',ref->'id','owner','operator','entity',original->'asset_ref','revision',NULL,'lineage','[]'::jsonb,
     'source','admitted_operator_seed','perceived_at',NULL,'time_uncertainty','unknown','status','premise','tier',NULL,'freshness','unknown','completeness','declared_only');
    IF ref IS DISTINCT FROM expected THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   WHEN 'cyber_terrain' THEN
    IF NOT EXISTS(SELECT 1 FROM jsonb_array_elements(p->'references') a WHERE a->>'owner'='operator' AND a->'entity'=ref->'asset_ref')
    OR ref-ARRAY['id','owner','source','entity','asset_ref','vantage_ref','revision','lineage','perceived_at','time_uncertainty',
     'evaluated_at','temporal_frontier','valid_until','ttl_seconds','status','tier','freshness','completeness','limitations','corroborated','conflicted','integrity_hold']<>'{}'
    THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   WHEN 'campaign_trajectory' THEN
    IF p#>>'{predecessor,availability}' IS DISTINCT FROM 'committed' OR ref->>'id' IS DISTINCT FROM 'previous_decision'
    OR ref-ARRAY['id','owner','source','entity','event','recorded_at','status']<>'{}'
    THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
   ELSE RAISE EXCEPTION 'run_pack_source_refused';
  END CASE;
 END LOOP;
 FOR candidate,ordinal IN SELECT value,n::integer FROM jsonb_array_elements(p->'candidates') WITH ORDINALITY a(value,n) LOOP
  SELECT value INTO ref FROM jsonb_array_elements(p->'references') WHERE value->>'owner'='operator' AND value->'id'=candidate->'reference';
  SELECT value INTO original FROM jsonb_array_elements(request#>'{context,candidates}') WHERE value->'asset_ref'=ref->'entity';
  IF ref IS NULL OR original IS NULL OR candidate->>'reference' IS DISTINCT FROM 'asset_'||(ordinal-1)::text
  OR candidate->>'status' IS DISTINCT FROM 'operator_premise' OR candidate->'priority' IS DISTINCT FROM original->'priority'
  OR candidate-ARRAY['reference','priority','status','terrain_references']<>'{}'
  THEN RAISE EXCEPTION 'run_pack_source_refused'; END IF;
 END LOOP;
END $$;
COMMIT;
