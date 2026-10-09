-- M1 prepared session fence: durable database writer fence only. A prepared
-- record is neither an execution grant nor host control; no effect-enabled
-- state exists in this slice. The Broker class holds EXECUTE on the fixed
-- functions only; every fence/history mutation happens inside them or the
-- owner-definer INSERT guards.
CREATE SCHEMA IF NOT EXISTS execution;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='dw_m1_broker') THEN
        CREATE ROLE dw_m1_broker NOLOGIN;
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS execution.session_fences (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    epoch bigint NOT NULL DEFAULT 0 CHECK (epoch >= 0),
    generation bigint NOT NULL DEFAULT 0 CHECK (generation >= 0),
    operation_id uuid,
    writer_oid oid,
    phase text NOT NULL DEFAULT 'idle' CHECK (phase IN ('idle','prepared_no_effects')),
    PRIMARY KEY (engagement_id, campaign_id),
    CHECK ((operation_id IS NULL) = (writer_oid IS NULL)),
    CHECK ((phase='idle') = (operation_id IS NULL))
);
CREATE TABLE IF NOT EXISTS execution.session_history (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    operator_ref uuid NOT NULL,
    expected_mission_revision bigint NOT NULL CHECK (expected_mission_revision = 1),
    registration_operation_id uuid NOT NULL,
    registration_event_id uuid NOT NULL,
    generation bigint NOT NULL CHECK (generation > 0),
    writer_oid oid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('prepared_no_effects','released_no_effects')),
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (engagement_id, campaign_id, operation_id, kind),
    UNIQUE (engagement_id, campaign_id, generation, kind)
);
-- Existing v1/v2 Mission scopes backfill as idle at generation zero.
INSERT INTO execution.session_fences (engagement_id, campaign_id)
SELECT engagement_id, campaign_id FROM mission.missions ON CONFLICT DO NOTHING;

GRANT USAGE ON SCHEMA execution TO dw_runtime, dw_m1_broker;
GRANT SELECT ON execution.session_fences, execution.session_history TO dw_runtime;

-- Every ordinary authority writer touches the same MVCC fence row before its
-- row is checked: a fixed Repeatable Read/Serializable snapshot cannot miss
-- a concurrent claim or withdrawal, and a waiting writer aborts or observes
-- the committed fence. The scope advisory lock orders same-scope writers;
-- hash collisions may conservatively serialize unrelated scopes but never
-- confer permission. The guard creates the idle row when necessary.
CREATE OR REPLACE FUNCTION execution.guard_mission_writer() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE claimed uuid;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(
        NEW.engagement_id::text || ':' || NEW.campaign_id::text, 0));
    INSERT INTO execution.session_fences (engagement_id, campaign_id)
    VALUES (NEW.engagement_id, NEW.campaign_id) ON CONFLICT DO NOTHING;
    UPDATE execution.session_fences SET epoch=epoch+1
    WHERE engagement_id=NEW.engagement_id AND campaign_id=NEW.campaign_id
    RETURNING operation_id INTO claimed;
    IF claimed IS NOT NULL THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='m1_session_fenced';
    END IF;
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION execution.guard_mission_writer() FROM PUBLIC;
DROP TRIGGER IF EXISTS m1_session_writer_fence ON mission.missions;
CREATE TRIGGER m1_session_writer_fence BEFORE INSERT
ON mission.missions FOR EACH ROW EXECUTE FUNCTION execution.guard_mission_writer();
DROP TRIGGER IF EXISTS m1_session_writer_fence ON mission.registration_outbox;
CREATE TRIGGER m1_session_writer_fence BEFORE INSERT
ON mission.registration_outbox FOR EACH ROW EXECUTE FUNCTION execution.guard_mission_writer();
DROP TRIGGER IF EXISTS m1_session_writer_fence ON mission.withdrawals;
CREATE TRIGGER m1_session_writer_fence BEFORE INSERT
ON mission.withdrawals FOR EACH ROW EXECUTE FUNCTION execution.guard_mission_writer();

-- Role membership comes from session_user plus the actual pg_roles OID; a
-- caller field, current_user under the definer or a custom GUC is never an
-- identity. The fence row is locked before the durable Mission source is
-- compared, so an earlier application read is never current authority.
CREATE OR REPLACE FUNCTION execution.prepare_session(
    e uuid, c uuid, op uuid, operator uuid, expected_revision bigint,
    expected_event jsonb
) RETURNS execution.session_history
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE
    fence execution.session_fences;
    prior execution.session_history;
    mission_row mission.missions;
    original mission.registration_outbox;
    policy jsonb;
    actor oid := (SELECT oid FROM pg_roles WHERE rolname=session_user);
    now_s bigint;
BEGIN
    IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER')
       OR expected_revision IS DISTINCT FROM 1
       OR e IS NULL OR c IS NULL OR op IS NULL OR operator IS NULL
       OR '00000000-0000-0000-0000-000000000000'::uuid IN (e,c,op,operator) THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='session_request_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    SELECT * INTO fence FROM execution.session_fences
    WHERE engagement_id=e AND campaign_id=c FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='session_mission_missing';
    END IF;
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    -- Same original op/operator/login replays its own latest record without a
    -- new generation, even after release or withdrawal; it grants nothing.
    SELECT * INTO prior FROM execution.session_history
    WHERE engagement_id=e AND campaign_id=c AND operation_id=op
      AND kind='prepared_no_effects';
    IF FOUND THEN
        IF prior.operator_ref IS DISTINCT FROM operator OR prior.writer_oid <> actor THEN
            RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='session_identity_conflict';
        END IF;
        SELECT * INTO prior FROM execution.session_history
        WHERE engagement_id=e AND campaign_id=c AND operation_id=op
        ORDER BY (kind='released_no_effects') DESC LIMIT 1;
        RETURN prior;
    END IF;
    SELECT * INTO mission_row FROM mission.missions
    WHERE engagement_id=e AND campaign_id=c;
    SELECT * INTO original FROM mission.registration_outbox
    WHERE engagement_id=e AND campaign_id=c AND operation_id=mission_row.operation_id;
    policy := original.contract->'fields'->'m1_permission';
    IF mission_row.operator_ref IS DISTINCT FROM operator
       OR mission_row.revision IS DISTINCT FROM expected_revision
       OR original.operation_id IS NULL
       OR original.contract IS DISTINCT FROM expected_event
       OR fence.operation_id IS NOT NULL
       OR EXISTS (SELECT 1 FROM mission.registration_outbox WHERE operation_id=op)
       OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE operation_id=op)
       OR EXISTS (SELECT 1 FROM execution.session_history WHERE operation_id=op)
       OR policy IS NULL OR policy='null'::jsonb
       OR (mission_row.starts_at <= now_s AND now_s < mission_row.ends_at) IS NOT TRUE
       OR ((policy->>'starts_at')::bigint <= now_s
               AND now_s < (policy->>'ends_at')::bigint) IS NOT TRUE
       OR EXISTS (SELECT 1 FROM mission.withdrawals
                  WHERE engagement_id=e AND campaign_id=c) THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='session_prepare_refused';
    END IF;
    UPDATE execution.session_fences
    SET generation=generation+1, operation_id=op, writer_oid=actor,
        phase='prepared_no_effects'
    WHERE engagement_id=e AND campaign_id=c RETURNING * INTO fence;
    INSERT INTO execution.session_history
        (engagement_id, campaign_id, operation_id, operator_ref,
         expected_mission_revision, registration_operation_id,
         registration_event_id, generation, writer_oid, kind, recorded_at)
    VALUES (e, c, op, operator, expected_revision, mission_row.operation_id,
            original.event_id, fence.generation, actor, 'prepared_no_effects',
            clock_timestamp())
    RETURNING * INTO prior;
    RETURN prior;
END $$;

-- Only the original prepared op/operator/login can release its exact current
-- generation; release appends a linked record and preserves history. A replay
-- of an already released operation returns that record and MUST NOT clear a
-- newer prepared generation. Future effect-enabled states cannot use this
-- function: activation must invalidate the prepared-only path first.
CREATE OR REPLACE FUNCTION execution.release_prepared_session(
    e uuid, c uuid, op uuid, operator uuid, expected_revision bigint
) RETURNS execution.session_history
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE
    fence execution.session_fences;
    prior execution.session_history;
    actor oid := (SELECT oid FROM pg_roles WHERE rolname=session_user);
BEGIN
    IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER')
       OR expected_revision IS DISTINCT FROM 1
       OR e IS NULL OR c IS NULL OR op IS NULL OR operator IS NULL THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='session_request_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text, 0));
    SELECT * INTO fence FROM execution.session_fences
    WHERE engagement_id=e AND campaign_id=c FOR UPDATE;
    SELECT * INTO prior FROM execution.session_history
    WHERE engagement_id=e AND campaign_id=c AND operation_id=op
    ORDER BY (kind='released_no_effects') DESC LIMIT 1;
    IF prior.operation_id IS NULL OR prior.writer_oid <> actor
       OR prior.operator_ref IS DISTINCT FROM operator
       OR prior.expected_mission_revision IS DISTINCT FROM expected_revision THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='session_identity_conflict';
    END IF;
    IF prior.kind='released_no_effects' THEN RETURN prior; END IF;
    IF fence.operation_id IS DISTINCT FROM op
       OR fence.generation <> prior.generation
       OR fence.writer_oid IS DISTINCT FROM actor
       OR fence.phase <> 'prepared_no_effects' THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='session_generation_conflict';
    END IF;
    UPDATE execution.session_fences SET operation_id=NULL, writer_oid=NULL,
        phase='idle' WHERE engagement_id=e AND campaign_id=c;
    prior.kind := 'released_no_effects';
    prior.recorded_at := clock_timestamp();
    INSERT INTO execution.session_history SELECT prior.* RETURNING * INTO prior;
    RETURN prior;
END $$;
REVOKE ALL ON FUNCTION execution.prepare_session(uuid,uuid,uuid,uuid,bigint,jsonb) FROM PUBLIC;
REVOKE ALL ON FUNCTION execution.release_prepared_session(uuid,uuid,uuid,uuid,bigint) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.prepare_session(uuid,uuid,uuid,uuid,bigint,jsonb),
    execution.release_prepared_session(uuid,uuid,uuid,uuid,bigint) TO dw_m1_broker;
