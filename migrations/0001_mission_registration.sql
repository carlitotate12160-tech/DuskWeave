-- M0A: mission registration -> durable outbox -> trajectory history.
-- Owners: schema mission (aggregate + outbox), schema trajectory (history).
-- Applied by admin; the runtime role receives least privilege only.

CREATE SCHEMA IF NOT EXISTS mission;
CREATE SCHEMA IF NOT EXISTS trajectory;

CREATE TABLE IF NOT EXISTS mission.missions (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operator_ref uuid NOT NULL,
    authority_ref uuid NOT NULL,
    authority_revision bigint NOT NULL CHECK (authority_revision > 0),
    goal_ref uuid NOT NULL,
    included_assets jsonb NOT NULL,
    excluded_assets jsonb NOT NULL,
    exercise_mode text NOT NULL CHECK (exercise_mode IN ('blind', 'defender_informed')),
    starts_at bigint NOT NULL,
    ends_at bigint NOT NULL CHECK (starts_at < ends_at),
    revision bigint NOT NULL CHECK (revision = 1),
    operation_id uuid NOT NULL,
    accepted_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
    PRIMARY KEY (engagement_id, campaign_id)
);

CREATE TABLE IF NOT EXISTS mission.registration_outbox (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL,
    contract jsonb NOT NULL,
    recorded_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
    PRIMARY KEY (engagement_id, campaign_id, operation_id),
    UNIQUE (engagement_id, campaign_id, event_id)
);

-- The history row is simultaneously the inbox/dedup/completion record for
-- obligation trajectory.registration_history.v1. status='accepted' rows are
-- the committed historical item; status='anomaly' rows record a detected
-- event integrity conflict and carry only safe identity + category.
CREATE TABLE IF NOT EXISTS trajectory.registration_history (
    engagement_id uuid NOT NULL,
    campaign_id uuid NOT NULL,
    producer text NOT NULL,
    operation_id uuid NOT NULL,
    event_id uuid NOT NULL,
    obligation text NOT NULL DEFAULT 'trajectory.registration_history.v1',
    status text NOT NULL CHECK (status IN ('accepted', 'anomaly')),
    anomaly_category text,
    contract jsonb,
    recorded_at timestamptz NOT NULL DEFAULT transaction_timestamp(),
    completed_at timestamptz,
    PRIMARY KEY (engagement_id, campaign_id, event_id, status)
);

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'dw_runtime') THEN
        CREATE ROLE dw_runtime NOLOGIN;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA mission, trajectory TO dw_runtime;
GRANT SELECT, INSERT ON mission.missions, mission.registration_outbox TO dw_runtime;
GRANT SELECT, INSERT ON trajectory.registration_history TO dw_runtime;
