use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use readtrace_core::*;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(
    name = "readtrace",
    version,
    about = "读迹 ReadTrace：视觉文本整理与带引用阅读 Agent"
)]
struct Cli {
    /// Human-readable output is the default. Use `--format json` for scripts
    /// and for preserving the complete machine-readable response.
    #[arg(long, global = true, value_enum, default_value_t = OutputFormat::Human)]
    format: OutputFormat,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Init {
        project: PathBuf,
    },
    WorkspaceInit {
        workspace: PathBuf,
    },
    VaultCreate {
        workspace: PathBuf,
        name: String,
    },
    VaultList {
        workspace: PathBuf,
    },
    /// List a workspace's Vaults or a Vault's batches and selectable units.
    #[command(name = "ls", alias = "list")]
    Ls {
        path: PathBuf,
    },
    VaultPath {
        workspace: PathBuf,
        name_or_id: String,
    },
    ProviderCheck {
        #[arg(long)]
        preset: Option<String>,
        /// Override the model after loading the preset/.env configuration.
        #[arg(long)]
        model: Option<String>,
        /// Override reasoning effort (for example low, medium or high).
        #[arg(long)]
        thinking: Option<String>,
        #[arg(long, value_enum)]
        speed: Option<SpeedProfile>,
    },
    /// Show the OCR executables selected from `.env`, PATH, or local fallbacks.
    OcrCheck,
    /// Send a minimal request to verify endpoint, authentication and JSON
    /// response parsing without touching a Vault or consuming OCR input.
    AiCheck {
        #[arg(long, value_enum, default_value = "http")]
        provider: LlmProviderKind,
        #[arg(long)]
        preset: Option<String>,
        /// Override the model after loading the preset/.env configuration.
        #[arg(long)]
        model: Option<String>,
        /// Override reasoning effort (for example low, medium or high).
        #[arg(long)]
        thinking: Option<String>,
        #[arg(long, value_enum)]
        speed: Option<SpeedProfile>,
        /// Ledger for this probe call; defaults to runtime/calls.jsonl in the
        /// current directory so temporary checks are included in usage scans.
        #[arg(long, default_value = "runtime/calls.jsonl")]
        ledger: PathBuf,
    },
    ImportFolder {
        project: PathBuf,
        folder: PathBuf,
        #[arg(long, default_value = "generic")]
        mode: String,
        #[arg(long, default_value = "filename")]
        order: String,
        #[arg(long)]
        target: Option<String>,
        /// Keep only a reference to the original file instead of copying it.
        #[arg(long)]
        no_copy: bool,
    },
    ImportFile {
        project: PathBuf,
        file: PathBuf,
        #[arg(long, default_value = "generic")]
        mode: String,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        no_copy: bool,
    },
    /// Publish one TXT/Markdown file directly to clean without an LLM call.
    /// Use `repair`/`build` when a text source should still be rewritten by a
    /// model, or when the input is a PDF/image.
    #[command(name = "direct-clean")]
    DirectClean {
        project: PathBuf,
        batch_id: String,
        /// Publish under clean/<name>/document.md (nested names are allowed).
        #[arg(long)]
        clean_name: Option<String>,
    },
    Ocr {
        project: PathBuf,
        batch_id: String,
        #[arg(long, default_value = "real")]
        provider: OcrProviderKind,
    },
    Normalize {
        project: PathBuf,
        batch_id: String,
        #[arg(long)]
        refresh: bool,
    },
    Propose {
        project: PathBuf,
        batch_id: String,
        #[arg(long, default_value = "mock")]
        provider: LlmProviderKind,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        thinking: Option<String>,
        #[arg(long)]
        prompt_file: Option<PathBuf>,
        #[arg(long)]
        refresh: bool,
        #[arg(long, value_enum)]
        speed: Option<SpeedProfile>,
    },
    /// Run full-page LLM repair with resumable per-page checkpoints.
    Repair {
        project: PathBuf,
        batch_id: String,
        #[arg(long, default_value = "mock")]
        provider: LlmProviderKind,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        thinking: Option<String>,
        #[arg(long)]
        prompt_file: Option<PathBuf>,
        #[arg(long)]
        refresh: bool,
        #[arg(long, value_enum)]
        speed: Option<SpeedProfile>,
    },
    /// Build a new immutable Markdown revision from repaired pages.
    Build {
        project: PathBuf,
        batch_id: String,
        #[arg(long)]
        target: Option<String>,
        /// Allow normalized OCR for visual pages without a successful LLM repair.
        #[arg(long)]
        allow_unrepaired: bool,
        /// Publish the generated Markdown under clean/<name>/document.md.
        #[arg(long)]
        clean_name: Option<String>,
    },
    /// Preview or permanently delete one batch and its derived files.
    #[command(name = "delete-batch", alias = "rm-batch")]
    DeleteBatch {
        project: PathBuf,
        batch_id: String,
        /// Execute the displayed deletion plan. Without this flag nothing is removed.
        #[arg(long)]
        confirm: bool,
    },
    /// Preview or permanently delete one source/clean merge unit.
    #[command(name = "delete-unit", alias = "rm-unit")]
    DeleteUnit {
        project: PathBuf,
        /// Full unit_id, source_id, relative path, or an unambiguous selector.
        unit: String,
        /// Execute the displayed deletion plan. Without this flag nothing is removed.
        #[arg(long)]
        confirm: bool,
    },
    /// Preview or confirm deterministic merging of all pages in a batch.
    #[command(name = "merge")]
    MergeBatch {
        project: PathBuf,
        batch_id: String,
        /// Actually create the combined revision after showing/accepting the plan.
        #[arg(long)]
        confirm: bool,
        #[arg(long)]
        target: Option<String>,
        /// Allow normalized OCR for visual pages without a successful LLM repair.
        #[arg(long)]
        allow_unrepaired: bool,
        /// Publish the merged Markdown under clean/<name>/document.md.
        #[arg(long)]
        clean_name: Option<String>,
    },
    /// Preview or confirm a merge composed from units in different batches.
    #[command(name = "merge-units", alias = "merge-sources")]
    MergeUnits {
        project: PathBuf,
        /// Unit id, source id, relative path, or batch id; repeat to select
        /// units in the desired order.
        units: Vec<String>,
        #[arg(long)]
        confirm: bool,
        #[arg(long)]
        target: Option<String>,
        /// Allow normalized OCR units without a successful LLM repair.
        #[arg(long)]
        allow_unrepaired: bool,
        /// Publish the merged Markdown under clean/<name>/document.md.
        #[arg(long)]
        clean_name: Option<String>,
    },
    /// List one-file source and clean-document units available for merging.
    #[command(name = "sources", alias = "source-list")]
    Sources {
        project: PathBuf,
        #[arg(long)]
        batch_id: Option<String>,
        #[arg(long)]
        kind: Option<String>,
    },
    /// Summarize runtime provider calls recorded in runtime/calls.jsonl.
    Usage {
        /// Vault path; optional when --scan-root is provided.
        project: Option<PathBuf>,
        #[arg(long)]
        batch_id: Option<String>,
        /// Recursively scan Vaults and tmp outputs below this root and merge
        /// records by call_id (safe to run repeatedly).
        #[arg(long)]
        scan_root: Option<PathBuf>,
        /// Also write the merged ledger JSON to this path.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Process {
        project: PathBuf,
        input: PathBuf,
        #[arg(long, default_value = "generic")]
        mode: String,
        #[arg(long, default_value = "filename")]
        order: String,
        #[arg(long)]
        target: Option<String>,
        /// Publish the generated Markdown under clean/<name>/document.md.
        #[arg(long)]
        clean_name: Option<String>,
        #[arg(long, value_enum, default_value = "real")]
        ocr: OcrProviderKind,
        #[arg(long, value_enum, default_value = "http")]
        llm: LlmProviderKind,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        thinking: Option<String>,
        /// Legacy spelling: full repair is built by default.
        #[arg(long, hide = true)]
        apply_safe: bool,
        /// Legacy spelling: run repair but skip build for manual inspection.
        #[arg(long)]
        no_apply: bool,
        /// Allow normalized OCR when a visual page has no successful repair.
        #[arg(long)]
        allow_unrepaired: bool,
        /// Required when a folder import contains multiple source files.
        #[arg(long)]
        confirm_merge: bool,
        /// Regenerate normalization.json even when a human-edited version exists.
        #[arg(long)]
        refresh_normalization: bool,
        /// Keep only source paths in the batch manifest.
        #[arg(long)]
        no_copy: bool,
        #[arg(long)]
        prompt_file: Option<PathBuf>,
        #[arg(long)]
        refresh_repair: bool,
        #[arg(long, value_enum)]
        speed: Option<SpeedProfile>,
    },
    #[command(name = "apply", alias = "apply-safe")]
    Apply {
        project: PathBuf,
        batch_id: String,
        #[arg(long)]
        target: Option<String>,
    },
    #[command(name = "edit-patch", alias = "review")]
    EditPatch {
        project: PathBuf,
        batch_id: String,
        correction_id: String,
        #[arg(long)]
        replacement: String,
    },
    Search {
        project: PathBuf,
        query: String,
        #[arg(long)]
        scope: Option<String>,
    },
    Answer {
        project: PathBuf,
        query: String,
        #[arg(long)]
        scope: Option<String>,
        #[arg(long, default_value = "mock")]
        provider: LlmProviderKind,
        #[arg(long)]
        preset: Option<String>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        thinking: Option<String>,
        #[arg(long, value_enum)]
        speed: Option<SpeedProfile>,
        /// Source references to include as evidence (repeat --source-ref).
        #[arg(long = "source-ref")]
        source_refs: Vec<String>,
        /// Inline text to include as evidence (repeat --quote).
        #[arg(long = "quote")]
        quotes: Vec<String>,
        /// Read an evidence quote from a UTF-8 file (repeat --quote-file).
        #[arg(long = "quote-file")]
        quote_files: Vec<PathBuf>,
        /// Continue an existing conversation session.
        #[arg(long)]
        session_id: Option<String>,
    },
    Note {
        project: PathBuf,
        title: String,
        content: String,
        #[arg(long)]
        source_ref: Vec<String>,
    },
    SessionExport {
        project: PathBuf,
        session_id: String,
    },
    SessionNew {
        project: PathBuf,
    },
    SessionImport {
        project: PathBuf,
        file: PathBuf,
    },
    Reindex {
        project: PathBuf,
    },
    Progress {
        project: PathBuf,
        document_id: String,
        position: String,
    },
    ExportVault {
        project: PathBuf,
        target: PathBuf,
    },
    Serve {
        project: PathBuf,
        #[arg(long, default_value = "127.0.0.1:8787")]
        bind: String,
    },
    Demo {
        project: PathBuf,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum OcrProviderKind {
    Real,
    Mock,
}
#[derive(Clone, Debug, ValueEnum)]
enum LlmProviderKind {
    Http,
    Mock,
    #[value(name = "codex-cli", alias = "codex")]
    CodexCli,
}
#[derive(Clone, Debug, ValueEnum)]
enum SpeedProfile {
    #[value(name = "low", alias = "fast")]
    Low,
    #[value(name = "mid", alias = "medium", alias = "balanced")]
    Mid,
    #[value(name = "high", alias = "quality")]
    High,
}

#[derive(Clone, Debug, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

fn emit_json_or_human(value: serde_json::Value, format: &OutputFormat) -> Result<()> {
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        print_human_json(&value, 0, None);
    }
    Ok(())
}

fn emit_ls(value: serde_json::Value, format: &OutputFormat) -> Result<()> {
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }
    let kind = value
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("path");
    let root = value
        .get("root")
        .map(human_scalar)
        .unwrap_or_else(|| "-".into());
    println!("{kind}: {root}");
    if kind == "workspace" {
        println!("vaults:");
        for vault in value
            .get("vaults")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            println!(
                "  - {}  (id: {}, path: {})",
                vault
                    .get("name")
                    .map(human_scalar)
                    .unwrap_or_else(|| "-".into()),
                vault
                    .get("vault_id")
                    .map(human_scalar)
                    .unwrap_or_else(|| "-".into()),
                vault
                    .get("relative_path")
                    .map(human_scalar)
                    .unwrap_or_else(|| "-".into())
            );
        }
    } else if kind == "vault" {
        println!("batches:");
        for batch in value
            .get("batches")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = batch
                .get("batch_id")
                .map(human_scalar)
                .unwrap_or_else(|| "-".into());
            let status = batch
                .get("status")
                .map(human_scalar)
                .unwrap_or_else(|| "-".into());
            let count = batch
                .get("source_files")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            println!("  - {id}  sources={count}  status={status}");
        }
        println!("merge units:");
        for unit in value
            .get("merge_units")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
        {
            let unit_id = unit
                .get("unit_id")
                .map(human_scalar)
                .unwrap_or_else(|| "-".into());
            let unit_kind = unit
                .get("kind")
                .map(human_scalar)
                .unwrap_or_else(|| "-".into());
            let path = unit
                .get("path")
                .map(human_scalar)
                .unwrap_or_else(|| "-".into());
            let pages = unit
                .get("page_ids")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len);
            println!("  - {unit_id}  kind={unit_kind}  pages={pages}  path={path}");
        }
    }
    Ok(())
}

fn emit_sources(units: serde_json::Value, format: &OutputFormat) -> Result<()> {
    if matches!(format, OutputFormat::Json) {
        println!("{}", serde_json::to_string_pretty(&units)?);
        return Ok(());
    }
    let Some(items) = units.as_array() else {
        print_human_json(&units, 0, None);
        return Ok(());
    };
    if items.is_empty() {
        println!("no merge units found");
        return Ok(());
    }
    println!(
        "{:<36} {:<10} {:<36} {:>5}  PATH",
        "UNIT_ID", "KIND", "BATCH_ID", "PAGES"
    );
    for unit in items {
        let unit_id = unit
            .get("unit_id")
            .map(human_scalar)
            .unwrap_or_else(|| "-".into());
        let kind = unit
            .get("kind")
            .map(human_scalar)
            .unwrap_or_else(|| "-".into());
        let batch = unit
            .get("batch_id")
            .map(human_scalar)
            .unwrap_or_else(|| "-".into());
        let pages = unit
            .get("page_ids")
            .and_then(serde_json::Value::as_array)
            .map_or(0, Vec::len);
        let path = unit
            .get("path")
            .map(human_scalar)
            .unwrap_or_else(|| "-".into());
        println!("{unit_id:<36} {kind:<10} {batch:<36} {pages:>5}  {path}");
    }
    Ok(())
}

fn print_human_json(value: &serde_json::Value, indent: usize, label: Option<&str>) {
    let pad = " ".repeat(indent);
    match value {
        serde_json::Value::Object(map) => {
            if let Some(label) = label {
                println!("{pad}{label}:");
            }
            for (key, child) in map {
                match child {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        print_human_json(
                            child,
                            indent + usize::from(label.is_some()) * 2 + 2,
                            Some(key),
                        );
                    }
                    _ => println!("{pad}  {key}: {}", human_scalar(child)),
                }
            }
        }
        serde_json::Value::Array(items) => {
            if let Some(label) = label {
                println!("{pad}{label}:");
            }
            if items.is_empty() {
                println!("{pad}  (empty)");
            } else {
                for child in items {
                    match child {
                        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                            println!("{pad}  -");
                            print_human_json(child, indent + 2, None);
                        }
                        _ => println!("{pad}  - {}", human_scalar(child)),
                    }
                }
            }
        }
        _ => println!("{pad}{}", human_scalar(value)),
    }
}

fn human_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => "-".into(),
        _ => value.to_string(),
    }
}
impl SpeedProfile {
    fn reasoning_speed(&self) -> ReasoningSpeed {
        match self {
            Self::Low => ReasoningSpeed::Low,
            Self::Mid => ReasoningSpeed::Mid,
            Self::High => ReasoningSpeed::High,
        }
    }
}

impl LlmProviderKind {
    fn backend(&self) -> LlmBackend {
        match self {
            Self::Http => LlmBackend::Http,
            Self::Mock => LlmBackend::Mock,
            Self::CodexCli => LlmBackend::CodexCli,
        }
    }
}

fn llm_config(
    preset: Option<&str>,
    model: Option<&str>,
    thinking: Option<&str>,
    speed: Option<&SpeedProfile>,
    provider: Option<&LlmProviderKind>,
) -> Result<AppConfig> {
    LlmOptions {
        backend: provider
            .map(LlmProviderKind::backend)
            .unwrap_or(LlmBackend::Http),
        preset: preset.map(str::to_owned),
        model: model.map(str::to_owned),
        thinking: thinking.map(str::to_owned),
        speed: speed.map(SpeedProfile::reasoning_speed),
    }
    .resolve()
    .map(|resolved| resolved.config)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load project-local configuration before dispatching commands. Explicit
    // process environment variables still take precedence over .env values.
    let _ = dotenvy::from_filename(".env");
    tracing_subscriber::fmt().with_env_filter("info").init();
    // Clap's derive-generated parser is large on Windows. Parse it on a
    // larger stack so adding repeatable evidence options cannot overflow the
    // small process-main stack before any command is dispatched.
    let cli = std::thread::Builder::new()
        .name("readtrace-cli-parse".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(Cli::parse)
        .context("failed to start CLI parser thread")?
        .join()
        .map_err(|_| anyhow::anyhow!("CLI parser thread panicked"))?;
    let output_format = cli.format;
    match cli.command {
        Commands::Init { project } => {
            ProjectStore::init(&project)?;
            println!("initialized {}", project.display());
        }
        Commands::WorkspaceInit { workspace } => {
            WorkspaceStore::init(&workspace)?;
            println!("initialized workspace {}", workspace.display());
        }
        Commands::VaultCreate { workspace, name } => {
            let workspace = WorkspaceStore::open(workspace)?;
            emit_json_or_human(
                serde_json::to_value(workspace.create_vault(&name)?)?,
                &output_format,
            )?;
        }
        Commands::VaultList { workspace } => {
            let workspace = WorkspaceStore::open(workspace)?;
            emit_json_or_human(
                serde_json::to_value(workspace.list_vaults()?)?,
                &output_format,
            )?;
        }
        Commands::Ls { path } => {
            if path.join("workspace.json").is_file() {
                let workspace = WorkspaceStore::open(&path)?;
                emit_ls(
                    serde_json::to_value(serde_json::json!({
                        "type": "workspace",
                        "root": path,
                        "vaults": workspace.list_vaults()?
                    }))?,
                    &output_format,
                )?;
            } else {
                let store = ProjectStore::open(&path)?;
                let mut batches = Vec::new();
                let raw_dir = store.path("raw");
                if raw_dir.exists() {
                    for entry in std::fs::read_dir(raw_dir)? {
                        let batch_dir = entry?.path();
                        let batch_file = batch_dir.join("batch.json");
                        if batch_file.is_file() {
                            batches.push(
                                store.load_batch(
                                    batch_dir
                                        .file_name()
                                        .and_then(|name| name.to_str())
                                        .unwrap_or_default(),
                                )?,
                            );
                        }
                    }
                }
                batches.sort_by_key(|batch| std::cmp::Reverse(batch.created_at));
                emit_ls(
                    serde_json::to_value(serde_json::json!({
                        "type": "vault",
                        "root": path,
                        "batches": batches,
                        "merge_units": store.list_merge_units()?
                    }))?,
                    &output_format,
                )?;
            }
        }
        Commands::VaultPath {
            workspace,
            name_or_id,
        } => {
            let workspace = WorkspaceStore::open(workspace)?;
            println!("{}", workspace.vault_path(&name_or_id)?.display());
        }
        Commands::ProviderCheck {
            preset,
            model,
            thinking,
            speed,
        } => {
            let config = llm_config(
                preset.as_deref(),
                model.as_deref(),
                thinking.as_deref(),
                speed.as_ref(),
                None,
            )?;
            emit_json_or_human(
                serde_json::to_value(config.provider_summary())?,
                &output_format,
            )?;
        }
        Commands::OcrCheck => {
            let provider = TesseractOcrProvider::new(AppConfig::from_env().ocr_languages);
            let executable_available = |value: &str| {
                PathBuf::from(value).is_file()
                    || std::process::Command::new(value)
                        .arg("--version")
                        .output()
                        .map(|result| result.status.success())
                        .unwrap_or(false)
            };
            emit_json_or_human(
                serde_json::to_value(serde_json::json!({
                    "tesseract_bin": provider.tesseract_bin,
                    "tesseract_available": executable_available(&provider.tesseract_bin),
                     "pdftoppm_bin": provider.pdftoppm_bin,
                     "pdftoppm_available": executable_available(&provider.pdftoppm_bin),
                     "pdfinfo_bin": provider.pdfinfo_bin,
                     "pdfinfo_available": executable_available(&provider.pdfinfo_bin),
                     "ocr_dpi": provider.dpi,
                     "ocr_concurrency": provider.ocr_concurrency,
                     "ocr_languages": provider.languages,
                    "tessdata_prefix": std::env::var("TESSDATA_PREFIX").ok()
                }))?,
                &output_format,
            )?;
        }
        Commands::AiCheck {
            provider,
            preset,
            model,
            thinking,
            speed,
            ledger,
        } => {
            let config = llm_config(
                preset.as_deref(),
                model.as_deref(),
                thinking.as_deref(),
                speed.as_ref(),
                Some(&provider),
            )?;
            let report = match provider {
                LlmProviderKind::Http => {
                    OpenAiCompatibleProvider::new(config.clone()).probe().await
                }
                LlmProviderKind::CodexCli => CodexCliProvider::new(&config).probe().await,
                LlmProviderKind::Mock => AiProbeReport {
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
            let provider_name = match report.endpoint.as_str() {
                "codex://local-cli" => "codex-cli",
                "mock://local" => "mock",
                _ => "openai-compatible",
            };
            let mut call = CallRecord::from_usage(
                provider_name,
                &report.endpoint,
                &report.model,
                "ai_check",
                report.usage.clone(),
                &config,
                report.elapsed_ms as u64,
                report.ok,
            );
            call.request_id = report.request_id.clone();
            call.phase = Some("probe".into());
            call.thinking_mode = Some(config.thinking_mode.clone());
            if !report.ok {
                call.error_type = Some("provider_error".into());
            }
            ProjectStore::append_runtime_call_at(&ledger, &call)?;
            emit_json_or_human(serde_json::to_value(&report)?, &output_format)?;
            if !report.ok {
                std::process::exit(1);
            }
        }
        Commands::ImportFolder {
            project,
            folder,
            mode,
            order,
            target,
            no_copy,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.import_folder_with_options(
                folder,
                mode.parse()?,
                &order,
                target,
                !no_copy,
            )?;
            emit_json_or_human(serde_json::to_value(&batch)?, &output_format)?;
        }
        Commands::ImportFile {
            project,
            file,
            mode,
            target,
            no_copy,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.import_file_with_options(file, mode.parse()?, target, !no_copy)?;
            emit_json_or_human(serde_json::to_value(&batch)?, &output_format)?;
        }
        Commands::DirectClean {
            project,
            batch_id,
            clean_name,
        } => {
            let store = ProjectStore::open(&project)?;
            let batch = store.load_batch(&batch_id)?;
            let artifact = store
                .build_direct_text_clean(&batch, clean_name.as_deref())
                .await?;
            let clean_path = store.clean_path_for_artifact(&artifact, clean_name.as_deref())?;
            emit_json_or_human(
                serde_json::json!({
                    "mode": "direct_text",
                    "batch_id": batch_id,
                    "artifact": artifact,
                    "clean_path": clean_path,
                    "llm_called": false
                }),
                &output_format,
            )?;
        }
        Commands::Ocr {
            project,
            batch_id,
            provider,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.load_batch(&batch_id)?;
            let cancel = CancellationToken::new();
            let p: Box<dyn OcrProvider> = match provider {
                OcrProviderKind::Real => Box::new(TesseractOcrProvider::new(
                    AppConfig::from_env().ocr_languages,
                )),
                OcrProviderKind::Mock => Box::new(MockOcrProvider),
            };
            let (tx, mut rx) = tokio::sync::mpsc::channel(128);
            let run = store.run_ocr(&batch, p.as_ref(), cancel.clone(), Some(tx));
            tokio::pin!(run);
            let pages = loop {
                tokio::select! { result=&mut run => break result?, Some(event)=rx.recv()=> println!("event: {:?}",event), _=tokio::signal::ctrl_c()=>{cancel.cancel(); println!("cancelling...");} }
            };
            println!("OCR pages: {}", pages.len());
        }
        Commands::Normalize {
            project,
            batch_id,
            refresh,
        } => {
            let store = ProjectStore::open(project)?;
            let pages = store.load_pages(&batch_id)?;
            let report = store.prepare_pages(&batch_id, &pages, refresh)?;
            let changes = report
                .pages
                .iter()
                .map(|page| page.changes.len())
                .sum::<usize>();
            println!(
                "normalized {} pages, {} deterministic changes: {}",
                report.pages.len(),
                changes,
                store
                    .path(format!("generated/{batch_id}/normalization.json"))
                    .display()
            );
        }
        Commands::Propose {
            project,
            batch_id,
            provider,
            preset,
            model,
            thinking,
            prompt_file,
            refresh,
            speed,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.load_batch(&batch_id)?;
            let config = llm_config(
                preset.as_deref(),
                model.as_deref(),
                thinking.as_deref(),
                speed.as_ref(),
                Some(&provider),
            )?;
            let p: Box<dyn LlmProvider> = match provider {
                LlmProviderKind::Mock => Box::new(MockLlmProvider),
                LlmProviderKind::Http => Box::new(OpenAiCompatibleProvider::new(config.clone())),
                LlmProviderKind::CodexCli => Box::new(CodexCliProvider::new(&config)),
            };
            let (prompt, prompt_path) =
                repair_prompt_for(&batch.mode, &store.root, prompt_file.as_deref());
            let run = store.repair_batch(
                &batch,
                p.as_ref(),
                &config,
                &prompt,
                prompt_path,
                refresh,
                None,
            );
            tokio::pin!(run);
            let result = run.await?;
            println!(
                "repaired {} pages ({} errors)",
                result.pages.len(),
                result.errors.len()
            );
        }
        Commands::Repair {
            project,
            batch_id,
            provider,
            preset,
            model,
            thinking,
            prompt_file,
            refresh,
            speed,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.load_batch(&batch_id)?;
            let config = llm_config(
                preset.as_deref(),
                model.as_deref(),
                thinking.as_deref(),
                speed.as_ref(),
                Some(&provider),
            )?;
            let p: Box<dyn LlmProvider> = match provider {
                LlmProviderKind::Mock => Box::new(MockLlmProvider),
                LlmProviderKind::Http => Box::new(OpenAiCompatibleProvider::new(config.clone())),
                LlmProviderKind::CodexCli => Box::new(CodexCliProvider::new(&config)),
            };
            let (prompt, prompt_path) =
                repair_prompt_for(&batch.mode, &store.root, prompt_file.as_deref());
            let run = store
                .repair_batch(
                    &batch,
                    p.as_ref(),
                    &config,
                    &prompt,
                    prompt_path,
                    refresh,
                    None,
                )
                .await?;
            let usage = store.runtime_usage_summary(Some(&batch_id))?;
            let repair_file = store.path(format!("generated/{batch_id}/repair.json"));
            let repair_directory = store.path(format!("generated/{batch_id}/repair"));
            let result_files = run
                .pages
                .iter()
                .map(|page| {
                    store
                        .path(format!("generated/{batch_id}/repair/{}.json", page.page_id))
                        .to_string_lossy()
                        .into_owned()
                })
                .collect::<Vec<_>>();
            let result_previews = run
                .pages
                .iter()
                .map(|page| {
                    serde_json::json!({
                        "page_id": page.page_id,
                        "source_ref": page.source_ref,
                        "repaired_text_preview": page.repaired_text.chars().take(500).collect::<String>()
                    })
                })
                .collect::<Vec<_>>();
            emit_json_or_human(
                serde_json::to_value(serde_json::json!({
                    "batch_id": batch_id,
                    "repaired_pages": run.pages.len(),
                    "errors": run.errors,
                        "repair_file": repair_file,
                        "repair_directory": repair_directory,
                        "result_files": result_files,
                        "result_previews": result_previews,
                        "usage": usage
                }))?,
                &output_format,
            )?;
        }
        Commands::Build {
            project,
            batch_id,
            target,
            allow_unrepaired,
            clean_name,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.load_batch(&batch_id)?;
            let artifact = store.build_artifact_with_options_named(
                &batch,
                target.as_deref(),
                allow_unrepaired,
                clean_name.as_deref(),
            )?;
            let clean_path = store.clean_path_for_artifact(&artifact, clean_name.as_deref())?;
            emit_json_or_human(
                serde_json::json!({"artifact": artifact, "clean_path": clean_path}),
                &output_format,
            )?;
        }
        Commands::DeleteBatch {
            project,
            batch_id,
            confirm,
        } => {
            let store = ProjectStore::open(&project)?;
            let plan = if confirm {
                store.delete_batch(&batch_id)?
            } else {
                store.plan_delete_batch(&batch_id)?
            };
            emit_json_or_human(serde_json::to_value(&plan)?, &output_format)?;
        }
        Commands::DeleteUnit {
            project,
            unit,
            confirm,
        } => {
            let store = ProjectStore::open(&project)?;
            let plan = if confirm {
                store.delete_unit(&unit)?
            } else {
                store.plan_delete_unit(&unit)?
            };
            emit_json_or_human(serde_json::to_value(&plan)?, &output_format)?;
        }
        Commands::MergeBatch {
            project,
            batch_id,
            confirm,
            target,
            allow_unrepaired,
            clean_name,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.load_batch(&batch_id)?;
            let plan = if confirm {
                store.confirm_merge_plan(&batch, target.as_deref())?
            } else {
                store.create_merge_plan(&batch, target.as_deref())?
            };
            let artifact = if confirm {
                Some(store.build_artifact_with_options_named(
                    &batch,
                    plan.target_document.as_deref(),
                    allow_unrepaired,
                    clean_name.as_deref(),
                )?)
            } else {
                None
            };
            emit_json_or_human(
                serde_json::to_value(serde_json::json!({
                    "plan": plan,
                    "plan_file": store.path(format!("generated/{batch_id}/merge_plan.json")),
                    "confirmation_required": !confirm,
                    "artifact": artifact,
                    "artifact_absolute_path": artifact.as_ref().map(|value| store.path(&value.path))
                    ,"clean_path": artifact.as_ref().and_then(|value| store.clean_path_for_artifact(value, clean_name.as_deref()).ok())
                    ,"allow_unrepaired": allow_unrepaired
                    ,"warning": allow_unrepaired.then_some("one or more visual pages use normalized OCR without LLM repair")
                }))?,
                &output_format,
            )?;
        }
        Commands::MergeUnits {
            project,
            units,
            confirm,
            target,
            allow_unrepaired,
            clean_name,
        } => {
            let store = ProjectStore::open(project)?;
            let plan = if confirm {
                let merge_id = units
                    .first()
                    .ok_or_else(|| anyhow::anyhow!("--confirm requires a merge id"))?;
                store.confirm_cross_batch_merge_plan(merge_id, target.as_deref())?
            } else {
                store.create_cross_batch_merge_plan(&units, target.as_deref())?
            };
            let artifact = if confirm {
                Some(store.build_cross_batch_artifact_with_options_named(
                    &plan.merge_id,
                    allow_unrepaired,
                    clean_name.as_deref(),
                )?)
            } else {
                None
            };
            emit_json_or_human(
                serde_json::to_value(serde_json::json!({
                    "plan": plan,
                    "plan_file": store.path(format!("generated/merges/{}/merge_plan.json", plan.merge_id)),
                    "confirmation_required": !confirm,
                    "artifact": artifact,
                    "artifact_absolute_path": artifact.as_ref().map(|value| store.path(&value.path))
                    ,"clean_path": artifact.as_ref().and_then(|value| store.clean_path_for_artifact(value, clean_name.as_deref()).ok())
                    ,"allow_unrepaired": allow_unrepaired
                    ,"warning": allow_unrepaired.then_some("one or more visual pages use normalized OCR without LLM repair")
                }))?,
                &output_format,
            )?;
        }
        Commands::Sources {
            project,
            batch_id,
            kind,
        } => {
            let store = ProjectStore::open(project)?;
            let units = store
                .list_merge_units()?
                .into_iter()
                .filter(|unit| {
                    batch_id
                        .as_deref()
                        .map(|batch| unit.batch_id.as_deref() == Some(batch))
                        .unwrap_or(true)
                })
                .filter(|unit| {
                    kind.as_deref()
                        .map(|kind| unit.kind.eq_ignore_ascii_case(kind))
                        .unwrap_or(true)
                })
                .collect::<Vec<_>>();
            emit_sources(serde_json::to_value(&units)?, &output_format)?;
        }
        Commands::Usage {
            project,
            batch_id,
            scan_root,
            out,
        } => {
            let calls = if let Some(root) = scan_root {
                ProjectStore::scan_runtime_calls(root)?
            } else {
                let project = project.ok_or_else(|| {
                    anyhow::anyhow!("usage requires <PROJECT> unless --scan-root is provided")
                })?;
                let store = ProjectStore::open(project)?;
                store.runtime_calls(batch_id.as_deref())?
            };
            let calls = if let Some(batch_id) = batch_id {
                calls
                    .into_iter()
                    .filter(|call| call.batch_id.as_deref() == Some(batch_id.as_str()))
                    .collect::<Vec<_>>()
            } else {
                calls
            };
            let summary = ProjectStore::runtime_usage_summary_for_calls(&calls);
            let value = serde_json::json!({"calls": calls, "summary": summary});
            if let Some(out) = out {
                if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&out, serde_json::to_vec_pretty(&value)?)?;
                if matches!(output_format, OutputFormat::Json) {
                    emit_json_or_human(value, &output_format)?;
                } else {
                    println!("wrote merged usage ledger {}", out.display());
                    println!(
                        "calls={} input_tokens={} cached_input_tokens={} output_tokens={} total_tokens={} cost_usd={} cost_cny={}",
                        summary.calls,
                        summary
                            .input_tokens
                            .map_or_else(|| "-".into(), |value| value.to_string()),
                        summary
                            .cached_input_tokens
                            .map_or_else(|| "-".into(), |value| value.to_string()),
                        summary
                            .output_tokens
                            .map_or_else(|| "-".into(), |value| value.to_string()),
                        summary
                            .total_tokens
                            .map_or_else(|| "-".into(), |value| value.to_string()),
                        summary
                            .cost_usd
                            .map_or_else(|| "-".into(), |value| value.to_string()),
                        summary
                            .cost_cny
                            .map_or_else(|| "-".into(), |value| value.to_string())
                    );
                }
            } else {
                emit_json_or_human(value, &output_format)?;
            }
        }
        Commands::Process {
            project,
            input,
            mode,
            order,
            target,
            clean_name,
            ocr,
            llm,
            preset,
            model,
            thinking,
            apply_safe,
            no_apply,
            allow_unrepaired,
            confirm_merge,
            refresh_normalization,
            no_copy,
            prompt_file,
            refresh_repair,
            speed,
        } => {
            let store = if project.join("metadata.json").exists() {
                ProjectStore::open(&project)?
            } else {
                ProjectStore::init(&project)?
            };
            let mode = mode.parse()?;
            let batch = if input.is_dir() {
                store.import_folder_with_options(&input, mode, &order, target.clone(), !no_copy)?
            } else {
                store.import_file_with_options(&input, mode, target.clone(), !no_copy)?
            };
            let ocr_provider: Box<dyn OcrProvider> = match ocr {
                OcrProviderKind::Real => Box::new(TesseractOcrProvider::new(
                    AppConfig::from_env().ocr_languages,
                )),
                OcrProviderKind::Mock => Box::new(MockOcrProvider),
            };
            let pages = store
                .run_ocr(
                    &batch,
                    ocr_provider.as_ref(),
                    CancellationToken::new(),
                    None,
                )
                .await?;
            let report = store.prepare_pages(&batch.batch_id, &pages, refresh_normalization)?;
            let config = llm_config(
                preset.as_deref(),
                model.as_deref(),
                thinking.as_deref(),
                speed.as_ref(),
                Some(&llm),
            )?;
            let llm_provider: Box<dyn LlmProvider> = match llm {
                LlmProviderKind::Mock => Box::new(MockLlmProvider),
                LlmProviderKind::Http => Box::new(OpenAiCompatibleProvider::new(config.clone())),
                LlmProviderKind::CodexCli => Box::new(CodexCliProvider::new(&config)),
            };
            let (prompt, prompt_path) =
                repair_prompt_for(&batch.mode, &store.root, prompt_file.as_deref());
            let repair = store
                .repair_batch(
                    &batch,
                    llm_provider.as_ref(),
                    &config,
                    &prompt,
                    prompt_path,
                    refresh_repair,
                    None,
                )
                .await?;
            let merge_required = batch.source_files.len() > 1;
            let merge_plan = if merge_required {
                Some(if confirm_merge {
                    store.confirm_merge_plan(&batch, target.as_deref())?
                } else {
                    store.create_merge_plan(&batch, target.as_deref())?
                })
            } else {
                None
            };
            let requested_build = !no_apply || apply_safe;
            let auto_build = requested_build && (!merge_required || confirm_merge);
            let artifact = if auto_build {
                Some(
                    store.build_artifact_with_options_named(
                        &batch,
                        merge_plan
                            .as_ref()
                            .and_then(|plan| plan.target_document.as_deref())
                            .or(target.as_deref()),
                        allow_unrepaired,
                        clean_name.as_deref(),
                    )?,
                )
            } else {
                None
            };
            emit_json_or_human(
                serde_json::to_value(serde_json::json!({
                    "batch_id": batch.batch_id,
                    "pages": pages.len(),
                    "normalization_changes": report.pages.iter().map(|page| page.changes.len()).sum::<usize>(),
                    "repaired_pages": repair.pages.len(),
                    "repair_errors": repair.errors,
                    "auto_built": auto_build,
                    "review_only": !auto_build,
                    "merge_required": merge_required,
                    "merge_confirmation_required": merge_required && !confirm_merge,
                    "merge_plan": merge_plan,
                    "merge_plan_file": merge_required.then(|| store.path(format!("generated/{}/merge_plan.json", batch.batch_id))),
                    "artifact": artifact,
                    "repair_file": store.path(format!("generated/{}/repair.json", batch.batch_id)),
                    "repair_directory": store.path(format!("generated/{}/repair", batch.batch_id)),
                    "result_files": repair.pages.iter().map(|page| store.path(format!("generated/{}/repair/{}.json", batch.batch_id, page.page_id))).collect::<Vec<_>>(),
                    "result_previews": repair.pages.iter().map(|page| serde_json::json!({"page_id": page.page_id, "source_ref": page.source_ref, "repaired_text_preview": page.repaired_text.chars().take(500).collect::<String>()})).collect::<Vec<_>>(),
                    "artifact_absolute_path": artifact.as_ref().map(|value| store.path(&value.path)),
                    "clean_path": artifact.as_ref().and_then(|value| store.clean_path_for_artifact(value, clean_name.as_deref()).ok()),
                    "usage": store.runtime_usage_summary(Some(&batch.batch_id))?,
                }))?,
                &output_format,
            )?;
        }
        Commands::Apply {
            project,
            batch_id,
            target,
        } => {
            let store = ProjectStore::open(project)?;
            let batch = store.load_batch(&batch_id)?;
            let artifact = if store
                .path(format!("generated/{batch_id}/repair.json"))
                .exists()
            {
                store.build_artifact(&batch, target.as_deref())?
            } else {
                let set: CorrectionSet = serde_json::from_slice(&std::fs::read(
                    store.path(format!("generated/{batch_id}/proposed_changes.json")),
                )?)?;
                store.apply_changes(&batch, &set, target.as_deref())?
            };
            let clean_path = store.clean_path_for_artifact(&artifact, None)?;
            println!(
                "generated {} ({})\npublished clean {}",
                artifact.path,
                artifact.operation,
                clean_path.display()
            );
        }
        Commands::EditPatch {
            project,
            batch_id,
            correction_id,
            replacement,
        } => {
            let store = ProjectStore::open(project)?;
            let set = store.edit_patch(&batch_id, &correction_id, replacement)?;
            println!(
                "updated legacy patch {}; {} entries in set; run apply to regenerate",
                correction_id,
                set.patches.len()
            );
        }
        Commands::Search {
            project,
            query,
            scope,
        } => {
            let store = ProjectStore::open(project)?;
            let hits = store.search(&query, scope.as_deref())?;
            if matches!(output_format, OutputFormat::Json) {
                emit_json_or_human(serde_json::to_value(&hits)?, &output_format)?;
            } else {
                for hit in hits {
                    println!("{}:{}", hit.path, hit.line);
                    if hit.context.is_empty() {
                        println!("  {}", hit.snippet);
                    } else {
                        for line in hit.context {
                            println!("  {}", line);
                        }
                    }
                }
            }
        }
        Commands::Answer {
            project,
            query,
            scope,
            provider,
            preset,
            model,
            thinking,
            speed,
            session_id,
            source_refs,
            quotes,
            quote_files,
        } => {
            let store = ProjectStore::open(project)?;
            let config = llm_config(
                preset.as_deref(),
                model.as_deref(),
                thinking.as_deref(),
                speed.as_ref(),
                Some(&provider),
            )?;
            let p: Box<dyn LlmProvider> = match provider {
                LlmProviderKind::Mock => Box::new(MockLlmProvider),
                LlmProviderKind::Http => Box::new(OpenAiCompatibleProvider::new(config.clone())),
                LlmProviderKind::CodexCli => Box::new(CodexCliProvider::new(&config)),
            };
            let mut quotes = quotes;
            for path in quote_files {
                quotes
                    .push(std::fs::read_to_string(&path).with_context(|| {
                        format!("failed to read quote file {}", path.display())
                    })?);
            }
            let request = ConversationRequest {
                message: query,
                scope,
                source_refs,
                quotes,
                session_id,
            };
            let (answer, call, session) =
                answer_with_request(&store, p.as_ref(), &request, &config).await?;
            if matches!(output_format, OutputFormat::Json) {
                emit_json_or_human(
                    serde_json::to_value(serde_json::json!({
                        "answer": answer,
                        "session_id": session.session_id,
                        "source_refs": session.messages.last().map(|message| message.source_refs.clone()).unwrap_or_default(),
                        "usage": call
                    }))?,
                    &output_format,
                )?;
            } else {
                println!("{answer}");
                println!("\n[session_id={}]", session.session_id);
                println!(
                    "[tokens input={:?} cached_input={:?} output={:?} total={:?}]",
                    call.input_tokens,
                    call.cached_input_tokens,
                    call.output_tokens,
                    call.total_tokens
                );
                println!(
                    "[cost usd={:?} cny={:?} pricing={}]",
                    call.cost_usd, call.cost_cny, call.pricing_version
                );
            }
        }
        Commands::Note {
            project,
            title,
            content,
            source_ref,
        } => {
            let store = ProjectStore::open(project)?;
            let p = store.save_note(&title, &content, &source_ref)?;
            println!("saved {}", p.display());
        }
        Commands::SessionExport {
            project,
            session_id,
        } => {
            let store = ProjectStore::open(project)?;
            let s = store.load_session(&session_id)?;
            emit_json_or_human(serde_json::to_value(&s)?, &output_format)?;
        }
        Commands::SessionNew { project } => {
            let store = ProjectStore::open(project)?;
            let s = store.new_session(AppConfig::from_env())?;
            println!("{}", s.session_id);
        }
        Commands::SessionImport { project, file } => {
            let store = ProjectStore::open(project)?;
            let s = store.import_session(file)?;
            println!("restored {}", s.session_id);
        }
        Commands::Reindex { project } => {
            let store = ProjectStore::open(project)?;
            let index = IndexStore::open(&store)?;
            index.rebuild(&store)?;
            println!("index rebuilt");
        }
        Commands::Progress {
            project,
            document_id,
            position,
        } => {
            let store = ProjectStore::open(project)?;
            let state = store.update_progress(&document_id, &position)?;
            emit_json_or_human(serde_json::to_value(&state)?, &output_format)?;
        }
        Commands::ExportVault { project, target } => {
            let store = ProjectStore::open(project)?;
            println!("exported {}", store.export_vault(target)?.display());
        }
        Commands::Serve { project, bind } => {
            readtrace_server::run(project, &bind).await?;
        }
        Commands::Demo { project } => demo(project).await?,
    }
    Ok(())
}

async fn demo(project: PathBuf) -> Result<()> {
    let store = ProjectStore::init(&project)?;
    let fixture = store.path("demo-input.txt");
    std::fs::write(
        &fixture,
        "第一章\n沈默的角色继续前进。\n他记录了这段阅读。\n",
    )?;
    let batch = store.import_file(&fixture, InputMode::default(), None)?;
    store
        .run_ocr(&batch, &MockOcrProvider, CancellationToken::new(), None)
        .await?;
    let config = AppConfig::default();
    let (prompt, prompt_path) = repair_prompt_for(&batch.mode, &store.root, None);
    let repair = store
        .repair_batch(
            &batch,
            &MockLlmProvider,
            &config,
            &prompt,
            prompt_path,
            false,
            None,
        )
        .await?;
    let artifact = store.build_artifact(&batch, None)?;
    let (answer, _) = answer_with_citations(
        &store,
        &MockLlmProvider,
        "角色",
        None,
        &AppConfig::default(),
    )
    .await?;
    println!(
        "Demo complete\nrepaired pages: {}\nartifact: {}\nanswer: {}",
        repair.pages.len(),
        artifact.path,
        answer
    );
    Ok(())
}
