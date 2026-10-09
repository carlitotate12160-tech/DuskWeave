CREATE SCHEMA IF NOT EXISTS execution;

CREATE TABLE execution.m1_session (
    campaign_id UUID NOT NULL,
    engagement_id UUID NOT NULL,
    generation BIGINT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('prepared', 'released')),
    prepared_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (campaign_id)
);
