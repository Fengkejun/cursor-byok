//! Migration of the model-provider ownership of existing Codex chats.
//!
//! Codex records the provider of every chat twice: in the `threads` table of
//! `~/.codex/state_N.sqlite` and in the `session_meta` head line of the
//! rollout transcript. Once the takeover switches the active provider to
//! `cursor-byok`, chats that were recorded under another provider make the
//! desktop client refuse to continue them ("模型提供商不匹配"). This module
//! rewrites that ownership so previous tasks can be resumed, and remembers the
//! original values so the change can be undone later.
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use serde::Serialize;
use serde_json::Value;
use sqlx::{sqlite::SqliteConnectOptions, sqlite::SqliteRow, Row};

use crate::{
    store::{MigratedThread, ThreadMigration},
    Error, Result,
};

/// Result of one migration or restore pass.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MigrationReport {
    /// Provider the chats use after the pass.
    pub provider: String,
    /// Chats whose provider was rewritten in the Codex database.
    pub threads_updated: usize,
    /// Transcripts whose `session_meta` head line was rewritten.
    pub rollouts_updated: usize,
    /// Chats that still point at another provider after the pass.
    pub pending: usize,
    /// Files that could not be rewritten, with the reason.
    pub skipped: Vec<String>,
}

/// The Codex home directory (`~/.codex`), honouring `CODEX_HOME` the same way
/// the Codex CLI does.
pub fn codex_home() -> Result<PathBuf> {
    if let Ok(value) = std::env::var("CODEX_HOME") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    dirs::home_dir()
        .map(|home| home.join(".codex"))
        .ok_or_else(|| Error::Config("无法定位用户主目录".into()))
}

/// The newest `state_N.sqlite` of the Codex home directory, if there is one.
pub fn state_db_path() -> Result<Option<PathBuf>> {
    let home = codex_home()?;
    let entries = std::fs::read_dir(&home)
        .map_err(|error| Error::Config(format!("无法读取 Codex 配置目录: {error}")))?;
    let mut newest: Option<(u64, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(version) = name
            .strip_prefix("state_")
            .and_then(|rest| rest.strip_suffix(".sqlite"))
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        if !path.is_file() {
            continue;
        }
        let better = newest
            .as_ref()
            .map(|(current, _)| version > *current)
            .unwrap_or(true);
        if better {
            newest = Some((version, path));
        }
    }
    Ok(newest.map(|(_, path)| path))
}

/// Every rollout transcript Codex keeps for past chats.
fn rollout_files() -> Vec<PathBuf> {
    let Ok(home) = codex_home() else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for directory in ["sessions", "archived_sessions"] {
        collect_jsonl(&home.join(directory), &mut files);
    }
    files
}

fn collect_jsonl(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
}

/// The provider recorded in a rollout head line.
fn head_provider(head: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(head).ok()?;
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    value
        .get("payload")?
        .get("model_provider")?
        .as_str()
        .map(|value| value.to_owned())
}

/// The session id recorded in a rollout head line.
fn head_session_id(head: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(head).ok()?;
    let value: Value = serde_json::from_str(text.trim()).ok()?;
    value
        .get("payload")?
        .get("session_id")?
        .as_str()
        .map(|value| value.to_owned())
}

/// Rewrites the `model_provider` field of a rollout head line.
///
/// Returns the provider that was replaced together with the session id, or
/// `None` when the transcript already points at the wanted provider.
fn patch_rollout_head(path: &Path, provider: &str) -> Result<Option<(String, String)>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut head = Vec::new();
    reader.read_until(b'\n', &mut head)?;
    if head.is_empty() {
        return Ok(None);
    }
    let Some(current) = head_provider(&head) else {
        return Ok(None);
    };
    if current == provider {
        return Ok(None);
    }
    let Some(patched) = replace_json_field(&head, "model_provider", provider) else {
        return Err(Error::Config(format!(
            "无法定位 model_provider 字段: {}",
            path.display()
        )));
    };
    // Safety net: the rewritten head must still parse and carry the new value.
    match head_provider(&patched) {
        Some(value) if value == provider => {}
        _ => {
            return Err(Error::Config(format!(
                "改写后的会话头无法校验: {}",
                path.display()
            )))
        }
    }
    let session_id = head_session_id(&head).unwrap_or_default();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("rollout");
    let temporary = path.with_file_name(format!("{file_name}.byok-tmp"));
    {
        let mut out = File::create(&temporary)?;
        out.write_all(&patched)?;
        std::io::copy(&mut reader, &mut out)?;
        out.flush()?;
        out.sync_all()?;
    }
    // Copy over the original instead of renaming: the transcript keeps its
    // path and never disappears while Codex may still hold a handle on it.
    std::fs::copy(&temporary, path)?;
    let _ = std::fs::remove_file(&temporary);
    Ok(Some((current, session_id)))
}

/// Replaces a JSON string field inside a raw JSON document, byte for byte.
fn replace_json_field(document: &[u8], key: &str, new_value: &str) -> Option<Vec<u8>> {
    let pattern = format!("\"{key}\"");
    let start = find_subslice(document, pattern.as_bytes())?;
    let mut index = start + pattern.len();
    while index < document.len() && (document[index] as char).is_whitespace() {
        index += 1;
    }
    if document.get(index) != Some(&b':') {
        return None;
    }
    index += 1;
    while index < document.len() && (document[index] as char).is_whitespace() {
        index += 1;
    }
    if document.get(index) != Some(&b'"') {
        return None;
    }
    let value_start = index + 1;
    let mut value_end = value_start;
    while value_end < document.len() && document[value_end] != b'"' {
        if document[value_end] == b'\\' {
            value_end += 1;
        }
        value_end += 1;
    }
    if value_end >= document.len() {
        return None;
    }
    let mut out = Vec::with_capacity(document.len() + new_value.len());
    out.extend_from_slice(&document[..value_start]);
    out.extend_from_slice(new_value.as_bytes());
    out.extend_from_slice(&document[value_end..]);
    Some(out)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

async fn open_state_db() -> Result<Option<sqlx::SqlitePool>> {
    let Some(path) = state_db_path()? else {
        return Ok(None);
    };
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(false)
        .busy_timeout(Duration::from_secs(15));
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|error| {
            Error::Config(format!(
                "无法打开 Codex 会话数据库 {}: {error}，请先退出 Codex 桌面端后重试",
                path.display()
            ))
        })?;
    Ok(Some(pool))
}

fn row_id(row: &SqliteRow) -> Option<String> {
    if let Ok(value) = row.try_get::<String, _>("id") {
        return Some(value);
    }
    if let Ok(value) = row.try_get::<i64, _>("id") {
        return Some(value.to_string());
    }
    None
}

/// Rewrites the provider of every indexed chat and reports what changed.
async fn migrate_database(provider: &str) -> Result<(usize, usize, Vec<MigratedThread>)> {
    let Some(pool) = open_state_db().await? else {
        return Ok((0, 0, Vec::new()));
    };
    let rows = match sqlx::query("SELECT id, model_provider, rollout_path FROM threads")
        .fetch_all(&pool)
        .await
    {
        Ok(rows) => rows,
        Err(error) => {
            tracing::warn!(%error, "codex thread migration skipped the state database");
            return Ok((0, 0, Vec::new()));
        }
    };
    let mut updated = 0;
    let mut pending = 0;
    let mut records = Vec::new();
    for row in rows {
        let Some(id) = row_id(&row) else {
            continue;
        };
        let current = row
            .try_get::<Option<String>, _>("model_provider")
            .unwrap_or(None)
            .filter(|value| !value.trim().is_empty());
        let rollout_path = row
            .try_get::<Option<String>, _>("rollout_path")
            .unwrap_or(None)
            .unwrap_or_default();
        let Some(current) = current else {
            continue;
        };
        if current == provider {
            continue;
        }
        match sqlx::query("UPDATE threads SET model_provider = ? WHERE id = ?")
            .bind(provider)
            .bind(&id)
            .execute(&pool)
            .await
        {
            Ok(result) => {
                updated += result.rows_affected() as usize;
                records.push(MigratedThread {
                    id,
                    rollout_path,
                    original_provider: current,
                });
            }
            Err(error) => {
                pending += 1;
                tracing::warn!(%error, "could not migrate a codex chat");
            }
        }
    }
    Ok((updated, pending, records))
}

/// Puts the original provider of every migrated chat back.
async fn restore_database(records: &[MigratedThread]) -> Result<(usize, usize)> {
    let Some(pool) = open_state_db().await? else {
        return Ok((0, 0));
    };
    let mut restored = 0;
    let mut pending = 0;
    for record in records {
        if record.id.is_empty() {
            continue;
        }
        match sqlx::query("UPDATE threads SET model_provider = ? WHERE id = ?")
            .bind(&record.original_provider)
            .bind(&record.id)
            .execute(&pool)
            .await
        {
            Ok(result) => restored += result.rows_affected() as usize,
            Err(error) => {
                pending += 1;
                tracing::warn!(%error, "could not restore a codex chat");
            }
        }
    }
    Ok((restored, pending))
}

/// Moves every existing Codex chat to `provider` so that previous tasks can be
/// continued under the takeover.
pub async fn migrate(provider: &str) -> Result<(ThreadMigration, MigrationReport)> {
    let provider = provider.trim();
    if provider.is_empty() {
        return Err(Error::Config("缺少供应商名称".into()));
    }
    let (threads_updated, pending, mut records) = migrate_database(provider).await?;
    let mut report = MigrationReport {
        provider: provider.to_owned(),
        threads_updated,
        pending,
        ..Default::default()
    };
    for path in rollout_files() {
        match patch_rollout_head(&path, provider) {
            Ok(Some((original, session_id))) => {
                report.rollouts_updated += 1;
                records.push(MigratedThread {
                    id: session_id,
                    rollout_path: path.to_string_lossy().into_owned(),
                    original_provider: original,
                });
            }
            Ok(None) => {}
            Err(error) => report.skipped.push(format!("{}: {error}", path.display())),
        }
    }
    let migration = ThreadMigration {
        provider: provider.to_owned(),
        migrated_at_ms: crate::store::now_ms(),
        threads: records,
    };
    Ok((migration, report))
}

/// Puts the original provider ownership of the migrated chats back.
pub async fn restore(migration: &ThreadMigration) -> Result<MigrationReport> {
    let mut report = MigrationReport {
        provider: migration.provider.clone(),
        ..Default::default()
    };
    if migration.threads.is_empty() {
        return Ok(report);
    }
    let (restored, pending) = restore_database(&migration.threads).await?;
    report.threads_updated = restored;
    report.pending = pending;
    for record in &migration.threads {
        if record.rollout_path.is_empty() {
            continue;
        }
        let path = PathBuf::from(&record.rollout_path);
        match patch_rollout_head(&path, &record.original_provider) {
            Ok(Some(_)) => report.rollouts_updated += 1,
            Ok(None) => {}
            Err(error) => report
                .skipped
                .push(format!("{}: {error}", record.rollout_path)),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migration_moves_threads_and_transcripts_and_can_be_undone() {
        let root = tempfile::tempdir().expect("temp dir");
        let home = root.path().join("codex");
        std::fs::create_dir_all(home.join("sessions").join("2026").join("11").join("11"))
            .expect("session directory");
        let rollout = home
            .join("sessions")
            .join("2026")
            .join("11")
            .join("11")
            .join("rollout-test.jsonl");
        let head = r#"{"timestamp":"2026-10-27T01:52:12.208Z","type":"session_meta","payload":{"session_id":"session-1","id":"session-1","model_provider":"openai","base_instructions":{"text":"keep"}}}
"#;
        let tail = r#"{"type":"event_msg","payload":{"type":"user_message","message":"hello"}}
"#;
        std::fs::write(&rollout, format!("{head}{tail}")).expect("rollout");

        let database = home.join("state_5.sqlite");
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&database)
                    .create_if_missing(true),
            )
            .await
            .expect("database");
        sqlx::query(
            "CREATE TABLE threads (id TEXT PRIMARY KEY, model_provider TEXT, rollout_path TEXT)",
        )
        .execute(&pool)
        .await
        .expect("threads table");
        sqlx::query("INSERT INTO threads (id, model_provider, rollout_path) VALUES (?, ?, ?)")
            .bind("session-1")
            .bind("openai")
            .bind(rollout.to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("thread row");
        pool.close().await;

        // The Codex home is read from `CODEX_HOME`, like the Codex CLI does.
        let previous = std::env::var("CODEX_HOME").ok();
        std::env::set_var("CODEX_HOME", &home);

        let (migration, report) = migrate("cursor-byok").await.expect("migrate");
        assert_eq!(report.threads_updated, 1);
        assert_eq!(report.rollouts_updated, 1);
        assert_eq!(migration.threads.len(), 2);

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&database)
                    .create_if_missing(false),
            )
            .await
            .expect("database");
        let provider: String =
            sqlx::query_scalar("SELECT model_provider FROM threads WHERE id = ?")
                .bind("session-1")
                .fetch_one(&pool)
                .await
                .expect("provider");
        assert_eq!(provider, "cursor-byok");
        pool.close().await;

        let patched = std::fs::read_to_string(&rollout).expect("rollout");
        assert!(patched.starts_with(
            head.trim_end()
                .replace("\"openai\"", "\"cursor-byok\"")
                .trim_end()
        ));
        assert!(patched.contains("user_message"));
        assert!(patched.ends_with(tail));

        restore(&migration).await.expect("restore");
        let restored = std::fs::read_to_string(&rollout).expect("rollout");
        assert_eq!(restored, format!("{head}{tail}"));

        match previous {
            Some(value) => std::env::set_var("CODEX_HOME", value),
            None => std::env::remove_var("CODEX_HOME"),
        }
    }
    #[test]
    fn replace_json_field_rewrites_the_value_in_place() {
        let document =
            br#"{"type":"session_meta","payload":{"session_id":"abc","model_provider":"openai"}}"#;
        let patched = replace_json_field(document, "model_provider", "cursor-byok").unwrap();
        let value: Value = serde_json::from_slice(&patched).unwrap();
        assert_eq!(value["payload"]["model_provider"], "cursor-byok");
        assert_eq!(value["payload"]["session_id"], "abc");
    }

    #[test]
    fn head_provider_reads_the_session_meta_provider() {
        let head = br#"{"timestamp":"2026-10-27T01:52:12.208Z","type":"session_meta","payload":{"session_id":"01a1","model_provider":"openai"}}"#;
        assert_eq!(head_provider(head).as_deref(), Some("openai"));
        assert_eq!(head_session_id(head).as_deref(), Some("01a1"));
    }

    #[test]
    fn replace_json_field_accepts_spaced_json() {
        let document = br#"{"payload": {"model_provider": "custom"}}"#;
        let patched = replace_json_field(document, "model_provider", "cursor-byok").unwrap();
        let value: Value = serde_json::from_slice(&patched).unwrap();
        assert_eq!(value["payload"]["model_provider"], "cursor-byok");
    }
}
