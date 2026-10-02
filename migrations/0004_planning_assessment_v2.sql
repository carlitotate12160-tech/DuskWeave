-- M0B-B2: version-2 PlanningAssessed events carry the scoped Mission basis
-- snapshot; accepted planning history admits contract versions 1 and 2.
-- Rows, the default, accepted-only unique indexes, the anomaly pair key,
-- permissions and append-only behavior are unchanged; reapplying is safe.
BEGIN;
ALTER TABLE trajectory.planning_history
    DROP CONSTRAINT IF EXISTS planning_history_version_check;
ALTER TABLE trajectory.planning_history
    ADD CONSTRAINT planning_history_version_check CHECK (version IN (1, 2));
COMMIT;
