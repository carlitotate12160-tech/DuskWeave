use crate::mission::{AssetRef, AuthorityRef, GoalRef, OperatorRef, RegistrationFields};
use crate::{Fail, Res};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum NameRule {
    Exact(String),
    LabelSuffix(String),
}

impl NameRule {
    fn name(&self) -> &str {
        match self {
            Self::Exact(name) | Self::LabelSuffix(name) => name,
        }
    }

    fn matches(&self, name: &str) -> bool {
        match self {
            Self::Exact(exact) => name == exact,
            Self::LabelSuffix(suffix) => {
                name == suffix
                    || name
                        .strip_suffix(suffix)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            }
        }
    }
}

fn canonical_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= 63
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

fn canonical_name(name: &str) -> bool {
    name.len() <= 253 && name.split('.').all(canonical_label)
}

pub(crate) fn validate_query_name(name: &str) -> Res<()> {
    let ipv4_like = name.split('.').count() == 4
        && name
            .split('.')
            .all(|label| label.bytes().all(|b| b.is_ascii_digit()));
    if !canonical_name(name) || ipv4_like {
        return Err(Fail::Input("invalid_name"));
    }
    Ok(())
}

fn check_rules(rules: &[&NameRule], minimum: usize) -> Res<()> {
    if rules.len() < minimum || rules.len() > 8 {
        return Err(Fail::Input("m1_rule_limit"));
    }
    for (index, rule) in rules.iter().enumerate() {
        if !canonical_name(rule.name()) || rules[..index].contains(rule) {
            return Err(Fail::Input("m1_invalid_rule"));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProviderDisclosure {
    CrtSh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContactRule {
    asset_ref: AssetRef,
    rule: NameRule,
    priority: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignLimits {
    pub episodes: i64,
    pub provider_calls: i64,
    pub dns_questions: i64,
    pub dns_followups: i64,
    pub tcp_connections: i64,
    pub head_requests: i64,
}

impl CampaignLimits {
    fn validate(&self) -> Res<()> {
        if [
            self.episodes,
            self.provider_calls,
            self.dns_questions,
            self.dns_followups,
            self.tcp_connections,
            self.head_requests,
        ]
        .iter()
        .any(|total| *total <= 0)
        {
            return Err(Fail::Input("m1_invalid_limits"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct M1Permission {
    policy_version: u32,
    ct_base_domain: String,
    provider_disclosure: ProviderDisclosure,
    vantage_ref: Uuid,
    resolver_ipv4: Ipv4Addr,
    discovery_rules: Vec<NameRule>,
    contact_rules: Vec<ContactRule>,
    excluded_names: Vec<NameRule>,
    approved_path: String,
    starts_at: i64,
    ends_at: i64,
    campaign_limits: CampaignLimits,
    concurrency: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionSummary {
    pub policy_version: u32,
    pub operator_ref: OperatorRef,
    pub authority_ref: AuthorityRef,
    pub authority_revision: u64,
    pub goal_ref: GoalRef,
    pub vantage_ref: Uuid,
    pub discovery_rules: usize,
    pub contact_rules: usize,
    pub excluded_names: usize,
    pub starts_at: i64,
    pub ends_at: i64,
    pub campaign_limits: CampaignLimits,
}

impl M1Permission {
    pub(crate) fn query_reason(
        &self,
        purpose: &crate::m1_policy::NamePurpose,
        now: i64,
    ) -> crate::m1_policy::PolicyReason {
        use crate::m1_policy::{NamePurpose, PolicyReason};
        if !(self.starts_at <= now && now < self.ends_at) {
            return PolicyReason::OutsideOperatingWindow;
        }
        let (name, matched) = match purpose {
            NamePurpose::DiscoveryDisclosure { name } => (
                name,
                self.discovery_rules.iter().any(|rule| rule.matches(name)),
            ),
            NamePurpose::Contact { name, asset_ref } => (
                name,
                self.contact_rules
                    .iter()
                    .any(|entry| entry.asset_ref == *asset_ref && entry.rule.matches(name)),
            ),
        };
        if matched && !self.excluded_names.iter().any(|rule| rule.matches(name)) {
            PolicyReason::MatchesNamePolicy
        } else {
            PolicyReason::NameNotPermitted
        }
    }

    fn check_envelope(&self, parent: &RegistrationFields) -> Res<()> {
        if (
            self.policy_version,
            self.concurrency,
            self.approved_path.as_str(),
        ) != (1, 1, "/")
        {
            return Err(Fail::Input("m1_unsupported_configuration"));
        }
        if self.vantage_ref.is_nil() {
            return Err(Fail::Input("nil_reference"));
        }
        if self.starts_at >= self.ends_at
            || self.starts_at < parent.starts_at()
            || self.ends_at > parent.ends_at()
        {
            return Err(Fail::Input("m1_invalid_window"));
        }
        self.campaign_limits.validate()
    }

    fn check_contacts(&self, parent: &RegistrationFields) -> Res<()> {
        check_rules(
            &self
                .contact_rules
                .iter()
                .map(|entry| &entry.rule)
                .collect::<Vec<_>>(),
            0,
        )?;
        for (index, entry) in self.contact_rules.iter().enumerate() {
            if entry.priority == 0
                || !parent.included_assets.contains(&entry.asset_ref)
                || parent.excluded_assets.contains(&entry.asset_ref)
                || self.contact_rules[..index]
                    .iter()
                    .any(|prior| prior.asset_ref == entry.asset_ref)
            {
                return Err(Fail::Input("m1_invalid_contact"));
            }
        }
        Ok(())
    }

    pub(crate) fn validate(&self, parent: &RegistrationFields) -> Res<()> {
        self.check_envelope(parent)?;
        check_rules(&self.discovery_rules.iter().collect::<Vec<_>>(), 1)?;
        check_rules(&self.excluded_names.iter().collect::<Vec<_>>(), 0)?;
        self.check_contacts(parent)?;
        self.check_disclosure()
    }

    fn check_disclosure(&self) -> Res<()> {
        if !canonical_name(&self.ct_base_domain)
            || !self
                .discovery_rules
                .iter()
                .any(|rule| rule.matches(&self.ct_base_domain))
            || self
                .excluded_names
                .iter()
                .any(|rule| rule.matches(&self.ct_base_domain))
        {
            return Err(Fail::Input("m1_invalid_disclosure"));
        }
        Ok(())
    }

    pub(crate) fn summary(&self, parent: &RegistrationFields) -> PermissionSummary {
        PermissionSummary {
            policy_version: self.policy_version,
            operator_ref: parent.operator_ref,
            authority_ref: parent.authority_ref,
            authority_revision: parent.authority_revision,
            goal_ref: parent.goal_ref,
            vantage_ref: self.vantage_ref,
            discovery_rules: self.discovery_rules.len(),
            contact_rules: self.contact_rules.len(),
            excluded_names: self.excluded_names.len(),
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            campaign_limits: self.campaign_limits.clone(),
        }
    }
}
