DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'dw_m1_terrain') THEN
        CREATE ROLE dw_m1_terrain NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS;
    END IF;
END $$;
CREATE SCHEMA terrain;

CREATE TABLE terrain.observations (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    observation_id UUID NOT NULL UNIQUE, correlation_id UUID NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind = 'operator_reviewed_dns'),
    vantage_ref UUID NOT NULL, resolver_ipv4 TEXT NOT NULL, subject_name TEXT NOT NULL, asset_ref UUID,
    perceived_at BIGINT NOT NULL CHECK (perceived_at > 0),
    recorded_at BIGINT NOT NULL CHECK (recorded_at >= perceived_at),
    request_hash TEXT NOT NULL, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.materials (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    material_id UUID NOT NULL UNIQUE, observation_id UUID NOT NULL UNIQUE,
    transport TEXT NOT NULL CHECK (transport IN ('udp', 'tcp')),
    qtype TEXT NOT NULL CHECK (qtype = 'A'),
    rcode TEXT NOT NULL CHECK (rcode IN ('noerror', 'nxdomain', 'servfail', 'refused', 'formerr', 'notimp')),
    truncated BOOLEAN NOT NULL,
    completeness TEXT NOT NULL CHECK (completeness IN ('complete', 'incomplete', 'truncated', 'timeout')),
    answers JSONB NOT NULL CHECK (jsonb_typeof(answers) = 'array' AND jsonb_array_length(answers) <= 4),
    payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.observations (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.evaluations (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    evaluation_id UUID NOT NULL UNIQUE, observation_id UUID NOT NULL UNIQUE,
    material_id UUID NOT NULL UNIQUE, claim_id UUID NOT NULL UNIQUE,
    outcome TEXT NOT NULL CHECK (outcome IN (
        'qualified_dns_claim', 'incomplete_material', 'truncated_material',
        'negative_or_error_rcode', 'insufficient_empty_answers', 'ttl_zero_not_usable', 'applicability_conflict')),
    usable_routing_premise BOOLEAN NOT NULL, lowest_ttl_seconds INTEGER, valid_until BIGINT, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.materials (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.claim_revisions (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    claim_id UUID NOT NULL UNIQUE, evaluation_id UUID NOT NULL UNIQUE,
    observation_id UUID NOT NULL UNIQUE, material_id UUID NOT NULL UNIQUE,
    subject_name TEXT NOT NULL, claim_kind TEXT NOT NULL CHECK (claim_kind = 'dns_hostname_ipv4_resolution'),
    vantage_ref UUID NOT NULL, resolver_ipv4 TEXT NOT NULL, asset_ref UUID,
    claim_revision INTEGER NOT NULL CHECK (claim_revision > 0),
    previous_claim_revision INTEGER NOT NULL CHECK (previous_claim_revision >= 0 AND previous_claim_revision < claim_revision),
    trust_tier TEXT NOT NULL CHECK (trust_tier = 'tier_1'),
    claim_status TEXT NOT NULL CHECK (claim_status IN ('observed', 'withheld', 'conflicted')),
    corroborated BOOLEAN NOT NULL CHECK (corroborated = FALSE), conflicted BOOLEAN NOT NULL,
    perceived_at BIGINT NOT NULL, valid_until BIGINT, answers JSONB NOT NULL, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    UNIQUE (engagement_id, campaign_id, subject_name, claim_kind, vantage_ref, resolver_ipv4, claim_revision),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.evaluations (engagement_id, campaign_id, operation_id));

CREATE TABLE terrain.current_claims (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL,
    subject_name TEXT NOT NULL, claim_kind TEXT NOT NULL CHECK (claim_kind = 'dns_hostname_ipv4_resolution'),
    vantage_ref UUID NOT NULL, resolver_ipv4 TEXT NOT NULL, asset_ref UUID,
    claim_revision INTEGER NOT NULL CHECK (claim_revision > 0),
    previous_claim_revision INTEGER NOT NULL CHECK (previous_claim_revision >= 0 AND previous_claim_revision < claim_revision),
    claim_id UUID NOT NULL UNIQUE, evaluation_id UUID NOT NULL UNIQUE,
    observation_id UUID NOT NULL UNIQUE, material_id UUID NOT NULL UNIQUE,
    operation_id UUID NOT NULL UNIQUE, event_id UUID NOT NULL UNIQUE,
    trust_tier TEXT NOT NULL CHECK (trust_tier = 'tier_1'),
    claim_status TEXT NOT NULL CHECK (claim_status IN ('observed', 'withheld', 'conflicted')),
    corroborated BOOLEAN NOT NULL CHECK (corroborated = FALSE), conflicted BOOLEAN NOT NULL,
    perceived_at BIGINT NOT NULL, valid_until BIGINT, answers JSONB NOT NULL, payload JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, subject_name, claim_kind, vantage_ref, resolver_ipv4));

CREATE TABLE terrain.outbox (
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    event_id UUID NOT NULL UNIQUE, observation_id UUID NOT NULL UNIQUE,
    material_id UUID NOT NULL UNIQUE, evaluation_id UUID NOT NULL UNIQUE, claim_id UUID NOT NULL UNIQUE,
    claim_revision INTEGER NOT NULL CHECK (claim_revision > 0),
    request_hash TEXT NOT NULL, contract JSONB NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    FOREIGN KEY (engagement_id, campaign_id, operation_id)
        REFERENCES terrain.claim_revisions (engagement_id, campaign_id, operation_id));

CREATE TABLE trajectory.terrain_history (
    history_seq BIGSERIAL PRIMARY KEY,
    engagement_id UUID NOT NULL, campaign_id UUID NOT NULL, operation_id UUID NOT NULL,
    event_id UUID NOT NULL, observation_id UUID NOT NULL, material_id UUID NOT NULL,
    evaluation_id UUID NOT NULL, claim_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('accepted', 'anomaly')),
    reason TEXT, contract JSONB NOT NULL, recorded_at BIGINT NOT NULL);
CREATE UNIQUE INDEX terrain_history_accepted_idx
    ON trajectory.terrain_history (engagement_id, campaign_id, operation_id) WHERE status = 'accepted';
CREATE UNIQUE INDEX terrain_history_event_accepted_idx
    ON trajectory.terrain_history (event_id) WHERE status = 'accepted';

CREATE FUNCTION terrain.accept_dns_observation(p_event jsonb)
RETURNS jsonb LANGUAGE plpgsql SET search_path = pg_catalog AS $$
DECLARE
    v_e uuid := (p_event->>'engagement_id')::uuid; v_c uuid := (p_event->>'campaign_id')::uuid;
    v_op uuid := (p_event->>'operation_id')::uuid; v_ev uuid := (p_event->>'event_id')::uuid;
    v_reg_op uuid := (p_event->>'registration_operation_id')::uuid;
    v_reg_ev uuid := (p_event->>'registration_event_id')::uuid;
    v_hash text := p_event->>'request_hash';
    v_obs jsonb := p_event->'observation'; v_mat jsonb := p_event->'material';
    v_eval jsonb := p_event->'evaluation'; v_clm jsonb := p_event->'claim';
    v_obs_id uuid := (v_obs->>'observation_id')::uuid; v_mat_id uuid := (v_mat->>'material_id')::uuid;
    v_eval_id uuid := (v_eval->>'evaluation_id')::uuid; v_clm_id uuid := (v_clm->>'claim_id')::uuid;
    v_rev integer := (v_clm->>'claim_revision')::integer; v_prev integer := (v_clm->>'previous_claim_revision')::integer;
    v_cur_rev integer;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_terrain', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_trajectory']) g WHERE pg_has_role(session_user, g, 'MEMBER')) THEN
        RAISE EXCEPTION 'unqualified_terrain_writer' USING ERRCODE = '42501';
    END IF;
    IF v_op IS NULL OR v_op = '00000000-0000-0000-0000-000000000000'::uuid
       OR EXISTS (SELECT 1 FROM mission.missions WHERE operation_id = v_op)
       OR EXISTS (SELECT 1 FROM mission.planning_assessments WHERE operation_id = v_op)
       OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE operation_id = v_op) THEN
        RAISE EXCEPTION 'operation_conflict' USING ERRCODE = 'P0001';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM mission.registration_outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_reg_op AND event_id = v_reg_ev)
       OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE engagement_id = v_e AND campaign_id = v_c) THEN
        RAISE EXCEPTION 'terrain_authority_unavailable' USING ERRCODE = 'P0001';
    END IF;
    SELECT claim_revision INTO v_cur_rev FROM terrain.current_claims
    WHERE engagement_id = v_e AND campaign_id = v_c AND subject_name = (v_clm->>'subject_name')
      AND claim_kind = (v_clm->>'claim_kind') AND vantage_ref = (v_clm->>'vantage_ref')::uuid
      AND resolver_ipv4 = (v_clm->>'resolver_ipv4') FOR UPDATE;
    IF COALESCE(v_cur_rev, 0) <> v_prev OR v_rev <> v_prev + 1 THEN
        RAISE EXCEPTION 'claim_revision_mismatch' USING ERRCODE = 'P0001';
    END IF;
    INSERT INTO terrain.observations VALUES (
        v_e, v_c, v_op, v_obs_id, (p_event->>'correlation_id')::uuid, v_obs->>'source_kind',
        (v_obs->>'vantage_ref')::uuid, v_obs->>'resolver_ipv4', v_obs->>'subject_name',
        NULLIF(v_obs->>'asset_ref', '')::uuid, (v_obs->>'perceived_at')::bigint,
        (v_obs->>'recorded_at')::bigint, v_hash, v_obs);
    INSERT INTO terrain.materials VALUES (
        v_e, v_c, v_op, v_mat_id, v_obs_id, v_mat->>'transport', v_mat->>'qtype',
        v_mat->>'rcode', (v_mat->>'truncated')::boolean, v_mat->>'completeness', v_mat->'answers', v_mat);
    INSERT INTO terrain.evaluations VALUES (
        v_e, v_c, v_op, v_eval_id, v_obs_id, v_mat_id, v_clm_id, v_eval->>'outcome',
        (v_eval->>'usable_routing_premise')::boolean, NULLIF(v_eval->>'lowest_ttl_seconds', '')::integer,
        NULLIF(v_eval->>'valid_until', '')::bigint, v_eval);
    INSERT INTO terrain.claim_revisions VALUES (
        v_e, v_c, v_op, v_clm_id, v_eval_id, v_obs_id, v_mat_id, v_clm->>'subject_name',
        v_clm->>'claim_kind', (v_clm->>'vantage_ref')::uuid, v_clm->>'resolver_ipv4',
        NULLIF(v_clm->>'asset_ref', '')::uuid, v_rev, v_prev, v_clm->>'trust_tier',
        v_clm->>'claim_status', (v_clm->>'corroborated')::boolean, (v_clm->>'conflicted')::boolean,
        (v_clm->>'perceived_at')::bigint, NULLIF(v_clm->>'valid_until', '')::bigint, v_clm->'answers', v_clm);
    INSERT INTO terrain.current_claims VALUES (
        v_e, v_c, v_clm->>'subject_name', v_clm->>'claim_kind', (v_clm->>'vantage_ref')::uuid,
        v_clm->>'resolver_ipv4', NULLIF(v_clm->>'asset_ref', '')::uuid, v_rev, v_prev,
        v_clm_id, v_eval_id, v_obs_id, v_mat_id, v_op, v_ev, v_clm->>'trust_tier',
        v_clm->>'claim_status', (v_clm->>'corroborated')::boolean, (v_clm->>'conflicted')::boolean,
        (v_clm->>'perceived_at')::bigint, NULLIF(v_clm->>'valid_until', '')::bigint, v_clm->'answers', v_clm
    ) ON CONFLICT (engagement_id, campaign_id, subject_name, claim_kind, vantage_ref, resolver_ipv4) DO UPDATE SET
        asset_ref = EXCLUDED.asset_ref, claim_revision = EXCLUDED.claim_revision,
        previous_claim_revision = EXCLUDED.previous_claim_revision, claim_id = EXCLUDED.claim_id,
        evaluation_id = EXCLUDED.evaluation_id, observation_id = EXCLUDED.observation_id,
        material_id = EXCLUDED.material_id, operation_id = EXCLUDED.operation_id,
        event_id = EXCLUDED.event_id, trust_tier = EXCLUDED.trust_tier,
        claim_status = EXCLUDED.claim_status, corroborated = EXCLUDED.corroborated,
        conflicted = EXCLUDED.conflicted, perceived_at = EXCLUDED.perceived_at,
        valid_until = EXCLUDED.valid_until, answers = EXCLUDED.answers, payload = EXCLUDED.payload;
    INSERT INTO terrain.outbox VALUES (v_e, v_c, v_op, v_ev, v_obs_id, v_mat_id, v_eval_id, v_clm_id, v_rev, v_hash, p_event);
    RETURN p_event;
END; $$;

CREATE FUNCTION trajectory.record_terrain_history(p_event jsonb)
RETURNS text LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog AS $$
DECLARE
    v_e uuid := (p_event->>'engagement_id')::uuid; v_c uuid := (p_event->>'campaign_id')::uuid;
    v_op uuid := (p_event->>'operation_id')::uuid; v_ev uuid := (p_event->>'event_id')::uuid;
    v_obs_id uuid := (p_event->'observation'->>'observation_id')::uuid;
    v_mat_id uuid := (p_event->'material'->>'material_id')::uuid;
    v_eval_id uuid := (p_event->'evaluation'->>'evaluation_id')::uuid;
    v_clm_id uuid := (p_event->'claim'->>'claim_id')::uuid;
    v_rev integer := (p_event->'claim'->>'claim_revision')::integer;
    v_reg_op uuid := (p_event->>'registration_operation_id')::uuid;
    v_reg_ev uuid := (p_event->>'registration_event_id')::uuid;
    v_hash text := p_event->>'request_hash';
    v_now bigint := floor(extract(epoch FROM statement_timestamp()))::bigint;
    v_source_ok boolean; v_hist record;
BEGIN
    IF NOT pg_has_role(session_user, 'dw_m1_trajectory', 'MEMBER')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['dw_runtime','dw_m1_broker','dw_m1_cognition','dw_m1_terrain']) g WHERE pg_has_role(session_user, g, 'MEMBER')) THEN
        RAISE EXCEPTION 'unqualified_trajectory_writer' USING ERRCODE = '42501';
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM mission.registration_outbox r JOIN trajectory.registration_history h
          ON h.engagement_id = r.engagement_id AND h.campaign_id = r.campaign_id
         AND h.operation_id = r.operation_id AND h.event_id = r.event_id AND h.status = 'accepted'
        WHERE r.engagement_id = v_e AND r.campaign_id = v_c AND r.operation_id = v_reg_op AND r.event_id = v_reg_ev
    ) OR EXISTS (SELECT 1 FROM trajectory.registration_history WHERE engagement_id = v_e AND campaign_id = v_c AND status = 'anomaly') THEN
        RETURN 'pending';
    END IF;
    IF EXISTS (SELECT 1 FROM trajectory.terrain_history WHERE ((engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op) OR event_id = v_ev) AND status = 'anomaly') THEN
        RETURN 'anomaly';
    END IF;
    SELECT EXISTS (
        SELECT 1 FROM terrain.outbox o
        JOIN terrain.observations ob ON ob.engagement_id = o.engagement_id AND ob.campaign_id = o.campaign_id AND ob.operation_id = o.operation_id
        JOIN terrain.materials m ON m.engagement_id = o.engagement_id AND m.campaign_id = o.campaign_id AND m.operation_id = o.operation_id
        JOIN terrain.evaluations ev ON ev.engagement_id = o.engagement_id AND ev.campaign_id = o.campaign_id AND ev.operation_id = o.operation_id
        JOIN terrain.claim_revisions cr ON cr.engagement_id = o.engagement_id AND cr.campaign_id = o.campaign_id AND cr.operation_id = o.operation_id
        WHERE o.engagement_id = v_e AND o.campaign_id = v_c AND o.operation_id = v_op
          AND o.event_id = v_ev AND o.observation_id = v_obs_id AND o.material_id = v_mat_id
          AND o.evaluation_id = v_eval_id AND o.claim_id = v_clm_id AND o.claim_revision = v_rev
          AND o.request_hash = v_hash AND o.contract = p_event
          AND ob.observation_id = v_obs_id AND ob.request_hash = v_hash AND ob.payload = (p_event->'observation')
          AND m.material_id = v_mat_id AND m.observation_id = v_obs_id AND m.payload = (p_event->'material')
          AND ev.evaluation_id = v_eval_id AND ev.claim_id = v_clm_id AND ev.payload = (p_event->'evaluation')
          AND cr.claim_id = v_clm_id AND cr.claim_revision = v_rev AND cr.payload = (p_event->'claim')
    ) INTO v_source_ok;
    SELECT * INTO v_hist FROM trajectory.terrain_history
    WHERE ((engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op) OR event_id = v_ev) AND status = 'accepted' LIMIT 1;
    IF FOUND THEN
        IF v_source_ok AND v_hist.engagement_id = v_e AND v_hist.campaign_id = v_c AND v_hist.operation_id = v_op
           AND v_hist.event_id = v_ev AND v_hist.observation_id = v_obs_id AND v_hist.material_id = v_mat_id
           AND v_hist.evaluation_id = v_eval_id AND v_hist.claim_id = v_clm_id AND v_hist.contract = p_event THEN
            RETURN 'completed';
        END IF;
        INSERT INTO trajectory.terrain_history (engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id, status, reason, contract, recorded_at)
        VALUES (v_e, v_c, v_op, v_ev, v_obs_id, v_mat_id, v_eval_id, v_clm_id, 'anomaly', 'contract_conflict', p_event, v_now);
        RETURN 'anomaly';
    END IF;
    IF NOT EXISTS (SELECT 1 FROM terrain.outbox WHERE engagement_id = v_e AND campaign_id = v_c AND operation_id = v_op) THEN
        RETURN 'uncommitted';
    END IF;
    IF NOT v_source_ok THEN
        INSERT INTO trajectory.terrain_history (engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id, status, reason, contract, recorded_at)
        VALUES (v_e, v_c, v_op, v_ev, v_obs_id, v_mat_id, v_eval_id, v_clm_id, 'anomaly', 'source_conflict', p_event, v_now);
        RETURN 'anomaly';
    END IF;
    INSERT INTO trajectory.terrain_history (engagement_id, campaign_id, operation_id, event_id, observation_id, material_id, evaluation_id, claim_id, status, reason, contract, recorded_at)
    VALUES (v_e, v_c, v_op, v_ev, v_obs_id, v_mat_id, v_eval_id, v_clm_id, 'accepted', NULL, p_event, v_now);
    RETURN 'completed';
END; $$;

REVOKE ALL ON SCHEMA terrain FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON ALL TABLES IN SCHEMA terrain FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA terrain FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON TABLE trajectory.terrain_history FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON SEQUENCE trajectory.terrain_history_history_seq_seq FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;
REVOKE ALL ON FUNCTION trajectory.record_terrain_history(jsonb) FROM PUBLIC, dw_runtime, dw_m1_broker, dw_m1_cognition, dw_m1_trajectory, dw_m1_terrain;

GRANT USAGE ON SCHEMA terrain TO dw_m1_terrain, dw_m1_trajectory;
GRANT SELECT, INSERT ON terrain.observations, terrain.materials, terrain.evaluations, terrain.claim_revisions, terrain.outbox TO dw_m1_terrain;
GRANT SELECT, INSERT, UPDATE ON terrain.current_claims TO dw_m1_terrain;
GRANT EXECUTE ON FUNCTION terrain.accept_dns_observation(jsonb) TO dw_m1_terrain;
GRANT SELECT ON terrain.observations, terrain.materials, terrain.evaluations, terrain.claim_revisions, terrain.current_claims, terrain.outbox TO dw_m1_trajectory;
GRANT SELECT ON TABLE trajectory.terrain_history TO dw_m1_trajectory, dw_m1_terrain, dw_runtime;
GRANT EXECUTE ON FUNCTION trajectory.record_terrain_history(jsonb) TO dw_m1_trajectory;
GRANT USAGE ON SCHEMA mission, trajectory TO dw_m1_terrain;
GRANT SELECT ON mission.missions, mission.planning_assessments, mission.registration_outbox, mission.withdrawals TO dw_m1_terrain;
