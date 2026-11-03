use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const HELPERS: &str = r#"
Append {
  (e.A) (e.B) = (<Append-Contents (e.A) (e.B)>);
}
Append-Contents {
  () (e.B) = e.B;
  (t.H e.T) (e.B) = t.H <Append-Contents (e.T) (e.B)>;
}
"#;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskId {
    #[default]
    AppendThree,
    AppendIdentity,
    AppendFour,
}

#[derive(Debug, Clone, Serialize)]
pub struct Contract {
    pub id: TaskId,
    pub title: &'static str,
    pub instruction: &'static str,
    pub domain: &'static str,
    pub variables: &'static [&'static str],
    pub pattern: &'static str,
    pub reference: &'static str,
    pub requirements: Vec<&'static str>,
    pub proof_scope: &'static str,
    pub fingerprint: String,
}

impl TaskId {
    pub fn contract(self) -> Contract {
        let (title, instruction, variables, pattern, reference): (_, _, &[_], _, _) = match self {
            Self::AppendThree => (
                "Reassociate three-list concatenation",
                "Rewrite the left-associated concatenation as a right-associated expression. Preserve every element, its order, and nested brackets. Return only the candidate RHS using Append and the declared lists.",
                &["A", "B", "C"],
                "(e.A) (e.B) (e.C)",
                "<Append <Append (e.A) (e.B)> (e.C)>",
            ),
            Self::AppendIdentity => (
                "Eliminate an empty-list append",
                "Simplify appending the empty list. Preserve every element and nested bracket; return the list directly, with no Append call. Return only the candidate RHS.",
                &["A"],
                "(e.A)",
                "<Append (e.A) ()>",
            ),
            Self::AppendFour => (
                "Reassociate four-list concatenation",
                "Rewrite the left-associated concatenation of four lists as a right-associated expression. Preserve order, empty lists, and nested brackets. Return only the candidate RHS using Append and the declared lists.",
                &["A", "B", "C", "D"],
                "(e.A) (e.B) (e.C) (e.D)",
                "<Append <Append <Append (e.A) (e.B)> (e.C)> (e.D)>",
            ),
        };
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(format!(
                "iaca-contract-v1\n{instruction}\n{pattern}\n{reference}\n{HELPERS}\n{}",
                crate::ENGINE_REVISION
            ))
        );
        Contract {
            id: self,
            title,
            instruction,
            domain: "Finite lists of Refal terms, including empty lists and arbitrarily nested brackets.",
            variables,
            pattern,
            reference,
            requirements: vec![
                "Only the candidate expression may change; the contract and helpers are fixed.",
                "The requested structural transformation must be present.",
                "Strict compiler checks and all independent execution tests must pass.",
                "Symbolic equivalence must be proved with every branch closed inside the budget.",
            ],
            proof_scope: "Refal Supersystem equivalence on this restricted expression language and fixed input domain. This is a research engine's proof result, not certification of arbitrary programs. Tests and reduction counts are separate evidence.",
            fingerprint,
        }
    }

    pub fn accepted(self) -> &'static str {
        match self {
            Self::AppendThree => "<Append (e.A) <Append (e.B) (e.C)>>",
            Self::AppendIdentity => "(e.A)",
            Self::AppendFour => "<Append (e.A) <Append (e.B) <Append (e.C) (e.D)>>>",
        }
    }

    pub fn mutation(self) -> &'static str {
        match self {
            Self::AppendThree => "<Append (e.A) <Append (e.C) (e.B)>>",
            Self::AppendIdentity => "()",
            Self::AppendFour => "<Append (e.A) <Append (e.B) <Append (e.D) (e.C)>>>",
        }
    }
}

pub fn all_contracts() -> Vec<Contract> {
    [
        TaskId::AppendThree,
        TaskId::AppendIdentity,
        TaskId::AppendFour,
    ]
    .into_iter()
    .map(TaskId::contract)
    .collect()
}
