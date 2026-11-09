//! Takeover and restore of external tool configurations (Codex CLI, Claude Code).
//!
//! The module rewrites only the few keys that Cursor BYOK owns, keeps a byte-for-byte
//! copy of every touched file, and can put the original files back later.
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use axum::{
    extract::{Path as RoutePath, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    api::byok,
    control::ControlService,
    store::{IntegrationsState, SavedConfigFile, ToolTakeoverState},
    Error, Result,
};

const CODEX_PROVIDER_ID: &str = "cursor-byok";
const CODEX_PROVIDER_NAME: &str = "Cursor BYOK";
const TOOL_CODEX: &str = "codex";
const TOOL_CLAUDE: &str = "claude";

static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn lock() -> &'static tokio::sync::Mutex<()> {
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrationStatus {
    pub tool: String,
    pub label: String,
    pub installed: bool,
    pub config_path: String,
    pub applied: bool,
    pub model: String,
    pub applied_at_ms: i64,
    pub backup_available: bool,
    pub current_model: String,
    pub current_provider: String,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct IntegrationsOverview {
    pub codex: IntegrationStatus,
    pub claude: IntegrationStatus,
    pub external_api_enabled: bool,
    pub has_api_key: bool,
    pub base_url: String,
    pub models: Vec<byok::PublicModel>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ApplyIntegrationInput {
    pub model: String,
}

struct CodexPaths {
    config: PathBuf,
    auth: PathBuf,
}

pub async fn overview(State(service): State<ControlService>) -> Result<Json<IntegrationsOverview>> {
    let state = service.store().integrations_state().await?;
    let external = service.store().external_api_settings().await?;
    let ports = service.store().port_settings().await?;
    let models = byok::public_models(service.store(), service.plugin_registry()).await?;
    Ok(Json(IntegrationsOverview {
        codex: codex_status(&state.codex)?,
        claude: claude_status(&state.claude)?,
        external_api_enabled: external.enabled,
        has_api_key: !external.api_key.trim().is_empty(),
        base_url: format!("http://127.0.0.1:{}/byok", ports.service_port),
        models,
    }))
}

pub async fn apply(
    State(service): State<ControlService>,
    RoutePath(tool): RoutePath<String>,
    Json(input): Json<ApplyIntegrationInput>,
) -> Result<Json<IntegrationStatus>> {
    let _guard = lock().lock().await;
    let tool = normalize_tool(&tool)?;
    let model = input.model.trim().to_owned();
    if model.is_empty() {
        return Err(Error::Config("请选择要接入的模型".into()));
    }
    let external = service.store().external_api_settings().await?;
    if !external.enabled {
        return Err(Error::Config(
            "外部 API 未开启，请先在「系统设置 → 外部 API」中开启并设置密钥".into(),
        ));
    }
    let api_key = external.api_key.trim().to_owned();
    if api_key.is_empty() {
        return Err(Error::Config(
            "外部 API 密钥为空，请先在「系统设置 → 外部 API」中设置密钥".into(),
        ));
    }
    let models = byok::public_models(service.store(), service.plugin_registry()).await?;
    if !models.iter().any(|item| item.id == model) {
        return Err(Error::Config(format!(
            "模型 {model} 不在可代理列表中，请刷新页面后重试"
        )));
    }
    let ports = service.store().port_settings().await?;
    let base_root = format!("http://127.0.0.1:{}", ports.service_port);

    let mut state = service.store().integrations_state().await?;
    let originals = {
        let slot = slot_mut(&mut state, tool);
        if slot.applied && !slot.originals.is_empty() {
            slot.originals.clone()
        } else {
            capture_originals(tool)?
        }
    };
    match tool {
        TOOL_CODEX => apply_codex(&originals, &model, &base_root, &api_key)?,
        TOOL_CLAUDE => apply_claude(&originals, &model, &base_root, &api_key)?,
        _ => unreachable!(),
    }
    let status = {
        let slot = slot_mut(&mut state, tool);
        slot.applied = true;
        slot.model = model;
        slot.applied_at_ms = crate::store::now_ms();
        slot.originals = originals;
        status_for(tool, slot)?
    };
    service.store().set_integrations_state(&state).await?;
    Ok(Json(status))
}

pub async fn restore(
    State(service): State<ControlService>,
    RoutePath(tool): RoutePath<String>,
) -> Result<Json<IntegrationStatus>> {
    let _guard = lock().lock().await;
    let tool = normalize_tool(&tool)?;
    let mut state = service.store().integrations_state().await?;
    let status = {
        let slot = slot_mut(&mut state, tool);
        for original in &slot.originals {
            restore_file(original)?;
        }
        slot.applied = false;
        slot.model.clear();
        slot.applied_at_ms = 0;
        slot.originals.clear();
        status_for(tool, slot)?
    };
    service.store().set_integrations_state(&state).await?;
    Ok(Json(status))
}

fn normalize_tool(tool: &str) -> Result<&'static str> {
    match tool.trim().to_ascii_lowercase().as_str() {
        "codex" => Ok(TOOL_CODEX),
        "claude" | "claude-code" | "claudecode" => Ok(TOOL_CLAUDE),
        other => Err(Error::Config(format!("不支持的工具: {other}"))),
    }
}

fn slot_mut<'a>(state: &'a mut IntegrationsState, tool: &str) -> &'a mut ToolTakeoverState {
    match tool {
        TOOL_CODEX => &mut state.codex,
        _ => &mut state.claude,
    }
}

fn status_for(tool: &str, saved: &ToolTakeoverState) -> Result<IntegrationStatus> {
    match tool {
        TOOL_CODEX => codex_status(saved),
        _ => claude_status(saved),
    }
}

fn home_dir() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| Error::Config("无法定位用户主目录".into()))
}

fn codex_paths() -> Result<CodexPaths> {
    let directory = home_dir()?.join(".codex");
    Ok(CodexPaths {
        config: directory.join("config.toml"),
        auth: directory.join("auth.json"),
    })
}

fn claude_settings_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".claude").join("settings.json"))
}

fn claude_local_settings_path() -> Result<PathBuf> {
    Ok(home_dir()?.join(".claude").join("settings.local.json"))
}

fn codex_status(saved: &ToolTakeoverState) -> Result<IntegrationStatus> {
    let paths = codex_paths()?;
    let config = read_text_optional(&paths.config)?;
    let auth = read_text_optional(&paths.auth)?;
    let installed = paths.config.exists() || paths.auth.exists();
    let (current_model, current_provider) =
        config.as_deref().map(codex_current).unwrap_or_default();
    let mut notes = Vec::new();
    if !installed {
        notes.push("未检测到 Codex CLI 配置（~/.codex）".into());
    }
    if saved.applied && current_provider != CODEX_PROVIDER_ID {
        notes.push(format!(
            "配置当前使用 {}，可能已被其他工具覆盖",
            if current_provider.is_empty() {
                "未知供应商"
            } else {
                current_provider.as_str()
            }
        ));
    }
    if saved.applied {
        notes.push("修改后需要重启 Codex CLI / 编辑器插件才会生效".into());
    }
    if auth
        .as_deref()
        .map(|content| content.contains("\"auth_mode\""))
        .unwrap_or(false)
    {
        notes.push(
            "当前 Codex 使用 ChatGPT 登录态，接管后会改成 API Key 模式（原登录信息已备份）".into(),
        );
    }
    Ok(IntegrationStatus {
        tool: TOOL_CODEX.into(),
        label: "Codex CLI".into(),
        installed,
        config_path: paths.config.display().to_string(),
        applied: saved.applied,
        model: saved.model.clone(),
        applied_at_ms: saved.applied_at_ms,
        backup_available: !saved.originals.is_empty(),
        current_model,
        current_provider,
        notes,
    })
}

fn claude_status(saved: &ToolTakeoverState) -> Result<IntegrationStatus> {
    let path = claude_settings_path()?;
    let local_path = claude_local_settings_path()?;
    let local_exists = local_path.exists();
    // settings.local.json 的优先级高于 settings.json，当前生效值按“settings → local”覆盖合并。
    let settings = read_text_optional(&path)?;
    let local = if local_exists {
        read_text_optional(&local_path)?
    } else {
        None
    };
    let installed = path.exists() || local_exists;
    let mut notes = Vec::new();
    if !installed {
        notes.push("未检测到 Claude Code 配置（~/.claude）".into());
    }
    if local_exists {
        notes.push(if saved.applied {
            "已同时写入 settings.local.json（原内容已备份）".into()
        } else {
            "检测到 settings.local.json，它的环境变量会覆盖 settings.json；接管时会一并修改并备份".into()
        });
    }
    if saved.applied {
        notes.push("修改后需要重启 Claude Code 才会生效".into());
    }
    let (current_model, current_base_url) = claude_effective(settings.as_deref(), local.as_deref());
    Ok(IntegrationStatus {
        tool: TOOL_CLAUDE.into(),
        label: "Claude Code".into(),
        installed,
        config_path: path.display().to_string(),
        applied: saved.applied,
        model: saved.model.clone(),
        applied_at_ms: saved.applied_at_ms,
        backup_available: !saved.originals.is_empty(),
        current_model,
        current_provider: current_base_url,
        notes,
    })
}

fn claude_current(content: &str) -> (String, String) {
    let Ok(value) = serde_json::from_str::<Value>(content) else {
        return (String::new(), String::new());
    };
    let env = value.get("env").and_then(Value::as_object);
    let read = |key: &str| {
        env.and_then(|env| env.get(key))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    (read("ANTHROPIC_MODEL"), read("ANTHROPIC_BASE_URL"))
}

fn claude_effective(settings: Option<&str>, local: Option<&str>) -> (String, String) {
    let mut model = String::new();
    let mut base_url = String::new();
    for content in [settings, local].into_iter().flatten() {
        let (next_model, next_base_url) = claude_current(content);
        if !next_model.is_empty() {
            model = next_model;
        }
        if !next_base_url.is_empty() {
            base_url = next_base_url;
        }
    }
    (model, base_url)
}

fn capture_originals(tool: &str) -> Result<Vec<SavedConfigFile>> {
    match tool {
        TOOL_CODEX => {
            let paths = codex_paths()?;
            Ok(vec![
                read_optional(&paths.config)?,
                read_optional(&paths.auth)?,
            ])
        }
        TOOL_CLAUDE => Ok(vec![
            read_optional(&claude_settings_path()?)?,
            read_optional(&claude_local_settings_path()?)?,
        ]),
        _ => Err(Error::Config(format!("不支持的工具: {tool}"))),
    }
}

fn apply_codex(
    originals: &[SavedConfigFile],
    model: &str,
    base_root: &str,
    api_key: &str,
) -> Result<()> {
    let paths = codex_paths()?;
    let current = read_text_optional(&paths.config)?
        .or_else(|| originals.first().map(|file| file.content.clone()))
        .unwrap_or_default();
    let base_url = format!("{base_root}/byok/v1");
    let rendered = render_codex_config(&current, model, &base_url);
    write_file(&paths.config, &rendered)?;
    let auth = format!(
        "{{\n  \"OPENAI_API_KEY\": {}\n}}\n",
        serde_json::to_string(api_key)?
    );
    write_file(&paths.auth, &auth)
}

fn apply_claude(
    originals: &[SavedConfigFile],
    model: &str,
    base_root: &str,
    api_key: &str,
) -> Result<()> {
    let settings_path = claude_settings_path()?;
    let local_path = claude_local_settings_path()?;
    // settings.local.json 优先级更高；存在时两份都写，否则只写 settings.json。
    let targets = [settings_path, local_path];
    let base_url = format!("{base_root}/byok");
    let mut writes: Vec<(PathBuf, String)> = Vec::new();
    for (index, path) in targets.into_iter().enumerate() {
        if index == 1 && !path.exists() {
            continue;
        }
        let current = read_text_optional(&path)?
            .or_else(|| originals.get(index).map(|file| file.content.clone()))
            .unwrap_or_default();
        let rendered = render_claude_settings(&current, &base_url, api_key, model)?;
        writes.push((path, rendered));
    }
    for (path, rendered) in writes {
        write_file(&path, &rendered)?;
    }
    Ok(())
}

fn restore_file(original: &SavedConfigFile) -> Result<()> {
    let path = PathBuf::from(&original.path);
    if original.existed {
        write_file(&path, &original.content)
    } else {
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Io(error)),
        }
    }
}

fn read_optional(path: &Path) -> Result<SavedConfigFile> {
    let content = read_text_optional(path)?;
    Ok(SavedConfigFile {
        path: path.display().to_string(),
        existed: content.is_some(),
        content: content.unwrap_or_default(),
    })
}

fn read_text_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read(path) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| Error::Config(format!("{} 不是合法的 UTF-8 文本", path.display()))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::Io(error)),
    }
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

fn codex_current(content: &str) -> (String, String) {
    (
        codex_top_level_value(content, "model").unwrap_or_default(),
        codex_top_level_value(content, "model_provider").unwrap_or_default(),
    )
}

fn codex_top_level_value(content: &str, key: &str) -> Option<String> {
    let mut in_table = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_table = true;
            continue;
        }
        if in_table {
            continue;
        }
        if top_level_key(trimmed) == Some(key) {
            let (_, value) = trimmed.split_once('=')?;
            return Some(unquote(value.trim()));
        }
    }
    None
}

fn top_level_key(line: &str) -> Option<&str> {
    if line.starts_with('[') {
        return None;
    }
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    Some(key)
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        return value[1..value.len() - 1].to_owned();
    }
    value.split('#').next().unwrap_or(value).trim().to_owned()
}

fn strip_section(content: &str, header: &str) -> String {
    let mut output: Vec<&str> = Vec::new();
    let mut skipping = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skipping = trimmed == header;
            if skipping {
                continue;
            }
        }
        if !skipping {
            output.push(line);
        }
    }
    output.join("\n")
}

fn render_codex_config(content: &str, model: &str, base_url: &str) -> String {
    let content = strip_section(content, "[model_providers.cursor-byok]");
    let mut lines: Vec<String> = Vec::new();
    let mut in_table = false;
    let mut have_model = false;
    let mut have_provider = false;
    let mut have_auth = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_table = true;
            lines.push(line.to_owned());
            continue;
        }
        if !in_table {
            match top_level_key(trimmed) {
                Some("model") => {
                    lines.push(format!("model = {}", quote(model)));
                    have_model = true;
                    continue;
                }
                Some("model_provider") => {
                    lines.push(format!("model_provider = {}", quote(CODEX_PROVIDER_ID)));
                    have_provider = true;
                    continue;
                }
                Some("preferred_auth_method") => {
                    lines.push("preferred_auth_method = \"apikey\"".into());
                    have_auth = true;
                    continue;
                }
                _ => {}
            }
        }
        lines.push(line.to_owned());
    }
    let insert_at = lines
        .iter()
        .position(|line| line.trim_start().starts_with('['))
        .unwrap_or(lines.len());
    let mut missing: Vec<String> = Vec::new();
    if !have_model {
        missing.push(format!("model = {}", quote(model)));
    }
    if !have_provider {
        missing.push(format!("model_provider = {}", quote(CODEX_PROVIDER_ID)));
    }
    if !have_auth {
        missing.push("preferred_auth_method = \"apikey\"".into());
    }
    for (offset, line) in missing.into_iter().enumerate() {
        lines.insert(insert_at + offset, line);
    }
    let mut rendered = lines.join("\n");
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered.push('\n');
    rendered.push_str(&format!(
        "[model_providers.{CODEX_PROVIDER_ID}]\nname = {}\nbase_url = {}\nwire_api = \"responses\"\nrequires_openai_auth = true\n",
        quote(CODEX_PROVIDER_NAME),
        quote(base_url)
    ));
    rendered
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

fn render_claude_settings(
    content: &str,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<String> {
    let mut value: Value = if content.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(content)
            .map_err(|error| Error::Config(format!("Claude 配置文件不是合法 JSON: {error}")))?
    };
    let root = value
        .as_object_mut()
        .ok_or_else(|| Error::Config("Claude 配置文件顶层必须是 JSON 对象".into()))?;
    let env = root
        .entry("env".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let env = env
        .as_object_mut()
        .ok_or_else(|| Error::Config("Claude 配置文件 env 必须是 JSON 对象".into()))?;
    for (key, value) in [
        ("ANTHROPIC_BASE_URL", base_url),
        ("ANTHROPIC_AUTH_TOKEN", api_key),
        ("ANTHROPIC_MODEL", model),
        ("ANTHROPIC_DEFAULT_OPUS_MODEL", model),
        ("ANTHROPIC_DEFAULT_SONNET_MODEL", model),
        ("ANTHROPIC_DEFAULT_HAIKU_MODEL", model),
    ] {
        env.insert(key.to_owned(), Value::String(value.to_owned()));
    }
    let mut rendered = serde_json::to_string_pretty(&value)?;
    rendered.push('\n');
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_config_keeps_user_sections_and_rewrites_owned_keys() {
        let original = r#"model = "old-model"
model_provider = "custom"
model_reasoning_effort = "high"

[model_providers.custom]
name = "custom"
base_url = "https://example.com/v1"
wire_api = "responses"

[mcp_servers.firecrawl]
command = "npx"
"#;
        let rendered = render_codex_config(
            original,
            "plugin:example:codex/gpt-6.1-sol",
            "http://127.0.0.1:61702/byok/v1",
        );
        assert!(rendered.contains("model = \"plugin:example:codex/gpt-6.1-sol\""));
        assert!(rendered.contains("model_provider = \"cursor-byok\""));
        assert!(rendered.contains("preferred_auth_method = \"apikey\""));
        assert!(rendered.contains("[model_providers.custom]"));
        assert!(rendered.contains("[mcp_servers.firecrawl]"));
        assert!(rendered.contains("[model_providers.cursor-byok]"));
        assert!(rendered.ends_with("requires_openai_auth = true\n"));
        assert_eq!(rendered.matches("[model_providers.cursor-byok]").count(), 1);
    }

    #[test]
    fn codex_config_replaces_previous_takeover_block() {
        let original = r#"model = "a"

[model_providers.cursor-byok]
name = "Cursor BYOK"
base_url = "http://127.0.0.1:1/byok/v1"

[other]
key = "value"
"#;
        let rendered = render_codex_config(original, "b", "http://127.0.0.1:2/byok/v1");
        assert_eq!(rendered.matches("[model_providers.cursor-byok]").count(), 1);
        assert!(rendered.contains("http://127.0.0.1:2/byok/v1"));
        assert!(rendered.contains("[other]"));
    }

    #[test]
    fn claude_settings_merge_env_and_keep_other_keys() {
        let original = r#"{"model":"opus","permissions":{"allow":["Bash"]}}"#;
        let rendered = render_claude_settings(
            original,
            "http://127.0.0.1:61702/byok",
            "test-key",
            "plugin:example:codex/gpt-6.1-sol",
        )
        .unwrap();
        let value: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(value["model"], "opus");
        assert_eq!(value["permissions"]["allow"][0], "Bash");
        assert_eq!(
            value["env"]["ANTHROPIC_BASE_URL"],
            "http://127.0.0.1:61702/byok"
        );
        assert_eq!(value["env"]["ANTHROPIC_AUTH_TOKEN"], "test-key");
        assert_eq!(
            value["env"]["ANTHROPIC_MODEL"],
            "plugin:example:codex/gpt-6.1-sol"
        );
    }

    #[test]
    fn claude_effective_reads_local_override_and_falls_back() {
        let settings =
            r#"{"env":{"ANTHROPIC_BASE_URL":"https://a.example","ANTHROPIC_MODEL":"model-a"}}"#;
        let local = r#"{"env":{"ANTHROPIC_MODEL":"model-b"}}"#;
        let (model, base_url) = claude_effective(Some(settings), Some(local));
        assert_eq!(model, "model-b");
        assert_eq!(base_url, "https://a.example");
        let (model, base_url) = claude_effective(Some(settings), None);
        assert_eq!(model, "model-a");
        assert_eq!(base_url, "https://a.example");
    }

    #[test]
    fn codex_top_level_value_ignores_tables() {
        let content = "model = \"top\"\nmodel_provider = \"p\"\n\n[model_providers.custom]\nmodel = \"nested\"\n";
        assert_eq!(codex_current(content), ("top".into(), "p".into()));
    }
}
