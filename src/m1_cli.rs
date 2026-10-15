//! Pure dispatch among existing M1 handlers; no state, I/O or shared aggregates.
use duskweave::{Fail, Res};
pub(super) fn command(command: &str, args: &[String]) -> Res<()> {
    match command {
        "m1-infer" | "m1-infer-inspect" | "m1-infer-recover" => {
            super::m1_reasoning_cli::command(command, args)
        }
        "m1-context-prepare" => super::m1_context_cli::command(args),
        "m1-session" | "m1-session-withdraw" => super::m1_session_cli::command(command, args),
        "m1-inference-budget" => super::m1_inference_budget_cli::command(args),
        _ => Err(Fail::Input("unknown_command")),
    }
}
