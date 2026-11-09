//! Autonomous binding validation and live qualification projection from Mission-owned data.
use super::*;

fn qualified_binding<'a>(
    basis: &'a ContextBasis,
    run: &crate::m1_autonomous_context::RunContextBinding,
) -> Res<&'a crate::m1_inference_permission::LiveBinding> {
    let binding = basis
        .mission
        .fields
        .m1_inference_permission()
        .and_then(|p| p.live_binding())
        .ok_or(Fail::State("context_basis_mismatch"))?;
    let resolver = basis
        .mission
        .fields
        .m1_permission()
        .map(|p| p.dns_vantage().1.to_string())
        .ok_or(Fail::State("context_basis_mismatch"))?;
    let valid = [
        binding.qualification_ref == run.qualification_ref,
        binding.release_sha256 == run.release_sha256,
        binding.resolver_ipv4 == resolver,
    ];
    if !valid.into_iter().all(|v| v) {
        return Err(Fail::State("context_basis_mismatch"));
    }
    binding.validate_for(&run.run_profile)?;
    Ok(binding)
}

pub(super) fn bind(
    pack: &mut Value,
    basis: &ContextBasis,
    autonomous: Option<&crate::m1_autonomous_context::AutonomousContext>,
) -> Res<()> {
    let Some(a) = autonomous.filter(|a| {
        crate::m1_inference_profile::live_binding(
            &a.binding.run_profile,
            &a.binding.execution_mode,
            a.binding.inference_profile.as_deref().unwrap_or_default(),
        )
    }) else {
        return Ok(());
    };
    let binding = qualified_binding(basis, &a.binding)?;
    pack["live_binding"] = json!(binding);
    pack["qualification_purpose"] = json!(
        crate::m1_inference_profile::sdk_versions(&a.binding.run_profile)?.qualification_purpose
    );
    Ok(())
}

pub(super) fn check_autonomous_basis(
    b: &ContextBasis,
    a: &crate::m1_autonomous_context::AutonomousContext,
) -> Res<()> {
    let configured = b
        .mission
        .fields
        .m1_inference_permission()
        .and_then(|p| p.autonomous_binding());
    let expected = Some((
        a.binding.run_profile.as_str(),
        a.binding.execution_mode.as_str(),
    ));
    let valid = [
        configured == expected,
        b.mission.engagement_id == a.binding.engagement_id,
        b.mission.campaign_id == a.binding.campaign_id,
    ];
    valid
        .into_iter()
        .all(|v| v)
        .then_some(())
        .ok_or(Fail::State("context_basis_mismatch"))
}

#[cfg(test)]
#[path = "../../tests/m1_live_dns/units/context_live.rs"]
mod tests;
