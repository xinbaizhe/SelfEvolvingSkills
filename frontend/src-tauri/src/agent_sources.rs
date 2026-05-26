use std::collections::HashMap;
use std::path::PathBuf;

use crate::{path_to_string, AgentSource};

pub(crate) fn agent_sources() -> Vec<AgentSource> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let config_dir = dirs::config_dir().unwrap_or_default();
    vec![
        AgentSource {
            id: "hermes",
            name: "Hermes",
            skills_path: Some(home.join(".hermes").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".hermes").join("runs")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "openclaw",
            name: "OpenClaw",
            skills_path: Some(home.join(".openclaw").join("skills")),
            agents_path: Some(home.join(".openclaw").join("agents")),
            sessions_path: Some(home.join(".openclaw").join("sessions")),
            projects_path: Some(home.join(".openclaw").join("projects")),
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "claude-code",
            name: "Claude Code",
            skills_path: Some(home.join(".claude").join("skills")),
            agents_path: Some(home.join(".claude").join("agents")),
            sessions_path: Some(home.join(".claude").join("sessions")),
            projects_path: Some(home.join(".claude").join("projects")),
            plugins_path: Some(home.join(".claude").join("plugins").join("cache")),
            memory_path: Some(home.join(".claude").join("projects")),
            default_enabled: true,
        },
        AgentSource {
            id: "codex",
            name: "Codex",
            skills_path: Some(home.join(".codex").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".codex").join("sessions")),
            projects_path: Some(home.join(".codex").join("projects")),
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "vscode",
            name: "VSCode",
            skills_path: None,
            agents_path: None,
            sessions_path: Some(
                config_dir
                    .join("Code")
                    .join("User")
                    .join("globalStorage")
                    .join("saoudrizwan.claude-dev"),
            ),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "cursor",
            name: "Cursor",
            skills_path: None,
            agents_path: None,
            sessions_path: Some(config_dir.join("Cursor").join("User")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: true,
        },
        AgentSource {
            id: "codebuddy",
            name: "CodeBuddy",
            skills_path: Some(home.join(".codebuddy").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".codebuddy").join("sessions")),
            projects_path: Some(home.join(".codebuddy").join("projects")),
            plugins_path: None,
            memory_path: None,
            default_enabled: false,
        },
        AgentSource {
            id: "trae",
            name: "TRAE",
            skills_path: Some(home.join(".trae").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".trae").join("sessions")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: false,
        },
        AgentSource {
            id: "zeelinclaw",
            name: "ZeeLinClaw",
            skills_path: Some(home.join(".zeelinclaw").join("skills")),
            agents_path: None,
            sessions_path: Some(home.join(".zeelinclaw").join("sessions")),
            projects_path: None,
            plugins_path: None,
            memory_path: None,
            default_enabled: false,
        },
    ]
}

pub(crate) fn source_default_paths(src: &AgentSource) -> HashMap<String, Option<String>> {
    let mut paths = HashMap::from([
        (
            "skills_path".into(),
            src.skills_path.as_ref().map(path_to_string),
        ),
        (
            "agents_path".into(),
            src.agents_path.as_ref().map(path_to_string),
        ),
        (
            "sessions_path".into(),
            src.sessions_path.as_ref().map(path_to_string),
        ),
        (
            "projects_path".into(),
            src.projects_path.as_ref().map(path_to_string),
        ),
        (
            "plugins_path".into(),
            src.plugins_path.as_ref().map(path_to_string),
        ),
        (
            "memory_path".into(),
            src.memory_path.as_ref().map(path_to_string),
        ),
    ]);
    if src.id == "claude-code" {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let agent_skills_path = home.join(".claude").join(".agents").join("skills");
        paths.insert(
            "extra_skills_path".into(),
            Some(path_to_string(&agent_skills_path)),
        );
        paths.insert(
            "extra_agents_path".into(),
            Some(path_to_string(&agent_skills_path)),
        );
    }
    paths
}
