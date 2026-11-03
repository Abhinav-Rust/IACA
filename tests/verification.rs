use iaca::{
    contract::TaskId,
    evidence::Decision,
    verifier::{NORMAL_PROOF_BUDGET, VerifyRequest, verify},
};

fn check(task: TaskId, candidate: &str, budget: usize) -> iaca::evidence::Verification {
    verify(&VerifyRequest {
        task,
        candidate: candidate.into(),
        proof_budget: budget,
    })
}

#[test]
fn real_equivalence_proofs_accept_all_three_transformations() {
    for task in [
        TaskId::AppendThree,
        TaskId::AppendIdentity,
        TaskId::AppendFour,
    ] {
        let report = check(task, task.accepted(), NORMAL_PROOF_BUDGET);
        assert_eq!(report.decision, Decision::Accepted, "{report:#?}");
        let tests = report.tests.unwrap();
        assert_eq!(tests.passed, tests.total);
        assert!(tests.candidate_steps < tests.reference_steps);
        assert!(report.proof.unwrap().complete);
    }
}

#[test]
fn mutations_are_rejected_with_reproducible_test_counterexamples() {
    for task in [
        TaskId::AppendThree,
        TaskId::AppendIdentity,
        TaskId::AppendFour,
    ] {
        let report = check(task, task.mutation(), NORMAL_PROOF_BUDGET);
        assert_eq!(report.decision, Decision::Rejected, "{report:#?}");
        let tests = report.tests.unwrap();
        assert!(tests.passed < tests.total);
        assert!(
            tests
                .cases
                .iter()
                .any(|case| !case.passed && case.actual != case.expected)
        );
    }
}

#[test]
fn passing_tests_do_not_replace_an_exhausted_proof() {
    let report = check(TaskId::AppendThree, TaskId::AppendThree.accepted(), 0);
    assert_eq!(report.decision, Decision::Unresolved, "{report:#?}");
    let tests = report.tests.unwrap();
    assert_eq!(tests.passed, tests.total);
    assert_eq!(report.proof.unwrap().verdict, "incomplete");
}

#[test]
fn injection_partial_calls_and_undeclared_variables_are_rejected_before_execution() {
    for candidate in [
        "(e.A); } $EXTERN System; Evil { = <System 'whoami'>; } F { = ()",
        "<System 'whoami'>",
        "<Append (e.A)>",
        "<Append e.A e.B>",
        "<Append-Contents (e.A) (e.B)>",
        "<Candidate (e.A) (e.B) (e.C)>",
        "(e.D)",
        "(s.A)",
        "(e.A), e.X : e.X = ()",
        "( <Append (e.A) (e.B)> )",
        "",
    ] {
        let report = check(TaskId::AppendThree, candidate, NORMAL_PROOF_BUDGET);
        assert_eq!(
            report.decision,
            Decision::Rejected,
            "{candidate}: {report:#?}"
        );
        assert!(!report.boundary.passed);
        assert!(report.tests.is_none());
    }
}

#[test]
fn nesting_and_size_limits_apply_before_recursive_parsing() {
    for candidate in [
        format!("{}e.A{}", "(".repeat(300), ")".repeat(300)),
        "a".repeat(1025),
    ] {
        let report = check(TaskId::AppendThree, &candidate, NORMAL_PROOF_BUDGET);
        assert_eq!(report.decision, Decision::Rejected);
        assert!(!report.boundary.passed);
    }
}

#[test]
fn unchanged_reference_is_not_a_completed_transformation() {
    let report = check(
        TaskId::AppendThree,
        TaskId::AppendThree.contract().reference,
        NORMAL_PROOF_BUDGET,
    );
    assert_eq!(report.decision, Decision::Rejected);
    assert!(!report.transformation.unwrap().passed);
    assert_eq!(report.proof.unwrap().verdict, "proved");
}

#[test]
fn refal_case_equivalence_does_not_bypass_the_boundary() {
    let report = check(
        TaskId::AppendThree,
        "<Append (e.a) <APPEND (e.b) (e.c)>>",
        NORMAL_PROOF_BUDGET,
    );
    assert_eq!(report.decision, Decision::Accepted, "{report:#?}");
}
