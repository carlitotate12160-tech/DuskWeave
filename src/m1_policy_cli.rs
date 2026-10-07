use duskweave::m1_policy::{M1PolicyResult, PolicyDisposition, check_name_policy};
use duskweave::m1_policy_input::read_policy_file;
use duskweave::postgres_mission::PgMissionStore;
use duskweave::{Fail, Res};
use std::path::Path;

pub(super) fn cmd_policy_check(args: &[String]) -> Res<()> {
    let mut query = None;
    let outcome = (|| {
        let flags = super::flags(args, &["input"])?;
        query = Some(read_policy_file(Path::new(super::flag(&flags, "input")?))?);
        let request = query.as_ref().ok_or(Fail::Input("invalid_args"))?;
        let mut reader = PgMissionStore::new(super::connect()?);
        check_name_policy(&mut reader, request)
    })();
    let receipt = match &outcome {
        Ok(receipt) => receipt.clone(),
        Err(error) => M1PolicyResult::unavailable(*error, query.as_ref()),
    };
    let json = serde_json::to_string(&receipt).map_err(|_| Fail::State("receipt_encode"))?;
    println!("{json}");
    let receipt = outcome?;
    if receipt.result == PolicyDisposition::Unavailable {
        return Err(Fail::State("mission_missing"));
    }
    Ok(())
}
