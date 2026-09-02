use anyhow::{anyhow, Result};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use readtrace_core::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap, convert::Infallible, net::SocketAddr, path::PathBuf, sync::Arc,
    time::Duration,
};
use tokio::{
    sync::{Mutex, RwLock},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MAX_UPLOAD_BYTES: usize = 512 * 1024 * 1024;
const MAX_UPLOAD_FILE_BYTES: usize = 256 * 1024 * 1024;
const MAX_UPLOAD_FILES: usize = 20_000;

#[derive(Clone)]
struct AppState {
    project: Arc<RwLock<Arc<ProjectStore>>>,
    workspace: Arc<RwLock<Option<WorkspaceStore>>>,
    tasks: TaskRegistry,
}

/// A local provider profile.  The optional API key is persisted only in the
/// user configuration directory (never in the project/Vault); it is omitted
/// from every JSON response so the browser can only see `key_present`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProviderProfile {
    id: String,
    name: String,
    kind: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default = "default_endpoint_path")]
    endpoint_path: String,
    model: String,
    #[serde(default)]
    api_key_env: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default = "default_auth_header")]
    auth_header: String,
    #[serde(default = "default_auth_scheme")]
    auth_scheme: String,
    #[serde(default = "default_max_tokens_field")]
    max_tokens_field: String,
    #[serde(default = "default_response_format")]
    response_format: String,
    #[serde(default)]
    thinking_mode: String,
    #[serde(default)]
    input_price_per_million: f64,
    #[serde(default)]
    cached_input_price_per_million: f64,
    #[serde(default)]
    output_price_per_million: f64,
    #[serde(default)]
    pricing_version: String,
    #[serde(default = "default_profile_enabled")]
    enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ProviderProfileView {
    id: String,
    name: String,
    kind: String,
    base_url: Option<String>,
    endpoint: Option<String>,
    endpoint_path: String,
    model: String,
    api_key_env: String,
    key_present: bool,
    auth_header: String,
    auth_scheme: String,
    max_tokens_field: String,
    response_format: String,
    thinking_mode: String,
    input_price_per_million: f64,
    cached_input_price_per_million: f64,
    output_price_per_million: f64,
    pricing_version: String,
    enabled: bool,
    builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProviderRegistry {
    schema_version: u32,
    #[serde(default)]
    profiles: Vec<ProviderProfile>,
}

#[derive(Debug, Deserialize)]
struct ProviderProfileRequest {
    id: Option<String>,
    name: String,
    kind: String,
    base_url: Option<String>,
    endpoint: Option<String>,
    endpoint_path: Option<String>,
    model: String,
    api_key_env: Option<String>,
    /// Only accepted on write; never returned by the API.
    api_key: Option<String>,
    clear_api_key: Option<bool>,
    auth_header: Option<String>,
    auth_scheme: Option<String>,
    max_tokens_field: Option<String>,
    response_format: Option<String>,
    thinking_mode: Option<String>,
    input_price_per_million: Option<f64>,
    cached_input_price_per_million: Option<f64>,
    output_price_per_million: Option<f64>,
    pricing_version: Option<String>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ProviderProfileIdRequest {
    id: String,
}

#[derive(Debug, Deserialize, Default)]
struct RepairPromptRequest {
    content: Option<String>,
    #[serde(default)]
    reset: bool,
}

fn default_endpoint_path() -> String {
    "chat/completions".into()
}
fn default_auth_header() -> String {
    "Authorization".into()
}
fn default_auth_scheme() -> String {
    "Bearer".into()
}
fn default_max_tokens_field() -> String {
    "max_tokens".into()
}
fn default_response_format() -> String {
    "json_object".into()
}
fn default_profile_enabled() -> bool {
    true
}

impl AppState {
    async fn current_project(&self) -> Arc<ProjectStore> {
        self.project.read().await.clone()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct TaskSnapshot {
    task_id: String,
    kind: String,
    batch_id: Option<String>,
    status: String,
    current: usize,
    total: usize,
    message: Option<String>,
    error: Option<String>,
    result: Option<serde_json::Value>,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

struct TaskEntry {
    snapshot: TaskSnapshot,
    cancel: CancellationToken,
}

#[derive(Clone)]
struct TaskRegistry {
    entries: Arc<Mutex<HashMap<String, TaskEntry>>>,
}

impl TaskRegistry {
    fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn start(&self, kind: &str, batch_id: Option<String>) -> (String, CancellationToken) {
        let task_id = format!("task-{}", Uuid::new_v4());
        let token = CancellationToken::new();
        let now = Utc::now();
        let snapshot = TaskSnapshot {
            task_id: task_id.clone(),
            kind: kind.into(),
            batch_id,
            status: "running".into(),
            current: 0,
            total: 0,
            message: None,
            error: None,
            result: None,
            created_at: now,
            updated_at: now,
        };
        self.entries.lock().await.insert(
            task_id.clone(),
            TaskEntry {
                snapshot,
                cancel: token.clone(),
            },
        );
        (task_id, token)
    }

    async fn update_event(&self, task_id: &str, event: &AgentEvent) {
        let mut entries = self.entries.lock().await;
        let Some(entry) = entries.get_mut(task_id) else {
            return;
        };
        match event {
            AgentEvent::Progress {
                current,
                total,
                message,
                ..
            } => {
                entry.snapshot.current = *current;
                entry.snapshot.total = *total;
                entry.snapshot.message = Some(message.clone());
            }
            AgentEvent::TaskCancelled { reason } => {
                entry.snapshot.status = "cancelled".into();
                entry.snapshot.message = Some(reason.clone());
            }
            AgentEvent::Error { message } => {
                entry.snapshot.status = "failed".into();
                entry.snapshot.error = Some(message.clone());
            }
            AgentEvent::Warning { message } => {
                // A repair run can finish with a usable subset of pages while
                // one or more pages failed. Keep that distinction visible to
                // the UI instead of overwriting it with "completed".
                entry.snapshot.status = "completed_with_errors".into();
                entry.snapshot.message = Some(message.clone());
            }
            AgentEvent::TaskCompleted { .. } => entry.snapshot.status = "completed".into(),
            _ => {}
        }
        entry.snapshot.updated_at = Utc::now();
    }

    async fn finish(&self, task_id: &str, result: Option<serde_json::Value>) {
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.get_mut(task_id) {
            entry.snapshot.status = if entry.cancel.is_cancelled() {
                "cancelled"
            } else {
                "completed"
            }
            .into();
            entry.snapshot.result = result;
            entry.snapshot.updated_at = Utc::now();
        }
    }

    async fn finish_repair(
        &self,
        task_id: &str,
        repaired_pages: usize,
        error_count: usize,
        result: Option<serde_json::Value>,
    ) {
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.get_mut(task_id) {
            entry.snapshot.status = if entry.cancel.is_cancelled() {
                "cancelled"
            } else if error_count == 0 {
                "completed"
            } else if repaired_pages == 0 {
                "failed"
            } else {
                "completed_with_errors"
            }
            .into();
            entry.snapshot.result = result;
            if error_count > 0 {
                entry.snapshot.error = Some(format!("{error_count} page(s) could not be repaired"));
            }
            entry.snapshot.updated_at = Utc::now();
        }
    }

    async fn fail(&self, task_id: &str, error: String) {
        let mut entries = self.entries.lock().await;
        if let Some(entry) = entries.get_mut(task_id) {
            entry.snapshot.status = if entry.cancel.is_cancelled() {
                "cancelled"
            } else {
                "failed"
            }
            .into();
            entry.snapshot.error = Some(error);
            entry.snapshot.updated_at = Utc::now();
        }
    }

    async fn cancel(&self, task_id: &str) -> bool {
        let entries = self.entries.lock().await;
        if let Some(entry) = entries.get(task_id) {
            entry.cancel.cancel();
            return true;
        }
        false
    }

    async fn cancel_batch(&self, batch_id: &str) -> Option<String> {
        let entries = self.entries.lock().await;
        entries
            .values()
            .find(|entry| {
                entry.snapshot.batch_id.as_deref() == Some(batch_id)
                    && entry.snapshot.status == "running"
            })
            .map(|entry| {
                entry.cancel.cancel();
                entry.snapshot.task_id.clone()
            })
    }

    async fn get(&self, task_id: &str) -> Option<TaskSnapshot> {
        self.entries
            .lock()
            .await
            .get(task_id)
            .map(|entry| entry.snapshot.clone())
    }

    async fn list(&self) -> Vec<TaskSnapshot> {
        let mut list = self
            .entries
            .lock()
            .await
            .values()
            .map(|entry| entry.snapshot.clone())
            .collect::<Vec<_>>();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }
}
#[derive(Deserialize)]
struct ImportRequest {
    path: String,
    mode: Option<String>,
    order: Option<String>,
    target: Option<String>,
    no_copy: Option<bool>,
}
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    scope: Option<String>,
}
#[derive(Deserialize)]
struct BatchRequest {
    batch_id: String,
    provider: Option<String>,
    #[serde(alias = "provider_profile")]
    profile_id: Option<String>,
    preset: Option<String>,
    model: Option<String>,
    thinking: Option<String>,
    /// Optional low/mid/high shortcut shared with the CLI.
    speed: Option<String>,
    target: Option<String>,
    prompt_file: Option<String>,
    refresh: Option<bool>,
    /// Explicitly permit building from normalized OCR when visual pages have
    /// no successful LLM repair. The default is strict/safe.
    #[serde(default)]
    allow_unrepaired: bool,
    /// Optional destination below clean/. The final file is document.md.
    clean_name: Option<String>,
}
#[derive(Deserialize)]
struct AnswerRequest {
    query: String,
    scope: Option<String>,
    provider: Option<String>,
    #[serde(alias = "provider_profile")]
    profile_id: Option<String>,
    preset: Option<String>,
    model: Option<String>,
    thinking: Option<String>,
    /// Optional low/mid/high shortcut shared with the CLI.
    speed: Option<String>,
    /// Existing source_ref or page_id values to ground the answer.
    #[serde(default)]
    source_refs: Vec<String>,
    /// Inline quoted excerpts that are not stored in this Vault.
    #[serde(default)]
    quotes: Vec<String>,
    /// Continue a previous conversation session.
    session_id: Option<String>,
}
#[derive(Deserialize)]
struct MergeRequest {
    batch_id: String,
    /// Preview by default; only `true` writes a combined revision.
    #[serde(default)]
    confirm: bool,
    target: Option<String>,
    /// Explicitly permit building from normalized OCR when visual pages have
    /// no successful LLM repair. The default is strict/safe.
    #[serde(default)]
    allow_unrepaired: bool,
    clean_name: Option<String>,
}
#[derive(Deserialize)]
struct MergeUnitsRequest {
    #[serde(default)]
    units: Vec<String>,
    merge_id: Option<String>,
    #[serde(default)]
    confirm: bool,
    target: Option<String>,
    #[serde(default)]
    allow_unrepaired: bool,
    clean_name: Option<String>,
}
#[derive(Deserialize)]
struct MergePlanQuery {
    batch_id: String,
}
#[derive(Deserialize)]
struct MergePlanUpdateRequest {
    batch_id: String,
    plan: MergePlan,
}
#[derive(Deserialize)]
struct SourceListQuery {
    batch_id: Option<String>,
    kind: Option<String>,
}
#[derive(Deserialize)]
struct ReviewRequest {
    batch_id: String,
    correction_id: String,
    replacement: Option<String>,
}
#[derive(Deserialize)]
struct VaultSelectRequest {
    name_or_id: String,
}
#[derive(Deserialize)]
struct ArtifactQuery {
    batch_id: String,
}

#[derive(Deserialize)]
struct WorkspaceInitRequest {
    path: String,
    #[serde(default)]
    vault_name: Option<String>,
}

#[derive(Deserialize)]
struct VaultCreateRequest {
    name: String,
    #[serde(default)]
    select: bool,
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

#[derive(Deserialize)]
struct FileSaveRequest {
    path: String,
    content: String,
}

#[derive(Deserialize, Default)]
struct FileListQuery {
    view: Option<String>,
}

#[derive(Deserialize)]
struct DeleteBatchRequest {
    batch_id: String,
    #[serde(default)]
    confirm: bool,
}

#[derive(Deserialize)]
struct DeleteUnitRequest {
    unit: String,
    #[serde(default)]
    confirm: bool,
}

pub async fn run(project: PathBuf, bind: &str) -> Result<()> {
    let (project, workspace) = if project.join("workspace.json").is_file() {
        let workspace = WorkspaceStore::open(&project)?;
        let first = workspace
            .list_vaults()?
            .into_iter()
            .next()
            .or_else(|| workspace.create_vault("默认").ok())
            .ok_or_else(|| anyhow::anyhow!("workspace has no Vault; create one first"))?;
        (
            ProjectStore::open(workspace.vault_path(&first.vault_id)?)?,
            Some(workspace),
        )
    } else {
        (ProjectStore::open(&project)?, None)
    };
    let state = AppState {
        project: Arc::new(RwLock::new(Arc::new(project))),
        workspace: Arc::new(RwLock::new(workspace)),
        tasks: TaskRegistry::new(),
    };
    let app = Router::new()
        .route("/", get(gui_index))
        .route("/assets/app.js", get(app_js))
        .route("/assets/styles.css", get(styles_css))
        .route("/api/health", get(health))
        .route("/api/vault", get(vault_info))
        .route("/api/vaults", get(vaults))
        .route("/api/vaults/select", post(select_vault))
        .route("/api/workspace/init", post(init_workspace))
        .route("/api/vaults/create", post(create_vault))
        .route("/api/providers", get(providers).post(save_provider))
        .route("/api/providers/delete", post(delete_provider))
        .route("/api/providers/check", post(check_provider))
        .route(
            "/api/prompts/repair",
            get(repair_prompt).post(save_repair_prompt),
        )
        .route("/api/batches", get(batches))
        .route("/api/files", get(files))
        .route("/api/file", get(file_preview).post(file_save))
        .route("/api/file/raw", get(file_raw))
        .route("/api/delete-batch", post(delete_batch))
        .route("/api/delete-unit", post(delete_unit))
        .route("/api/artifact", get(artifact))
        .route("/api/import", post(import))
        .route(
            "/api/import-upload",
            post(import_upload).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES)),
        )
        .route("/api/direct-clean", post(direct_clean))
        .route("/api/ocr", post(ocr))
        .route("/api/normalize", post(normalize))
        .route("/api/propose", post(propose))
        .route("/api/repair", post(propose))
        .route("/api/apply", post(apply))
        .route("/api/build", post(apply))
        .route("/api/merge", post(merge))
        .route("/api/merge-plan", get(merge_plan).post(update_merge_plan))
        .route("/api/merge-units", post(merge_units))
        .route("/api/apply-safe", post(apply))
        .route("/api/edit-patch", post(edit_patch))
        .route("/api/review", post(edit_patch))
        .route("/api/answer", post(answer))
        .route("/api/cancel", post(cancel))
        .route("/api/tasks", get(tasks))
        .route("/api/tasks/{task_id}", get(task))
        .route("/api/tasks/{task_id}/cancel", post(cancel_task))
        .route("/api/sessions", get(sessions))
        .route("/api/sessions/{session_id}", get(session))
        .route("/api/search", get(search))
        .route("/api/sources", get(sources))
        .route("/api/usage", get(usage))
        .route("/api/activity", get(activity))
        .route("/api/events", get(events))
        .with_state(state);
    let addr: SocketAddr = bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("ReadTrace Web listening on http://{}", addr);
    axum::serve(listener, app).await?;
    Ok(())
}
async fn gui_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}
async fn app_js() -> (
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        include_str!("../static/app.js"),
    )
}
async fn styles_css() -> (
    [(axum::http::header::HeaderName, &'static str); 1],
    &'static str,
) {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../static/styles.css"),
    )
}
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok":true,"now":Utc::now()}))
}
async fn vault_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let workspace = state.workspace.read().await.clone();
    Json(serde_json::json!({
        "ok": true,
        "root": project.root,
        "workspace": workspace.map(|w| w.root),
    }))
}
async fn vaults(State(state): State<AppState>) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let selected = project.root.to_string_lossy().to_string();
    let workspace = state.workspace.read().await.clone();
    match workspace.as_ref().map(WorkspaceStore::list_vaults) {
        Some(Ok(items)) => {
            let selected_vault = items.iter().find(|vault| {
                workspace
                    .as_ref()
                    .and_then(|ws| ws.vault_path(&vault.vault_id).ok())
                    .as_deref()
                    == Some(project.root.as_path())
            });
            Json(
                serde_json::json!({"ok": true, "selected": selected, "selected_vault": selected_vault, "workspace": workspace.as_ref().map(|w| w.root.clone()), "vaults": items}),
            )
        }
        Some(Err(error)) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
        None => Json(serde_json::json!({"ok": true, "selected": selected, "vaults": []})),
    }
}
async fn select_vault(
    State(state): State<AppState>,
    Json(req): Json<VaultSelectRequest>,
) -> Json<serde_json::Value> {
    let workspace = state.workspace.read().await.clone();
    let Some(workspace) = workspace.as_ref() else {
        return Json(
            serde_json::json!({"ok": false, "error": "server was started on one Vault; pass a workspace path to serve for Vault switching"}),
        );
    };
    match workspace.open_vault(&req.name_or_id) {
        Ok(project) => {
            let root = project.root.clone();
            *state.project.write().await = Arc::new(project);
            Json(serde_json::json!({"ok": true, "root": root}))
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn init_workspace(
    State(state): State<AppState>,
    Json(req): Json<WorkspaceInitRequest>,
) -> Json<serde_json::Value> {
    let path = PathBuf::from(req.path.trim());
    if path.as_os_str().is_empty() {
        return Json(serde_json::json!({"ok": false, "error": "workspace path cannot be empty"}));
    }
    let result = (|| -> Result<(WorkspaceStore, ProjectStore, Option<VaultRecord>)> {
        let workspace = WorkspaceStore::init(&path)?;
        let vault = match req
            .vault_name
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            Some(name) => {
                if let Some(existing) = workspace
                    .list_vaults()?
                    .into_iter()
                    .find(|vault| vault.name == name)
                {
                    Some(existing)
                } else {
                    Some(workspace.create_vault(name)?)
                }
            }
            None => workspace.list_vaults()?.into_iter().next(),
        };
        let vault =
            vault.ok_or_else(|| anyhow::anyhow!("workspace has no Vault; provide vault_name"))?;
        let project = workspace.open_vault(&vault.vault_id)?;
        Ok((workspace, project, Some(vault)))
    })();
    match result {
        Ok((workspace, project, vault)) => {
            *state.workspace.write().await = Some(workspace.clone());
            *state.project.write().await = Arc::new(project);
            Json(serde_json::json!({"ok": true, "workspace": workspace.root, "vault": vault}))
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn create_vault(
    State(state): State<AppState>,
    Json(req): Json<VaultCreateRequest>,
) -> Json<serde_json::Value> {
    let workspace = state.workspace.read().await.clone();
    let Some(workspace) = workspace else {
        return Json(
            serde_json::json!({"ok": false, "error": "serve a Workspace path to manage multiple Vaults"}),
        );
    };
    match workspace.create_vault(&req.name) {
        Ok(vault) => {
            if req.select {
                if let Ok(project) = workspace.open_vault(&vault.vault_id) {
                    *state.project.write().await = Arc::new(project);
                }
            }
            Json(serde_json::json!({"ok": true, "vault": vault, "selected": req.select}))
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

fn builtin_provider_profiles() -> Vec<ProviderProfile> {
    let env = AppConfig::from_env();
    let school_base = env
        .base_url
        .clone()
        .or_else(|| Some("https://lab.cs.tsinghua.edu.cn/ai-platform/api/v1".into()));
    let school_key_env = if env.api_key_value.is_some() {
        env.api_key_env.clone()
    } else if env.api_key_env.trim().is_empty() {
        "THU_AI_PLATFORM_API_KEY".into()
    } else {
        env.api_key_env.clone()
    };
    let school = |id: &str, name: &str, model: &str| ProviderProfile {
        id: id.into(),
        name: name.into(),
        kind: "http".into(),
        base_url: school_base.clone(),
        endpoint: None,
        endpoint_path: "chat/completions".into(),
        model: model.into(),
        api_key_env: school_key_env.clone(),
        api_key: None,
        auth_header: "Authorization".into(),
        auth_scheme: "Bearer".into(),
        max_tokens_field: "max_tokens".into(),
        response_format: "json_object".into(),
        thinking_mode: if model == "glm-5.3-flash" {
            "low"
        } else {
            "none"
        }
        .into(),
        input_price_per_million: 0.0,
        cached_input_price_per_million: 0.0,
        output_price_per_million: 0.0,
        pricing_version: String::new(),
        enabled: true,
    };
    vec![
        school(
            "tsinghua-glm-5.3-flash",
            "清华 GLM-5.3 Flash（最快）",
            "glm-5.3-flash",
        ),
        school("tsinghua-glm-5.2", "清华 GLM-5.2（可关闭思考）", "glm-5.2"),
        ProviderProfile {
            id: "codex-luna".into(),
            name: "Codex Luna High".into(),
            kind: "codex-cli".into(),
            base_url: None,
            endpoint: None,
            endpoint_path: default_endpoint_path(),
            model: "gpt-5.6-luna".into(),
            api_key_env: String::new(),
            api_key: None,
            auth_header: default_auth_header(),
            auth_scheme: default_auth_scheme(),
            max_tokens_field: "max_completion_tokens".into(),
            response_format: "none".into(),
            thinking_mode: "high".into(),
            input_price_per_million: 0.0,
            cached_input_price_per_million: 0.0,
            output_price_per_million: 0.0,
            pricing_version: String::new(),
            enabled: true,
        },
        ProviderProfile {
            id: "mock".into(),
            name: "Mock（本地测试，不计费）".into(),
            kind: "mock".into(),
            base_url: None,
            endpoint: None,
            endpoint_path: default_endpoint_path(),
            model: "mock".into(),
            api_key_env: String::new(),
            api_key: None,
            auth_header: default_auth_header(),
            auth_scheme: default_auth_scheme(),
            max_tokens_field: default_max_tokens_field(),
            response_format: "none".into(),
            thinking_mode: "none".into(),
            input_price_per_million: 0.0,
            cached_input_price_per_million: 0.0,
            output_price_per_million: 0.0,
            pricing_version: "non-billable-mock".into(),
            enabled: true,
        },
    ]
}

fn provider_store_path(project: &ProjectStore) -> PathBuf {
    if let Ok(path) = std::env::var("READTRACE_PROVIDER_STORE") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local_app_data)
            .join("ReadTrace")
            .join("providers.json");
    }
    project.root.join(".readtrace").join("providers.json")
}

fn provider_store_is_explicit() -> bool {
    std::env::var("READTRACE_PROVIDER_STORE")
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

fn provider_fallback_store_path(project: &ProjectStore) -> PathBuf {
    project.root.join(".readtrace").join("providers.json")
}

fn read_custom_provider_profiles(project: &ProjectStore) -> Result<Vec<ProviderProfile>> {
    let path = provider_store_path(project);
    if path.is_file() {
        let registry: ProviderRegistry = serde_json::from_slice(&std::fs::read(&path)?)?;
        return Ok(registry.profiles);
    }
    if !provider_store_is_explicit() {
        let fallback = provider_fallback_store_path(project);
        if fallback.is_file() {
            let registry: ProviderRegistry = serde_json::from_slice(&std::fs::read(fallback)?)?;
            return Ok(registry.profiles);
        }
    }
    Ok(Vec::new())
}

fn write_provider_registry(
    path: &std::path::Path,
    profiles: &[ProviderProfile],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    let registry = ProviderRegistry {
        schema_version: 1,
        profiles: profiles.to_vec(),
    };
    let content = serde_json::to_vec_pretty(&registry)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()))?;
    if let Err(error) = std::fs::write(&temporary, content) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    if path.exists() {
        if let Err(error) = std::fs::remove_file(path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

fn write_custom_provider_profiles(
    project: &ProjectStore,
    profiles: &[ProviderProfile],
) -> Result<PathBuf> {
    let path = provider_store_path(project);
    match write_provider_registry(&path, profiles) {
        Ok(()) => Ok(path),
        Err(error)
            if !provider_store_is_explicit()
                && error.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            let fallback = provider_fallback_store_path(project);
            write_provider_registry(&fallback, profiles)?;
            Ok(fallback)
        }
        Err(error) => Err(error.into()),
    }
}

fn provider_store_display_path(project: &ProjectStore) -> PathBuf {
    let preferred = provider_store_path(project);
    if preferred.is_file() || provider_store_is_explicit() {
        preferred
    } else {
        let fallback = provider_fallback_store_path(project);
        if fallback.is_file() {
            fallback
        } else {
            preferred
        }
    }
}

fn provider_is_builtin(id: &str) -> bool {
    matches!(
        id,
        "tsinghua-glm-5.3-flash" | "tsinghua-glm-5.2" | "codex-luna" | "mock"
    )
}

fn effective_profile_pricing(profile: &ProviderProfile) -> (f64, f64, f64, String) {
    if profile.input_price_per_million > 0.0
        || profile.cached_input_price_per_million > 0.0
        || profile.output_price_per_million > 0.0
    {
        return (
            profile.input_price_per_million,
            profile.cached_input_price_per_million,
            profile.output_price_per_million,
            profile.pricing_version.clone(),
        );
    }
    let mut config = AppConfig {
        model: profile.model.clone(),
        ..AppConfig::default()
    };
    if config.apply_official_model_pricing() {
        (
            config.input_price_per_million,
            config.cached_input_price_per_million,
            config.output_price_per_million,
            config.pricing_version,
        )
    } else {
        (
            profile.input_price_per_million,
            profile.cached_input_price_per_million,
            profile.output_price_per_million,
            profile.pricing_version.clone(),
        )
    }
}

fn provider_view(profile: &ProviderProfile) -> ProviderProfileView {
    let key_present = profile
        .api_key
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || (!profile.api_key_env.trim().is_empty()
            && std::env::var(&profile.api_key_env)
                .ok()
                .is_some_and(|value| !value.trim().is_empty()));
    let (
        input_price_per_million,
        cached_input_price_per_million,
        output_price_per_million,
        pricing_version,
    ) = effective_profile_pricing(profile);
    ProviderProfileView {
        id: profile.id.clone(),
        name: profile.name.clone(),
        kind: profile.kind.clone(),
        base_url: profile.base_url.clone(),
        endpoint: profile.endpoint.clone(),
        endpoint_path: profile.endpoint_path.clone(),
        model: profile.model.clone(),
        api_key_env: profile.api_key_env.clone(),
        key_present,
        auth_header: profile.auth_header.clone(),
        auth_scheme: profile.auth_scheme.clone(),
        max_tokens_field: profile.max_tokens_field.clone(),
        response_format: profile.response_format.clone(),
        thinking_mode: profile.thinking_mode.clone(),
        input_price_per_million,
        cached_input_price_per_million,
        output_price_per_million,
        pricing_version,
        enabled: profile.enabled,
        builtin: provider_is_builtin(&profile.id),
    }
}

async fn load_provider_profiles(state: &AppState) -> Result<Vec<ProviderProfile>> {
    let project = state.current_project().await;
    let mut profiles = builtin_provider_profiles();
    for custom in read_custom_provider_profiles(&project)? {
        if let Some(existing) = profiles.iter_mut().find(|item| item.id == custom.id) {
            *existing = custom;
        } else {
            profiles.push(custom);
        }
    }
    Ok(profiles)
}

async fn find_provider_profile(state: &AppState, id: &str) -> Result<ProviderProfile> {
    load_provider_profiles(state)
        .await?
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| anyhow::anyhow!("provider profile not found: {id}"))
}

async fn providers(State(state): State<AppState>) -> Json<serde_json::Value> {
    match load_provider_profiles(&state).await {
        Ok(profiles) => {
            let project = state.current_project().await;
            Json(serde_json::json!({
                "ok": true,
                "profiles": profiles.iter().map(provider_view).collect::<Vec<_>>(),
                "store": provider_store_display_path(project.as_ref()).display().to_string(),
            }))
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

fn clean_profile_value(value: Option<String>, fallback: &str) -> String {
    value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.into())
}

async fn save_provider(
    State(state): State<AppState>,
    Json(req): Json<ProviderProfileRequest>,
) -> Json<serde_json::Value> {
    let kind_name = req.kind.trim().to_ascii_lowercase();
    if let Err(error) = kind_name.parse::<LlmBackend>() {
        return Json(serde_json::json!({"ok": false, "error": error.to_string()}));
    }
    if req.name.trim().is_empty() {
        return Json(serde_json::json!({"ok": false, "error": "provider name cannot be empty"}));
    }
    if req.model.trim().is_empty() {
        return Json(serde_json::json!({"ok": false, "error": "provider model cannot be empty"}));
    }
    if kind_name == "http"
        && req
            .base_url
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && req
            .endpoint
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Json(serde_json::json!({
            "ok": false,
            "error": "HTTP 来源至少需要填写 Base URL 或完整 Endpoint"
        }));
    }
    let id = req
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("custom-{}", Uuid::new_v4().simple()));
    if !id
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_')
    {
        return Json(
            serde_json::json!({"ok": false, "error": "provider id may contain only letters, numbers, '-' and '_'"}),
        );
    }
    let project = state.current_project().await;
    let mut custom = match read_custom_provider_profiles(&project) {
        Ok(profiles) => profiles,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    let previous = load_provider_profiles(&state)
        .await
        .ok()
        .and_then(|profiles| profiles.into_iter().find(|profile| profile.id == id));
    let key = if req.clear_api_key.unwrap_or(false) {
        None
    } else {
        req.api_key
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                previous
                    .as_ref()
                    .and_then(|profile| profile.api_key.clone())
            })
    };
    let key_env_default = if kind_name == "http" {
        "READTRACE_API_KEY"
    } else {
        ""
    };
    let profile = ProviderProfile {
        id: id.clone(),
        name: req.name.trim().to_owned(),
        kind: kind_name,
        base_url: req.base_url.filter(|value| !value.trim().is_empty()),
        endpoint: req.endpoint.filter(|value| !value.trim().is_empty()),
        endpoint_path: clean_profile_value(req.endpoint_path, "chat/completions"),
        model: req.model.trim().to_owned(),
        api_key_env: clean_profile_value(req.api_key_env, key_env_default),
        api_key: key,
        auth_header: clean_profile_value(req.auth_header, "Authorization"),
        auth_scheme: clean_profile_value(req.auth_scheme, "Bearer"),
        max_tokens_field: clean_profile_value(req.max_tokens_field, "max_tokens"),
        response_format: clean_profile_value(req.response_format, "json_object"),
        thinking_mode: clean_profile_value(req.thinking_mode, "default"),
        input_price_per_million: req.input_price_per_million.unwrap_or(0.0).max(0.0),
        cached_input_price_per_million: req.cached_input_price_per_million.unwrap_or(0.0).max(0.0),
        output_price_per_million: req.output_price_per_million.unwrap_or(0.0).max(0.0),
        pricing_version: req.pricing_version.unwrap_or_default(),
        enabled: req.enabled.unwrap_or(true),
    };
    if let Some(existing) = custom.iter_mut().find(|item| item.id == id) {
        *existing = profile.clone();
    } else {
        custom.push(profile.clone());
    }
    match write_custom_provider_profiles(&project, &custom) {
        Ok(path) => {
            Json(serde_json::json!({"ok": true, "profile": provider_view(&profile), "store": path}))
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn delete_provider(
    State(state): State<AppState>,
    Json(req): Json<ProviderProfileIdRequest>,
) -> Json<serde_json::Value> {
    if provider_is_builtin(&req.id) {
        return Json(
            serde_json::json!({"ok": false, "error": "内置来源不能删除；可以通过保存覆盖配置"}),
        );
    }
    let project = state.current_project().await;
    let mut profiles = match read_custom_provider_profiles(&project) {
        Ok(profiles) => profiles,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    let old_len = profiles.len();
    profiles.retain(|profile| profile.id != req.id);
    if old_len == profiles.len() {
        return Json(serde_json::json!({"ok": false, "error": "自定义来源不存在"}));
    }
    match write_custom_provider_profiles(&project, &profiles) {
        Ok(_) => Json(serde_json::json!({"ok": true, "deleted": req.id})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

fn repair_prompt_payload(project: &ProjectStore) -> Result<serde_json::Value> {
    let path = project.root.join("prompts/repair.md");
    let custom = path.is_file();
    let content = if custom {
        std::fs::read_to_string(&path)?
    } else {
        repair_prompt_template()
    };
    Ok(serde_json::json!({
        "ok": true,
        "content": content,
        "custom": custom,
        "source": if custom { "vault" } else { "builtin" },
        "path": "prompts/repair.md",
        "hint": "保留 {mode} 占位符可让同一提示词适配不同输入类型。"
    }))
}

async fn repair_prompt(State(state): State<AppState>) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    match repair_prompt_payload(&project) {
        Ok(value) => Json(value),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn save_repair_prompt(
    State(state): State<AppState>,
    Json(req): Json<RepairPromptRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let path = project.root.join("prompts/repair.md");
    let result = if req.reset {
        if path.is_file() {
            std::fs::remove_file(&path)
        } else {
            Ok(())
        }
    } else {
        let content = req.content.unwrap_or_default();
        if content.trim().is_empty() {
            return Json(serde_json::json!({
                "ok": false,
                "error": "修复提示词不能为空；如需恢复默认值，请使用 reset"
            }));
        }
        if content.len() > 200_000 {
            return Json(serde_json::json!({
                "ok": false,
                "error": "修复提示词过大，单次保存上限为 200 KB"
            }));
        }
        std::fs::create_dir_all(project.root.join("prompts"))
            .and_then(|_| std::fs::write(&path, content))
    };
    if let Err(error) = result {
        return Json(serde_json::json!({"ok": false, "error": error.to_string()}));
    }
    match repair_prompt_payload(&project) {
        Ok(mut value) => {
            value["saved"] = serde_json::Value::Bool(true);
            Json(value)
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

fn apply_provider_profile(resolved: &mut ResolvedLlm, profile: &ProviderProfile) -> Result<()> {
    resolved.backend = profile.kind.parse::<LlmBackend>()?;
    let config = &mut resolved.config;
    if profile.base_url.is_some() {
        config.base_url = profile.base_url.clone();
    }
    if profile.endpoint.is_some() {
        config.endpoint = profile.endpoint.clone().unwrap_or_default();
        config.base_url = None;
    }
    config.endpoint_path = profile.endpoint_path.clone();
    config.model = profile.model.clone();
    config.api_key_env = profile.api_key_env.clone();
    config.api_key_value = profile.api_key.clone();
    config.auth_header = profile.auth_header.clone();
    config.auth_scheme = profile.auth_scheme.clone();
    config.max_tokens_field = profile.max_tokens_field.clone();
    config.response_format = profile.response_format.clone();
    config.thinking_mode = profile.thinking_mode.clone();
    config.input_price_per_million = profile.input_price_per_million;
    config.cached_input_price_per_million = profile.cached_input_price_per_million;
    config.output_price_per_million = profile.output_price_per_million;
    config.pricing_version = profile.pricing_version.clone();
    if resolved.backend != LlmBackend::Http {
        config.api_key_required = false;
    }
    if config.input_price_per_million == 0.0
        && config.cached_input_price_per_million == 0.0
        && config.output_price_per_million == 0.0
    {
        config.apply_official_model_pricing();
    }
    Ok(())
}

async fn resolve_provider_request(
    state: &AppState,
    profile_id: Option<&str>,
    preset: Option<&str>,
    model: Option<&str>,
    thinking: Option<&str>,
    speed: Option<&str>,
    provider: Option<&str>,
) -> Result<ResolvedLlm> {
    // Older clients sent the profile id in `provider` instead of the
    // backend kind. Resolve that shape as well, so a cached page cannot turn
    // a valid profile such as `tsinghua-glm-5.2` into an unknown backend.
    let profile = match profile_id.filter(|value| !value.trim().is_empty()) {
        Some(id) => Some(find_provider_profile(state, id).await?),
        None => match provider.filter(|value| !value.trim().is_empty()) {
            Some(value) if value.parse::<LlmBackend>().is_err() => load_provider_profiles(state)
                .await?
                .into_iter()
                .find(|profile| profile.id == value),
            _ => None,
        },
    };
    let backend = profile
        .as_ref()
        .map(|profile| profile.kind.as_str())
        .or(provider);
    let mut resolved = llm_config(preset, model, thinking, speed, backend)?;
    if let Some(profile) = profile.as_ref() {
        apply_provider_profile(&mut resolved, profile)?;
    }
    Ok(resolved)
}

async fn check_provider(
    State(state): State<AppState>,
    Json(req): Json<ProviderProfileIdRequest>,
) -> Json<serde_json::Value> {
    let profile = match find_provider_profile(&state, &req.id).await {
        Ok(profile) => profile,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    let mut resolved = match llm_config(
        None,
        Some(&profile.model),
        Some(&profile.thinking_mode),
        None,
        Some(&profile.kind),
    ) {
        Ok(resolved) => resolved,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    if let Err(error) = apply_provider_profile(&mut resolved, &profile) {
        return Json(serde_json::json!({"ok": false, "error": error.to_string()}));
    }
    let report = match resolved.backend {
        LlmBackend::Http => {
            OpenAiCompatibleProvider::new(resolved.config.clone())
                .probe()
                .await
        }
        LlmBackend::CodexCli => CodexCliProvider::new(&resolved.config).probe().await,
        LlmBackend::Mock => AiProbeReport {
            ok: true,
            endpoint: "mock://local".into(),
            model: "mock".into(),
            status_code: None,
            elapsed_ms: 0,
            response_preview: Some("OK".into()),
            request_id: None,
            usage: Usage::unknown(),
            error: None,
        },
    };
    // A connection test is a real provider call too. Record it in the same
    // append-only ledger as repair and answer calls so the backend tab never
    // under-reports API traffic. Unknown usage remains unknown (for example,
    // Codex CLI may not expose token counts), but the call itself is visible.
    let endpoint = report.endpoint.clone();
    let provider_name = match resolved.backend {
        LlmBackend::Http => "http",
        LlmBackend::CodexCli => "codex-cli",
        LlmBackend::Mock => "mock",
    };
    let mut call = CallRecord::from_usage(
        provider_name,
        &endpoint,
        &report.model,
        "provider_check",
        report.usage.clone(),
        &resolved.config,
        report.elapsed_ms as u64,
        report.ok,
    );
    call.request_id = report.request_id.clone();
    call.phase = Some("probe".into());
    call.thinking_mode = Some(resolved.config.thinking_mode.clone());
    if !report.ok {
        call.error_type = Some("provider_probe_failed".into());
    }
    let project = state.current_project().await;
    let ledger_error = project
        .append_runtime_call(&call)
        .err()
        .map(|error| error.to_string());
    Json(serde_json::json!({
        "ok": report.ok,
        "profile": provider_view(&profile),
        "report": report,
        "ledger_call_id": call.call_id,
        "ledger_error": ledger_error,
    }))
}

#[derive(Debug, serde::Serialize)]
struct VaultFile {
    path: String,
    category: String,
    name: String,
    kind: String,
    size: u64,
    modified: Option<chrono::DateTime<Utc>>,
    previewable: bool,
    raw_url: String,
}

fn safe_vault_file(project: &ProjectStore, value: &str) -> Result<PathBuf> {
    let value = value.trim().replace('\\', "/");
    let relative = std::path::Path::new(&value);
    if value.is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        || value.starts_with('.')
        || value.contains("/.env")
    {
        return Err(anyhow::anyhow!("invalid Vault file path"));
    }
    let root = std::fs::canonicalize(&project.root)?;
    let candidate = project.path(relative);
    let canonical = std::fs::canonicalize(&candidate)?;
    if !canonical.starts_with(&root) || !canonical.is_file() {
        return Err(anyhow::anyhow!("file is outside the current Vault"));
    }
    Ok(canonical)
}

fn resolve_readable_file(project: &ProjectStore, value: &str) -> Result<PathBuf> {
    if let Ok(path) = safe_vault_file(project, value) {
        return Ok(path);
    }
    let requested = value.replace('\\', "/");
    let external = project
        .list_merge_units()?
        .into_iter()
        .find(|unit| unit.kind == "source" && unit.path == requested)
        .and_then(|unit| unit.external_path)
        .ok_or_else(|| anyhow::anyhow!("file is not present in the current Vault"))?;
    let path = std::fs::canonicalize(external)?;
    if !path.is_file() {
        return Err(anyhow::anyhow!("external source is missing"));
    }
    Ok(path)
}

fn file_kind(path: &std::path::Path) -> (&'static str, bool) {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "md" | "txt" | "json" | "jsonl" | "toml" | "yaml" | "yml" | "csv" | "log" => ("text", true),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => ("image", false),
        "pdf" => ("pdf", false),
        _ => ("binary", false),
    }
}

fn encode_query(value: &str) -> String {
    value.bytes().fold(String::new(), |mut output, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
        output
    })
}

fn collect_vault_files(
    root: &std::path::Path,
    current: &std::path::Path,
    files: &mut Vec<VaultFile>,
) -> Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".readtrace" || name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_vault_files(root, &path, files)?;
            continue;
        }
        let metadata = entry.metadata()?;
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        let category = relative.split('/').next().unwrap_or("other").to_owned();
        let (kind, previewable) = file_kind(&path);
        let modified = metadata.modified().ok().map(chrono::DateTime::<Utc>::from);
        files.push(VaultFile {
            raw_url: format!("/api/file/raw?path={}", encode_query(&relative)),
            path: relative,
            category,
            name,
            kind: kind.into(),
            size: metadata.len(),
            modified,
            previewable,
        });
    }
    Ok(())
}

async fn files(
    State(state): State<AppState>,
    Query(query): Query<FileListQuery>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let mut items = Vec::new();
    match collect_vault_files(&project.root, &project.root, &mut items) {
        Ok(()) => {
            if let Ok(units) = project.list_merge_units() {
                let known = items
                    .iter()
                    .map(|item| item.path.clone())
                    .collect::<std::collections::HashSet<_>>();
                for unit in units.into_iter().filter(|unit| {
                    unit.kind == "source"
                        && unit.external_path.is_some()
                        && !known.contains(&unit.path)
                }) {
                    if let Some(external) = unit
                        .external_path
                        .as_deref()
                        .and_then(|path| std::fs::metadata(path).ok())
                    {
                        let path = unit.path.clone();
                        let (kind, previewable) = file_kind(std::path::Path::new(&path));
                        items.push(VaultFile {
                            raw_url: format!("/api/file/raw?path={}", encode_query(&path)),
                            name: std::path::Path::new(&path)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or(&path)
                                .to_owned(),
                            category: "sources".into(),
                            kind: kind.into(),
                            size: external.len(),
                            modified: external.modified().ok().map(chrono::DateTime::<Utc>::from),
                            previewable,
                            path,
                        });
                    }
                }
            }
            if query.view.as_deref().unwrap_or("essential") != "all" {
                items.retain(|item| {
                    !matches!(
                        item.category.as_str(),
                        "raw" | "runtime" | "events" | "sessions"
                    ) && !matches!(item.path.as_str(), "metadata.json" | "correction_log.json")
                });
            }
            items.sort_by(|a, b| a.path.cmp(&b.path));
            Json(
                serde_json::json!({"ok": true, "files": items, "root": project.root, "view": query.view.as_deref().unwrap_or("essential")}),
            )
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn file_preview(
    State(state): State<AppState>,
    Query(query): Query<FileQuery>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let path = match resolve_readable_file(&project, &query.path) {
        Ok(path) => path,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    let (kind, previewable) = file_kind(&path);
    let metadata = std::fs::metadata(&path).ok();
    let mut value = serde_json::json!({
        "ok": true,
        "path": query.path.replace('\\', "/"),
        "name": path.file_name().and_then(|v| v.to_str()).unwrap_or_default(),
        "kind": kind,
        "previewable": previewable,
        "size": metadata.as_ref().map(std::fs::Metadata::len).unwrap_or(0),
        "raw_url": format!("/api/file/raw?path={}", encode_query(&query.path.replace('\\', "/"))),
    });
    if previewable {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let truncated = content.chars().take(120_000).collect::<String>();
                value["content"] = serde_json::Value::String(truncated);
                value["truncated"] = serde_json::Value::Bool(content.chars().count() > 120_000);
            }
            Err(error) => value["error"] = serde_json::Value::String(error.to_string()),
        }
    }
    Json(value)
}

async fn file_save(
    State(state): State<AppState>,
    Json(req): Json<FileSaveRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let relative = req.path.trim().replace('\\', "/");
    let path = match safe_vault_file(&project, &relative) {
        Ok(path) => path,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "md" | "txt") {
        return Json(serde_json::json!({
            "ok": false,
            "error": "只允许编辑 Markdown 或 TXT 文件"
        }));
    }
    let first_component = relative.split('/').next().unwrap_or_default();
    if matches!(
        first_component,
        "raw" | "sources" | "events" | "runtime" | "sessions"
    ) {
        return Json(serde_json::json!({
            "ok": false,
            "error": "原始素材和审计目录不可直接编辑；请编辑 clean 或 generated 文档"
        }));
    }
    if req.content.len() > 2_000_000 {
        return Json(serde_json::json!({"ok": false, "error": "文件过大，单次保存上限为 2 MB"}));
    }
    if let Err(error) = std::fs::write(&path, req.content.as_bytes()) {
        return Json(serde_json::json!({"ok": false, "error": error.to_string()}));
    }
    let index_error = IndexStore::open(&project)
        .and_then(|index| index.rebuild(&project))
        .err()
        .map(|error| error.to_string());
    let mut response = serde_json::json!({
        "ok": true,
        "path": relative,
        "size": req.content.len(),
        "index_rebuilt": index_error.is_none(),
    });
    if let Some(error) = index_error {
        response["index_error"] = serde_json::Value::String(error);
    }
    Json(response)
}

async fn file_raw(State(state): State<AppState>, Query(query): Query<FileQuery>) -> Response {
    let project = state.current_project().await;
    let path = match resolve_readable_file(&project, &query.path) {
        Ok(path) => path,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) => return (StatusCode::NOT_FOUND, error.to_string()).into_response(),
    };
    let mime = match path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "pdf" => "application/pdf",
        "json" | "jsonl" => "application/json; charset=utf-8",
        "md" | "txt" | "log" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    };
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

async fn delete_batch(
    State(state): State<AppState>,
    Json(req): Json<DeleteBatchRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let result = if req.confirm {
        project.delete_batch(&req.batch_id)
    } else {
        project.plan_delete_batch(&req.batch_id)
    };
    match result {
        Ok(plan) => Json(serde_json::json!({"ok": true, "plan": plan})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn delete_unit(
    State(state): State<AppState>,
    Json(req): Json<DeleteUnitRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let result = if req.confirm {
        project.delete_unit(&req.unit)
    } else {
        project.plan_delete_unit(&req.unit)
    };
    match result {
        Ok(plan) => Json(serde_json::json!({"ok": true, "plan": plan})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}
async fn artifact(
    State(state): State<AppState>,
    Query(query): Query<ArtifactQuery>,
) -> Json<serde_json::Value> {
    if query.batch_id.is_empty()
        || query.batch_id == "."
        || query.batch_id == ".."
        || query.batch_id.contains('/')
        || query.batch_id.contains('\\')
    {
        return Json(serde_json::json!({"ok": false, "error": "invalid batch_id"}));
    }
    let project = state.current_project().await;
    let current = project.path(format!("generated/{}/current.json", query.batch_id));
    let value: serde_json::Value = match std::fs::read_to_string(&current)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
    {
        Some(value) => value,
        None => {
            return Json(
                serde_json::json!({"ok": false, "error": "artifact not found; run build first"}),
            )
        }
    };
    let Some(path) = value.get("path").and_then(serde_json::Value::as_str) else {
        return Json(serde_json::json!({"ok": false, "error": "artifact path is invalid"}));
    };
    let relative = std::path::Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        || !path.replace('\\', "/").starts_with("generated/")
    {
        return Json(
            serde_json::json!({"ok": false, "error": "artifact path is outside generated/"}),
        );
    }
    let path = project.path(path);
    match std::fs::read_to_string(&path) {
        Ok(content) => Json(serde_json::json!({"ok": true, "artifact": value, "content": content})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}
async fn batches(State(state): State<AppState>) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let metadata: serde_json::Value = match std::fs::read_to_string(project.path("metadata.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
    {
        Some(value) => value,
        None => return Json(serde_json::json!({"ok": false, "error": "metadata not found"})),
    };
    Json(
        serde_json::json!({"ok": true, "batches": metadata.get("batches").cloned().unwrap_or_else(|| serde_json::json!([]))}),
    )
}
async fn import(
    State(state): State<AppState>,
    Json(req): Json<ImportRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let mode = req
        .mode
        .unwrap_or_else(|| "generic".into())
        .parse()
        .unwrap_or_default();
    let result = if std::path::Path::new(&req.path).is_dir() {
        project.import_folder_with_options(
            &req.path,
            mode,
            &req.order.unwrap_or_else(|| "filename".into()),
            req.target,
            !req.no_copy.unwrap_or(false),
        )
    } else {
        project.import_file_with_options(&req.path, mode, req.target, !req.no_copy.unwrap_or(false))
    };
    match result {
        Ok(b) => Json(serde_json::json!({"ok":true,"batch":b})),
        Err(e) => Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    }
}

/// Import files selected by a browser file/folder picker.
///
/// Browser security rules intentionally hide the user's absolute path, so
/// this endpoint stages the multipart payload inside the current Vault and
/// then delegates to the same core folder importer used by the CLI. Uploaded
/// material is therefore always copied into `sources/`; the path-based import
/// endpoint remains available when an external reference (`no_copy`) is
/// required.
async fn import_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let upload_id = format!("upload-{}", Uuid::new_v4());
    let staging = project.path(format!("tmp/web-import/{upload_id}"));
    let result = async {
        std::fs::create_dir_all(&staging)?;
        let mut mode = None::<String>;
        let mut order = None::<String>;
        let mut target = None::<String>;
        let mut uploaded_files = 0usize;
        let mut uploaded_bytes = 0usize;
        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|error| anyhow!("读取上传内容失败: {error}"))?
        {
            let is_file = field.file_name().is_some();
            if !is_file {
                let name = field.name().unwrap_or_default().to_owned();
                let value = field
                    .text()
                    .await
                    .map_err(|error| anyhow!("读取上传选项失败: {error}"))?;
                match name.as_str() {
                    "mode" => mode = Some(value),
                    "order" => order = Some(value),
                    "target" => target = (!value.trim().is_empty()).then_some(value),
                    _ => {}
                }
                continue;
            }
            if uploaded_files >= MAX_UPLOAD_FILES {
                return Err(anyhow!("上传文件数量超过上限（{} 个）", MAX_UPLOAD_FILES));
            }
            let filename = field
                .file_name()
                .ok_or_else(|| anyhow!("上传文件缺少文件名"))?;
            let relative = safe_upload_relative_path(filename)?;
            let bytes = field
                .bytes()
                .await
                .map_err(|error| anyhow!("读取上传文件 {} 失败: {error}", relative.display()))?;
            if bytes.len() > MAX_UPLOAD_FILE_BYTES {
                return Err(anyhow!(
                    "文件 {} 超过单文件大小上限（{} MB）",
                    relative.display(),
                    MAX_UPLOAD_FILE_BYTES / 1024 / 1024
                ));
            }
            uploaded_bytes = uploaded_bytes
                .checked_add(bytes.len())
                .ok_or_else(|| anyhow!("上传总大小溢出"))?;
            if uploaded_bytes > MAX_UPLOAD_BYTES {
                return Err(anyhow!(
                    "上传总大小超过上限（{} MB）",
                    MAX_UPLOAD_BYTES / 1024 / 1024
                ));
            }
            let destination = staging.join(&relative);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(destination, &bytes)?;
            uploaded_files += 1;
        }
        if uploaded_files == 0 {
            return Err(anyhow!("没有收到文件；请至少选择一个文件或文件夹"));
        }
        let input_mode = mode
            .unwrap_or_else(|| "generic".into())
            .parse()
            .unwrap_or_default();
        let order_rule = order.unwrap_or_else(|| "filename".into());
        let batch =
            project.import_folder_with_options(&staging, input_mode, &order_rule, target, true)?;
        Ok::<_, anyhow::Error>(serde_json::json!({
            "ok": true,
            "batch": batch,
            "uploaded_files": uploaded_files,
            "uploaded_bytes": uploaded_bytes,
            "copied": true,
            "source": "browser-upload"
        }))
    }
    .await;
    let cleanup_error = std::fs::remove_dir_all(&staging).err();
    match result {
        Ok(mut value) => {
            if let Some(error) = cleanup_error {
                value["cleanup_warning"] = serde_json::Value::String(error.to_string());
            }
            Json(value)
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

/// Publish a single already-readable TXT/Markdown batch to `clean/` without
/// spending an LLM call.  Keeping this as a separate endpoint makes the
/// choice explicit in both the queue UI and API clients; visual sources must
/// continue through OCR and full-page repair.
async fn direct_clean(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let batch = match project.load_batch(&req.batch_id) {
        Ok(batch) => batch,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    match project
        .build_direct_text_clean(&batch, req.clean_name.as_deref())
        .await
    {
        Ok(artifact) => {
            let clean_path = project
                .clean_path_for_artifact(&artifact, req.clean_name.as_deref())
                .ok()
                .map(|path| path.to_string_lossy().replace('\\', "/"));
            Json(serde_json::json!({
                "ok": true,
                "mode": "direct_text",
                "batch_id": req.batch_id,
                "artifact": artifact,
                "clean_path": clean_path,
                "llm_called": false
            }))
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

fn safe_upload_relative_path(raw: &str) -> Result<PathBuf> {
    let normalized = raw.replace('\\', "/");
    let path = std::path::Path::new(&normalized);
    if normalized.trim().is_empty() || path.is_absolute() {
        return Err(anyhow!("上传文件名必须是相对路径"));
    }
    let mut safe = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => safe.push(part),
            _ => return Err(anyhow!("上传文件名包含非法路径: {raw}")),
        }
    }
    if safe.file_name().is_none() {
        return Err(anyhow!("上传文件名不能为空"));
    }
    Ok(safe)
}
async fn ocr(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Json<serde_json::Value> {
    let project_store = state.current_project().await;
    let batch = match project_store.load_batch(&req.batch_id) {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    };
    let (task_id, token) = state.tasks.start("ocr", Some(req.batch_id.clone())).await;
    let project = project_store.clone();
    let tasks = state.tasks.clone();
    let worker_token = token.clone();
    let monitor_task_id = task_id.clone();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
    let monitor = tasks.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            monitor.update_event(&monitor_task_id, &event).await;
        }
    });
    let provider: Box<dyn OcrProvider> = if req.provider.as_deref() == Some("mock") {
        Box::new(MockOcrProvider)
    } else {
        Box::new(TesseractOcrProvider::new(
            AppConfig::from_env().ocr_languages,
        ))
    };
    let response_task_id = task_id.clone();
    let worker_task_id = task_id.clone();
    tokio::spawn(async move {
        let result = project
            .run_ocr(&batch, provider.as_ref(), token, Some(event_tx))
            .await;
        match result {
            Ok(pages) => {
                if !worker_token.is_cancelled() {
                    let _ = project.append_event(&AgentEvent::TaskCompleted {
                        task_id: worker_task_id.clone(),
                    });
                }
                tasks
                    .finish(
                        &worker_task_id,
                        Some(serde_json::json!({"pages": pages.len()})),
                    )
                    .await
            }
            Err(error) => {
                if !worker_token.is_cancelled() {
                    let _ = project.append_event(&AgentEvent::Error {
                        message: error.to_string(),
                    });
                }
                tasks.fail(&worker_task_id, error.to_string()).await
            }
        }
    });
    Json(
        serde_json::json!({"ok":true,"task_id":response_task_id,"batch_id":req.batch_id,"status":"started"}),
    )
}
async fn normalize(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let pages = match project.load_pages(&req.batch_id) {
        Ok(pages) => pages,
        Err(error) => return Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    };
    match project.prepare_pages(&req.batch_id, &pages, req.refresh.unwrap_or(false)) {
        Ok(report) => Json(serde_json::json!({"ok": true, "report": report})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}
async fn propose(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Json<serde_json::Value> {
    let project_store = state.current_project().await;
    let batch = match project_store.load_batch(&req.batch_id) {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    };
    let resolved = match resolve_provider_request(
        &state,
        req.profile_id.as_deref(),
        req.preset.as_deref(),
        req.model.as_deref(),
        req.thinking.as_deref(),
        req.speed.as_deref(),
        req.provider.as_deref(),
    )
    .await
    {
        Ok(config) => config,
        Err(error) => return Json(serde_json::json!({"ok":false,"error":error.to_string()})),
    };
    let config = resolved.config.clone();
    let provider = resolved.provider();
    let (prompt, prompt_path) = repair_prompt_for(
        &batch.mode,
        &project_store.root,
        req.prompt_file.as_deref().map(std::path::Path::new),
    );
    let project = project_store.clone();
    let tasks = state.tasks.clone();
    let batch_id = req.batch_id.clone();
    let refresh = req.refresh.unwrap_or(false);
    let (task_id, token) = tasks.start("repair", Some(batch_id.clone())).await;
    let worker_token = token.clone();
    let monitor_task_id = task_id.clone();
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(64);
    let monitor = tasks.clone();
    let response_task_id = task_id.clone();
    let worker_task_id = task_id.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            monitor.update_event(&monitor_task_id, &event).await;
        }
    });
    tokio::spawn(async move {
        let result = project
            .repair_batch_with_cancel(
                &batch,
                provider.as_ref(),
                &config,
                &prompt,
                prompt_path,
                refresh,
                token,
                Some(event_tx),
            )
            .await;
        match result {
            Ok(run) => {
                let repaired_pages = run.pages.len();
                let error_count = run.errors.len();
                if !worker_token.is_cancelled() {
                    if error_count == 0 {
                        let _ = project.append_event(&AgentEvent::TaskCompleted {
                            task_id: worker_task_id.clone(),
                        });
                    } else if repaired_pages == 0 {
                        let _ = project.append_event(&AgentEvent::Error {
                            message: format!(
                                "LLM repair failed: {error_count} page(s) could not be repaired"
                            ),
                        });
                    } else {
                        let _ = project.append_event(&AgentEvent::Warning {
                            message: format!(
                                "LLM repair completed with {error_count} page error(s); {repaired_pages} page(s) are available"
                            ),
                        });
                    }
                }
                tasks
                    .finish_repair(
                        &worker_task_id,
                        repaired_pages,
                        error_count,
                        Some(serde_json::json!({
                            "repaired_pages": repaired_pages,
                            "errors": error_count
                        })),
                    )
                    .await
            }
            Err(error) => {
                if !worker_token.is_cancelled() {
                    let _ = project.append_event(&AgentEvent::Error {
                        message: error.to_string(),
                    });
                }
                tasks.fail(&worker_task_id, error.to_string()).await
            }
        }
    });
    Json(
        serde_json::json!({"ok":true,"task_id":response_task_id,"batch_id":req.batch_id,"status":"started"}),
    )
}
async fn apply(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let batch = match project.load_batch(&req.batch_id) {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    };
    match project.build_artifact_with_options_named(
        &batch,
        req.target.as_deref(),
        req.allow_unrepaired,
        req.clean_name.as_deref(),
    ) {
        Ok(a) => {
            let clean_path = project
                .clean_path_for_artifact(&a, req.clean_name.as_deref())
                .ok()
                .map(|path| path.to_string_lossy().replace('\\', "/"));
            Json(serde_json::json!({"ok":true,"artifact":a,"clean_path":clean_path}))
        }
        Err(e) => Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    }
}
async fn merge(
    State(state): State<AppState>,
    Json(req): Json<MergeRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let batch = match project.load_batch(&req.batch_id) {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    };
    let plan = if req.confirm {
        project.confirm_merge_plan(&batch, req.target.as_deref())
    } else {
        project.create_merge_plan(&batch, req.target.as_deref())
    };
    let plan = match plan {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    };
    let visual_source_ids = plan
        .sources
        .iter()
        .filter(|source| matches!(source.kind.as_str(), "image" | "pdf"))
        .map(|source| source.source_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let visual_page_ids = plan
        .pages
        .iter()
        .filter(|page| visual_source_ids.contains(page.source_id.as_str()))
        .map(|page| page.page_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let repair_incomplete = if visual_page_ids.is_empty() {
        false
    } else {
        match project.load_repair_run(&req.batch_id) {
            Ok(run) => {
                let repaired = run
                    .pages
                    .iter()
                    .map(|page| page.page_id.as_str())
                    .collect::<std::collections::HashSet<_>>();
                visual_page_ids
                    .iter()
                    .any(|page_id| !repaired.contains(page_id))
                    || run
                        .errors
                        .iter()
                        .any(|error| visual_page_ids.contains(error.page_id.as_str()))
            }
            Err(_) => true,
        }
    };
    let artifact = if req.confirm {
        match project.build_artifact_with_options_named(
            &batch,
            plan.target_document.as_deref(),
            req.allow_unrepaired,
            req.clean_name.as_deref(),
        ) {
            Ok(v) => Some(v),
            Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
        }
    } else {
        None
    };
    Json(serde_json::json!({
        "ok": true,
        "plan": plan,
        "confirmation_required": !req.confirm,
        "artifact": artifact,
        "clean_path": artifact.as_ref().and_then(|value| project.clean_path_for_artifact(value, req.clean_name.as_deref()).ok()).map(|path| path.to_string_lossy().replace('\\', "/")),
        "allow_unrepaired": req.allow_unrepaired,
        "warning": if req.allow_unrepaired || repair_incomplete {
            Some("one or more visual pages use normalized OCR without LLM repair")
        } else {
            None
        },
    }))
}
async fn merge_plan(
    State(state): State<AppState>,
    Query(query): Query<MergePlanQuery>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    match project.load_merge_plan(&query.batch_id) {
        Ok(plan) => Json(serde_json::json!({"ok": true, "plan": plan})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}
async fn update_merge_plan(
    State(state): State<AppState>,
    Json(req): Json<MergePlanUpdateRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    match project.update_merge_plan(&req.batch_id, req.plan) {
        Ok(plan) => {
            Json(serde_json::json!({"ok": true, "plan": plan, "confirmation_required": true}))
        }
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}
async fn merge_units(
    State(state): State<AppState>,
    Json(req): Json<MergeUnitsRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let plan = if req.confirm {
        let Some(merge_id) = req.merge_id.as_deref() else {
            return Json(serde_json::json!({
                "ok": false,
                "error": "merge_id is required when confirm=true"
            }));
        };
        project.confirm_cross_batch_merge_plan(merge_id, req.target.as_deref())
    } else {
        project.create_cross_batch_merge_plan(&req.units, req.target.as_deref())
    };
    let plan = match plan {
        Ok(v) => v,
        Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    };
    let artifact = if req.confirm {
        match project.build_cross_batch_artifact_with_options_named(
            &plan.merge_id,
            req.allow_unrepaired,
            req.clean_name.as_deref(),
        ) {
            Ok(v) => Some(v),
            Err(e) => return Json(serde_json::json!({"ok":false,"error":e.to_string()})),
        }
    } else {
        None
    };
    Json(serde_json::json!({
        "ok": true,
        "plan": plan,
        "confirmation_required": !req.confirm,
        "artifact": artifact,
        "clean_path": artifact.as_ref().and_then(|value| project.clean_path_for_artifact(value, req.clean_name.as_deref()).ok()).map(|path| path.to_string_lossy().replace('\\', "/")),
        "allow_unrepaired": req.allow_unrepaired,
        "warning": req.allow_unrepaired.then_some("one or more visual pages use normalized OCR without LLM repair"),
    }))
}
async fn edit_patch(
    State(state): State<AppState>,
    Json(req): Json<ReviewRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    match req.replacement {
        Some(replacement) => {
            match project.edit_patch(&req.batch_id, &req.correction_id, replacement) {
                Ok(set) => Json(serde_json::json!({"ok":true,"patches":set.patches})),
                Err(e) => Json(serde_json::json!({"ok":false,"error":e.to_string()})),
            }
        }
        None => Json(serde_json::json!({
            "ok": false,
            "error": "replacement is required; edit the artifact directly to restore text"
        })),
    }
}
async fn answer(
    State(state): State<AppState>,
    Json(req): Json<AnswerRequest>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let resolved = match resolve_provider_request(
        &state,
        req.profile_id.as_deref(),
        req.preset.as_deref(),
        req.model.as_deref(),
        req.thinking.as_deref(),
        req.speed.as_deref(),
        req.provider.as_deref(),
    )
    .await
    {
        Ok(config) => config,
        Err(error) => return Json(serde_json::json!({"ok":false,"error":error.to_string()})),
    };
    let config = resolved.config.clone();
    let provider = resolved.provider();
    let request = ConversationRequest {
        message: req.query,
        scope: req.scope,
        source_refs: req.source_refs,
        quotes: req.quotes,
        session_id: req.session_id,
    };
    match answer_with_request(&project, provider.as_ref(), &request, &config).await {
        Ok((answer, call, session)) => Json(serde_json::json!({
            "ok": true,
            "answer": answer,
            "session_id": session.session_id,
            "source_refs": session.messages.last().map(|message| message.source_refs.clone()).unwrap_or_default(),
            "usage": call
        })),
        Err(e) => Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    }
}
async fn cancel(
    State(state): State<AppState>,
    Json(req): Json<BatchRequest>,
) -> Json<serde_json::Value> {
    match state.tasks.cancel_batch(&req.batch_id).await {
        Some(task_id) => {
            Json(serde_json::json!({"ok":true,"task_id":task_id,"status":"cancelling"}))
        }
        None => Json(serde_json::json!({"ok":false,"error":"task not running"})),
    }
}
async fn tasks(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"ok": true, "tasks": state.tasks.list().await}))
}
async fn task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Json<serde_json::Value> {
    match state.tasks.get(&task_id).await {
        Some(snapshot) => Json(serde_json::json!({"ok": true, "task": snapshot})),
        None => Json(serde_json::json!({"ok": false, "error": "task not found"})),
    }
}
async fn cancel_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Json<serde_json::Value> {
    if state.tasks.cancel(&task_id).await {
        Json(serde_json::json!({"ok": true, "task_id": task_id, "status": "cancelling"}))
    } else {
        Json(serde_json::json!({"ok": false, "error": "task not found"}))
    }
}

#[derive(Debug, Serialize)]
struct SessionSummary {
    session_id: String,
    title: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    status: String,
    message_count: usize,
    last_message: Option<String>,
    source_refs: Vec<String>,
}

fn summarize_session(session: &Session) -> SessionSummary {
    let first_user = session
        .messages
        .iter()
        .find(|message| message.role.eq_ignore_ascii_case("user"))
        .map(|message| message.content.trim().to_owned())
        .filter(|message| !message.is_empty());
    let title = first_user
        .as_deref()
        .map(|message| message.chars().take(48).collect::<String>())
        .filter(|message: &String| !message.is_empty())
        .unwrap_or_else(|| "新对话".into());
    let last_message = session
        .messages
        .iter()
        .rev()
        .find(|message| message.role.eq_ignore_ascii_case("assistant"))
        .map(|message| message.content.chars().take(96).collect::<String>());
    let source_refs = session
        .messages
        .iter()
        .flat_map(|message| message.source_refs.iter().cloned())
        .chain(
            session
                .evidence
                .iter()
                .map(|excerpt| excerpt.source_ref.clone()),
        )
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    SessionSummary {
        session_id: session.session_id.clone(),
        title,
        created_at: session.created_at,
        updated_at: session.updated_at,
        status: session.status.clone(),
        message_count: session.messages.len(),
        last_message,
        source_refs,
    }
}

async fn sessions(State(state): State<AppState>) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let mut items = Vec::new();
    let directory = project.path("sessions");
    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(session) = serde_json::from_slice::<Session>(&bytes) {
                    items.push(summarize_session(&session));
                }
            }
        }
    }
    items.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Json(serde_json::json!({"ok": true, "sessions": items}))
}

async fn session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    if !session_id
        .chars()
        .all(|value| value.is_ascii_alphanumeric() || value == '-' || value == '_')
    {
        return Json(serde_json::json!({"ok": false, "error": "invalid session id"}));
    }
    let project = state.current_project().await;
    match project.load_session(&session_id) {
        Ok(value) => Json(serde_json::json!({"ok": true, "session": value})),
        Err(error) => Json(serde_json::json!({"ok": false, "error": error.to_string()})),
    }
}

async fn search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    match project.search(&q.q, q.scope.as_deref()) {
        Ok(v) => Json(serde_json::json!({"ok":true,"hits":v})),
        Err(e) => Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    }
}
async fn sources(
    State(state): State<AppState>,
    Query(query): Query<SourceListQuery>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    match project.list_merge_units() {
        Ok(units) => Json(serde_json::json!({
            "ok": true,
            "units": units.into_iter().filter(|unit| {
                query.batch_id.as_deref().map(|batch| unit.batch_id.as_deref() == Some(batch)).unwrap_or(true)
                    && query.kind.as_deref().map(|kind| unit.kind.eq_ignore_ascii_case(kind)).unwrap_or(true)
            }).collect::<Vec<_>>()
        })),
        Err(e) => Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    }
}
async fn usage(
    State(state): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let batch_id = q.get("batch_id").map(String::as_str);
    match project.runtime_usage_summary(batch_id) {
        Ok(summary) => Json(serde_json::json!({"ok":true,"summary":summary})),
        Err(e) => Json(serde_json::json!({"ok":false,"error":e.to_string()})),
    }
}

async fn activity(State(state): State<AppState>) -> Json<serde_json::Value> {
    let project = state.current_project().await;
    let events_path = project.path("events/events.jsonl");
    let events = std::fs::read_to_string(events_path)
        .unwrap_or_default()
        .lines()
        .rev()
        .take(80)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .collect::<Vec<_>>();
    let usage = project.runtime_usage_summary(None).ok();
    Json(serde_json::json!({
        "ok": true,
        "events": events,
        "tasks": state.tasks.list().await,
        "usage": usage,
    }))
}
async fn events(
    State(state): State<AppState>,
) -> Sse<impl futures_core::Stream<Item = Result<axum::response::sse::Event, Infallible>>> {
    let project = state.current_project().await;
    let path = project.path("events/events.jsonl");
    let stream = async_stream::stream! {let mut offset=0usize; loop {if let Ok(text)=tokio::fs::read_to_string(&path).await {if text.len()>offset {for line in text[offset..].lines(){yield Ok(axum::response::sse::Event::default().data(line));} offset=text.len();}} sleep(Duration::from_secs(1)).await;}};
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

/// Keep Web requests on the same model/preset rules as the CLI. In
/// particular, a Codex preset must not be overwritten by a stale model from
/// the project `.env`; a private HTTP gateway may still keep its own endpoint.
fn llm_config(
    preset: Option<&str>,
    model: Option<&str>,
    thinking: Option<&str>,
    speed: Option<&str>,
    provider: Option<&str>,
) -> Result<ResolvedLlm> {
    let backend = provider.unwrap_or("http").parse::<LlmBackend>()?;
    if backend == LlmBackend::CodexCli
        && model
            .filter(|value| !value.trim().is_empty())
            .is_some_and(|value| value.trim().to_ascii_lowercase().starts_with("glm"))
    {
        return Err(anyhow!(
            "Codex CLI 不能使用 GLM 模型；请使用 --provider http --model glm-5.3-flash，或选择 codex-luna"
        ));
    }
    let speed = speed
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.parse::<ReasoningSpeed>())
        .transpose()?;
    let preset = if backend == LlmBackend::CodexCli && preset.is_none() && model.is_none() {
        Some("codex-luna".to_owned())
    } else {
        preset.map(str::to_owned)
    };
    LlmOptions {
        backend,
        preset,
        model: model.map(str::to_owned),
        thinking: thinking.map(str::to_owned),
        speed,
    }
    .resolve()
}

#[cfg(test)]
mod tests {
    use super::{
        llm_config, resolve_provider_request, safe_upload_relative_path, AgentEvent, AppState,
        TaskRegistry,
    };
    use readtrace_core::{LlmBackend, ProjectStore};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[test]
    fn codex_preset_wins_over_stale_env_model() {
        let config = llm_config(
            Some("codex-luna"),
            None,
            None,
            Some("high"),
            Some("codex-cli"),
        )
        .expect("valid config");
        assert_eq!(config.config.model, "gpt-5.6-luna");
        assert_eq!(config.config.thinking_mode, "high");
        assert_eq!(config.config.input_price_per_million, 0.20);
        assert_eq!(config.config.cached_input_price_per_million, 0.02);
    }

    #[test]
    fn codex_without_selection_defaults_to_luna() {
        let config = llm_config(None, None, None, Some("high"), Some("codex-cli"))
            .expect("Codex default should resolve");
        assert_eq!(config.config.model, "gpt-5.6-luna");
        assert_eq!(config.config.thinking_mode, "high");
    }

    #[test]
    fn codex_rejects_glm_model_mismatch() {
        let error = llm_config(
            None,
            Some("glm-5.3-flash"),
            None,
            Some("high"),
            Some("codex-cli"),
        )
        .expect_err("Codex and GLM should not be mixed");
        assert!(error.to_string().contains("不能使用 GLM"));
    }

    #[test]
    fn web_speed_rejects_unknown_values() {
        let error = llm_config(None, None, None, Some("turbo"), Some("mock"))
            .expect_err("unknown speed should fail");
        assert!(error.to_string().contains("use low, mid or high"));
    }

    #[test]
    fn upload_paths_keep_folder_names_but_reject_traversal() {
        assert_eq!(
            safe_upload_relative_path("chapter\\page-01.png").unwrap(),
            std::path::PathBuf::from("chapter/page-01.png")
        );
        assert!(safe_upload_relative_path("../outside.txt").is_err());
        assert!(safe_upload_relative_path("C:/outside.txt").is_err());
    }

    #[tokio::test]
    async fn legacy_profile_id_in_provider_field_is_resolved() {
        let root =
            std::env::temp_dir().join(format!("readtrace-provider-{}", uuid::Uuid::new_v4()));
        let project = ProjectStore::init(&root).expect("temporary project");
        let state = AppState {
            project: Arc::new(RwLock::new(Arc::new(project))),
            workspace: Arc::new(RwLock::new(None)),
            tasks: TaskRegistry::new(),
        };
        let resolved = resolve_provider_request(
            &state,
            None,
            None,
            None,
            None,
            None,
            Some("tsinghua-glm-5.2"),
        )
        .await
        .expect("legacy profile id should resolve");
        assert_eq!(resolved.backend, LlmBackend::Http);
        assert_eq!(resolved.config.model, "glm-5.2");
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn task_registry_exposes_progress_and_cancellation() {
        let registry = TaskRegistry::new();
        let (task_id, token) = registry.start("repair", Some("batch-1".into())).await;
        registry
            .update_event(
                &task_id,
                &AgentEvent::Progress {
                    stage: "repair".into(),
                    current: 2,
                    total: 5,
                    message: "page 2".into(),
                },
            )
            .await;
        assert_eq!(registry.get(&task_id).await.unwrap().current, 2);
        assert!(registry.cancel(&task_id).await);
        assert!(token.is_cancelled());
        registry.finish(&task_id, None).await;
        assert_eq!(registry.get(&task_id).await.unwrap().status, "cancelled");
    }

    #[tokio::test]
    async fn repair_task_with_only_errors_is_failed_not_completed() {
        let registry = TaskRegistry::new();
        let (task_id, _token) = registry.start("repair", Some("batch-1".into())).await;
        registry
            .finish_repair(&task_id, 0, 2, Some(serde_json::json!({"errors": 2})))
            .await;
        assert_eq!(registry.get(&task_id).await.unwrap().status, "failed");
    }

    #[tokio::test]
    async fn repair_task_with_partial_errors_is_warning() {
        let registry = TaskRegistry::new();
        let (task_id, _token) = registry.start("repair", Some("batch-1".into())).await;
        registry
            .finish_repair(&task_id, 1, 1, Some(serde_json::json!({"errors": 1})))
            .await;
        assert_eq!(
            registry.get(&task_id).await.unwrap().status,
            "completed_with_errors"
        );
    }
}
