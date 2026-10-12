-- Pre-send reservations only. There is no may_send or transport transition.
-- Current Mission provenance remains behind an owner-owned SQL read port.
CREATE OR REPLACE FUNCTION mission.current_inference_publication(e uuid,c uuid,op uuid)
RETURNS jsonb LANGUAGE sql VOLATILE SET search_path=pg_catalog AS $$
    SELECT o.contract FROM mission.registration_outbox o JOIN mission.missions m
    USING (engagement_id,campaign_id,operation_id)
    WHERE o.engagement_id=e AND o.campaign_id=c AND m.revision=1
    AND '00000000-0000-0000-0000-000000000000'::uuid NOT IN (o.operation_id,o.event_id,m.authority_ref)
    AND (o.contract-ARRAY['fields','occurred_at','recorded_at'])=jsonb_build_object(
        'version',3,'event_id',o.event_id,'operation_id',o.operation_id,
        'engagement_id',e,'campaign_id',c,'producer','mission','affected_entity',c,
        'owner_revision',1,'kind','mission_registered','causation_id',o.operation_id,
        'correlation_id',o.operation_id)
    AND o.contract->'occurred_at'=o.contract->'recorded_at'
    AND jsonb_typeof(o.contract->'recorded_at')='number'
    AND (o.contract->>'recorded_at') ~ '^[0-9]+$'
    AND ((o.contract->'fields')-ARRAY['m1_permission','m1_inference_permission']) =
        (to_jsonb(m)-ARRAY['engagement_id','campaign_id','operation_id','revision','accepted_at'])
    AND NOT EXISTS (SELECT 1 FROM mission.registration_outbox x
        WHERE x.engagement_id=e AND x.campaign_id=c AND x.operation_id=op
        UNION ALL SELECT 1 FROM mission.planning_assessments x
        WHERE x.engagement_id=e AND x.campaign_id=c AND x.operation_id=op
        UNION ALL SELECT 1 FROM mission.withdrawals x
        WHERE x.engagement_id=e AND x.campaign_id=c AND x.operation_id=op)
    AND NOT EXISTS (SELECT 1 FROM mission.withdrawals w
                    WHERE w.engagement_id=e AND w.campaign_id=c)
$$;
REVOKE ALL ON FUNCTION mission.current_inference_publication(uuid,uuid,uuid) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION mission.current_inference_publication(uuid,uuid,uuid) TO dw_m1_broker;

CREATE TABLE IF NOT EXISTS execution.inference_reservations (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    generation bigint NOT NULL CHECK (generation>0),
    record_revision bigint NOT NULL CHECK (record_revision>0),
    writer_oid oid NOT NULL,
    request jsonb NOT NULL,
    registration_operation_id uuid NOT NULL,
    registration_event_id uuid NOT NULL,
    generation_call_id uuid NOT NULL,
    repair_call_id uuid NOT NULL,
    capacity jsonb NOT NULL,
    kind text NOT NULL CHECK (kind IN ('reserved_not_sent','cancelled_never_sent')),
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (engagement_id,campaign_id,operation_id,record_revision),
    UNIQUE (engagement_id,campaign_id,generation,record_revision),
    CHECK (record_revision=CASE kind WHEN 'reserved_not_sent' THEN 1 ELSE 2 END)
);
GRANT SELECT ON execution.inference_reservations TO dw_runtime;
ALTER TABLE execution.inference_reservations ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS inference_reservations_read ON execution.inference_reservations;
CREATE POLICY inference_reservations_read ON execution.inference_reservations
    FOR SELECT USING (true);
CREATE OR REPLACE TRIGGER inference_reservations_no_truncate BEFORE TRUNCATE
ON execution.inference_reservations FOR EACH STATEMENT
EXECUTE FUNCTION execution.deny_session_mutation();

-- The existing session claim path must also respect an outstanding inference
-- owner. Both directions run under the same scope ordering and MVCC row lock.
CREATE OR REPLACE FUNCTION execution.guard_inference_owner() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
BEGIN
    IF NEW.operation_id IS NOT NULL AND NEW.operation_id IS DISTINCT FROM OLD.operation_id
       AND EXISTS (SELECT 1 FROM execution.inference_reservations h
        WHERE h.engagement_id=NEW.engagement_id AND h.campaign_id=NEW.campaign_id
        AND h.record_revision=1 AND NOT EXISTS (
            SELECT 1 FROM execution.inference_reservations x
            WHERE (x.engagement_id,x.campaign_id,x.operation_id)=
                  (h.engagement_id,h.campaign_id,h.operation_id) AND x.record_revision=2)) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_busy';
    END IF;
    RETURN NEW;
END $$;
REVOKE ALL ON FUNCTION execution.guard_inference_owner() FROM PUBLIC;
CREATE OR REPLACE TRIGGER inference_owner_fence BEFORE UPDATE ON execution.session_fences
FOR EACH ROW EXECUTE FUNCTION execution.guard_inference_owner();

-- Actor is the authenticated login, never caller JSON or definer current_user.
-- The shared Mission epoch makes old fixed snapshots abort after a writer wins.
CREATE OR REPLACE FUNCTION execution.inference_reservation(
    action text,op uuid,q jsonb,expected_source jsonb
) RETURNS execution.inference_reservations
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog AS $$
DECLARE
    e uuid; c uuid;
    actor oid := (SELECT oid FROM pg_roles WHERE rolname=session_user);
    prior execution.inference_reservations;
    source jsonb; consent jsonb; limits jsonb; totals jsonb;
    cap jsonb; held jsonb;
    now_s bigint; next_generation bigint; claimed uuid;
BEGIN
    IF NOT pg_has_role(session_user,'dw_m1_broker','MEMBER')
       OR EXISTS (SELECT 1 FROM pg_roles WHERE oid=actor AND (rolsuper OR rolbypassrls))
       OR current_setting('fsync')<>'on' OR current_setting('full_page_writes')<>'on'
       OR current_setting('synchronous_commit')<>'on'
       OR action NOT IN ('reserve','recover','cancel') OR action IS NULL
       OR op IS NULL OR op='00000000-0000-0000-0000-000000000000'
       OR jsonb_typeof(q) IS DISTINCT FROM 'object'
       OR q->'version' IS DISTINCT FROM '"m1_inference_reservation_request_v1"'::jsonb
       OR q->'expected_mission_revision' IS DISTINCT FROM '1'::jsonb
       OR q->>'expected_mission_revision' IS DISTINCT FROM '1'
       OR coalesce(q->>'exercise_mode','') NOT IN ('blind','defender_informed')
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['engagement_id','campaign_id',
           'operator_ref','permission_id','goal_ref']) k
           WHERE jsonb_typeof(q->k) IS DISTINCT FROM 'string'
           OR coalesce(q->>k,'') !~ '^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$'
           OR q->>k='00000000-0000-0000-0000-000000000000')
       OR q-ARRAY['version','engagement_id','campaign_id','operator_ref',
                   'expected_mission_revision','permission_id','goal_ref','exercise_mode']<>'{}' THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_request_refused';
    END IF;
    e := (q->>'engagement_id')::uuid; c := (q->>'campaign_id')::uuid;
    -- Recover is historical/read-only. Mutating actions order before their one
    -- identity lookup; duplicate receipts require no epoch or ledger mutation.
    IF action<>'recover' THEN
        PERFORM pg_advisory_xact_lock(hashtextextended(e::text || ':' || c::text,0));
    END IF;
    SELECT * INTO prior FROM execution.inference_reservations
    WHERE engagement_id=e AND campaign_id=c AND operation_id=op
    ORDER BY record_revision DESC LIMIT 1;
    IF FOUND THEN
        IF prior.writer_oid<>actor OR prior.request IS DISTINCT FROM q THEN
            RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_identity_conflict';
        END IF;
        IF action<>'cancel' OR prior.kind='cancelled_never_sent' THEN RETURN prior; END IF;
    ELSIF action='recover' THEN RETURN NULL;
    ELSIF action='cancel' THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_missing_reservation';
    END IF;
    UPDATE execution.session_fences SET epoch=epoch+1
    WHERE engagement_id=e AND campaign_id=c RETURNING operation_id INTO claimed;
    IF NOT FOUND THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_mission_missing';
    END IF;
    IF prior.operation_id IS NOT NULL THEN
        IF prior.kind<>'reserved_not_sent' THEN
            RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_cancellation_refused';
        END IF;
        prior.kind := 'cancelled_never_sent'; prior.record_revision := 2;
        prior.recorded_at := clock_timestamp();
        INSERT INTO execution.inference_reservations SELECT prior.* RETURNING * INTO prior;
        RETURN prior;
    END IF;
    IF claimed IS NOT NULL THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_busy';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.session_history
        WHERE engagement_id=e AND campaign_id=c AND operation_id=op)
       OR EXISTS (SELECT 1 FROM execution.prepared_withdrawals
        WHERE engagement_id=e AND campaign_id=c AND operation_id=op) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_identity_conflict';
    END IF;
    source := mission.current_inference_publication(e,c,op);
    IF source IS NULL OR source IS DISTINCT FROM expected_source THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_authority_unavailable';
    END IF;
    consent := source->'fields'->'m1_inference_permission';
    now_s := floor(extract(epoch FROM clock_timestamp()))::bigint;
    IF q IS DISTINCT FROM jsonb_build_object('version','m1_inference_reservation_request_v1',
       'engagement_id',e,'campaign_id',c,'operator_ref',source->'fields'->'operator_ref',
       'expected_mission_revision',1,'permission_id',consent->'permission_id',
       'goal_ref',source->'fields'->'goal_ref','exercise_mode',source->'fields'->'exercise_mode')
       OR (source->'fields'->>'starts_at')::bigint>now_s
       OR now_s>=(source->'fields'->>'ends_at')::bigint
       OR (consent->>'starts_at')::bigint>now_s OR now_s>=(consent->>'ends_at')::bigint
       OR consent->>'profile' IS DISTINCT FROM 'deepseek-responses-v1'
       OR consent->>'recipient' IS DISTINCT FROM 'https://api.deepseek.com/responses'
       OR consent->'provider_terms_approved' IS DISTINCT FROM 'true'::jsonb THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_consent_refused';
    END IF;
    IF consent->>'version' IS DISTINCT FROM 'm1_inference_permission_v1'
       OR (consent->>'provider_terms_ref')::uuid='00000000-0000-0000-0000-000000000000'
       OR (consent->'engagement_id',consent->'campaign_id',consent->'operator_ref',
           consent->'authority_ref',consent->'authority_revision',consent->'goal_ref',
           consent->'exercise_mode',consent->'mission_revision') IS DISTINCT FROM
          (source->'engagement_id',source->'campaign_id',source->'fields'->'operator_ref',
           source->'fields'->'authority_ref',source->'fields'->'authority_revision',
           source->'fields'->'goal_ref',source->'fields'->'exercise_mode','1'::jsonb)
       OR (consent->>'starts_at')::bigint<(source->'fields'->>'starts_at')::bigint
       OR (consent->>'ends_at')::bigint>(source->'fields'->>'ends_at')::bigint THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_consent_refused';
    END IF;
    limits := consent->'operation_limits'; totals := consent->'campaign_limits';
    -- Validate the raw SQL boundary too: nulls, unknown fields and numeric strings
    -- must not turn malformed consent into a reservation.
    IF (SELECT count(*) FROM jsonb_object_keys(consent))<>19
       OR jsonb_typeof(consent->'provider_terms_ref') IS DISTINCT FROM 'string'
       OR (consent->>'provider_terms_ref')::uuid IS NULL
       OR jsonb_typeof(consent->'field_classes') IS DISTINCT FROM 'array'
       OR jsonb_array_length(consent->'field_classes') NOT BETWEEN 1 AND 8
       OR (SELECT count(DISTINCT v) FROM jsonb_array_elements(consent->'field_classes') v)
          <>jsonb_array_length(consent->'field_classes')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements(consent->'field_classes') v
           WHERE jsonb_typeof(v) IS DISTINCT FROM 'string')
       OR EXISTS (SELECT 1 FROM jsonb_array_elements_text(consent->'field_classes') v
           WHERE v NOT IN ('mission_question_constraints','scoped_opaque_references',
           'owner_qualified_summaries','admitted_operator_premises','manifest_availability',
           'omissions_unknown','capacity','admitted_repair_fields_codes'))
       OR (SELECT count(*) FROM jsonb_object_keys(limits))<>16
       OR (SELECT count(*) FROM jsonb_object_keys(totals))<>5
       OR EXISTS (SELECT 1 FROM unnest(ARRAY['starts_at','ends_at',
           'authority_revision','mission_revision']) k
           WHERE jsonb_typeof(consent->k) IS DISTINCT FROM 'number'
           OR consent->>k !~ '^-?[0-9]+$') THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_consent_refused';
    END IF;
    -- Cardinality plus all required names below closes the numeric objects.
    -- One profile table validates each operation field's type and interval.
    IF EXISTS (SELECT 1 FROM (VALUES ('request_bytes',1,16384),('response_bytes',1,65536),
        ('decision_wall_time_ms',1,20000),('context_bytes',1,4096),('candidates',1,8),
        ('configuration_bytes',1,16384),('connect_time_ms',1,5000),('proposal_bytes',1,8192),
        ('repair_bytes',1,2048),('text_field_bytes',1,1024),('repair_error_codes',1,8),
        ('json_depth',1,8),('collection_entries',1,64),('calls_per_decision',2,2),
        ('output_tokens_per_call',4096,4096),('concurrency',1,1)) AS b(name,floor,ceiling)
        WHERE CASE WHEN jsonb_typeof(limits->b.name)='number'
            AND limits->>b.name ~ '^[0-9]+$'
            THEN (limits->>b.name)::numeric<b.floor
                OR (limits->>b.name)::numeric>b.ceiling
            ELSE true END) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_limits_refused';
    END IF;
    IF (limits->>'connect_time_ms')::bigint>(limits->>'decision_wall_time_ms')::bigint THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_limits_refused';
    END IF;
    cap := jsonb_build_object('calls',2,'request_bytes',2*(limits->>'request_bytes')::bigint,
        'response_bytes',2*(limits->>'response_bytes')::bigint,'output_token_capacity',8192,
        'wall_time_ms',(limits->>'decision_wall_time_ms')::bigint);
    SELECT jsonb_object_agg(k,used) INTO held FROM (
        SELECT v.key AS k,coalesce(sum((h.capacity->>v.key)::numeric),0) AS used
        FROM jsonb_each(cap) v LEFT JOIN execution.inference_reservations h
        ON h.engagement_id=e AND h.campaign_id=c AND h.record_revision=1
        AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x
            WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,h.operation_id)
            AND x.kind='cancelled_never_sent') GROUP BY v.key) s;
    IF EXISTS (SELECT 1 FROM jsonb_each(cap) v
        WHERE jsonb_typeof(totals->v.key) IS DISTINCT FROM 'number'
        OR totals->>v.key !~ '^[0-9]+$' OR (totals->>v.key)::numeric<=0
        OR (totals->>v.key)::numeric>9223372036854775807
        OR (held->>v.key)::numeric+(v.value::text)::numeric>(totals->>v.key)::numeric) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_capacity_exhausted';
    END IF;
    IF EXISTS (SELECT 1 FROM execution.inference_reservations h
        WHERE h.engagement_id=e AND h.campaign_id=c AND h.record_revision=1
        AND NOT EXISTS (SELECT 1 FROM execution.inference_reservations x
            WHERE (x.engagement_id,x.campaign_id,x.operation_id)=(e,c,h.operation_id)
            AND x.kind='cancelled_never_sent')) THEN
        RAISE EXCEPTION USING ERRCODE='P0001',MESSAGE='inference_busy';
    END IF;
    SELECT coalesce(max(generation),0)+1 INTO next_generation
    FROM execution.inference_reservations WHERE engagement_id=e AND campaign_id=c;
    INSERT INTO execution.inference_reservations
        (engagement_id,campaign_id,operation_id,generation,record_revision,writer_oid,
         request,registration_operation_id,registration_event_id,generation_call_id,
         repair_call_id,capacity,kind)
    VALUES (e,c,op,next_generation,1,actor,q,(source->>'operation_id')::uuid,
        (source->>'event_id')::uuid,gen_random_uuid(),gen_random_uuid(),cap,'reserved_not_sent')
    RETURNING * INTO prior;
    RETURN prior;
END $$;
REVOKE ALL ON FUNCTION execution.inference_reservation(text,uuid,jsonb,jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION execution.inference_reservation(text,uuid,jsonb,jsonb) TO dw_m1_broker;
