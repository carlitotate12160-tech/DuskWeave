//! Broker-owned read projection. A snapshot never reserves capacity or grants send.
use crate::m1_inference_budget::ReservedCapacity;
use crate::{Fail, Res};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextCapacity {
    pub version: String,
    pub epoch: i64,
    pub session_claimed: bool,
    pub active_holds: i64,
    pub charged: ReservedCapacity,
}
impl ContextCapacity {
    pub fn remaining(&self, totals: [i64; 5]) -> Res<ReservedCapacity> {
        let c = &self.charged;
        let used = [
            c.calls,
            c.request_bytes,
            c.response_bytes,
            c.output_token_capacity,
            c.wall_time_ms,
        ];
        self.check_snapshot(&used)?;
        self.check_charge(&used)?;
        let mut remain = [0; 5];
        for i in 0..5 {
            remain[i] = totals[i]
                .checked_sub(used[i])
                .filter(|n| *n >= 0)
                .ok_or(Fail::Store("context_capacity_unavailable"))?;
        }
        Ok(ReservedCapacity {
            calls: remain[0],
            request_bytes: remain[1],
            response_bytes: remain[2],
            output_token_capacity: remain[3],
            wall_time_ms: remain[4],
        })
    }
    fn check_snapshot(&self, used: &[i64; 5]) -> Res<()> {
        if !matches!(
            self.version.as_str(),
            "m1_inference_capacity_snapshot_v1" | "m1_inference_capacity_snapshot_v2"
        ) || self.epoch < 0
            || !(0..=1).contains(&self.active_holds)
            || used.iter().any(|n| *n < 0)
        {
            return Err(Fail::Store("context_capacity_unavailable"));
        }
        Ok(())
    }
    fn check_charge(&self, used: &[i64; 5]) -> Res<()> {
        if self.version == "m1_inference_capacity_snapshot_v2" {
            return Ok(());
        }
        if self.active_holds == 0 {
            if used.iter().any(|n| *n != 0) {
                return Err(Fail::Store("context_capacity_unavailable"));
            }
        } else if self.session_claimed
            || (used[0], used[3]) != (2, 8192)
            || [(used[1], 32768), (used[2], 131072), (used[4], 20000)]
                .iter()
                .any(|(n, cap)| *n <= 0 || n > cap)
        {
            return Err(Fail::Store("context_capacity_unavailable"));
        }
        Ok(())
    }
}
