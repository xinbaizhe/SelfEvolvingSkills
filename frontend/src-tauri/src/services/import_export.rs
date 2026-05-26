use crate::{path_to_string, sanitize_file_name, yaml_scalar};
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::{Component, Path, PathBuf},
};

use super::scan::{self, parse_skill_file, upsert_skills};

#[derive(Debug, Deserialize)]
struct ImportSkillPayload {
    agent_id: String,
    filename: Option<String>,
    content: String,
    content_base64: Option<String>,
}

pub(crate) fn import_skills(conn: &Connection, body: Option<Value>) -> Result<Value> {
    scan::sync_source_configs(conn)?;
    let payload: ImportSkillPayload = serde_json::from_value(body.unwrap_or(Value::Null))?;
    let agent_id = payload.agent_id.trim();
    if agent_id.is_empty() {
        return Err(anyhow!("agent_id is required"));
    }
    if payload.content.trim().is_empty() {
        return Err(anyhow!("import content is required"));
    }

    let target = scan::list_sources(conn)?
        .into_iter()
        .find(|source| source.agent_id == agent_id)
        .ok_or_else(|| anyhow!("agent not found: {}", agent_id))?;
    let install_dir = target
        .paths
        .get("skills_path")
        .and_then(Clone::clone)
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("{} has no skills_path configured", target.agent_name))?;

    fs::create_dir_all(&install_dir)?;
    if looks_like_zip(&payload.filename) {
        let encoded = payload
            .content_base64
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("zip import requires base64 binary content"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .context("failed to decode zip base64 content")?;
        let (imported, skipped) =
            import_skills_from_zip(conn, &install_dir, &target.agent_id, &bytes)?;
        return Ok(json!({
            "agent_id": target.agent_id,
            "agent_name": target.agent_name,
            "skills_path": path_to_string(&install_dir),
            "imported": imported,
            "skipped": skipped,
            "count": imported.len()
        }));
    }
    let imported = if looks_like_json(&payload.filename, &payload.content) {
        import_skills_from_json(conn, &install_dir, &target.agent_id, &payload.content)?
    } else {
        let name = skill_name_from_markdown(&payload.content)
            .or_else(|| payload.filename.as_deref().map(file_stem_name))
            .unwrap_or_else(|| "imported-skill".to_string());
        let path = write_skill_markdown(&install_dir, &name, &payload.content)?;
        upsert_imported_skill(conn, &path, &target.agent_id)?;
        vec![json!({ "name": name, "path": path_to_string(&path) })]
    };

    Ok(json!({
        "agent_id": target.agent_id,
        "agent_name": target.agent_name,
        "skills_path": path_to_string(&install_dir),
        "imported": imported,
        "count": imported.len()
    }))
}

fn import_skills_from_json(
    conn: &Connection,
    install_dir: &Path,
    agent_id: &str,
    content: &str,
) -> Result<Vec<Value>> {
    let value: Value =
        serde_json::from_str(content).context("invalid JSON; cannot import Skills")?;
    let items = match value {
        Value::Array(items) => items,
        Value::Object(map) => vec![Value::Object(map)],
        _ => {
            return Err(anyhow!(
                "JSON must be a Skill object or an array of Skill objects"
            ))
        }
    };
    if items.is_empty() {
        return Err(anyhow!("JSON does not contain any importable Skill"));
    }

    let mut imported = Vec::new();
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("JSON Skill is missing required field: name"))?;
        let body = item
            .get("content")
            .or_else(|| item.get("body_text"))
            .or_else(|| item.get("body"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let markdown = if body.trim_start().starts_with("---") {
            body.to_string()
        } else {
            let description = item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            format!(
                "---\nname: {}\ndescription: {}\n---\n\n{}",
                yaml_scalar(name),
                yaml_scalar(description),
                body
            )
        };
        let path = write_skill_markdown(install_dir, name, &markdown)?;
        upsert_imported_skill(conn, &path, agent_id)?;
        imported.push(json!({ "name": name, "path": path_to_string(&path) }));
    }
    Ok(imported)
}

fn import_skills_from_zip(
    conn: &Connection,
    install_dir: &Path,
    agent_id: &str,
    bytes: &[u8],
) -> Result<(Vec<Value>, Vec<Value>)> {
    const MAX_ZIP_BYTES: usize = 100 * 1024 * 1024;
    const MAX_FILES: usize = 2_000;
    const MAX_TOTAL_UNCOMPRESSED: u64 = 300 * 1024 * 1024;
    const MAX_SINGLE_FILE: u64 = 50 * 1024 * 1024;

    if bytes.len() > MAX_ZIP_BYTES {
        return Err(anyhow!(
            "zip file is too large; maximum supported size is 100MB"
        ));
    }

    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).context("invalid or corrupted zip file")?;
    if archive.len() > MAX_FILES {
        return Err(anyhow!(
            "zip contains too many files; maximum supported file count is {}",
            MAX_FILES
        ));
    }

    let mut safe_entries = Vec::new();
    let mut skill_roots: HashSet<PathBuf> = HashSet::new();
    let mut skipped = Vec::new();
    let mut total_uncompressed = 0u64;

    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let raw_name = file.name().to_string();
        if file.is_dir() {
            continue;
        }
        let Some(relative_path) = safe_zip_relative_path(&raw_name) else {
            skipped.push(json!({ "path": raw_name, "reason": "unsafe path skipped" }));
            continue;
        };
        let size = file.size();
        if size > MAX_SINGLE_FILE {
            skipped.push(json!({ "path": raw_name, "reason": "single file exceeds 50MB" }));
            continue;
        }
        total_uncompressed = total_uncompressed.saturating_add(size);
        if total_uncompressed > MAX_TOTAL_UNCOMPRESSED {
            return Err(anyhow!(
                "zip uncompressed content is too large; maximum supported size is 300MB"
            ));
        }
        if relative_path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            if let Some(root) = skill_root_from_skill_md(&relative_path) {
                skill_roots.insert(root);
            }
        }
        safe_entries.push((index, raw_name, relative_path, size));
    }

    if skill_roots.is_empty() {
        return Err(anyhow!("zip does not contain any SKILL.md file"));
    }

    let mut imported_paths: HashMap<PathBuf, PathBuf> = HashMap::new();
    for (index, raw_name, relative_path, _) in safe_entries {
        let Some(root) = matching_skill_root(&relative_path, &skill_roots) else {
            skipped.push(
                json!({ "path": raw_name, "reason": "not under a directory containing SKILL.md" }),
            );
            continue;
        };
        let target_root = install_dir.join(sanitize_file_name(&skill_root_name(root)));
        let inside_skill = if root.as_os_str().is_empty() {
            relative_path.as_path()
        } else {
            relative_path.strip_prefix(root).unwrap_or(&relative_path)
        };
        let target_path = target_root.join(inside_skill);
        ensure_child_path(install_dir, &target_path)?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = archive.by_index(index)?;
        let mut out = fs::File::create(&target_path)?;
        std::io::copy(&mut file, &mut out)?;
        if inside_skill.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
            imported_paths.insert(root.clone(), target_path);
        }
    }

    let mut imported = Vec::new();
    for (root, skill_path) in imported_paths {
        upsert_imported_skill(conn, &skill_path, agent_id)?;
        imported.push(json!({
            "name": skill_root_name(&root),
            "path": path_to_string(&skill_path)
        }));
    }

    if imported.is_empty() {
        return Err(anyhow!("zip was extracted but no Skill was imported"));
    }
    Ok((imported, skipped))
}

fn safe_zip_relative_path(raw_name: &str) -> Option<PathBuf> {
    if raw_name.trim().is_empty() || raw_name.contains('\0') || raw_name.contains(':') {
        return None;
    }
    let normalized = raw_name.replace('\\', "/");
    let path = Path::new(&normalized);
    if path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn skill_root_from_skill_md(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}

fn matching_skill_root<'a>(path: &Path, roots: &'a HashSet<PathBuf>) -> Option<&'a PathBuf> {
    roots
        .iter()
        .filter(|root| root.as_os_str().is_empty() || path.starts_with(root.as_path()))
        .max_by_key(|root| root.components().count())
}

fn skill_root_name(root: &Path) -> String {
    root.file_name()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "imported-skill".to_string())
}

fn ensure_child_path(parent: &Path, child: &Path) -> Result<()> {
    let parent = parent
        .canonicalize()
        .unwrap_or_else(|_| parent.to_path_buf());
    let child_parent = child
        .parent()
        .ok_or_else(|| anyhow!("invalid target path: {}", child.display()))?;
    let child_parent = child_parent
        .canonicalize()
        .unwrap_or_else(|_| child_parent.to_path_buf());
    if !child_parent.starts_with(&parent) {
        return Err(anyhow!("zip path traversal rejected: {}", child.display()));
    }
    Ok(())
}

fn write_skill_markdown(install_dir: &Path, name: &str, content: &str) -> Result<PathBuf> {
    let skill_dir = install_dir.join(sanitize_file_name(name));
    fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    fs::write(&skill_path, content)?;
    Ok(skill_path)
}

fn upsert_imported_skill(conn: &Connection, path: &Path, agent_id: &str) -> Result<()> {
    let skill = parse_skill_file(path, "user", agent_id, None)
        .ok_or_else(|| anyhow!("imported file is not a valid SKILL.md: {}", path.display()))?;
    upsert_skills(conn, &[skill])?;
    Ok(())
}

fn looks_like_zip(filename: &Option<String>) -> bool {
    filename
        .as_deref()
        .and_then(|name| Path::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

fn looks_like_json(filename: &Option<String>, content: &str) -> bool {
    filename
        .as_deref()
        .and_then(|name| Path::new(name).extension())
        .and_then(|value| value.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or_else(|| {
            content.trim_start().starts_with('{') || content.trim_start().starts_with('[')
        })
}

fn skill_name_from_markdown(content: &str) -> Option<String> {
    let (_, parsed, _) = crate::parse_frontmatter(content);
    parsed
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn file_stem_name(filename: &str) -> String {
    Path::new(filename)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "imported-skill".to_string())
}

pub(crate) fn export_skills(conn: &Connection, query: &crate::PageQuery) -> Result<Value> {
    let format = query.format.clone().unwrap_or_else(|| "json".to_string());
    if format == "csv" {
        let mut stmt = conn.prepare(
            "SELECT name, description, category, source_type, plugin_name, origin,
                    usage_count, session_count, file_path, agent_source
             FROM skills WHERE (?1 IS NULL OR agent_source = ?1) ORDER BY name ASC",
        )?;
        let mut csv = String::from(
            "name,description,category,source_type,agent_source,plugin_name,origin,usage_count,session_count,file_path\n",
        );
        let rows = stmt.query_map([query.agent_source.clone()], |row| {
            Ok(vec![
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(9)?.unwrap_or_default(),
                row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                row.get::<_, i64>(6)?.to_string(),
                row.get::<_, i64>(7)?.to_string(),
                row.get::<_, String>(8)?,
            ])
        })?;
        for row in rows {
            csv.push_str(
                &row?
                    .into_iter()
                    .map(csv_escape)
                    .collect::<Vec<_>>()
                    .join(","),
            );
            csv.push('\n');
        }
        return Ok(json!({ "content": csv, "filename": "skills.csv", "content_type": "text/csv" }));
    }

    let mut stmt = conn.prepare(
        "SELECT name, description, category, source_type, agent_source, plugin_name, origin,
                usage_count, session_count, file_path, body_text
         FROM skills WHERE (?1 IS NULL OR agent_source = ?1) ORDER BY name ASC",
    )?;
    let items = stmt
        .query_map([query.agent_source.clone()], |row| {
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "description": row.get::<_, Option<String>>(1)?,
                "category": row.get::<_, Option<String>>(2)?,
                "source_type": row.get::<_, String>(3)?,
                "agent_source": row.get::<_, Option<String>>(4)?,
                "plugin_name": row.get::<_, Option<String>>(5)?,
                "origin": row.get::<_, Option<String>>(6)?,
                "usage_count": row.get::<_, i64>(7)?,
                "session_count": row.get::<_, i64>(8)?,
                "file_path": row.get::<_, String>(9)?,
                "body_text": row.get::<_, Option<String>>(10)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({
        "content": serde_json::to_string_pretty(&items)?,
        "filename": "skills.json",
        "content_type": "application/json"
    }))
}

pub(crate) fn export_agents(conn: &Connection) -> Result<Value> {
    let mut stmt = conn.prepare(
        "SELECT name, description, tools, model, file_path FROM agents ORDER BY name ASC",
    )?;
    let items = stmt
        .query_map([], |row| {
            let tools: Option<String> = row.get(2)?;
            Ok(json!({
                "name": row.get::<_, String>(0)?,
                "description": row.get::<_, Option<String>>(1)?,
                "tools": tools.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
                "model": row.get::<_, Option<String>>(3)?,
                "file_path": row.get::<_, String>(4)?,
            }))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(json!({
        "content": serde_json::to_string_pretty(&items)?,
        "filename": "agents.json",
        "content_type": "application/json"
    }))
}

fn csv_escape(value: String) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value
    }
}
