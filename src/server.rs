use crate::{
    ENGINE_REVISION,
    contract::{TaskId, all_contracts},
    evidence::{Decision, Verification},
    provider::{COMPLETION_TOKEN_LIMIT, META_MODEL, MetaProvider, Proposal, Usage, feedback},
    verifier::{MAX_CANDIDATE_BYTES, NORMAL_PROOF_BUDGET, VerifyRequest},
    worker::isolated_verify,
};
use anyhow::{Result, bail};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::{RwLock, Semaphore};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    #[default]
    Fixture,
    Meta,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    #[default]
    Repair,
    Accepted,
    Rejected,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    #[serde(default)]
    pub task: TaskId,
    #[serde(default)]
    pub provider: ProviderKind,
    #[serde(default)]
    pub scenario: Scenario,
    pub candidate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    pub number: usize,
    pub candidate: String,
    pub explanation: String,
    pub provenance: String,
    pub generation_ms: u64,
    pub usage: Usage,
    pub verification: Verification,
    pub feedback: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub at_ms: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub created_at: u64,
    pub finished_at: Option<u64>,
    pub task: TaskId,
    pub provider: ProviderKind,
    pub scenario: Scenario,
    pub contract_fingerprint: String,
    pub status: String,
    pub phase: String,
    pub decision: Option<Decision>,
    pub attempts: Vec<Attempt>,
    pub events: Vec<Event>,
    pub error: Option<String>,
    pub cancel_requested: bool,
}

#[derive(Clone)]
pub struct AppState {
    runs: Arc<RwLock<BTreeMap<String, Run>>>,
    provider: Arc<MetaProvider>,
    slots: Arc<Semaphore>,
    executable: PathBuf,
    runs_dir: PathBuf,
}

type ApiResult<T> = std::result::Result<T, ApiError>;
struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub async fn serve(address: SocketAddr, web_dir: PathBuf, runs_dir: PathBuf) -> Result<()> {
    if !address.ip().is_loopback() {
        bail!("IACA binds to loopback; use an SSH tunnel for access from another machine.");
    }
    if !web_dir.join("index.html").is_file() {
        bail!("Web build not found. Run npm ci and npm run build in web/ first.");
    }
    let state = AppState {
        runs: Arc::new(RwLock::new(load_history(&runs_dir).await)),
        provider: Arc::new(MetaProvider::from_env()?),
        slots: Arc::new(Semaphore::new(1)),
        executable: std::env::current_exe()?,
        runs_dir,
    };
    let router = router(state)
        .fallback_service(ServeDir::new(&web_dir).not_found_service(ServeFile::new(web_dir.join("index.html"))))
        .layer(SetResponseHeaderLayer::if_not_present(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::if_not_present(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer")))
        .layer(SetResponseHeaderLayer::if_not_present(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'")));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("IACA is running at http://{}", listener.local_addr()?);
    println!("Fixtures use no API credits. Live Meta calls require an explicit run.");
    axum::serve(listener, router).await?;
    Ok(())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/tasks", get(|| async { Json(all_contracts()) }))
        .route("/api/runs", get(list_runs).post(start_run))
        .route("/api/runs/{id}", get(get_run))
        .route("/api/runs/{id}/export", get(export_run))
        .route("/api/runs/{id}/cancel", post(cancel_run))
        .layer(DefaultBodyLimit::max(8_192))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status":"ok", "version":env!("CARGO_PKG_VERSION"), "engine_revision":ENGINE_REVISION,
        "model":META_MODEL, "meta_configured":state.provider.configured(),
        "live_call_limit":state.provider.limit(), "live_calls_used":state.provider.used(),
        "live_calls_remaining":state.provider.remaining(), "completion_token_limit":COMPLETION_TOKEN_LIMIT,
        "max_attempts":2, "worker_deadline_seconds":8,
    }))
}

async fn list_runs(State(state): State<AppState>) -> Json<Vec<Run>> {
    let mut runs = state
        .runs
        .read()
        .await
        .values()
        .cloned()
        .collect::<Vec<_>>();
    runs.sort_by_key(|run| std::cmp::Reverse(run.created_at));
    Json(runs)
}

async fn get_run(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Run>> {
    state
        .runs
        .read()
        .await
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Run not found".into()))
}

async fn export_run(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Response> {
    let run = state
        .runs
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Run not found".into()))?;
    let filename = format!("attachment; filename=\"iaca-{}.json\"", run.id);
    let artifact = json!({"schema_version":1,"application_version":env!("CARGO_PKG_VERSION"),"contract":run.task.contract(),"engine_revision":ENGINE_REVISION,"run":run});
    Ok(([(header::CONTENT_DISPOSITION, filename)], Json(artifact)).into_response())
}

fn same_origin(headers: &HeaderMap) -> ApiResult<()> {
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        == Some("cross-site")
    {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "Cross-site run requests are not allowed".into(),
        ));
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let host = headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if origin.to_str().ok() != Some(format!("http://{host}").as_str()) {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "Request origin does not match this application".into(),
            ));
        }
    }
    Ok(())
}

async fn start_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RunRequest>,
) -> ApiResult<(StatusCode, Json<Run>)> {
    same_origin(&headers)?;
    if request.provider == ProviderKind::Meta
        && (!state.provider.configured() || state.provider.remaining() == 0)
    {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Live Meta is unavailable or this session's call limit is exhausted.".into(),
        ));
    }
    if request.provider == ProviderKind::Manual
        && request
            .candidate
            .as_ref()
            .is_none_or(|candidate| candidate.is_empty() || candidate.len() > MAX_CANDIDATE_BYTES)
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("Provide a candidate of 1–{MAX_CANDIDATE_BYTES} bytes."),
        ));
    }
    if request.provider != ProviderKind::Manual && request.candidate.is_some() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Candidate input is only supported in manual mode.".into(),
        ));
    }
    if request.provider != ProviderKind::Fixture && request.scenario != Scenario::Repair {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Scenarios are only available for labelled fixtures.".into(),
        ));
    }
    let permit = state.slots.clone().try_acquire_owned().map_err(|_| {
        ApiError(
            StatusCode::CONFLICT,
            "A run is already active. Wait for it or cancel it.".into(),
        )
    })?;
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = now_ms();
    let run = Run {
        id: id.clone(),
        created_at,
        finished_at: None,
        task: request.task,
        provider: request.provider,
        scenario: request.scenario,
        contract_fingerprint: request.task.contract().fingerprint,
        status: "running".into(),
        phase: "Preparing contract".into(),
        decision: None,
        attempts: vec![],
        events: vec![Event {
            at_ms: created_at,
            message: "Task contract frozen; acceptance policy is outside model control.".into(),
        }],
        error: None,
        cancel_requested: false,
    };
    {
        let mut runs = state.runs.write().await;
        if runs.len() >= 24
            && let Some(oldest) = runs
                .values()
                .filter(|run| run.status != "running")
                .min_by_key(|run| run.created_at)
                .map(|run| run.id.clone())
        {
            runs.remove(&oldest);
        }
        runs.insert(id.clone(), run.clone());
    }
    tokio::spawn(async move {
        let _permit = permit;
        execute_run(&state, &id, request).await;
        persist(&state, &id).await;
    });
    Ok((StatusCode::ACCEPTED, Json(run)))
}

async fn cancel_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> ApiResult<Json<Value>> {
    same_origin(&headers)?;
    let mut runs = state.runs.write().await;
    let run = runs
        .get_mut(&id)
        .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "Run not found".into()))?;
    if run.status == "running" {
        run.cancel_requested = true;
        run.phase = "Stopping after the in-flight operation".into();
        run.events.push(Event { at_ms: now_ms(), message: "Cancellation requested. An in-flight Meta request may still be billed; no further call will start.".into() });
    }
    Ok(Json(json!({"cancel_requested":run.cancel_requested})))
}

async fn cancelled(state: &AppState, id: &str) -> bool {
    let mut runs = state.runs.write().await;
    if let Some(run) = runs.get_mut(id)
        && run.cancel_requested
    {
        run.status = "cancelled".into();
        run.phase = "Cancelled".into();
        run.finished_at = Some(now_ms());
        run.decision = Some(Decision::Unresolved);
        return true;
    }
    false
}

async fn phase(state: &AppState, id: &str, message: String) {
    if let Some(run) = state.runs.write().await.get_mut(id) {
        run.phase = message.clone();
        run.events.push(Event {
            at_ms: now_ms(),
            message,
        });
    }
}

async fn execute_run(state: &AppState, id: &str, request: RunRequest) {
    let contract = request.task.contract();
    let max_attempts = match (request.provider, request.scenario) {
        (ProviderKind::Meta, _) => 2.min(state.provider.remaining()),
        (ProviderKind::Fixture, Scenario::Repair) => 2,
        _ => 1,
    };
    let mut previous = Vec::new();
    for index in 0..max_attempts {
        if cancelled(state, id).await {
            return;
        }
        phase(
            state,
            id,
            format!(
                "{} candidate {}",
                if request.provider == ProviderKind::Meta {
                    "Meta is generating"
                } else {
                    "Preparing"
                },
                index + 1
            ),
        )
        .await;
        let (proposal, usage, generation_ms, provenance) = match request.provider {
            ProviderKind::Meta => match state.provider.generate(&contract, &previous).await {
                Ok(generated) => (
                    generated.proposal,
                    generated.usage,
                    generated.elapsed_ms,
                    format!("Live Meta API · {META_MODEL}"),
                ),
                Err(error) => {
                    if let Some(run) = state.runs.write().await.get_mut(id) {
                        run.status = "failed".into();
                        run.phase = "Generation stopped".into();
                        run.error = Some(error.to_string());
                        run.decision = Some(Decision::Unresolved);
                        run.finished_at = Some(now_ms());
                        run.events.push(Event {
                            at_ms: now_ms(),
                            message: error.to_string(),
                        });
                    }
                    return;
                }
            },
            ProviderKind::Fixture => {
                let mutation = request.scenario == Scenario::Rejected
                    || (request.scenario == Scenario::Repair && index == 0);
                let explanation = if mutation {
                    "Deliberate mutation fixture: list contents are reordered or dropped. This is not a model-generated mistake."
                } else if index > 0 {
                    "Scripted repair fixture: replace the mutation with the correct transformation after receiving real checker feedback."
                } else {
                    "Known-correct fixture used to exercise the actual verification pipeline."
                };
                (
                    Proposal {
                        candidate: if mutation {
                            request.task.mutation()
                        } else {
                            request.task.accepted()
                        }
                        .into(),
                        explanation: explanation.into(),
                    },
                    Usage::default(),
                    0,
                    "Deterministic fixture · no API call".into(),
                )
            }
            ProviderKind::Manual => (
                Proposal {
                    candidate: request.candidate.clone().unwrap_or_default(),
                    explanation:
                        "User-supplied expression, checked against the same fixed contract.".into(),
                },
                Usage::default(),
                0,
                "Manual candidate · no API call".into(),
            ),
        };
        if cancelled(state, id).await {
            return;
        }
        phase(
            state,
            id,
            format!("Checking candidate {} with the Refal engine", index + 1),
        )
        .await;
        let proof_budget = if request.provider == ProviderKind::Fixture
            && request.scenario == Scenario::Unresolved
        {
            0
        } else {
            NORMAL_PROOF_BUDGET
        };
        let verification = isolated_verify(
            &state.executable,
            &VerifyRequest {
                task: request.task,
                candidate: proposal.candidate.clone(),
                proof_budget,
            },
        )
        .await;
        let decision = verification.decision;
        let feedback = feedback(&verification);
        if let Some(run) = state.runs.write().await.get_mut(id) {
            run.attempts.push(Attempt {
                number: index + 1,
                candidate: proposal.candidate.clone(),
                explanation: proposal.explanation.clone(),
                provenance,
                generation_ms,
                usage,
                verification: verification.clone(),
                feedback,
            });
            run.events.push(Event {
                at_ms: now_ms(),
                message: format!(
                    "Candidate {}: {}",
                    index + 1,
                    match decision {
                        Decision::Accepted => "accepted; all required evidence passed",
                        Decision::Rejected => "rejected; concrete contract violation",
                        Decision::Unresolved => "unresolved; required evidence is incomplete",
                    }
                ),
            });
        }
        if cancelled(state, id).await {
            return;
        }
        previous.push((proposal, verification));
        if decision == Decision::Accepted || index + 1 == max_attempts {
            if let Some(run) = state.runs.write().await.get_mut(id) {
                run.status = "completed".into();
                run.phase = "Run complete".into();
                run.decision = Some(decision);
                run.finished_at = Some(now_ms());
            }
            return;
        }
        phase(
            state,
            id,
            "Returning actual checker evidence for revision".into(),
        )
        .await;
    }
}

async fn persist(state: &AppState, id: &str) {
    let run = state.runs.read().await.get(id).cloned();
    if let Some(run) = run {
        let save = async {
            tokio::fs::create_dir_all(&state.runs_dir).await?;
            let temporary = state.runs_dir.join(format!("{}.tmp", run.id));
            tokio::fs::write(&temporary, serde_json::to_vec_pretty(&run)?).await?;
            tokio::fs::rename(temporary, state.runs_dir.join(format!("{}.json", run.id))).await?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if save.is_err()
            && let Some(run) = state.runs.write().await.get_mut(id)
        {
            run.events.push(Event {
                at_ms: now_ms(),
                message:
                    "Could not save run history to disk; use Export evidence to retain this run."
                        .into(),
            });
        }
    }
}

async fn load_history(directory: &std::path::Path) -> BTreeMap<String, Run> {
    let mut runs = Vec::new();
    if let Ok(mut entries) = tokio::fs::read_dir(directory).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("json")
            {
                continue;
            }
            if entry
                .metadata()
                .await
                .map_or(true, |metadata| metadata.len() > 262_144)
            {
                continue;
            }
            if let Ok(bytes) = tokio::fs::read(entry.path()).await
                && let Ok(mut run) = serde_json::from_slice::<Run>(&bytes)
            {
                if run.contract_fingerprint != run.task.contract().fingerprint
                    || uuid::Uuid::parse_str(&run.id).is_err()
                {
                    continue;
                }
                if run.status == "running" {
                    run.status = "failed".into();
                    run.decision = Some(Decision::Unresolved);
                    run.phase = "Server stopped before completion".into();
                }
                runs.push(run);
            }
        }
    }
    runs.sort_by_key(|run| std::cmp::Reverse(run.created_at));
    runs.into_iter()
        .take(24)
        .map(|run| (run.id.clone(), run))
        .collect()
}
