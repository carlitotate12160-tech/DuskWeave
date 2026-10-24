use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub version: String,
    pub hypothesis: String,
    pub considered_alternative: String,
    pub references: Vec<String>,
    pub expected_observation: String,
    pub falsifying_observation: String,
    pub uncertainty: String,
    pub rationale: String,
    pub next_intent: Intent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    pub category: Category,
    pub references: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_request: Option<ViewRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Validate,
    ObserveMore,
    SynthesizeCandidateImplementation,
    Retain,
    Defer,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewRequest {
    pub owner: ViewOwner,
    pub selectors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewOwner {
    CyberTerrain,
    FootholdGraph,
    AttackPathView,
    ObjectiveState,
    CampaignTrajectory,
}

pub enum Admission {
    Accepted(Box<Proposal>),
    Repairable(Vec<&'static str>),
    Rejected(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingPremises {
    Fresh,
    ExpiredByTtl,
}

/// Successor history preserves the screened proposal separately from current use.
pub enum RecordedProposal {
    Current(Box<Proposal>),
    StalePremises(Box<Proposal>),
}

// Concrete current-use and safe-text admission remain unmeasured dependencies.
pub trait ProposalBasis: Send {
    fn reference_eligible(&self, reference: &str) -> bool;
    fn manifest_eligible(&self, reference: &str) -> bool;
    fn current_authority_and_premises(&mut self) -> Result<(), &'static str>;
    fn before_provider_release(&mut self) -> Result<(), &'static str> {
        self.current_authority_and_premises()
    }
    fn current_authority_for_references(
        &mut self,
        _references: &[String],
    ) -> Result<(), &'static str> {
        self.current_authority_and_premises()
    }
    fn safe_explanation(&self, text: &str) -> bool;
    fn recording_premises(&mut self) -> Result<RecordingPremises, &'static str> {
        self.current_authority_and_premises()?;
        Ok(RecordingPremises::Fresh)
    }
}

fn structural(value: Value) -> Result<Proposal, &'static str> {
    if value["next_intent"]["category"] != "observe_more"
        && value["next_intent"].get("view_request").is_some()
    {
        return Err("unexpected_view_request");
    }
    let p: Proposal = serde_json::from_value(value).map_err(|_| "proposal_shape")?;
    if p.version != "m1_inference_proposal_v1" {
        return Err("proposal_version");
    }
    if texts(&p)
        .iter()
        .any(|s| s.trim().is_empty() || s.len() > 1024)
    {
        return Err("proposal_text");
    }
    Ok(p)
}

fn texts(p: &Proposal) -> [&str; 6] {
    [
        &p.hypothesis,
        &p.considered_alternative,
        &p.expected_observation,
        &p.falsifying_observation,
        &p.uncertainty,
        &p.rationale,
    ]
}

fn references(p: &Proposal, basis: &dyn ProposalBasis) -> Result<(), &'static str> {
    if !matches!(p.next_intent.category, Category::Defer | Category::Stop)
        && p.references.is_empty()
    {
        return Err("unsupported_premise");
    }
    if p.references
        .iter()
        .chain(&p.next_intent.references)
        .any(|r| !basis.reference_eligible(r))
    {
        return Err("reference_ineligible");
    }
    if p.next_intent
        .manifest_reference
        .as_deref()
        .is_some_and(|r| !basis.manifest_eligible(r))
    {
        return Err("manifest_ineligible");
    }
    if let Some(view) = &p.next_intent.view_request
        && view.selectors.iter().any(|r| !basis.reference_eligible(r))
    {
        return Err("view_selector_ineligible");
    }
    Ok(())
}

fn cited_references(p: &Proposal) -> Vec<String> {
    let mut out = Vec::new();
    let selectors = p
        .next_intent
        .view_request
        .iter()
        .flat_map(|v| v.selectors.iter());
    for r in p
        .references
        .iter()
        .chain(&p.next_intent.references)
        .chain(selectors)
    {
        if !out.contains(r) {
            out.push(r.clone());
        }
    }
    out
}

pub fn admit(raw: &[u8], basis: &mut dyn ProposalBasis) -> Admission {
    let p = match crate::m1_reasoning::bounded_json::parse(raw, 8192, 1024).and_then(structural) {
        Ok(p) => p,
        Err(code) => return Admission::Repairable(vec![code]),
    };
    if !texts(&p).into_iter().all(|s| basis.safe_explanation(s)) {
        return Admission::Rejected("unsafe_explanation");
    }
    if let Err(code) = references(&p, basis) {
        return Admission::Rejected(code);
    }
    let cited = cited_references(&p);
    if let Err(code) = basis.current_authority_for_references(&cited) {
        return Admission::Rejected(code);
    }
    Admission::Accepted(Box::new(p))
}
