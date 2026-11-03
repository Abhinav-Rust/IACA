use crate::{
    ENGINE_REVISION,
    contract::{HELPERS, TaskId},
    evidence::*,
};
use refal_ast::{
    Item, Program, Term, TermKind, VariableKind, Visibility, canonical_identifier,
    canonical_variable_index,
};
use refal_core::{EquivalenceLeaf, ProofVerdict};
use refal_runtime::{Evaluator, Value};
use refal_semantics::{Mode, check_program_with_mode, failing};
use refal_syntax::{Lexer, Parser, TokenKind};
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub const NORMAL_PROOF_BUDGET: usize = 5_000;
pub const MAX_CANDIDATE_BYTES: usize = 1_024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifyRequest {
    pub task: TaskId,
    pub candidate: String,
    pub proof_budget: usize,
}

fn parse(source: &str) -> Result<Program, String> {
    let tokens = Lexer::new(source).tokenize().map_err(|e| e.message)?;
    Parser::new(tokens).parse_program().map_err(|e| e.message)
}

/// Validate tokens before parsing, then validate the AST. A model cannot inject
/// a sentence, directive, helper definition, block, condition or builtin call.
fn candidate_program(request: &VerifyRequest) -> Result<(Program, Vec<Term>, String), String> {
    if request.candidate.trim().is_empty() || request.candidate.len() > MAX_CANDIDATE_BYTES {
        return Err(format!(
            "Candidate must contain 1–{MAX_CANDIDATE_BYTES} bytes."
        ));
    }
    let tokens = Lexer::new(&request.candidate)
        .tokenize()
        .map_err(|e| e.message)?;
    let mut stack = Vec::new();
    for token in &tokens {
        match token.kind {
            TokenKind::LParen | TokenKind::LAngle => {
                stack.push(&token.kind);
                if stack.len() > 12 { return Err("Expression nesting exceeds 12 levels.".into()); }
            }
            TokenKind::RParen | TokenKind::RAngle => {
                let matched = matches!((stack.pop(), &token.kind),
                    (Some(TokenKind::LParen), TokenKind::RParen) | (Some(TokenKind::LAngle), TokenKind::RAngle));
                if !matched { return Err("Mismatched expression delimiters.".into()); }
            }
            TokenKind::Identifier(_) | TokenKind::Variable { .. } | TokenKind::Number(_) | TokenKind::Char(_) | TokenKind::Eof => {}
            _ => return Err("Only a RHS expression is allowed; declarations, sentences and conditions are forbidden.".into()),
        }
    }
    if !stack.is_empty() {
        return Err("Unclosed expression delimiter.".into());
    }
    let contract = request.task.contract();
    let wrapper = format!(
        "Candidate {{ {} = {}; }}",
        contract.pattern, request.candidate
    );
    let ast = parse(&wrapper)?;
    let [Item::Function(function)] = ast.items.as_slice() else {
        return Err("Expected exactly one candidate function.".into());
    };
    if function.name != "Candidate"
        || function.visibility != Visibility::Local
        || function.sentences.len() != 1
        || !function.sentences[0].conditions.is_empty()
    {
        return Err("Candidate changed the trusted function structure.".into());
    }
    let result = function.sentences[0].result.clone();
    let mut count = 0;
    check_terms(&result, contract.variables, &mut count)?;
    // A bracket or a total Append of two bracket-valued expressions is a list.
    // This grammar prevents partial helper invocation or executable recursion.
    if result.len() != 1 || !list_expression(&result[0]) {
        return Err("Return one bracketed list or Append of exactly two list expressions.".into());
    }
    let source = format!(
        "$ENTRY Go {{ {} = <Candidate {}>; }}\nReference {{ {} = {}; }}\n{wrapper}\n{HELPERS}",
        contract.pattern, contract.pattern, contract.pattern, contract.reference,
    );
    Ok((parse(&source)?, result, source))
}

fn check_terms(terms: &[Term], variables: &[&str], count: &mut usize) -> Result<(), String> {
    for term in terms {
        *count += 1;
        if *count > 96 {
            return Err("Expression exceeds 96 syntax nodes.".into());
        }
        match &term.kind {
            TermKind::Variable(v)
                if v.kind == VariableKind::Expression
                    && variables.contains(&canonical_variable_index(&v.name).as_str()) => {}
            TermKind::Variable(_) => {
                return Err("Only declared expression variables may be used.".into());
            }
            TermKind::Symbol(_) => {}
            TermKind::Bracket(inner) => check_terms(inner, variables, count)?,
            TermKind::Call { name, args } if canonical_identifier(name) == "APPEND" => {
                check_terms(args, variables, count)?
            }
            TermKind::Call { .. } => {
                return Err("Only the fixed Append helper may be called.".into());
            }
            TermKind::Block { .. } => return Err("Executable blocks are forbidden.".into()),
        }
    }
    Ok(())
}

fn list_expression(term: &Term) -> bool {
    match &term.kind {
        TermKind::Bracket(inner) => pure_data(inner),
        TermKind::Call { name, args } => {
            canonical_identifier(name) == "APPEND"
                && args.len() == 2
                && args.iter().all(list_expression)
        }
        _ => false,
    }
}

fn pure_data(terms: &[Term]) -> bool {
    terms.iter().all(|t| match &t.kind {
        TermKind::Symbol(_) | TermKind::Variable(_) => true,
        TermKind::Bracket(inner) => pure_data(inner),
        _ => false,
    })
}

fn transformed(task: TaskId, result: &[Term]) -> bool {
    fn right_associated(term: &Term) -> bool {
        match &term.kind {
            TermKind::Bracket(_) => true,
            TermKind::Call { args, .. } => {
                matches!(args[0].kind, TermKind::Bracket(_)) && right_associated(&args[1])
            }
            _ => false,
        }
    }
    match task {
        TaskId::AppendIdentity => matches!(result[0].kind, TermKind::Bracket(_)),
        _ => matches!(result[0].kind, TermKind::Call { .. }) && right_associated(&result[0]),
    }
}

pub fn unavailable(request: &VerifyRequest, reason: String) -> Verification {
    Verification {
        decision: Decision::Unresolved,
        reason,
        contract_fingerprint: request.task.contract().fingerprint,
        engine_revision: ENGINE_REVISION.into(),
        candidate: request.candidate.clone(),
        source: None,
        boundary: Gate {
            passed: false,
            details: vec!["Verification did not complete.".into()],
        },
        compiler: None,
        transformation: None,
        tests: None,
        proof: None,
        elapsed_ms: 0,
    }
}

pub fn verify(request: &VerifyRequest) -> Verification {
    let started = Instant::now();
    let mut evidence = unavailable(request, "Verification did not complete.".into());
    if request.proof_budget > NORMAL_PROOF_BUDGET {
        evidence.reason = "Proof budget exceeds the application limit.".into();
        return evidence;
    }
    let (program, result, source) = match candidate_program(request) {
        Ok(parsed) => parsed,
        Err(error) => {
            evidence.decision = Decision::Rejected;
            evidence.reason = "Candidate violates the fixed expression boundary.".into();
            evidence.boundary.details = vec![error];
            evidence.elapsed_ms = started.elapsed().as_millis() as u64;
            return evidence;
        }
    };
    evidence.source = Some(source);
    evidence.boundary = Gate { passed: true, details: vec!["Parsed and structurally restricted to total list expressions and the fixed Append helper.".into()] };
    let diagnostics = check_program_with_mode(&program, Mode::Strict);
    let compiler_ok = failing(&diagnostics, Mode::Strict).is_empty();
    evidence.compiler = Some(Gate {
        passed: compiler_ok,
        details: diagnostics.iter().map(|d| d.render()).collect(),
    });
    let shape_ok = transformed(request.task, &result);
    evidence.transformation = Some(Gate {
        passed: shape_ok,
        details: vec![
            if shape_ok {
                "The requested structural transformation is present."
            } else {
                "The expression does not perform the requested structural transformation."
            }
            .into(),
        ],
    });
    if !compiler_ok {
        evidence.decision = Decision::Rejected;
        evidence.reason = "Strict compiler checks failed.".into();
    } else {
        match concrete_tests(&program, request.task) {
            Err(reason) => {
                evidence.reason = reason;
            }
            Ok(tests) => {
                let tests_ok = tests.passed == tests.total;
                evidence.tests = Some(tests);
                let core = refal_core::lower_program(&program);
                match refal_core::prove_equivalence(
                    &core,
                    "Reference",
                    "Candidate",
                    request.proof_budget,
                ) {
                    Err(error) => {
                        evidence.reason =
                            format!("Symbolic engine could not evaluate the claim: {error}");
                    }
                    Ok(report) => {
                        let proof_ok = report.verdict == ProofVerdict::Proved
                            && report.complete
                            && !report.leaves.is_empty();
                        let refuted = matches!(report.verdict, ProofVerdict::Refuted { .. });
                        evidence.proof = Some(proof_evidence(&report, request.proof_budget));
                        if !tests_ok || refuted {
                            evidence.decision = Decision::Rejected;
                            evidence.reason = "Candidate disagrees with the fixed contract; inspect the concrete counterexample and symbolic evidence.".into();
                        } else if !shape_ok {
                            evidence.decision = Decision::Rejected;
                            evidence.reason = "Behavior is consistent with the reference, but the requested transformation was not performed.".into();
                        } else if proof_ok {
                            evidence.decision = Decision::Accepted;
                            evidence.reason = "Boundary, transformation, strict checks, execution tests, and symbolic equivalence all passed.".into();
                        } else {
                            evidence.reason = "Execution tests passed, but the required symbolic proof did not close. No candidate was accepted.".into();
                        }
                    }
                }
                // A witnessed failure remains rejected even if the symbolic engine errors.
                if !tests_ok || !shape_ok {
                    evidence.decision = Decision::Rejected;
                }
            }
        }
    }
    evidence.elapsed_ms = started.elapsed().as_millis() as u64;
    evidence
}

fn proof_evidence(report: &refal_core::EquivalenceReport, budget: usize) -> ProofEvidence {
    ProofEvidence {
        verdict: match report.verdict {
            ProofVerdict::Proved => "proved",
            ProofVerdict::Refuted { .. } => "refuted",
            ProofVerdict::Open => "open",
            ProofVerdict::Incomplete { .. } => "incomplete",
        }
        .into(),
        complete: report.complete,
        steps: report.steps,
        budget,
        report: refal_core::format_equivalence_report(report),
        leaves: report
            .leaves
            .iter()
            .map(|leaf| {
                let mut item = ProofLeaf {
                    kind: String::new(),
                    depth: None,
                    ancestor: None,
                    left: None,
                    right: None,
                };
                match leaf {
                    EquivalenceLeaf::Reflexive { depth } => {
                        item.kind = "reflexive".into();
                        item.depth = Some(*depth);
                    }
                    EquivalenceLeaf::Folded { depth, ancestor } => {
                        item.kind = "folded".into();
                        item.depth = Some(*depth);
                        item.ancestor = Some(*ancestor);
                    }
                    EquivalenceLeaf::Generalized { depth, ancestor } => {
                        item.kind = "generalized".into();
                        item.depth = Some(*depth);
                        item.ancestor = Some(*ancestor);
                    }
                    EquivalenceLeaf::Stuck { depth } => {
                        item.kind = "stuck".into();
                        item.depth = Some(*depth);
                    }
                    EquivalenceLeaf::Refuted { left, right } => {
                        item.kind = "refuted".into();
                        item.left = Some(left.clone());
                        item.right = Some(right.clone());
                    }
                }
                item
            })
            .collect(),
    }
}

fn display(values: &[Value]) -> String {
    values
        .iter()
        .map(|value| match value {
            Value::Char(c) => format!("'{c}'"),
            Value::Identifier(name) | Value::Number(name) => name.clone(),
            Value::Bracket(inner) => format!("({})", display(inner)),
            Value::Unknown(_) => "<unknown>".into(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn concrete_tests(program: &Program, task: TaskId) -> Result<TestEvidence, String> {
    let count = task.contract().variables.len();
    let samples = [
        vec![],
        vec![Value::Char('a')],
        vec![Value::Char('b'), Value::Char('a')],
        vec![Value::bracket(vec![])],
        vec![Value::bracket(vec![
            Value::Char('x'),
            Value::bracket(vec![Value::Number("7".into())]),
        ])],
        vec![
            Value::Identifier("Node".into()),
            Value::Number("42".into()),
            Value::Char('z'),
        ],
    ];
    let total = samples.len().pow(count as u32);
    let mut evidence = TestEvidence {
        total, passed: 0, cases: vec![],
        oracle: "Independent Rust concatenation of list contents; all combinations of six declared empty, atomic, multi-element and nested samples.".into(),
        reference_steps: 0, candidate_steps: 0,
    };
    for index in 0..total {
        let mut choice = index;
        let mut input = Vec::with_capacity(count);
        let mut expected_contents = Vec::new();
        for _ in 0..count {
            let list = &samples[choice % samples.len()];
            choice /= samples.len();
            expected_contents.extend(list.clone());
            input.push(Value::bracket(list.clone()));
        }
        let expected = vec![Value::bracket(expected_contents)];
        let evaluator = Evaluator::with_max_call_depth(program, 128);
        let reference = evaluator
            .evaluate_function("Reference", &input)
            .map_err(|e| format!("Reference execution failed: {e}"))?;
        let reference_steps = evaluator.steps();
        if reference != expected {
            return Err(
                "Trusted reference disagrees with the independent oracle; verification stopped."
                    .into(),
            );
        }
        let actual = evaluator.evaluate_function("Candidate", &input);
        let candidate_steps = evaluator.steps() - reference_steps;
        let (passed, actual) = match actual {
            Ok(output) => (output == expected, display(&output)),
            Err(error) => (false, format!("Runtime error: {error}")),
        };
        evidence.passed += usize::from(passed);
        evidence.reference_steps += reference_steps;
        evidence.candidate_steps += candidate_steps;
        // Preserve all failures up to 12 and representative successes. Aggregate
        // totals and step counts cover every test, even those omitted from the UI.
        let failures_saved = evidence.cases.iter().filter(|case| !case.passed).count();
        if (passed && evidence.cases.len() < 6) || (!passed && failures_saved < 12) {
            evidence.cases.push(TestCase {
                name: format!("case-{:04}", index + 1),
                input: display(&input),
                expected: display(&expected),
                actual,
                passed,
                reference_steps,
                candidate_steps,
            });
        }
    }
    evidence.cases.sort_by_key(|case| case.passed);
    Ok(evidence)
}
