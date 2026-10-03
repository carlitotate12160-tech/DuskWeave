-- Mission owns the append-only marker and its immutable required publication.
CREATE TABLE IF NOT EXISTS mission.withdrawals (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL,
    registration_operation_id uuid NOT NULL,
    owner_revision bigint NOT NULL DEFAULT 2 CHECK (owner_revision = 2),
    producer text NOT NULL DEFAULT 'mission' CHECK (producer = 'mission'),
    kind text NOT NULL DEFAULT 'mission_authority_withdrawn' CHECK (kind = 'mission_authority_withdrawn'),
    version integer NOT NULL DEFAULT 1 CHECK (version = 1),
    publication_obligation text NOT NULL DEFAULT 'trajectory.withdrawal_history.v1'
        CHECK (publication_obligation = 'trajectory.withdrawal_history.v1'),
    contract jsonb NOT NULL CHECK (jsonb_typeof(contract) = 'object'),
    recorded_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
    PRIMARY KEY (engagement_id, campaign_id),
    UNIQUE (engagement_id, campaign_id, operation_id),
    UNIQUE (engagement_id, campaign_id, event_id)
);

-- Trajectory's accepted row is also its atomic consumer-completion record.
CREATE TABLE IF NOT EXISTS trajectory.withdrawal_history (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL,
    producer text NOT NULL DEFAULT 'mission' CHECK (producer = 'mission'),
    kind text NOT NULL DEFAULT 'mission_authority_withdrawn' CHECK (kind = 'mission_authority_withdrawn'),
    version integer NOT NULL DEFAULT 1 CHECK (version = 1),
    obligation text NOT NULL DEFAULT 'trajectory.withdrawal_history.v1'
        CHECK (obligation = 'trajectory.withdrawal_history.v1'),
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
CREATE UNIQUE INDEX IF NOT EXISTS withdrawal_history_accepted_event
    ON trajectory.withdrawal_history (engagement_id, campaign_id, event_id) WHERE status = 'accepted';
CREATE UNIQUE INDEX IF NOT EXISTS withdrawal_history_accepted_operation
    ON trajectory.withdrawal_history (engagement_id, campaign_id, operation_id) WHERE status = 'accepted';
GRANT SELECT, INSERT ON mission.withdrawals, trajectory.withdrawal_history TO dw_runtime;
