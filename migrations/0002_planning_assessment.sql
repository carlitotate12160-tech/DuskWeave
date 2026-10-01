-- Mission owns the immutable decision and required Trajectory publication.
CREATE TABLE IF NOT EXISTS mission.planning_assessments (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL,
    contract jsonb NOT NULL CHECK (jsonb_typeof(contract) = 'object'),
    publication_obligation text NOT NULL
        CHECK (publication_obligation = 'trajectory.planning_history.v1'),
    recorded_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    UNIQUE (engagement_id, campaign_id, event_id)
);

GRANT SELECT, INSERT ON mission.planning_assessments TO dw_runtime;
