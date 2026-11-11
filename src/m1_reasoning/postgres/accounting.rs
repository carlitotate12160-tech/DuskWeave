//! Broker-owned call accounting; optional admission metadata shares the original write.
use super::*;
use crate::m1_reasoning::proposal_diagnostic::ProposalDiagnostic;

impl PgBroker {
    pub(super) fn state(&mut self, id: Uuid, state: &str, usage: Option<&Usage>) -> Res<()> {
        let value = serde_json::to_value(usage).map_err(|_| "inference_usage_encode")?;
        self.state_value(id, state, value)
    }

    fn state_value(&mut self, id: Uuid, state: &str, value: Value) -> Res<()> {
        self.client
            .query_one(
                "SELECT execution.inference_call_state($1,$2,$3)",
                &[&id, &state, &value],
            )
            .map_err(|_| "inference_accounting_unknown")?;
        Ok(())
    }

    pub(super) fn complete_diagnostic(
        &mut self,
        id: Uuid,
        usage: &Usage,
        diagnostic: Option<&ProposalDiagnostic>,
    ) -> Res<()> {
        let mut value = serde_json::to_value(usage).map_err(|_| "inference_usage_encode")?;
        if let Some(diagnostic) = diagnostic {
            value["proposal_diagnostic"] =
                serde_json::to_value(diagnostic).map_err(|_| "inference_usage_encode")?;
            self.client
                .query_one(
                    "SELECT execution.complete_inference_proposal($1,$2)",
                    &[&id, &value],
                )
                .map_err(|_| "inference_accounting_unknown")?;
            return Ok(());
        }
        self.state_value(id, "completed", value)
    }

    pub(super) fn uncertain_report(
        &mut self,
        id: Uuid,
        report: Option<&super::super::inference::ProviderReport>,
    ) -> Res<()> {
        let value = serde_json::to_value(report).map_err(|_| "inference_usage_encode")?;
        self.state_value(id, "usage_unknown", value)
    }
}
