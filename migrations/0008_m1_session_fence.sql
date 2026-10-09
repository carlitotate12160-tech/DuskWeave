CREATE SCHEMA IF NOT EXISTS execution;

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'dw_m1_broker') THEN
        CREATE ROLE dw_m1_broker NOLOGIN;
    END IF;
END
$$;

CREATE TABLE execution.session_fences (
    engagement_id UUID NOT NULL,
    campaign_id UUID NOT NULL,
    epoch BIGINT NOT NULL,
    generation BIGINT NOT NULL,
    operation_id UUID,
    login_oid OID,
    phase TEXT NOT NULL,
    PRIMARY KEY (engagement_id, campaign_id),
    CONSTRAINT valid_phase CHECK (phase IN ('idle', 'prepared_no_effects')),
    CONSTRAINT consistency CHECK (
        (phase = 'idle' AND operation_id IS NULL AND login_oid IS NULL) OR
        (phase = 'prepared_no_effects' AND operation_id IS NOT NULL AND login_oid IS NOT NULL)
    )
);

CREATE TABLE execution.session_history (
    engagement_id UUID NOT NULL,
    campaign_id UUID NOT NULL,
    operation_id UUID NOT NULL,
    operator_ref UUID NOT NULL,
    expected_mission_revision BIGINT NOT NULL,
    registration_operation_id UUID NOT NULL,
    registration_event_id UUID NOT NULL,
    generation BIGINT NOT NULL,
    writer_oid OID NOT NULL,
    kind TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT valid_kind CHECK (kind IN ('prepared_no_effects', 'released_no_effects')),
    CONSTRAINT unique_op_kind UNIQUE (engagement_id, campaign_id, operation_id, kind),
    CONSTRAINT unique_gen_kind UNIQUE (engagement_id, campaign_id, generation, kind)
);

GRANT USAGE ON SCHEMA execution TO dw_runtime;
GRANT SELECT ON execution.session_fences TO dw_runtime;
GRANT SELECT ON execution.session_history TO dw_runtime;

INSERT INTO execution.session_fences (engagement_id, campaign_id, epoch, generation, operation_id, login_oid, phase)
SELECT engagement_id, campaign_id, 0, 0, NULL, NULL, 'idle'
FROM mission.missions
ON CONFLICT (engagement_id, campaign_id) DO NOTHING;

CREATE OR REPLACE FUNCTION execution.guard_mission_writer()
RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog
AS $$
DECLARE
    v_hash BIGINT;
    v_phase TEXT;
    v_epoch BIGINT;
BEGIN
    v_hash := pg_catalog.hashtext(NEW.engagement_id::text || NEW.campaign_id::text);
    PERFORM pg_catalog.pg_advisory_xact_lock(v_hash);

    SELECT phase, epoch INTO v_phase, v_epoch
    FROM execution.session_fences
    WHERE engagement_id = NEW.engagement_id AND campaign_id = NEW.campaign_id;

    IF NOT FOUND THEN
        INSERT INTO execution.session_fences (
            engagement_id, campaign_id, epoch, generation, operation_id, login_oid, phase
        ) VALUES (
            NEW.engagement_id, NEW.campaign_id, 0, 0, NULL, NULL, 'idle'
        );
    ELSE
        UPDATE execution.session_fences
        SET epoch = v_epoch + 1
        WHERE engagement_id = NEW.engagement_id AND campaign_id = NEW.campaign_id;

        IF v_phase != 'idle' THEN
            RAISE EXCEPTION 'not_authorized' USING ERRCODE = 'insufficient_privilege';
        END IF;
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER guard_writer_missions
BEFORE INSERT ON mission.missions
FOR EACH ROW EXECUTE FUNCTION execution.guard_mission_writer();

CREATE TRIGGER guard_writer_outbox
BEFORE INSERT ON mission.registration_outbox
FOR EACH ROW EXECUTE FUNCTION execution.guard_mission_writer();

CREATE TRIGGER guard_writer_withdrawals
BEFORE INSERT ON mission.withdrawals
FOR EACH ROW EXECUTE FUNCTION execution.guard_mission_writer();

CREATE OR REPLACE FUNCTION execution.prepare_m1_session(claim JSONB)
RETURNS JSONB
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog
AS $$
DECLARE
    v_engagement_id UUID := (claim->>'engagement_id')::UUID;
    v_campaign_id UUID := (claim->>'campaign_id')::UUID;
    v_operation_id UUID := (claim->>'operation_id')::UUID;
    v_operator_ref UUID := (claim->>'operator_ref')::UUID;
    v_expected_revision BIGINT := (claim->>'expected_mission_revision')::BIGINT;
    v_reg_operation_id UUID := (claim->>'registration_operation_id')::UUID;
    v_contract JSONB := claim->'contract';
    v_hash BIGINT;
    v_fence execution.session_fences%ROWTYPE;
    v_login_oid OID;
    v_history execution.session_history%ROWTYPE;
    v_stored_contract JSONB;
BEGIN
    IF NOT pg_catalog.has_role(session_user, 'dw_m1_broker', 'MEMBER') THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'not_authorized');
    END IF;

    v_hash := pg_catalog.hashtext(v_engagement_id::text || v_campaign_id::text);
    PERFORM pg_catalog.pg_advisory_xact_lock(v_hash);

    SELECT oid INTO v_login_oid FROM pg_catalog.pg_roles WHERE rolname = session_user;

    SELECT * INTO v_fence FROM execution.session_fences
    WHERE engagement_id = v_engagement_id AND campaign_id = v_campaign_id;

    SELECT payload INTO v_stored_contract
    FROM mission.registration_outbox
    WHERE engagement_id = v_engagement_id AND campaign_id = v_campaign_id AND operation_id = v_reg_operation_id;

    IF v_stored_contract IS NULL THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'mission_missing');
    END IF;

    IF v_stored_contract != v_contract THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'source_conflict');
    END IF;

    IF EXISTS (SELECT 1 FROM mission.registration_outbox WHERE operation_id = v_operation_id) OR
       EXISTS (SELECT 1 FROM mission.withdrawals WHERE operation_id = v_operation_id) THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'operation_reuse');
    END IF;

    IF v_fence.engagement_id IS NULL THEN
        INSERT INTO execution.session_fences (engagement_id, campaign_id, epoch, generation, operation_id, login_oid, phase)
        VALUES (v_engagement_id, v_campaign_id, 0, 0, NULL, NULL, 'idle')
        RETURNING * INTO v_fence;
    END IF;

    IF v_fence.phase != 'idle' THEN
        IF v_fence.operation_id = v_operation_id AND v_fence.login_oid = v_login_oid THEN
            SELECT * INTO v_history FROM execution.session_history
            WHERE engagement_id = v_engagement_id AND campaign_id = v_campaign_id AND operation_id = v_operation_id AND kind = 'prepared_no_effects';

            IF FOUND AND v_history.operator_ref = v_operator_ref THEN
                SELECT * INTO v_history FROM execution.session_history
                WHERE engagement_id = v_engagement_id AND campaign_id = v_campaign_id AND operation_id = v_operation_id
                ORDER BY recorded_at DESC LIMIT 1;
                RETURN pg_catalog.to_jsonb(v_history);
            ELSE
                RETURN pg_catalog.jsonb_build_object('refused', 'operator_mismatch');
            END IF;
        END IF;
        RETURN pg_catalog.jsonb_build_object('refused', 'already_claimed');
    END IF;

    UPDATE execution.session_fences
    SET epoch = v_fence.epoch + 1,
        generation = v_fence.generation + 1,
        operation_id = v_operation_id,
        login_oid = v_login_oid,
        phase = 'prepared_no_effects'
    WHERE engagement_id = v_engagement_id AND campaign_id = v_campaign_id
    RETURNING * INTO v_fence;

    INSERT INTO execution.session_history (
        engagement_id, campaign_id, operation_id, operator_ref, expected_mission_revision,
        registration_operation_id, registration_event_id, generation, writer_oid, kind, recorded_at
    ) VALUES (
        v_engagement_id, v_campaign_id, v_operation_id, v_operator_ref, v_expected_revision,
        v_reg_operation_id, (v_contract->>'event_id')::UUID, v_fence.generation, v_login_oid, 'prepared_no_effects', pg_catalog.clock_timestamp()
    ) RETURNING * INTO v_history;

    RETURN pg_catalog.to_jsonb(v_history);
END;
$$;

CREATE OR REPLACE FUNCTION execution.release_m1_session(
    p_engagement_id UUID, p_campaign_id UUID, p_operation_id UUID, p_operator_ref UUID, p_expected_revision BIGINT
)
RETURNS JSONB
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = pg_catalog
AS $$
DECLARE
    v_hash BIGINT;
    v_fence execution.session_fences%ROWTYPE;
    v_login_oid OID;
    v_history execution.session_history%ROWTYPE;
BEGIN
    IF NOT pg_catalog.has_role(session_user, 'dw_m1_broker', 'MEMBER') THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'not_authorized');
    END IF;

    v_hash := pg_catalog.hashtext(p_engagement_id::text || p_campaign_id::text);
    PERFORM pg_catalog.pg_advisory_xact_lock(v_hash);

    SELECT oid INTO v_login_oid FROM pg_catalog.pg_roles WHERE rolname = session_user;

    SELECT * INTO v_fence FROM execution.session_fences
    WHERE engagement_id = p_engagement_id AND campaign_id = p_campaign_id;

    SELECT * INTO v_history FROM execution.session_history
    WHERE engagement_id = p_engagement_id AND campaign_id = p_campaign_id AND operation_id = p_operation_id AND kind = 'released_no_effects';

    IF FOUND THEN
        IF v_history.operator_ref != p_operator_ref THEN
            RETURN pg_catalog.jsonb_build_object('refused', 'operator_mismatch');
        END IF;
        IF v_history.writer_oid != v_login_oid THEN
            RETURN pg_catalog.jsonb_build_object('refused', 'writer_mismatch');
        END IF;
        RETURN pg_catalog.to_jsonb(v_history);
    END IF;

    SELECT * INTO v_history FROM execution.session_history
    WHERE engagement_id = p_engagement_id AND campaign_id = p_campaign_id AND operation_id = p_operation_id AND kind = 'prepared_no_effects';

    IF NOT FOUND THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'no_prepared_claim');
    END IF;

    IF v_history.operator_ref != p_operator_ref THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'operator_mismatch');
    END IF;
    IF v_history.writer_oid != v_login_oid THEN
        RETURN pg_catalog.jsonb_build_object('refused', 'writer_mismatch');
    END IF;

    IF v_fence.phase = 'prepared_no_effects' AND v_fence.operation_id = p_operation_id AND v_fence.login_oid = v_login_oid THEN
        UPDATE execution.session_fences
        SET epoch = epoch + 1,
            phase = 'idle',
            operation_id = NULL,
            login_oid = NULL
        WHERE engagement_id = p_engagement_id AND campaign_id = p_campaign_id;
    END IF;

    INSERT INTO execution.session_history (
        engagement_id, campaign_id, operation_id, operator_ref, expected_mission_revision,
        registration_operation_id, registration_event_id, generation, writer_oid, kind, recorded_at
    ) VALUES (
        p_engagement_id, p_campaign_id, p_operation_id, p_operator_ref, p_expected_revision,
        v_history.registration_operation_id, v_history.registration_event_id, v_history.generation, v_login_oid, 'released_no_effects', pg_catalog.clock_timestamp()
    ) RETURNING * INTO v_history;

    RETURN pg_catalog.to_jsonb(v_history);
END;
$$;

REVOKE EXECUTE ON FUNCTION execution.prepare_m1_session(JSONB) FROM PUBLIC;
REVOKE EXECUTE ON FUNCTION execution.release_m1_session(UUID, UUID, UUID, UUID, BIGINT) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.prepare_m1_session(JSONB) TO dw_m1_broker;
GRANT EXECUTE ON FUNCTION execution.release_m1_session(UUID, UUID, UUID, UUID, BIGINT) TO dw_m1_broker;
