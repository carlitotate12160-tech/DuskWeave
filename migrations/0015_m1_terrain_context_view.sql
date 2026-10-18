-- Read-only Terrain public DNS context view for Broker/Cognition ContextPack assembly and at-use rechecks.
CREATE FUNCTION terrain.current_dns_context_view(p_query jsonb)
RETURNS jsonb LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    q jsonb; k text; elem jsonb;
    v_e uuid; v_c uuid; v_vantage uuid; v_resolver text;
    v_assets uuid[] := ARRAY[]::uuid[];
    v_claim_ids uuid[] := ARRAY[]::uuid[];
    v_has_claim_filter boolean := false;
    m mission.missions%ROWTYPE; reg mission.registration_outbox%ROWTYPE;
    f jsonb; policy jsonb; infer_perm jsonb;
    v_now bigint := floor(extract(epoch FROM transaction_timestamp()))::bigint;
    v_total integer := 0; v_omitted integer := 0;
    v_claims jsonb := '[]'::jsonb;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_broker', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_m1_cognition','dw_m1_trajectory','dw_m1_terrain']) g WHERE pg_has_role(session_user, g, 'MEMBER'))
       OR current_setting('fsync') <> 'on' OR current_setting('full_page_writes') <> 'on' OR current_setting('synchronous_commit') <> 'on'
       OR EXISTS (SELECT 1 FROM pg_roles a WHERE pg_has_role(session_user, a.oid, 'SET') AND (
            a.rolsuper OR a.rolbypassrls OR a.rolcreaterole OR a.rolcreatedb
            OR EXISTS (SELECT 1 FROM pg_namespace n WHERE n.nspname IN ('mission','trajectory','execution','cognition','terrain')
                       AND (has_schema_privilege(a.oid, n.oid, 'CREATE') OR pg_has_role(a.oid, n.nspowner, 'MEMBER')))
            OR EXISTS (SELECT 1 FROM pg_class t JOIN pg_namespace n ON n.oid = t.relnamespace
                       WHERE n.nspname IN ('mission','trajectory','execution','cognition','terrain') AND t.relkind = 'r'
                       AND (pg_has_role(a.oid, t.relowner, 'MEMBER')
                            OR has_table_privilege(a.oid, t.oid, 'UPDATE,DELETE,TRUNCATE,TRIGGER')
                            OR has_any_column_privilege(a.oid, t.oid, 'UPDATE')
                            OR (n.nspname = 'terrain' AND (has_table_privilege(a.oid, t.oid, 'SELECT,INSERT') OR has_any_column_privilege(a.oid, t.oid, 'SELECT,INSERT')))
                            OR (n.nspname IN ('cognition','execution') AND (has_table_privilege(a.oid, t.oid, 'INSERT') OR has_any_column_privilege(a.oid, t.oid, 'INSERT'))))))) THEN
        RAISE EXCEPTION 'unqualified_terrain_reader' USING ERRCODE = '42501';
    END IF;
    IF jsonb_typeof(p_query) IS DISTINCT FROM 'object' OR octet_length(p_query::text) > 4096 THEN
        RAISE EXCEPTION 'terrain_view_query_refused';
    END IF;
    q := '{"claim_ids":null}'::jsonb || p_query;
    IF (q - ARRAY['version','engagement_id','campaign_id','operator_ref','expected_mission_revision','goal_ref','exercise_mode','selected_assets','claim_ids']) <> '{}'::jsonb THEN
        RAISE EXCEPTION 'terrain_view_query_refused';
    END IF;
    FOREACH k IN ARRAY ARRAY['version','engagement_id','campaign_id','operator_ref','goal_ref','exercise_mode'] LOOP
        IF jsonb_typeof(q->k) IS DISTINCT FROM 'string' THEN RAISE EXCEPTION 'terrain_view_query_refused'; END IF;
    END LOOP;
    FOREACH k IN ARRAY ARRAY['engagement_id','campaign_id','operator_ref','goal_ref'] LOOP
        IF (q->>k) !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           OR (q->>k)::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'terrain_view_query_refused';
        END IF;
    END LOOP;
    IF q->>'version' <> 'm1_terrain_context_view_query_v1'
       OR jsonb_typeof(q->'expected_mission_revision') IS DISTINCT FROM 'number'
       OR (q->>'expected_mission_revision') <> '1'
       OR q->>'exercise_mode' NOT IN ('blind','defender_informed')
       OR jsonb_typeof(q->'selected_assets') IS DISTINCT FROM 'array'
       OR jsonb_array_length(q->'selected_assets') NOT BETWEEN 1 AND 8
       OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(q->'selected_assets')) <> jsonb_array_length(q->'selected_assets') THEN
        RAISE EXCEPTION 'terrain_view_query_refused';
    END IF;
    FOR elem IN SELECT value FROM jsonb_array_elements(q->'selected_assets') LOOP
        IF jsonb_typeof(elem) IS DISTINCT FROM 'string'
           OR (elem#>>'{}') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
           OR (elem#>>'{}')::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'terrain_view_query_refused';
        END IF;
        v_assets := array_append(v_assets, (elem#>>'{}')::uuid);
    END LOOP;
    IF q->'claim_ids' <> 'null'::jsonb THEN
        IF jsonb_typeof(q->'claim_ids') IS DISTINCT FROM 'array'
           OR jsonb_array_length(q->'claim_ids') NOT BETWEEN 1 AND 8
           OR (SELECT count(DISTINCT value) FROM jsonb_array_elements(q->'claim_ids')) <> jsonb_array_length(q->'claim_ids') THEN
            RAISE EXCEPTION 'terrain_view_query_refused';
        END IF;
        v_has_claim_filter := true;
        FOR elem IN SELECT value FROM jsonb_array_elements(q->'claim_ids') LOOP
            IF jsonb_typeof(elem) IS DISTINCT FROM 'string'
               OR (elem#>>'{}') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
               OR (elem#>>'{}')::uuid = '00000000-0000-0000-0000-000000000000'::uuid THEN
                RAISE EXCEPTION 'terrain_view_query_refused';
            END IF;
            v_claim_ids := array_append(v_claim_ids, (elem#>>'{}')::uuid);
        END LOOP;
    END IF;
    v_e := (q->>'engagement_id')::uuid; v_c := (q->>'campaign_id')::uuid;
    SELECT * INTO m FROM mission.missions WHERE engagement_id = v_e AND campaign_id = v_c;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    SELECT * INTO reg FROM mission.registration_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = m.operation_id;
    IF NOT FOUND THEN RAISE EXCEPTION 'terrain_authority_unavailable'; END IF;
    f := reg.contract->'fields'; policy := f->'m1_permission'; infer_perm := f->'m1_inference_permission';
    IF ((reg.contract @> jsonb_build_object('engagement_id',m.engagement_id,'campaign_id',m.campaign_id,'operation_id',m.operation_id,
          'event_id',reg.event_id,'producer','mission','kind','mission_registered','owner_revision',m.revision)) IS NOT TRUE
       OR (f @> jsonb_build_object('operator_ref',m.operator_ref,'goal_ref',m.goal_ref,'authority_ref',m.authority_ref,
          'authority_revision',m.authority_revision,'included_assets',m.included_assets,'excluded_assets',m.excluded_assets,
          'exercise_mode',m.exercise_mode,'starts_at',m.starts_at,'ends_at',m.ends_at)) IS NOT TRUE
       OR m.revision <> 1 OR q->>'operator_ref' <> m.operator_ref::text OR q->>'goal_ref' <> m.goal_ref::text
       OR q->>'exercise_mode' <> m.exercise_mode OR policy->>'policy_version' IS DISTINCT FROM '1'
       OR NOT (m.starts_at <= v_now AND v_now < m.ends_at)
       OR NOT ((policy->>'starts_at')::bigint <= v_now AND v_now < (policy->>'ends_at')::bigint)
       OR NOT ((infer_perm->>'starts_at')::bigint <= v_now AND v_now < (infer_perm->>'ends_at')::bigint)
       OR NOT coalesce(infer_perm->'field_classes' ? 'owner_qualified_summaries', false)
       OR NOT EXISTS (SELECT 1 FROM trajectory.registration_history h
                      WHERE h.engagement_id = v_e AND h.campaign_id = v_c
                        AND h.operation_id = m.operation_id AND h.event_id = reg.event_id AND h.status = 'accepted')
       OR EXISTS (SELECT 1 FROM trajectory.registration_history h
                  WHERE h.engagement_id = v_e AND h.campaign_id = v_c AND h.status = 'anomaly')
       OR EXISTS (SELECT 1 FROM mission.withdrawals w WHERE w.engagement_id = v_e AND w.campaign_id = v_c)) IS NOT FALSE THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    IF EXISTS (SELECT 1 FROM unnest(v_assets) a
               WHERE NOT (m.included_assets @> to_jsonb(a)) OR (m.excluded_assets @> to_jsonb(a))) THEN
        RAISE EXCEPTION 'terrain_authority_unavailable';
    END IF;
    v_vantage := (policy->>'vantage_ref')::uuid; v_resolver := policy->>'resolver_ipv4';
    IF EXISTS (
        SELECT 1 FROM terrain.current_claims cc
        LEFT JOIN terrain.outbox o ON o.engagement_id = cc.engagement_id AND o.campaign_id = cc.campaign_id AND o.operation_id = cc.operation_id
        LEFT JOIN terrain.observations ob ON ob.engagement_id = cc.engagement_id AND ob.campaign_id = cc.campaign_id AND ob.operation_id = cc.operation_id
        LEFT JOIN terrain.materials mat ON mat.engagement_id = cc.engagement_id AND mat.campaign_id = cc.campaign_id AND mat.operation_id = cc.operation_id
        LEFT JOIN terrain.evaluations ev ON ev.engagement_id = cc.engagement_id AND ev.campaign_id = cc.campaign_id AND ev.operation_id = cc.operation_id
        LEFT JOIN terrain.claim_revisions cr ON cr.engagement_id = cc.engagement_id AND cr.campaign_id = cc.campaign_id AND cr.operation_id = cc.operation_id
        WHERE cc.engagement_id = v_e AND cc.campaign_id = v_c
          AND (
              cc.asset_ref = ANY(v_assets)
              OR cr.asset_ref = ANY(v_assets)
              OR ob.asset_ref = ANY(v_assets)
              OR (cc.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a)
              OR (cr.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a)
              OR (ob.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a)
              OR (o.contract#>>'{claim,asset_ref}') = ANY(SELECT a::text FROM unnest(v_assets) a)
          )
          AND (
              NOT v_has_claim_filter
              OR cc.claim_id = ANY(v_claim_ids)
              OR cr.claim_id = ANY(v_claim_ids)
              OR o.claim_id = ANY(v_claim_ids)
              OR (cc.payload->>'claim_id') = ANY(SELECT cid::text FROM unnest(v_claim_ids) cid)
              OR (o.contract#>>'{claim,claim_id}') = ANY(SELECT cid::text FROM unnest(v_claim_ids) cid)
          )
          AND (
              o.operation_id IS NULL OR ob.operation_id IS NULL OR mat.operation_id IS NULL OR ev.operation_id IS NULL OR cr.operation_id IS NULL
              OR o.event_id IS DISTINCT FROM cc.event_id OR o.event_id IS DISTINCT FROM (o.contract->>'event_id')::uuid
              OR o.observation_id IS DISTINCT FROM cc.observation_id OR o.observation_id IS DISTINCT FROM ob.observation_id
              OR o.material_id IS DISTINCT FROM cc.material_id OR o.material_id IS DISTINCT FROM mat.material_id OR mat.observation_id IS DISTINCT FROM ob.observation_id
              OR o.evaluation_id IS DISTINCT FROM cc.evaluation_id OR o.evaluation_id IS DISTINCT FROM ev.evaluation_id OR ev.claim_id IS DISTINCT FROM cc.claim_id
              OR o.claim_id IS DISTINCT FROM cc.claim_id OR o.claim_revision IS DISTINCT FROM cc.claim_revision
              OR cr.claim_id IS DISTINCT FROM cc.claim_id OR cr.claim_revision IS DISTINCT FROM cc.claim_revision
              OR ob.request_hash IS DISTINCT FROM o.request_hash OR o.request_hash IS DISTINCT FROM (o.contract->>'request_hash')
              OR (o.contract->>'kind') IS DISTINCT FROM 'terrain_changed'
              OR (o.contract->'version') IS DISTINCT FROM '1'::jsonb
              OR (o.contract->>'producer') IS DISTINCT FROM 'terrain'
              OR (o.contract->>'publication_obligation') IS DISTINCT FROM 'trajectory.terrain_history.v1'
              OR (o.contract->>'engagement_id')::uuid IS DISTINCT FROM cc.engagement_id
              OR (o.contract->>'campaign_id')::uuid IS DISTINCT FROM cc.campaign_id
              OR (o.contract->>'operation_id')::uuid IS DISTINCT FROM cc.operation_id
              OR ob.payload IS DISTINCT FROM (o.contract->'observation')
              OR mat.payload IS DISTINCT FROM (o.contract->'material') OR ev.payload IS DISTINCT FROM (o.contract->'evaluation')
              OR cr.payload IS DISTINCT FROM (o.contract->'claim') OR cc.payload IS DISTINCT FROM cr.payload
              OR to_jsonb(cr) IS DISTINCT FROM (to_jsonb(cc) - 'event_id')
              OR cc.payload IS DISTINCT FROM ((to_jsonb(cc) - ARRAY['engagement_id','campaign_id','operation_id','event_id','payload']) || jsonb_build_object('temporal_frontier', cc.payload->'temporal_frontier'))
              OR ob.payload IS DISTINCT FROM ((to_jsonb(ob) - ARRAY['request_hash','payload']) || jsonb_build_object('uncertainty_seconds', ob.payload->'uncertainty_seconds'))
              OR mat.payload IS DISTINCT FROM ((to_jsonb(mat) - ARRAY['engagement_id','campaign_id','operation_id','payload']) || jsonb_build_object('qclass', 'IN', 'limitations', mat.payload->'limitations'))
              OR ev.payload IS DISTINCT FROM (to_jsonb(ev) - ARRAY['engagement_id','campaign_id','operation_id','payload'])
              OR ob.correlation_id IS DISTINCT FROM (o.contract->>'correlation_id')::uuid
              OR ob.vantage_ref IS DISTINCT FROM cc.vantage_ref OR ob.resolver_ipv4 IS DISTINCT FROM cc.resolver_ipv4
              OR ob.subject_name IS DISTINCT FROM cc.subject_name OR ob.asset_ref IS DISTINCT FROM cc.asset_ref
              OR ob.perceived_at IS DISTINCT FROM cc.perceived_at OR ob.recorded_at < ob.perceived_at
              OR jsonb_typeof(ob.payload->'uncertainty_seconds') IS DISTINCT FROM 'number'
              OR (ob.payload->>'uncertainty_seconds')::integer NOT BETWEEN 0 AND 300
              OR mat.answers IS DISTINCT FROM cc.answers
              OR jsonb_typeof(mat.payload->'limitations') IS DISTINCT FROM 'array'
              OR jsonb_array_length(mat.payload->'limitations') > 4
              OR jsonb_typeof(cc.answers) IS DISTINCT FROM 'array'
              OR jsonb_array_length(cc.answers) > 4
              OR ev.observation_id IS DISTINCT FROM cc.observation_id OR ev.material_id IS DISTINCT FROM cc.material_id
              OR ev.valid_until IS DISTINCT FROM cc.valid_until
              OR ev.usable_routing_premise IS DISTINCT FROM (ev.outcome = 'qualified_dns_claim' AND cc.claim_status = 'observed' AND NOT cc.conflicted)
              OR cc.previous_claim_revision IS DISTINCT FROM (cc.claim_revision - 1)
              OR jsonb_typeof(cc.payload->'temporal_frontier') IS DISTINCT FROM 'number'
              OR (cc.payload->>'temporal_frontier')::bigint < cc.perceived_at
              OR (o.contract->>'registration_operation_id')::uuid IS DISTINCT FROM m.operation_id
              OR (o.contract->>'registration_event_id')::uuid IS DISTINCT FROM reg.event_id
              OR (o.contract->>'mission_revision')::integer IS DISTINCT FROM 1
              OR ob.source_kind IS DISTINCT FROM 'operator_reviewed_dns'
              OR cc.claim_kind IS DISTINCT FROM 'dns_hostname_ipv4_resolution'
              OR cc.trust_tier IS DISTINCT FROM 'tier_1' OR cc.corroborated IS DISTINCT FROM false
              OR cc.vantage_ref IS DISTINCT FROM v_vantage OR cc.resolver_ipv4 IS DISTINCT FROM v_resolver
              OR NOT EXISTS (
                  SELECT 1 FROM jsonb_array_elements(policy->'contact_rules') cr_rule
                  WHERE (cr_rule->>'asset_ref')::uuid = cc.asset_ref
                    AND (cr_rule->'rule'->>'exact' = cc.subject_name
                         OR cr_rule->'rule'->>'label_suffix' = cc.subject_name
                         OR right(cc.subject_name, length(cr_rule->'rule'->>'label_suffix') + 1) = '.' || (cr_rule->'rule'->>'label_suffix')))
              OR EXISTS (
                  SELECT 1 FROM jsonb_array_elements(policy->'excluded_names') ex_rule
                  WHERE ex_rule->>'exact' = cc.subject_name
                     OR ex_rule->>'label_suffix' = cc.subject_name
                     OR right(cc.subject_name, length(ex_rule->>'label_suffix') + 1) = '.' || (ex_rule->>'label_suffix'))
          )
    ) OR EXISTS (
        SELECT 1 FROM terrain.claim_revisions cr
        WHERE cr.engagement_id = v_e AND cr.campaign_id = v_c
          AND (cr.asset_ref = ANY(v_assets) OR (cr.payload->>'asset_ref') = ANY(SELECT a::text FROM unnest(v_assets) a))
          AND (NOT v_has_claim_filter OR cr.claim_id = ANY(v_claim_ids) OR (cr.payload->>'claim_id') = ANY(SELECT cid::text FROM unnest(v_claim_ids) cid))
          AND NOT EXISTS (
              SELECT 1 FROM terrain.current_claims cc
              WHERE cc.engagement_id = cr.engagement_id AND cc.campaign_id = cr.campaign_id
                AND cc.subject_name = cr.subject_name AND cc.claim_kind = cr.claim_kind
                AND cc.vantage_ref = cr.vantage_ref AND cc.resolver_ipv4 = cr.resolver_ipv4
                AND cc.claim_revision >= cr.claim_revision
          )
    ) THEN
        RAISE EXCEPTION 'terrain_lineage_unavailable';
    END IF;
    SELECT count(*)::integer INTO v_total
    FROM terrain.current_claims cc
    WHERE cc.engagement_id = v_e AND cc.campaign_id = v_c
      AND cc.asset_ref = ANY(v_assets)
      AND (NOT v_has_claim_filter OR cc.claim_id = ANY(v_claim_ids));
    v_omitted := GREATEST(v_total - 8, 0);
    SELECT coalesce(jsonb_agg(item ORDER BY sort_id), '[]'::jsonb) INTO v_claims
    FROM (
        SELECT cc.claim_id AS sort_id,
               jsonb_build_object(
                   'claim_id', cc.claim_id,
                   'claim_revision', cc.claim_revision,
                   'previous_claim_revision', cc.previous_claim_revision,
                   'operation_id', cc.operation_id,
                   'event_id', cc.event_id,
                   'observation_id', cc.observation_id,
                   'material_id', cc.material_id,
                   'evaluation_id', cc.evaluation_id,
                   'asset_ref', cc.asset_ref,
                   'vantage_ref', cc.vantage_ref,
                   'source_kind', ob.source_kind,
                   'claim_kind', cc.claim_kind,
                   'trust_tier', cc.trust_tier,
                   'claim_status', cc.claim_status,
                   'corroborated', cc.corroborated,
                   'conflicted', cc.conflicted,
                   'integrity_hold', held.frontier IS NOT NULL,
                   'hold_frontier', held.frontier,
                   'temporal_frontier', GREATEST((cc.payload->>'temporal_frontier')::bigint, COALESCE(held.frontier, (cc.payload->>'temporal_frontier')::bigint)),
                   'perceived_at', cc.perceived_at,
                   'uncertainty_seconds', (ob.payload->>'uncertainty_seconds')::integer,
                   'valid_until', cc.valid_until,
                   'lowest_ttl_seconds', ev.lowest_ttl_seconds,
                   'answer_count', jsonb_array_length(cc.answers),
                   'completeness', mat.completeness,
                   'limitations', mat.payload->'limitations',
                   'outcome', ev.outcome
               ) AS item
        FROM terrain.current_claims cc
        JOIN terrain.observations ob ON ob.engagement_id = cc.engagement_id AND ob.campaign_id = cc.campaign_id AND ob.operation_id = cc.operation_id
        JOIN terrain.materials mat ON mat.engagement_id = cc.engagement_id AND mat.campaign_id = cc.campaign_id AND mat.operation_id = cc.operation_id
        JOIN terrain.evaluations ev ON ev.engagement_id = cc.engagement_id AND ev.campaign_id = cc.campaign_id AND ev.operation_id = cc.operation_id
        LEFT JOIN LATERAL (
            SELECT max(h.temporal_frontier) AS frontier
            FROM terrain.claim_integrity_holds h
            WHERE h.engagement_id = cc.engagement_id AND h.campaign_id = cc.campaign_id
              AND h.subject_name = cc.subject_name AND h.claim_kind = cc.claim_kind
              AND h.vantage_ref = cc.vantage_ref AND h.resolver_ipv4 = cc.resolver_ipv4
              AND h.temporal_frontier >= (cc.payload->>'temporal_frontier')::bigint
        ) held ON true
        WHERE cc.engagement_id = v_e AND cc.campaign_id = v_c
          AND cc.asset_ref = ANY(v_assets)
          AND (NOT v_has_claim_filter OR cc.claim_id = ANY(v_claim_ids))
        ORDER BY cc.claim_id ASC
        LIMIT 8
    ) s;
    RETURN jsonb_build_object(
        'version', 'm1_terrain_dns_context_view_v1',
        'engagement_id', v_e,
        'campaign_id', v_c,
        'mission_revision', 1,
        'registration_operation_id', m.operation_id,
        'registration_event_id', reg.event_id,
        'vantage_ref', v_vantage,
        'evaluated_at', v_now,
        'selected_assets', q->'selected_assets',
        'claims', v_claims,
        'omitted_by_claim_limit', v_omitted
    );
EXCEPTION
    WHEN data_exception THEN RAISE EXCEPTION 'terrain_view_query_refused';
END; $$;

REVOKE ALL ON FUNCTION terrain.current_dns_context_view(jsonb) FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
GRANT USAGE ON SCHEMA terrain TO dw_m1_broker;
GRANT EXECUTE ON FUNCTION terrain.current_dns_context_view(jsonb) TO dw_m1_broker;

-- Order Terrain current-claim and integrity-hold mutations with Broker's short release fence.
CREATE FUNCTION terrain.guard_terrain_release_fence()
RETURNS trigger LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
BEGIN
    IF TG_OP <> 'DELETE' THEN
        PERFORM pg_catalog.pg_advisory_xact_lock(
            pg_catalog.hashtextextended(NEW.engagement_id::text || ':' || NEW.campaign_id::text, 0)
        );
        IF TG_TABLE_SCHEMA = 'terrain' AND TG_TABLE_NAME = 'integrity_conflicts' THEN
            PERFORM pg_catalog.pg_advisory_xact_lock(
                pg_catalog.hashtextextended(cr.engagement_id::text || ':' || cr.campaign_id::text, 0)
            )
            FROM terrain.claim_revisions cr WHERE cr.claim_id = NEW.claim_id;
        END IF;
    END IF;
    IF TG_OP <> 'INSERT' THEN
        PERFORM pg_catalog.pg_advisory_xact_lock(
            pg_catalog.hashtextextended(OLD.engagement_id::text || ':' || OLD.campaign_id::text, 0)
        );
        IF TG_TABLE_SCHEMA = 'terrain' AND TG_TABLE_NAME = 'integrity_conflicts' THEN
            PERFORM pg_catalog.pg_advisory_xact_lock(
                pg_catalog.hashtextextended(cr.engagement_id::text || ':' || cr.campaign_id::text, 0)
            )
            FROM terrain.claim_revisions cr WHERE cr.claim_id = OLD.claim_id;
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END; $$;

CREATE TRIGGER terrain_current_claims_release_fence
BEFORE INSERT OR UPDATE OR DELETE ON terrain.current_claims
FOR EACH ROW EXECUTE FUNCTION terrain.guard_terrain_release_fence();

CREATE TRIGGER terrain_integrity_conflicts_release_fence
BEFORE INSERT OR UPDATE OR DELETE ON terrain.integrity_conflicts
FOR EACH ROW EXECUTE FUNCTION terrain.guard_terrain_release_fence();

CREATE TRIGGER terrain_history_release_fence
BEFORE INSERT OR UPDATE OR DELETE ON trajectory.terrain_history
FOR EACH ROW EXECUTE FUNCTION terrain.guard_terrain_release_fence();

REVOKE ALL ON FUNCTION terrain.guard_terrain_release_fence() FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
