//! The `meshy-dcc` Skill, embedded in the CLI and written into agents' skill
//! directories.
//!
//! Only this entry Skill goes to the agent; host and workflow skills stay
//! behind the gateway. Every write lands in `<skills dir>/meshy-dcc/` and is
//! recorded in a marker file there, so updates and removal touch only the
//! files this command wrote, and a directory without the marker is left alone.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const SKILL_NAME: &str = "meshy-dcc";
const MARKER: &str = ".meshy-dcc-skill.json";

/// Embedded files, relative to the Skill directory.
const FILES: &[(&str, &str)] = &[
    (
        "SKILL.md",
        include_str!("../../../../skills/meshy-dcc/SKILL.md"),
    ),
    (
        "references/3dsmax.md",
        include_str!("../../../../skills/meshy-dcc/references/3dsmax.md"),
    ),
    (
        "references/authoring.md",
        include_str!("../../../../skills/meshy-dcc/references/authoring.md"),
    ),
    (
        "references/blender.md",
        include_str!("../../../../skills/meshy-dcc/references/blender.md"),
    ),
    (
        "references/c4d.md",
        include_str!("../../../../skills/meshy-dcc/references/c4d.md"),
    ),
    (
        "references/connectors.md",
        include_str!("../../../../skills/meshy-dcc/references/connectors.md"),
    ),
    (
        "references/freecad.md",
        include_str!("../../../../skills/meshy-dcc/references/freecad.md"),
    ),
    (
        "references/godot.md",
        include_str!("../../../../skills/meshy-dcc/references/godot.md"),
    ),
    (
        "references/houdini.md",
        include_str!("../../../../skills/meshy-dcc/references/houdini.md"),
    ),
    (
        "references/material-maker.md",
        include_str!("../../../../skills/meshy-dcc/references/material-maker.md"),
    ),
    (
        "references/maya.md",
        include_str!("../../../../skills/meshy-dcc/references/maya.md"),
    ),
    (
        "references/openscad.md",
        include_str!("../../../../skills/meshy-dcc/references/openscad.md"),
    ),
    (
        "references/roblox.md",
        include_str!("../../../../skills/meshy-dcc/references/roblox.md"),
    ),
    (
        "references/substance3d-painter.md",
        include_str!("../../../../skills/meshy-dcc/references/substance3d-painter.md"),
    ),
    (
        "references/unity.md",
        include_str!("../../../../skills/meshy-dcc/references/unity.md"),
    ),
    (
        "references/unreal.md",
        include_str!("../../../../skills/meshy-dcc/references/unreal.md"),
    ),
    (
        "references/zbrush.md",
        include_str!("../../../../skills/meshy-dcc/references/zbrush.md"),
    ),
];

#[derive(Debug, Serialize, Deserialize)]
struct Marker {
    digest: String,
    files: Vec<String>,
}

/// One skills directory to install into, and the agents that read it.
#[derive(Debug, Clone)]
pub struct Target {
    /// Agent ids from [`AGENTS`], or `custom` for a `--dir`.
    pub agents: Vec<String>,
    pub skills_dir: PathBuf,
}

/// Where a path starts.
#[derive(Clone, Copy)]
enum Root {
    Home,
    /// `$XDG_CONFIG_HOME`, else `~/.config` (on every platform, like skills.sh).
    Config,
    /// An environment variable, else this path under the home directory;
    /// an empty fallback means "only when the variable is set".
    Env(&'static str, &'static str),
    Abs,
}

use Root::{Abs, Config, Env, Home};

/// An agent: id, display name, global skills directory, and the paths whose
/// existence means it is installed on this computer.
type AgentDef = (
    &'static str,
    &'static str,
    (Root, &'static str),
    &'static [(Root, &'static str)],
);

/// The skills.sh agent table (`vercel-labs/skills` `src/agents.ts` at
/// `13e4063`), global installs only. Project-only agents (Eve, PromptScript)
/// and detection that looks at the current directory are left out.
// ponytail: a copied table; re-sync from skills.sh when agents change.
#[rustfmt::skip]
const AGENTS: &[AgentDef] = &[
    ("claude-code", "Claude Code", (Env("CLAUDE_CONFIG_DIR", ".claude"), "skills"), &[(Env("CLAUDE_CONFIG_DIR", ".claude"), "")]),
    ("codex", "Codex", (Home, ".agents/skills"), &[(Env("CODEX_HOME", ".codex"), "")]),
    ("cursor", "Cursor", (Home, ".cursor/skills"), &[(Home, ".cursor")]),
    ("gemini-cli", "Gemini CLI", (Home, ".gemini/skills"), &[(Home, ".gemini")]),
    ("github-copilot", "GitHub Copilot", (Home, ".copilot/skills"), &[(Home, ".copilot")]),
    ("opencode", "OpenCode", (Config, "opencode/skills"), &[(Config, "opencode")]),
    ("windsurf", "Windsurf", (Home, ".codeium/windsurf/skills"), &[(Home, ".codeium/windsurf")]),
    ("kiro-cli", "Kiro CLI", (Home, ".kiro/skills"), &[(Home, ".kiro")]),
    ("trae", "Trae", (Home, ".trae/skills"), &[(Home, ".trae")]),
    ("trae-cn", "Trae CN", (Home, ".trae-cn/skills"), &[(Home, ".trae-cn")]),
    ("qwen-code", "Qwen Code", (Home, ".qwen/skills"), &[(Home, ".qwen")]),
    ("codebuddy", "CodeBuddy", (Home, ".codebuddy/skills"), &[(Home, ".codebuddy")]),
    ("openclaw", "OpenClaw", (Home, ".openclaw/skills"), &[(Home, ".openclaw")]),
    ("hermes-agent", "Hermes Agent", (Env("HERMES_HOME", ".hermes"), "skills"), &[(Env("HERMES_HOME", ".hermes"), "")]),
    ("cline", "Cline", (Home, ".agents/skills"), &[(Home, ".cline")]),
    ("goose", "Goose", (Config, "goose/skills"), &[(Config, "goose")]),
    ("antigravity", "Antigravity", (Home, ".gemini/antigravity/skills"), &[(Home, ".gemini/antigravity")]),
    ("antigravity-cli", "Antigravity CLI", (Home, ".gemini/antigravity-cli/skills"), &[(Home, ".gemini/antigravity-cli")]),
    ("amp", "Amp", (Config, "agents/skills"), &[(Config, "amp")]),
    ("aider-desk", "AiderDesk", (Home, ".aider-desk/skills"), &[(Home, ".aider-desk")]),
    ("astrbot", "AstrBot", (Home, ".astrbot/data/skills"), &[(Home, ".astrbot")]),
    ("autohand-code", "Autohand Code CLI", (Env("AUTOHAND_HOME", ".autohand"), "skills"), &[(Env("AUTOHAND_HOME", ".autohand"), "")]),
    ("augment", "Augment", (Home, ".augment/skills"), &[(Home, ".augment")]),
    ("bob", "IBM Bob", (Home, ".bob/skills"), &[(Home, ".bob")]),
    ("codearts-agent", "CodeArts Agent", (Home, ".codeartsdoer/skills"), &[(Home, ".codeartsdoer")]),
    ("codemaker", "Codemaker", (Home, ".codemaker/skills"), &[(Home, ".codemaker")]),
    ("codestudio", "Code Studio", (Home, ".codestudio/skills"), &[(Home, ".codestudio")]),
    ("command-code", "Command Code", (Home, ".commandcode/skills"), &[(Home, ".commandcode")]),
    ("continue", "Continue", (Home, ".continue/skills"), &[(Home, ".continue")]),
    ("cortex", "Cortex Code", (Home, ".snowflake/cortex/skills"), &[(Home, ".snowflake/cortex")]),
    ("crush", "Crush", (Home, ".config/crush/skills"), &[(Home, ".config/crush")]),
    ("deepagents", "Deep Agents", (Home, ".deepagents/agent/skills"), &[(Home, ".deepagents")]),
    ("devin", "Devin for Terminal", (Config, "devin/skills"), &[(Config, "devin")]),
    ("dexto", "Dexto", (Home, ".agents/skills"), &[(Home, ".dexto")]),
    ("droid", "Droid", (Home, ".factory/skills"), &[(Home, ".factory")]),
    ("firebender", "Firebender", (Home, ".firebender/skills"), &[(Home, ".firebender")]),
    ("forgecode", "ForgeCode", (Home, ".forge/skills"), &[(Home, ".forge")]),
    ("fx", "fx", (Home, ".agents/skills"), &[(Home, ".fx")]),
    ("grok", "Grok Build", (Env("GROK_HOME", ".grok"), "skills"), &[(Env("GROK_HOME", ".grok"), "")]),
    ("inference-sh", "inference.sh", (Home, ".inferencesh/skills"), &[(Home, ".inferencesh")]),
    ("jazz", "Jazz", (Home, ".jazz/skills"), &[(Home, ".jazz")]),
    ("junie", "Junie", (Home, ".junie/skills"), &[(Home, ".junie")]),
    ("iflow-cli", "iFlow CLI", (Home, ".iflow/skills"), &[(Home, ".iflow")]),
    ("kilo", "Kilo Code", (Home, ".kilo/skills"), &[(Home, ".kilo"), (Home, ".kilocode")]),
    ("kimchi", "Kimchi", (Home, ".config/kimchi/harness/skills"), &[(Home, ".config/kimchi")]),
    ("kimi-code-cli", "Kimi Code CLI", (Home, ".agents/skills"), &[(Home, ".kimi-code"), (Home, ".kimi")]),
    ("kode", "Kode", (Home, ".kode/skills"), &[(Home, ".kode")]),
    ("lingma", "Lingma", (Home, ".lingma/skills"), &[(Home, ".lingma")]),
    ("loaf", "Loaf", (Home, ".agents/skills"), &[(Home, ".loaf")]),
    ("mcpjam", "MCPJam", (Home, ".mcpjam/skills"), &[(Home, ".mcpjam")]),
    ("minimax-code", "MiniMax Code", (Home, ".minimax/skills"), &[(Home, ".minimax"), (Abs, "/Applications/MiniMax Code.app")]),
    ("mistral-vibe", "Mistral Vibe", (Env("VIBE_HOME", ".vibe"), "skills"), &[(Env("VIBE_HOME", ".vibe"), "")]),
    ("moxby", "Moxby", (Home, ".moxby/skills"), &[(Home, ".moxby")]),
    ("mux", "Mux", (Home, ".mux/skills"), &[(Home, ".mux")]),
    ("openhands", "OpenHands", (Home, ".openhands/skills"), &[(Home, ".openhands")]),
    ("ona", "Ona", (Home, ".ona/skills"), &[(Home, ".ona")]),
    ("pi", "Pi", (Home, ".agents/skills"), &[(Home, ".pi/agent")]),
    ("posit-assistant", "Posit Assistant", (Home, ".posit/assistant/skills"), &[(Home, ".posit/assistant"), (Home, ".positai")]),
    ("qoder", "Qoder", (Home, ".qoder/skills"), &[(Home, ".qoder")]),
    ("qoder-cn", "Qoder CN", (Home, ".qoder-cn/skills"), &[(Home, ".qoder-cn")]),
    ("reasonix", "Reasonix", (Home, ".reasonix/skills"), &[(Home, ".reasonix")]),
    ("rovodev", "Rovo Dev", (Home, ".rovodev/skills"), &[(Home, ".rovodev")]),
    ("roo", "Roo Code", (Home, ".roo/skills"), &[(Home, ".roo")]),
    ("sarvam-code", "Sarvam Code", (Home, ".agents/skills"), &[(Env("SARVAM_HOME", ".sarvam"), "")]),
    ("tabnine-cli", "Tabnine CLI", (Home, ".tabnine/agent/skills"), &[(Home, ".tabnine")]),
    ("terramind", "Terramind", (Home, ".terramind/skills"), &[(Home, ".terramind")]),
    ("tinycloud", "Tinycloud", (Home, ".tinycloud/skills"), &[(Home, ".tinycloud")]),
    ("warp", "Warp", (Home, ".agents/skills"), &[(Home, ".warp")]),
    ("zed", "Zed", (Home, ".agents/skills"), &[(Config, "zed"), (Env("APPDATA", ""), "Zed")]),
    ("zcode", "ZCode", (Home, ".zcode/skills"), &[(Home, ".zcode"), (Abs, "/Applications/ZCode.app")]),
    ("zencoder", "Zencoder", (Home, ".zencoder/skills"), &[(Home, ".zencoder")]),
    ("zenflow", "Zenflow", (Home, ".zencoder/skills"), &[(Home, ".zencoder")]),
    ("neovate", "Neovate", (Home, ".neovate/skills"), &[(Home, ".neovate")]),
    ("pochi", "Pochi", (Home, ".pochi/skills"), &[(Home, ".pochi")]),
    ("adal", "AdaL", (Home, ".adal/skills"), &[(Home, ".adal")]),
];

fn resolve(home: &Path, (root, rel): (Root, &str)) -> Option<PathBuf> {
    let env = |var: &str| {
        std::env::var_os(var)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let base = match root {
        Home => home.to_path_buf(),
        Config => env("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
        Env(var, fallback) => match env(var) {
            Some(path) => path,
            None if fallback.is_empty() => return None,
            None => home.join(fallback),
        },
        Abs => PathBuf::new(),
    };
    Some(if rel.is_empty() { base } else { base.join(rel) })
}

fn agent_detected(home: &Path, agent: &AgentDef) -> bool {
    agent
        .3
        .iter()
        .filter_map(|loc| resolve(home, *loc))
        .any(|path| path.exists())
}

fn agent_def(id: &str) -> Option<&'static AgentDef> {
    AGENTS.iter().find(|agent| agent.0 == id)
}

/// Group agents by skills directory, keeping table order.
fn targets_for(home: &Path, agents: &[&'static AgentDef]) -> Vec<Target> {
    let mut out: Vec<Target> = Vec::new();
    for agent in agents {
        let Some(dir) = resolve(home, agent.2) else {
            continue;
        };
        match out.iter_mut().find(|target| target.skills_dir == dir) {
            Some(target) => target.agents.push(agent.0.to_string()),
            None => out.push(Target {
                agents: vec![agent.0.to_string()],
                skills_dir: dir,
            }),
        }
    }
    out
}

/// Targets for the agents named by `--agent` (`*` for every agent in the table).
pub fn named_targets(ids: &[String]) -> anyhow::Result<Vec<Target>> {
    let home = dirs::home_dir().context("no home directory")?;
    let agents = if ids.iter().any(|id| id == "*") {
        AGENTS.iter().collect()
    } else {
        ids.iter()
            .map(|id| {
                agent_def(id).with_context(|| {
                    let known: Vec<&str> = AGENTS.iter().map(|agent| agent.0).collect();
                    format!("unknown agent `{id}`; known agents: {}", known.join(", "))
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?
    };
    Ok(targets_for(&home, &agents))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Missing,
    Current,
    Outdated,
    /// A `meshy-dcc` directory (or link) this command did not write.
    Foreign,
}

fn digest() -> String {
    let mut hasher = Sha256::new();
    for (path, contents) in FILES {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(contents.as_bytes());
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Agents installed on this computer, by the skills.sh detection rules.
pub fn detected_targets() -> Vec<Target> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let detected: Vec<_> = AGENTS
        .iter()
        .filter(|agent| agent_detected(&home, agent))
        .collect();
    targets_for(&home, &detected)
}

fn read_marker(dir: &Path) -> Option<Marker> {
    serde_json::from_slice(&fs::read(dir.join(MARKER)).ok()?).ok()
}

fn state(dir: &Path) -> State {
    match fs::symlink_metadata(dir) {
        Err(_) => State::Missing,
        Ok(meta) if !meta.is_dir() => State::Foreign,
        Ok(_) => match read_marker(dir) {
            None => State::Foreign,
            Some(marker) if marker.digest == digest() => State::Current,
            Some(_) => State::Outdated,
        },
    }
}

fn write_atomic(path: &Path, contents: &str) -> anyhow::Result<()> {
    let parent = path.parent().context("file has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
    if fs::symlink_metadata(parent)?.file_type().is_symlink() {
        bail!("{} is a symlink; not writing through it", parent.display());
    }
    let tmp = parent.join(format!(
        ".{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    fs::write(&tmp, contents).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
}

fn install(dir: &Path) -> anyhow::Result<()> {
    let previous = read_marker(dir)
        .map(|marker| marker.files)
        .unwrap_or_default();
    for (path, contents) in FILES {
        write_atomic(&dir.join(path), contents)?;
    }
    // Files an older CLI wrote that this one no longer ships.
    for stale in previous
        .iter()
        .filter(|old| !FILES.iter().any(|(path, _)| path == old))
    {
        let _ = fs::remove_file(dir.join(stale));
    }
    let marker = Marker {
        digest: digest(),
        files: FILES.iter().map(|(path, _)| (*path).to_string()).collect(),
    };
    write_atomic(&dir.join(MARKER), &serde_json::to_string_pretty(&marker)?)
}

fn remove(dir: &Path) -> anyhow::Result<()> {
    let marker = read_marker(dir).context("no install marker")?;
    for file in &marker.files {
        let _ = fs::remove_file(dir.join(file));
    }
    fs::remove_file(dir.join(MARKER))?;
    // Only directories left empty go; anything the user added stays.
    let _ = fs::remove_dir(dir.join("references"));
    let _ = fs::remove_dir(dir);
    Ok(())
}

/// Plan, and with `execute` apply, installing (or removing) the Skill.
pub fn run(targets: &[Target], remove_skill: bool, execute: bool) -> anyhow::Result<Value> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut rows = Vec::new();
    let mut failed = false;
    for target in targets {
        let dir = target.skills_dir.join(SKILL_NAME);
        let state = state(&dir);
        let action = match (remove_skill, state) {
            (false, State::Missing) => "install",
            (false, State::Outdated) => "update",
            (true, State::Current | State::Outdated) => "remove",
            (_, State::Foreign) => "skip",
            _ => "none",
        };
        let agents: Vec<Value> = target
            .agents
            .iter()
            .map(|id| match agent_def(id) {
                Some(agent) => json!({
                    "id": id,
                    "name": agent.1,
                    "detected": agent_detected(&home, agent),
                }),
                None => json!({ "id": id }),
            })
            .collect();
        let mut row = json!({
            "agent": target.agents.first(),
            "agents": agents,
            "path": dir,
            "state": state,
            "action": action,
        });
        if state == State::Foreign {
            row["reason"] =
                json!("this directory was not written by meshy-dcc; it is left as it is");
        }
        if execute && matches!(action, "install" | "update" | "remove") {
            let result = if remove_skill {
                remove(&dir)
            } else {
                install(&dir)
            };
            match result {
                Ok(()) => row["result"] = json!("done"),
                Err(error) => {
                    failed = true;
                    row["result"] = json!("failed");
                    row["error"] = json!(format!("{error:#}"));
                }
            }
        }
        rows.push(row);
    }
    Ok(json!({
        "skill": SKILL_NAME,
        "digest": digest(),
        "executed": execute,
        "failed": failed,
        "targets": rows,
    }))
}

/// One line per target of a [`run`] result, for people (`--output human`).
pub fn human_summary(value: &Value) -> String {
    let home = dirs::home_dir().map(|home| home.display().to_string());
    let executed = value["executed"] == true;
    let mut out = String::new();
    for row in value["targets"].as_array().into_iter().flatten() {
        let names: Vec<&str> = row["agents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|agent| agent["name"].as_str().or(agent["id"].as_str()))
            .collect();
        let mut path = row["path"].as_str().unwrap_or_default().to_string();
        if let Some(home) = &home
            && let Some(rest) = path.strip_prefix(home.as_str())
        {
            path = format!("~{rest}");
        }
        let what = match (row["action"].as_str(), row["result"].as_str(), executed) {
            (_, Some("failed"), _) => format!(
                "failed: {}",
                row["error"].as_str().unwrap_or("unknown error")
            ),
            (Some("install"), _, true) => "installed".into(),
            (Some("update"), _, true) => "updated".into(),
            (Some("remove"), _, true) => "removed".into(),
            (Some(action @ ("install" | "update" | "remove")), _, false) => {
                format!("will {action} (add --execute)")
            }
            (Some("skip"), _, _) => "skipped, the folder was not written by meshy-dcc".into(),
            _ => "already up to date".into(),
        };
        out.push_str(&format!("  {}: {what} ({path})\n", names.join(", ")));
    }
    if out.is_empty() {
        out.push_str("  No agents found on this computer. Name one with --agent <id>, or a folder with --dir.\n");
    }
    out
}

/// Bring Skills this CLI installed earlier up to its own version. Release
/// builds only, so a development build never rewrites an installed Skill.
pub fn refresh_installed() {
    if cfg!(debug_assertions) {
        return;
    }
    for target in detected_targets() {
        let dir = target.skills_dir.join(SKILL_NAME);
        if state(&dir) == State::Outdated
            && let Err(error) = install(&dir)
        {
            eprintln!("warning: could not update {}: {error:#}", dir.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file added to skills/meshy-dcc but not to FILES would never reach agents.
    #[test]
    fn every_skill_file_is_embedded() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills/meshy-dcc");
        let mut on_disk = Vec::new();
        for dir in [root.clone(), root.join("references")] {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_file() {
                    on_disk.push(
                        path.strip_prefix(&root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        on_disk.sort();
        let mut embedded: Vec<String> = FILES.iter().map(|(name, _)| name.to_string()).collect();
        embedded.sort();
        assert_eq!(on_disk, embedded);
    }

    fn target(dir: &Path) -> Vec<Target> {
        vec![Target {
            agents: vec!["test".into()],
            skills_dir: dir.to_path_buf(),
        }]
    }

    #[test]
    fn agents_sharing_a_directory_become_one_target() {
        let home = Path::new("/home/u");
        let picked: Vec<_> = ["codex", "claude-code", "cline", "cursor"]
            .iter()
            .map(|id| agent_def(id).unwrap())
            .collect();
        let targets = targets_for(home, &picked);
        let dirs: Vec<_> = targets.iter().map(|t| t.skills_dir.clone()).collect();
        assert_eq!(dirs.len(), 3);
        assert_eq!(dirs[0], home.join(".agents/skills"));
        assert_eq!(targets[0].agents, ["codex", "cline"]);
        assert!(named_targets(&["no-such-agent".into()]).is_err());
        let ids: std::collections::HashSet<_> = AGENTS.iter().map(|a| a.0).collect();
        assert_eq!(ids.len(), AGENTS.len(), "agent ids are unique");
    }

    #[test]
    fn detection_follows_the_listed_paths() {
        let home = tempfile::tempdir().unwrap();
        let cursor = agent_def("cursor").unwrap();
        assert!(!agent_detected(home.path(), cursor));
        fs::create_dir(home.path().join(".cursor")).unwrap();
        assert!(agent_detected(home.path(), cursor));
    }

    #[test]
    fn embeds_every_file_of_the_skill_directory() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills/meshy-dcc");
        let mut on_disk: Vec<String> = ["", "references"]
            .iter()
            .flat_map(|sub| fs::read_dir(root.join(sub)).unwrap())
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file())
            .map(|path| {
                path.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        on_disk.sort();
        let mut embedded: Vec<String> = FILES.iter().map(|(path, _)| (*path).to_string()).collect();
        embedded.sort();
        assert_eq!(on_disk, embedded);
    }

    #[test]
    fn plans_installs_updates_and_removes_only_its_own_files() {
        let temp = tempfile::tempdir().unwrap();
        let targets = target(temp.path());
        let dir = temp.path().join(SKILL_NAME);

        let plan = run(&targets, false, false).unwrap();
        assert_eq!(plan["targets"][0]["action"], "install");
        assert!(!dir.exists(), "a plan writes nothing");

        run(&targets, false, true).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("SKILL.md")).unwrap(),
            FILES[0].1
        );
        assert_eq!(
            run(&targets, false, false).unwrap()["targets"][0]["state"],
            "current"
        );

        // An older install: different digest and a file this CLI no longer ships.
        fs::write(dir.join("references/old.md"), "old").unwrap();
        let stale = Marker {
            digest: "old".into(),
            files: vec!["SKILL.md".into(), "references/old.md".into()],
        };
        fs::write(dir.join(MARKER), serde_json::to_string(&stale).unwrap()).unwrap();
        assert_eq!(
            run(&targets, false, true).unwrap()["targets"][0]["action"],
            "update"
        );
        assert!(!dir.join("references/old.md").exists());

        fs::write(dir.join("notes.md"), "the user's own file").unwrap();
        run(&targets, true, true).unwrap();
        assert!(!dir.join("SKILL.md").exists());
        assert!(dir.join("notes.md").exists(), "files it did not write stay");
    }

    #[test]
    fn leaves_a_directory_it_did_not_write() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join(SKILL_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("SKILL.md"), "someone else's").unwrap();

        let result = run(&target(temp.path()), false, true).unwrap();
        assert_eq!(result["targets"][0]["state"], "foreign");
        assert_eq!(result["targets"][0]["action"], "skip");
        assert_eq!(
            fs::read_to_string(dir.join("SKILL.md")).unwrap(),
            "someone else's"
        );
    }
}
