use crate::{
    evidence::Verification,
    verifier::{VerifyRequest, unavailable},
};
use anyhow::{Context, Result, bail};
use std::{
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

pub const WORKER_TIMEOUT: Duration = Duration::from_secs(8);
const OUTPUT_LIMIT: usize = 65_536;

async fn bounded_read(reader: impl AsyncRead + Unpin) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    reader
        .take(OUTPUT_LIMIT as u64 + 1)
        .read_to_end(&mut output)
        .await?;
    if output.len() > OUTPUT_LIMIT {
        bail!("Verifier output exceeded 64 KiB");
    }
    Ok(output)
}

/// The worker has no API credentials and receives no command-line code or
/// shell expression. Dropping or timing out this future kills the child.
pub async fn isolated_verify(executable: &Path, request: &VerifyRequest) -> Verification {
    let started = Instant::now();
    let result = tokio::time::timeout(WORKER_TIMEOUT, execute(executable, request)).await;
    let mut evidence = match result {
        Ok(Ok(evidence)) => evidence,
        Ok(Err(error)) => unavailable(request, format!("Verifier unavailable: {error}")),
        Err(_) => unavailable(
            request,
            "Verifier exceeded its eight-second deadline and was stopped.".into(),
        ),
    };
    evidence.elapsed_ms = started.elapsed().as_millis() as u64;
    evidence
}

async fn execute(executable: &Path, request: &VerifyRequest) -> Result<Verification> {
    let mut command = Command::new(executable);
    command
        .arg("_worker")
        .env_clear()
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Windows needs this for native library loading. No inherited key or PATH.
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    let mut child = command
        .spawn()
        .context("Could not start the verification worker")?;
    let mut stdin = child.stdin.take().context("Missing worker stdin")?;
    let stdout = child.stdout.take().context("Missing worker stdout")?;
    let stderr = child.stderr.take().context("Missing worker stderr")?;
    let bytes = serde_json::to_vec(request)?;
    let send = async {
        stdin.write_all(&bytes).await?;
        stdin.shutdown().await?;
        drop(stdin);
        Ok::<_, anyhow::Error>(())
    };
    let wait = async { Ok::<_, anyhow::Error>(child.wait().await?) };
    let (_, output, _errors, status) =
        tokio::try_join!(send, bounded_read(stdout), bounded_read(stderr), wait)?;
    if !status.success() {
        bail!("Verification process exited without valid evidence ({status})");
    }
    let evidence: Verification =
        serde_json::from_slice(&output).context("Worker returned invalid JSON")?;
    if evidence.candidate != request.candidate
        || evidence.contract_fingerprint != request.task.contract().fingerprint
        || evidence.engine_revision != crate::ENGINE_REVISION
    {
        bail!("Verification evidence does not match the submitted contract and candidate");
    }
    Ok(evidence)
}
