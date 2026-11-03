use anyhow::{Result, bail};
use iaca::{
    contract::TaskId,
    evidence::{Decision, Verification},
    verifier::{NORMAL_PROOF_BUDGET, VerifyRequest, verify},
    worker::isolated_verify,
};
use std::io::{Read, Write};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("_worker") => {
            let mut input = String::new();
            std::io::stdin().take(8_193).read_to_string(&mut input)?;
            if input.len() > 8_192 {
                bail!("Worker input exceeds limit");
            }
            let request: VerifyRequest = serde_json::from_str(&input)?;
            serde_json::to_writer(std::io::stdout().lock(), &verify(&request))?;
            std::io::stdout().flush()?;
        }
        Some("verify-demo") => {
            let variant = args.get(1).map(String::as_str).unwrap_or("accepted");
            let candidate = match variant {
                "accepted" | "unresolved" => TaskId::AppendThree.accepted(),
                "rejected" => TaskId::AppendThree.mutation(),
                _ => bail!("Use accepted, rejected, or unresolved"),
            };
            let request = VerifyRequest {
                task: TaskId::AppendThree,
                candidate: candidate.into(),
                proof_budget: if variant == "unresolved" {
                    0
                } else {
                    NORMAL_PROOF_BUDGET
                },
            };
            print_and_exit(isolated_verify(&std::env::current_exe()?, &request).await)?;
        }
        Some("verify") => {
            let candidate = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("Provide a quoted candidate expression"))?;
            let request = VerifyRequest {
                task: TaskId::AppendThree,
                candidate: candidate.clone(),
                proof_budget: NORMAL_PROOF_BUDGET,
            };
            print_and_exit(isolated_verify(&std::env::current_exe()?, &request).await)?;
        }
        Some("serve") => {
            let address = args
                .get(1)
                .map(String::as_str)
                .unwrap_or("127.0.0.1:8787")
                .parse()?;
            let web_dir = std::env::var_os("IACA_WEB_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| "web/dist".into());
            let runs_dir = std::env::var_os("IACA_RUNS_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| "runs".into());
            iaca::server::serve(address, web_dir, runs_dir).await?;
        }
        _ => println!(
            "IACA — Intelligent Autonomous Coding Agent\n\n  iaca serve [127.0.0.1:8787]\n  iaca verify-demo accepted|rejected|unresolved\n  iaca verify '<Append (e.A) <Append (e.B) (e.C)>>'"
        ),
    }
    Ok(())
}

fn print_and_exit(evidence: Verification) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    std::io::stdout().flush()?;
    std::process::exit(match evidence.decision {
        Decision::Accepted => 0,
        Decision::Rejected => 1,
        Decision::Unresolved => 2,
    });
}
