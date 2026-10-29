//! Successor profile admission and its fixed operation-limit contract.
use super::*;

impl InferencePermission {
    pub(super) fn check_limits(&self) -> Res<()> {
        if matches!(
            self.version.as_str(),
            "m1_inference_permission_v4" | "m1_inference_permission_v5"
        ) {
            return self.check_successor_limits();
        }
        let autonomous = self.version == "m1_inference_permission_v3";
        self.operation_limits.validate(
            self.envelope_depth(),
            if autonomous { 8192 } else { 4096 },
            if autonomous { 45000 } else { 20000 },
        )?;
        self.operation_limits.check_timers(autonomous)
    }

    pub(super) fn check_successor(&self) -> Res<()> {
        let profile = crate::m1_inference_profile::lookup(&self.profile)?;
        let valid = [
            self.version != "m1_inference_permission_v5"
                || self.operation_limits.request_bytes == 16384,
            (self.version == "m1_inference_permission_v5")
                == (profile.backend == crate::m1_inference_profile::Backend::Genai),
            self.recipient == profile.connection.recipient,
            self.run_profile.as_deref() == Some(crate::m1_inference_profile::RUN_PROFILE),
            self.execution_mode.as_deref() == Some("controlled_fixture"),
            self.provider_terms_approved,
            !self.permission_id.is_nil(),
            !self.provider_terms_ref.is_nil(),
            self.field_classes
                .contains(&SafeFieldClass::AdmittedCampaignDecisionFeedback),
        ];
        if !valid.into_iter().all(|v| v) {
            return Err(Fail::Input("inference_unsupported_profile"));
        }
        self.check_field_classes()?;
        self.validate_native_profile()
    }

    fn check_successor_limits(&self) -> Res<()> {
        let o = &self.operation_limits;
        let t = &crate::m1_inference_profile::REASONING_BOUNDED;
        o.validate(32, 8192, t.decision_ms as i64)?;
        let actual = [
            o.soft_completion_ms,
            o.per_call_wall_time_ms,
            o.shared_wall_time_ms,
            Some(o.decision_wall_time_ms),
            o.phase_wall_time_ms,
            o.cancel_grace_ms,
            o.accounting_window_ms,
        ];
        let expected = [
            t.soft_ms,
            t.hard_ms,
            t.shared_ms,
            t.decision_ms,
            t.phase_ms,
            t.stop_ms,
            t.accounting_ms,
        ]
        .map(|n| Some(n as i64));
        if actual != expected {
            return Err(Fail::Input("inference_unsupported_operation_profile"));
        }
        Ok(())
    }
}
