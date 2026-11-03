use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Accepted,
    Rejected,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gate {
    pub passed: bool,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub input: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub reference_steps: usize,
    pub candidate_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEvidence {
    pub total: usize,
    pub passed: usize,
    pub cases: Vec<TestCase>,
    pub oracle: String,
    pub reference_steps: usize,
    pub candidate_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofLeaf {
    pub kind: String,
    pub depth: Option<usize>,
    pub ancestor: Option<usize>,
    pub left: Option<String>,
    pub right: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofEvidence {
    pub verdict: String,
    pub complete: bool,
    pub steps: usize,
    pub budget: usize,
    pub leaves: Vec<ProofLeaf>,
    pub report: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    pub decision: Decision,
    pub reason: String,
    pub contract_fingerprint: String,
    pub engine_revision: String,
    pub candidate: String,
    pub source: Option<String>,
    pub boundary: Gate,
    pub compiler: Option<Gate>,
    pub transformation: Option<Gate>,
    pub tests: Option<TestEvidence>,
    pub proof: Option<ProofEvidence>,
    pub elapsed_ms: u64,
}
