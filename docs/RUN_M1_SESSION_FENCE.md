# M1 Session Fence Runbook

## Overview
The `m1-session` command controls the execution session lifecycle. Currently, only `prepare`, `recover`, and `release` actions are supported for `prepared_no_effects` fences.

## Operator Procedure
1. Create a `SessionRequest` payload containing the target `EngagementId`, `CampaignId`, `OperatorRef`, and `expected_mission_revision`.
2. Save the payload as a JSON file, e.g., `req.json`.
3. Run the CLI with the `dw_m1_broker` role:
   ```bash
   duskweave m1-session --action prepare --operation <op_uuid> --input req.json
   ```
4. Recover a lost ACK:
   ```bash
   duskweave m1-session --action recover --operation <op_uuid> --input req.json
   ```
5. Release the fence when finished:
   ```bash
   duskweave m1-session --action release --operation <op_uuid> --input req.json
   ```

## Honest Failure and Recovery Limitations
- An invalid mission or mission revision results in an honest refusal without creating a session.
- Concurrent claims on the same scope trigger a `serialization_retry` or an `already_claimed` refusal.
- Network loss after commit requires operators to use the `recover` action. Recovery does not steal claims from other brokers or repair mismatched revisions.
- Attempting to bypass the broker and insert effects directly via SQL or old binaries is rejected by the database `enforce_session_fence` trigger when a session is active.
