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
        "references/blender.md",
        include_str!("../../../../skills/meshy-dcc/references/blender.md"),
    ),
    (
        "references/godot.md",
        include_str!("../../../../skills/meshy-dcc/references/godot.md"),
    ),
    (
        "references/unity.md",
        include_str!("../../../../skills/meshy-dcc/references/unity.md"),
    ),
    (
        "references/unreal.md",
        include_str!("../../../../skills/meshy-dcc/references/unreal.md"),
    ),
];

#[derive(Debug, Serialize, Deserialize)]
struct Marker {
    digest: String,
    files: Vec<String>,
}

/// One skills directory to install into.
#[derive(Debug, Clone)]
pub struct Target {
    pub agent: String,
    pub skills_dir: PathBuf,
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

/// Agents whose home directory exists: Claude Code and Codex.
pub fn detected_targets() -> Vec<Target> {
    let mut out = Vec::new();
    let Some(home) = dirs::home_dir() else {
        return out;
    };
    if home.join(".claude").is_dir() {
        out.push(Target {
            agent: "claude-code".into(),
            skills_dir: home.join(".claude").join("skills"),
        });
    }
    let codex_home = std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".codex"));
    if codex_home.is_dir() {
        out.push(Target {
            agent: "codex".into(),
            skills_dir: codex_home.join("skills"),
        });
    }
    out
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
        let mut row = json!({
            "agent": target.agent,
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

    fn target(dir: &Path) -> Vec<Target> {
        vec![Target {
            agent: "test".into(),
            skills_dir: dir.to_path_buf(),
        }]
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
