//! Closed Proposal-admission metadata; raw values and Serde errors remain transient.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    MissingField,
    UnknownField,
    InvalidType,
    InvalidValue,
    InvalidEnum,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalField {
    Unclassified,
    Version,
    Hypothesis,
    ConsideredAlternative,
    References,
    ExpectedObservation,
    FalsifyingObservation,
    Uncertainty,
    Rationale,
    NextIntent,
    Category,
    ManifestReference,
    ViewRequest,
    Owner,
    Selectors,
}

const FIELDS: &[(&str, ProposalField)] = &[
    ("version", ProposalField::Version),
    ("hypothesis", ProposalField::Hypothesis),
    (
        "considered_alternative",
        ProposalField::ConsideredAlternative,
    ),
    ("references", ProposalField::References),
    ("expected_observation", ProposalField::ExpectedObservation),
    (
        "falsifying_observation",
        ProposalField::FalsifyingObservation,
    ),
    ("uncertainty", ProposalField::Uncertainty),
    ("rationale", ProposalField::Rationale),
    ("next_intent", ProposalField::NextIntent),
    ("category", ProposalField::Category),
    ("manifest_reference", ProposalField::ManifestReference),
    ("view_request", ProposalField::ViewRequest),
    ("owner", ProposalField::Owner),
    ("selectors", ProposalField::Selectors),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ShapeDiagnostic {
    kind: ShapeKind,
    field: ProposalField,
}

impl ShapeDiagnostic {
    /// Compare only Serde's fixed labels; never retain the error or a supplied name.
    pub(crate) fn from_serde(error: &serde_json::Error) -> Self {
        let message = error.to_string();
        Self {
            kind: shape_kind(&message),
            field: missing_field(&message),
        }
    }

    fn unclassified() -> Self {
        Self {
            kind: ShapeKind::Other,
            field: ProposalField::Unclassified,
        }
    }
}

fn shape_kind(message: &str) -> ShapeKind {
    const LABELS: &[(&str, ShapeKind)] = &[
        ("missing field `", ShapeKind::MissingField),
        ("unknown field `", ShapeKind::UnknownField),
        ("invalid type:", ShapeKind::InvalidType),
        ("invalid value:", ShapeKind::InvalidValue),
        ("invalid length", ShapeKind::InvalidValue),
        ("unknown variant `", ShapeKind::InvalidEnum),
    ];
    LABELS
        .iter()
        .find_map(|(label, kind)| message.starts_with(label).then_some(*kind))
        .unwrap_or(ShapeKind::Other)
}

fn missing_field(message: &str) -> ProposalField {
    let name = message
        .strip_prefix("missing field `")
        .and_then(|name| name.strip_suffix('`'));
    FIELDS
        .iter()
        .find_map(|(label, field)| (name == Some(*label)).then_some(*field))
        .unwrap_or(ProposalField::Unclassified)
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Reason {
    JsonSize,
    JsonStructure,
    JsonTrailing,
    ProposalShape,
    ProposalVersion,
    ProposalText,
    UnexpectedViewRequest,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ProposalDiagnostic {
    version: &'static str,
    reason: Reason,
    shape: Option<ShapeDiagnostic>,
}

impl ProposalDiagnostic {
    pub(crate) fn structural(shape: ShapeDiagnostic) -> Self {
        Self {
            version: "proposal_admission_diagnostic_v1",
            reason: Reason::ProposalShape,
            shape: Some(shape),
        }
    }

    pub(crate) fn from_code(code: &str) -> Option<Self> {
        const CODES: &[(&str, Reason)] = &[
            ("json_size", Reason::JsonSize),
            ("json_structure", Reason::JsonStructure),
            ("json_trailing", Reason::JsonTrailing),
            ("proposal_shape", Reason::ProposalShape),
            ("proposal_version", Reason::ProposalVersion),
            ("proposal_text", Reason::ProposalText),
            ("unexpected_view_request", Reason::UnexpectedViewRequest),
        ];
        let reason = CODES
            .iter()
            .find_map(|(label, reason)| (code == *label).then_some(*reason))?;
        let shape = (code == "proposal_shape").then(ShapeDiagnostic::unclassified);
        Some(Self {
            version: "proposal_admission_diagnostic_v1",
            reason,
            shape,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_reasoning::proposal::{self, Admission, Proposal, ProposalBasis};
    use serde_json::{Value, json};

    struct Basis;
    impl ProposalBasis for Basis {
        fn reference_eligible(&self, _: &str) -> bool {
            true
        }
        fn manifest_eligible(&self, _: &str) -> bool {
            true
        }
        fn safe_explanation(&self, _: &str) -> bool {
            true
        }
        fn current_authority_and_premises(&mut self) -> Result<(), &'static str> {
            Ok(())
        }
    }

    fn valid() -> Value {
        json!({"version":"m1_inference_proposal_v1","hypothesis":"synthetic",
            "considered_alternative":"synthetic","references":["mission"],
            "expected_observation":"synthetic","falsifying_observation":"synthetic",
            "uncertainty":"synthetic","rationale":"synthetic",
            "next_intent":{"category":"stop","references":[],"manifest_reference":null}})
    }

    fn admit(value: &Value) -> Admission {
        proposal::admit(&serde_json::to_vec(value).unwrap(), &mut Basis)
    }

    fn diagnostic(value: &Value) -> Value {
        let admission = admit(value);
        assert_eq!(
            admission.repair_codes(),
            Some(["proposal_shape"].as_slice())
        );
        serde_json::to_value(admission.diagnostic().unwrap()).unwrap()
    }

    #[test]
    fn real_proposal_admission_preserves_legacy_codes_and_closed_missing_field() {
        let mut value = valid();
        assert!(matches!(admit(&value), Admission::Accepted(_)));
        value.as_object_mut().unwrap().remove("hypothesis");
        assert_eq!(
            diagnostic(&value),
            json!({
            "version":"proposal_admission_diagnostic_v1","reason":"proposal_shape",
            "shape":{"kind":"missing_field","field":"hypothesis"}})
        );
    }

    #[test]
    fn untrusted_unknown_names_values_and_enum_members_never_enter_metadata() {
        let mut value = valid();
        value["unknown secret: missing field `hypothesis`"] = json!("private sentinel payload");
        let result = diagnostic(&value);
        assert_eq!(
            result["shape"],
            json!({"kind":"unknown_field","field":"unclassified"})
        );
        let text = serde_json::to_string(&result).unwrap();
        assert!(!text.contains("sentinel") && !text.contains("hypothesis"));
        let mut value = valid();
        value["hypothesis"] = json!(123456);
        assert_eq!(
            diagnostic(&value)["shape"],
            json!({"kind":"invalid_type","field":"unclassified"})
        );
        let mut value = valid();
        value["next_intent"]["category"] = json!("private sentinel enum");
        assert_eq!(
            diagnostic(&value)["shape"],
            json!({"kind":"invalid_enum","field":"unclassified"})
        );
    }

    #[test]
    fn retained_nonshape_reasons_are_closed_and_shape_is_null() {
        let admission = proposal::admit(b"{", &mut Basis);
        assert_eq!(
            admission.repair_codes(),
            Some(["json_structure"].as_slice())
        );
        let result = serde_json::to_value(admission.diagnostic().unwrap()).unwrap();
        assert_eq!(result["reason"], "json_structure");
        assert!(result["shape"].is_null());
        assert!(ProposalDiagnostic::from_code("private sentinel").is_none());
        assert!(admit(&valid()).diagnostic().is_none());
    }

    #[test]
    fn unsupported_serde_errors_have_no_untrusted_detail() {
        let error = serde_json::from_value::<Proposal>(json!({"untrusted":"value"})).unwrap_err();
        let detail = ShapeDiagnostic::from_serde(&error);
        let rendered = serde_json::to_string(&detail).unwrap();
        assert!(!rendered.contains("untrusted") && !rendered.contains("value"));
    }
}
