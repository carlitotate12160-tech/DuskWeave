-- Immutable execution attribution for a Mission-owned prepared-only withdrawal.
CREATE TABLE IF NOT EXISTS execution.prepared_withdrawals (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    session_operation_id uuid NOT NULL,
    generation bigint NOT NULL CHECK (generation > 0),
    writer_oid oid NOT NULL,
    contract jsonb NOT NULL CHECK (jsonb_typeof(contract)='object'),
    PRIMARY KEY (engagement_id,campaign_id,operation_id),
    UNIQUE (engagement_id,campaign_id,generation)
);
GRANT SELECT ON execution.prepared_withdrawals TO dw_runtime;
ALTER TABLE execution.prepared_withdrawals ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS prepared_withdrawals_read ON execution.prepared_withdrawals;
CREATE POLICY prepared_withdrawals_read ON execution.prepared_withdrawals
    FOR SELECT USING (true);
CREATE OR REPLACE TRIGGER prepared_withdrawals_no_truncate BEFORE TRUNCATE
ON execution.prepared_withdrawals FOR EACH STATEMENT
EXECUTE FUNCTION execution.deny_session_mutation();

-- Always retain the advisory ordering and MVCC epoch touch before testing
-- authority. Registration writers have no exception, including Broker SQL.
CREATE OR REPLACE FUNCTION execution.guard_mission_writer() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE fence execution.session_fences;
BEGIN
    PERFORM pg_advisory_xact_lock(hashtextextended(
        NEW.engagement_id::text || ':' || NEW.campaign_id::text,0));
    INSERT INTO execution.session_fences (engagement_id,campaign_id)
    VALUES (NEW.engagement_id,NEW.campaign_id) ON CONFLICT DO NOTHING;
    UPDATE execution.session_fences SET epoch=epoch+1
    WHERE engagement_id=NEW.engagement_id AND campaign_id=NEW.campaign_id
    RETURNING * INTO fence;
    IF fence.operation_id IS NULL THEN RETURN NEW; END IF;
    IF TG_TABLE_SCHEMA='mission' AND TG_TABLE_NAME='withdrawals' THEN
        IF EXISTS (
            SELECT 1 FROM execution.prepared_withdrawals p
            WHERE (p.engagement_id,p.campaign_id,p.operation_id)
                =(NEW.engagement_id,NEW.campaign_id,NEW.operation_id)
            AND fence.phase='prepared_no_effects'
            AND (p.session_operation_id,p.generation,p.writer_oid)
                =(fence.operation_id,fence.generation,fence.writer_oid)
            AND p.writer_oid=(SELECT oid FROM pg_roles WHERE rolname=session_user)
            AND p.contract=NEW.contract
            AND NEW.event_id::text=p.contract->>'event_id'
            AND NEW.registration_operation_id::text=p.contract->>'registration_operation_id'
            AND NEW.owner_revision=2 AND NEW.version=1 AND NEW.producer='mission'
            AND NEW.kind='mission_authority_withdrawn'
            AND NEW.publication_obligation='trajectory.withdrawal_history.v1'
        ) THEN RETURN NEW; END IF;
    END IF;
    RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='m1_session_fenced';
END $$;
REVOKE ALL ON FUNCTION execution.guard_mission_writer() FROM PUBLIC;

CREATE OR REPLACE FUNCTION execution.withdraw_prepared_session(
    e uuid,c uuid,op uuid,session_op uuid,expected_generation bigint,
    operator uuid,expected_revision bigint,reason text,expected_event jsonb
) RETURNS execution.prepared_withdrawals
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE
    fence execution.session_fences;
    prior execution.prepared_withdrawals;
    mission_row mission.missions;
    original mission.registration_outbox;
    prepared execution.session_history;
    actor oid := (SELECT oid FROM pg_roles WHERE rolname=session_user);
    event_id uuid;
    time_s bigint;
    payload jsonb;
BEGIN
    IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER')
       OR expected_revision IS DISTINCT FROM 1 OR expected_generation IS NULL
       OR expected_generation <= 0 OR e IS NULL OR c IS NULL OR op IS NULL
       OR session_op IS NULL OR operator IS NULL OR op=session_op
       OR '00000000-0000-0000-0000-000000000000'::uuid IN (e,c,op,session_op,operator)
       OR reason IS NULL OR reason NOT IN
           ('operator_requested','authorization_ended','scope_concern') THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='prepared_withdrawal_refused';
    END IF;
    PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text,0));
    SELECT * INTO fence FROM execution.session_fences
    WHERE engagement_id=e AND campaign_id=c FOR UPDATE;
    SELECT * INTO prior FROM execution.prepared_withdrawals
    WHERE engagement_id=e AND campaign_id=c AND operation_id=op;
    IF FOUND THEN
        IF (prior.session_operation_id,prior.generation,prior.writer_oid)
           IS DISTINCT FROM (session_op,expected_generation,actor)
           OR prior.contract->'request' IS DISTINCT FROM jsonb_build_object(
               'engagement_id',e,'campaign_id',c,'operator_ref',operator,
               'expected_mission_revision',expected_revision,'reason',reason)
           OR NOT EXISTS (SELECT 1 FROM mission.withdrawals w
               WHERE (w.engagement_id,w.campaign_id,w.operation_id)=(e,c,op)
               AND w.contract=prior.contract
               AND w.event_id::text=prior.contract->>'event_id'
               AND w.registration_operation_id::text=prior.contract->>'registration_operation_id'
               AND w.owner_revision=2 AND w.version=1 AND w.producer='mission'
               AND w.kind='mission_authority_withdrawn'
               AND w.publication_obligation='trajectory.withdrawal_history.v1') THEN
            RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='prepared_withdrawal_conflict';
        END IF;
        RETURN prior;
    END IF;
    SELECT * INTO mission_row FROM mission.missions
    WHERE engagement_id=e AND campaign_id=c;
    SELECT * INTO original FROM mission.registration_outbox
    WHERE engagement_id=e AND campaign_id=c AND operation_id=mission_row.operation_id;
    SELECT * INTO prepared FROM execution.session_history
    WHERE engagement_id=e AND campaign_id=c AND operation_id=session_op
        AND kind='prepared_no_effects';
    IF fence.phase IS DISTINCT FROM 'prepared_no_effects'
       OR (fence.operation_id,fence.generation,fence.writer_oid)
          IS DISTINCT FROM (session_op,expected_generation,actor)
       OR mission_row.revision IS DISTINCT FROM expected_revision
       OR mission_row.operator_ref IS DISTINCT FROM operator
       OR original.operation_id IS NULL
       OR original.contract IS DISTINCT FROM expected_event
       OR original.event_id::text IS DISTINCT FROM expected_event->>'event_id'
       OR original.operation_id::text IS DISTINCT FROM expected_event->>'operation_id'
       OR e::text IS DISTINCT FROM expected_event->>'engagement_id'
       OR c::text IS DISTINCT FROM expected_event->>'campaign_id'
       OR prepared.operator_ref IS DISTINCT FROM operator
       OR prepared.expected_mission_revision IS DISTINCT FROM expected_revision
       OR prepared.generation IS DISTINCT FROM expected_generation
       OR prepared.writer_oid IS DISTINCT FROM actor
       OR prepared.registration_operation_id IS DISTINCT FROM original.operation_id
       OR prepared.registration_event_id IS DISTINCT FROM original.event_id
       OR EXISTS (SELECT 1 FROM mission.withdrawals WHERE engagement_id=e AND campaign_id=c)
       OR EXISTS (SELECT 1 FROM mission.registration_outbox
           WHERE engagement_id=e AND campaign_id=c AND operation_id=op)
       OR EXISTS (SELECT 1 FROM mission.planning_assessments
           WHERE engagement_id=e AND campaign_id=c AND operation_id=op)
       OR EXISTS (SELECT 1 FROM execution.session_history
           WHERE engagement_id=e AND campaign_id=c AND operation_id=op) THEN
        RAISE EXCEPTION USING ERRCODE='P0001', MESSAGE='prepared_withdrawal_refused';
    END IF;
    -- This read preserves Mission's scoped assessment/withdrawal SSI dependency.
    PERFORM count(*) FROM mission.planning_assessments WHERE engagement_id=e AND campaign_id=c;
    event_id := gen_random_uuid();
    time_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    payload := jsonb_build_object(
        'event_id',event_id,'operation_id',op,'registration_operation_id',original.operation_id,
        'request',jsonb_build_object('engagement_id',e,'campaign_id',c,
            'operator_ref',operator,'expected_mission_revision',expected_revision,'reason',reason),
        'producer','mission','affected_entity',c,'owner_revision',2,
        'kind','mission_authority_withdrawn','version',1,
        'causation_id',op,'correlation_id',op,'occurred_at',time_s,'recorded_at',time_s);
    -- A failure at either INSERT rolls back both; no temporary fence release.
    INSERT INTO execution.prepared_withdrawals
    VALUES (e,c,op,session_op,expected_generation,actor,payload) RETURNING * INTO prior;
    INSERT INTO mission.withdrawals
        (engagement_id,campaign_id,operation_id,event_id,registration_operation_id,contract)
    VALUES (e,c,op,event_id,original.operation_id,payload);
    RETURN prior;
END $$;
REVOKE ALL ON FUNCTION execution.withdraw_prepared_session(
    uuid,uuid,uuid,uuid,bigint,uuid,bigint,text,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.withdraw_prepared_session(
    uuid,uuid,uuid,uuid,bigint,uuid,bigint,text,jsonb) TO dw_m1_broker;
