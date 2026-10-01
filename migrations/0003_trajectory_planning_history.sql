-- Trajectory owns acceptance, deduplication and completion in one logged row.
CREATE TABLE IF NOT EXISTS trajectory.planning_history (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL,
    producer text NOT NULL DEFAULT 'mission' CHECK (producer = 'mission'),
    kind text NOT NULL DEFAULT 'planning_assessed' CHECK (kind = 'planning_assessed'),
    version integer NOT NULL DEFAULT 1 CHECK (version = 1),
    obligation text NOT NULL DEFAULT 'trajectory.planning_history.v1'
        CHECK (obligation = 'trajectory.planning_history.v1'),
    status text NOT NULL CHECK (status IN ('accepted', 'anomaly')),
    contract jsonb,
    anomaly_category text,
    recorded_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
    completed_at timestamptz,
    CHECK ((status = 'accepted' AND jsonb_typeof(contract) = 'object'
            AND contract IS NOT NULL AND completed_at IS NOT NULL AND anomaly_category IS NULL)
        OR (status = 'anomaly' AND contract IS NULL AND completed_at IS NULL
            AND anomaly_category = 'conflicting_identity' AND anomaly_category IS NOT NULL)),
    PRIMARY KEY (engagement_id, campaign_id, event_id, status),
    UNIQUE (engagement_id, campaign_id, operation_id, status)
);

GRANT SELECT, INSERT ON trajectory.planning_history TO dw_runtime;
