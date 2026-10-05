-- M0C-C1b: version-3 PlanningAssessed events record a durable refusal
-- grounded in a committed mission-authority withdrawal. Only the named version
-- check widens to IN (1,2,3); defaults, rows, accepted-only unique indexes, the
-- anomaly pair key, permissions and append-only behavior are unchanged.
-- Reapplying is safe.
BEGIN;
ALTER TABLE trajectory.planning_history
    DROP CONSTRAINT IF EXISTS planning_history_version_check;
ALTER TABLE trajectory.planning_history
    ADD CONSTRAINT planning_history_version_check CHECK (version IN (1, 2, 3));
COMMIT;
