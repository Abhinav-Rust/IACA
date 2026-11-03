use iaca::{
    contract::TaskId, evidence::Decision, server::Run, verifier::VerifyRequest,
    worker::isolated_verify,
};
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

#[tokio::test]
async fn real_http_loop_preserves_evidence_and_never_uses_api_credits() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("index.html"),
        "<!doctype html><title>IACA integration test</title>",
    )
    .unwrap();
    let mut server = Command::new(env!("CARGO_BIN_EXE_iaca"))
        .arg("serve")
        .arg("127.0.0.1:0")
        .env("MODEL_API_KEY", "")
        .env("IACA_LIVE_CALL_LIMIT", "0")
        .env("IACA_WEB_DIR", directory.path())
        .env("IACA_RUNS_DIR", directory.path().join("runs"))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut lines = BufReader::new(server.stdout.take().unwrap()).lines();
    let line = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let base = line
        .strip_prefix("IACA is running at ")
        .unwrap()
        .to_string();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .unwrap();

    let health: Value = client
        .get(format!("{base}/api/health"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["live_calls_used"], 0);
    assert_eq!(health["live_call_limit"], 0);
    assert_eq!(health["meta_configured"], false);
    assert!(!health.to_string().contains("MODEL_API_KEY"));

    let response = client.get(&base).send().await.unwrap();
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(
        response.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("frame-ancestors 'none'")
    );

    let response = client
        .post(format!("{base}/api/runs"))
        .header("Origin", "https://untrusted.invalid")
        .json(&json!({"provider":"fixture"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let response = client
        .post(format!("{base}/api/runs"))
        .json(&json!({"provider":"meta"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let response = client
        .post(format!("{base}/api/runs"))
        .json(&json!({"provider":"fixture","proof_budget":0}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        422,
        "The client cannot change proof policy through unrecognized fields"
    );

    for (scenario, expected, expected_attempts) in [
        ("repair", Decision::Accepted, 2),
        ("rejected", Decision::Rejected, 1),
        ("unresolved", Decision::Unresolved, 1),
    ] {
        let response = client
            .post(format!("{base}/api/runs"))
            .json(&json!({"task":"append_three","provider":"fixture","scenario":scenario}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 202);
        let created: Run = response.json().await.unwrap();
        let run = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let run: Run = client
                    .get(format!("{base}/api/runs/{}", created.id))
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap();
                if run.status != "running" {
                    break run;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(run.decision, Some(expected), "{run:#?}");
        assert_eq!(run.attempts.len(), expected_attempts);
        assert!(
            run.attempts.iter().all(
                |attempt| attempt.verification.contract_fingerprint == run.contract_fingerprint
            )
        );
        if scenario == "repair" {
            assert_eq!(run.attempts[0].verification.decision, Decision::Rejected);
            assert!(run.attempts[0].feedback.contains("counterexamples"));
            assert!(
                run.events
                    .iter()
                    .any(|event| event.message.contains("actual checker evidence"))
            );
        }
        let exported = client
            .get(format!("{base}/api/runs/{}/export", run.id))
            .send()
            .await
            .unwrap();
        assert!(exported.headers().contains_key("content-disposition"));
        let artifact: Value = exported.json().await.unwrap();
        assert_eq!(artifact["run"]["id"], run.id);
        assert_eq!(
            artifact["contract"]["fingerprint"],
            run.contract_fingerprint
        );
    }

    let response = client
        .post(format!("{base}/api/runs"))
        .json(&json!({"provider":"manual","candidate":"<System 'echo not allowed'>"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 202);
    let created: Run = response.json().await.unwrap();
    let rejected = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let run: Run = client
                .get(format!("{base}/api/runs/{}", created.id))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            if run.status != "running" {
                break run;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(rejected.decision, Some(Decision::Rejected));
    assert!(rejected.attempts[0].verification.tests.is_none());
    let health: Value = client
        .get(format!("{base}/api/health"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["live_calls_used"], 0);
    server.kill().await.unwrap();
    server.wait().await.unwrap();
}

#[tokio::test]
async fn absent_verifier_is_unresolved_and_cannot_accept() {
    let directory = tempfile::tempdir().unwrap();
    let request = VerifyRequest {
        task: TaskId::AppendThree,
        candidate: TaskId::AppendThree.accepted().into(),
        proof_budget: 5000,
    };
    let report = isolated_verify(&directory.path().join("does-not-exist"), &request).await;
    assert_eq!(report.decision, Decision::Unresolved);
    assert!(report.tests.is_none());
    assert!(report.proof.is_none());
}

#[tokio::test]
async fn cli_exit_codes_distinguish_acceptance_rejection_and_uncertainty() {
    for (scenario, exit_code) in [("accepted", 0), ("rejected", 1), ("unresolved", 2)] {
        let output = Command::new(env!("CARGO_BIN_EXE_iaca"))
            .arg("verify-demo")
            .arg(scenario)
            .output()
            .await
            .unwrap();
        assert_eq!(output.status.code(), Some(exit_code));
        let report: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["decision"], scenario);
    }
}
