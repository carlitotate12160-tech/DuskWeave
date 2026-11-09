//! Live request identity admission, separate from deployment or execution grants.
use super::*;

impl RunRequest {
    pub fn live(&self) -> bool {
        self.version == "m1_autonomous_run_request_v4"
    }

    pub(super) fn validate_live(&self) -> Res<()> {
        let valid = [
            self.inference_profile.as_deref().is_some_and(|name| {
                crate::m1_inference_profile::live_binding(
                    &self.run_profile,
                    &self.execution_mode,
                    name,
                )
            }),
            !self.run_id.is_nil(),
            self.deadline_at_ms > 0,
            self.seed_names.len() == 1,
            self.seed_names.len() == self.context.candidates.len(),
            self.live_binding
                .as_ref()
                .is_some_and(|b| b.validate_for(&self.run_profile).is_ok()),
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("invalid_run_request"));
        }
        self.validate_seeds()?;
        if self.run_profile == crate::m1_inference_profile::LAB_RUN_PROFILE
            && self.seed_names[0]
                .name
                .strip_suffix(".duskweave.test")
                .is_none_or(|s| s.is_empty())
        {
            return Err(Fail::Input("invalid_run_seeds"));
        }
        Ok(())
    }
}
