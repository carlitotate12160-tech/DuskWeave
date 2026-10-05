BEGIN;
ALTER TABLE trajectory.planning_history
    DROP CONSTRAINT IF EXISTS planning_history_version_check;
ALTER TABLE trajectory.planning_history
    ADD CONSTRAINT planning_history_version_check CHECK (version IN (1, 2, 3, 4));
COMMIT;
