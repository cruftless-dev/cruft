pub(crate) fn is_agent_process_tool_specifier(tool: &str) -> bool {
    matches!(tool, "process" | "shell" | "exec" | "spawn")
}

use crate::json_string_literal;

#[derive(Clone)]
pub(crate) struct AgentProcessCommand {
    pub(crate) name: String,
    pub(crate) path: String,
}

#[derive(Clone)]
pub(crate) struct AgentProcessCwd {
    pub(crate) root: String,
}

#[derive(Clone)]
pub(crate) struct AgentProcessEnv {
    pub(crate) key: String,
    pub(crate) value: String,
}

pub(crate) fn agent_collect_process_commands(
    specs: &[(String, String)],
) -> Result<Vec<AgentProcessCommand>, String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (name, raw_path) in specs {
        if name.is_empty()
            || name.contains('/')
            || name.contains('\\')
            || name == "."
            || name == ".."
        {
            return Err(format!(
                "--process-command name must be a non-empty simple identifier: {name:?}"
            ));
        }
        if !seen.insert(name.clone()) {
            return Err(format!("duplicate --process-command name {name:?}"));
        }
        let path = std::fs::canonicalize(raw_path)
            .map_err(|e| format!("cannot canonicalize --process-command {raw_path}: {e}"))?;
        let meta = std::fs::metadata(&path)
            .map_err(|e| format!("cannot inspect --process-command {}: {e}", path.display()))?;
        if !meta.is_file() {
            return Err(format!(
                "--process-command {} is not a file",
                path.display()
            ));
        }
        out.push(AgentProcessCommand {
            name: name.clone(),
            path: path.to_string_lossy().into_owned(),
        });
    }
    Ok(out)
}

pub(crate) fn agent_process_commands_js(commands: &[AgentProcessCommand]) -> String {
    commands
        .iter()
        .map(|command| {
            format!(
                "{{name:{},path:{}}}",
                json_string_literal(&command.name),
                json_string_literal(&command.path)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn agent_collect_process_cwds(paths: &[String]) -> Result<Vec<AgentProcessCwd>, String> {
    let mut out = Vec::new();
    for raw_path in paths {
        if raw_path.is_empty() {
            return Err("--process-cwd requires a non-empty path".to_string());
        }
        let root = std::fs::canonicalize(raw_path)
            .map_err(|e| format!("cannot canonicalize --process-cwd {raw_path}: {e}"))?;
        let meta = std::fs::metadata(&root)
            .map_err(|e| format!("cannot inspect --process-cwd {}: {e}", root.display()))?;
        if !meta.is_dir() {
            return Err(format!(
                "--process-cwd {} is not a directory",
                root.display()
            ));
        }
        out.push(AgentProcessCwd {
            root: root.to_string_lossy().into_owned(),
        });
    }
    Ok(out)
}

pub(crate) fn agent_process_cwds_js(cwds: &[AgentProcessCwd]) -> String {
    cwds.iter()
        .map(|cwd| format!("{{root:{}}}", json_string_literal(&cwd.root)))
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn agent_collect_process_env(
    specs: &[(String, String)],
) -> Result<Vec<AgentProcessEnv>, String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (key, value) in specs {
        if key.is_empty()
            || key.contains('=')
            || key.contains('\0')
            || !key.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
        {
            return Err(format!(
                "--process-env key must be a non-empty ASCII env key: {key:?}"
            ));
        }
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate --process-env key {key:?}"));
        }
        out.push(AgentProcessEnv {
            key: key.clone(),
            value: value.clone(),
        });
    }
    Ok(out)
}

pub(crate) fn agent_process_env_js(env: &[AgentProcessEnv]) -> String {
    env.iter()
        .map(|entry| {
            format!(
                "{{key:{},value:{}}}",
                json_string_literal(&entry.key),
                json_string_literal(&entry.value)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}
