//! The Skills page of `meshy-dcc ui`.
//!
//! - `GET  /api/skill-library`        — the skills each open app offers, which
//!   instances loaded them, where each came from, and the user's skill folders.
//! - `POST /api/skill-library/reload` — the open apps re-scan their skill
//!   folders (loaded skills stay loaded; changed ones reload).
//!
//! A host's catalogue does not say where a skill came from (every folder
//! reaches it as one list), so the folders are read here: a listed skill whose
//! name and description match a copy in the user's folders is `mine`, one in
//! the marketplace install folder is `marketplace`, anything else ships with
//! Meshy. A user copy whose name the host lists with another description lost
//! to an earlier copy of that name, which a host never reports.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use uuid::Uuid;

use super::super::state::AdminState;
use super::skill_reload::reload_skill_paths_and_refresh_backends;
use crate::gateway::aggregator::list_skills_by_instance;
use crate::gateway::capability::RefreshReason;

/// A skill folder (`<dir>/SKILL.md`) found on disk.
#[derive(Debug, Clone, PartialEq)]
struct DiskSkill {
    name: String,
    description: String,
    dcc: String,
}

/// A folder of skills the user owns: the per-app default or one they added.
#[derive(Debug)]
struct UserFolder {
    path: PathBuf,
    /// Set for `~/.meshy-dcc/<app>/skills`, which only that app reads.
    dcc_type: Option<String>,
    /// Row id of a folder added on the Skills page.
    custom_id: Option<i64>,
    skills: Vec<DiskSkill>,
}

/// The skills directly inside `dir`; one level, as users lay them out.
fn scan_folder(dir: &Path) -> Vec<DiskSkill> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut skills: Vec<DiskSkill> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("SKILL.md").is_file())
        .filter_map(|path| meshy_dcc_skills::parse_skill_md(&path))
        .map(|meta| DiskSkill {
            name: meta.name,
            description: meta.description,
            dcc: meta.dcc,
        })
        .collect();
    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

fn same_text(a: &str, b: &str) -> bool {
    a.split_whitespace().eq(b.split_whitespace())
}

/// Whether a skill declared for `skill_dcc` is read by `app`.
fn applies_to(skill_dcc: &str, app: &str) -> bool {
    skill_dcc.is_empty() || skill_dcc == "any" || skill_dcc.eq_ignore_ascii_case(app)
}

/// One row per (app, skill name), merged across that app's instances.
#[derive(Debug, Default)]
struct Listed {
    description: String,
    stage: Value,
    tool_count: Value,
    loaded_on: Vec<String>,
}

fn library_payload(
    per_instance: Vec<(Uuid, String, Result<Vec<Value>, String>)>,
    folders: &[UserFolder],
    marketplace: &BTreeMap<String, Vec<DiskSkill>>,
) -> Value {
    let mut instances = Vec::new();
    let mut listed: BTreeMap<(String, String), Listed> = BTreeMap::new();
    for (instance_id, dcc_type, rows) in per_instance {
        let (rows, error) = match rows {
            Ok(rows) => (rows, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let mut loaded = Vec::new();
        for row in &rows {
            let Some(name) = row.get("name").and_then(Value::as_str) else {
                continue;
            };
            let is_loaded = row.get("loaded").and_then(Value::as_bool) == Some(true);
            if is_loaded {
                loaded.push(name.to_string());
            }
            let entry = listed
                .entry((dcc_type.clone(), name.to_string()))
                .or_insert_with(|| Listed {
                    description: row
                        .get("description")
                        .or_else(|| row.get("summary"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    stage: row.get("stage").cloned().unwrap_or(Value::Null),
                    tool_count: row.get("tool_count").cloned().unwrap_or(Value::Null),
                    loaded_on: Vec::new(),
                });
            if is_loaded {
                entry.loaded_on.push(instance_id.to_string());
            }
        }
        instances.push(json!({
            "instance_id": instance_id.to_string(),
            "dcc_type": dcc_type,
            "skill_count": rows.len(),
            "loaded": loaded,
            "error": error,
        }));
    }
    let open: BTreeSet<&str> = listed.keys().map(|(dcc, _)| dcc.as_str()).collect();

    // Where an app finds a name first: its default folder, then the
    // marketplace (app, then host-neutral), then folders added on this page.
    let copies_for = |app: &str| -> Vec<(&'static str, &DiskSkill)> {
        let mut copies = Vec::new();
        for folder in folders
            .iter()
            .filter(|f| f.dcc_type.as_deref() == Some(app))
        {
            copies.extend(folder.skills.iter().map(|skill| ("mine", skill)));
        }
        for key in [app.to_ascii_lowercase().as_str(), "any"] {
            if let Some(skills) = marketplace.get(key) {
                copies.extend(skills.iter().map(|skill| ("marketplace", skill)));
            }
        }
        for folder in folders.iter().filter(|f| f.custom_id.is_some()) {
            copies.extend(
                folder
                    .skills
                    .iter()
                    .filter(|skill| applies_to(&skill.dcc, app))
                    .map(|skill| ("mine", skill)),
            );
        }
        copies
    };

    let skills: Vec<Value> = listed
        .iter()
        .map(|((dcc_type, name), row)| {
            let source = copies_for(dcc_type)
                .into_iter()
                .find(|(_, skill)| {
                    &skill.name == name && same_text(&skill.description, &row.description)
                })
                .map_or("builtin", |(source, _)| source);
            json!({
                "dcc_type": dcc_type,
                "name": name,
                "description": row.description,
                "stage": row.stage,
                "tool_count": row.tool_count,
                "source": source,
                "loaded_on": row.loaded_on,
            })
        })
        .collect();

    let folders: Vec<Value> = folders
        .iter()
        .map(|folder| {
            let skills: Vec<Value> = folder
                .skills
                .iter()
                .map(|skill| {
                    let apps: Vec<&str> = match folder.dcc_type.as_deref() {
                        Some(app) => open.iter().copied().filter(|open| *open == app).collect(),
                        None => open
                            .iter()
                            .copied()
                            .filter(|app| applies_to(&skill.dcc, app))
                            .collect(),
                    };
                    let rows: Vec<&Listed> = apps
                        .iter()
                        .filter_map(|app| listed.get(&(app.to_string(), skill.name.clone())))
                        .collect();
                    let state = if apps.is_empty() {
                        "app_closed"
                    } else if rows
                        .iter()
                        .any(|row| same_text(&row.description, &skill.description))
                    {
                        "active"
                    } else if rows.is_empty() {
                        "pending"
                    } else {
                        "shadowed"
                    };
                    json!({
                        "name": skill.name,
                        "description": skill.description,
                        "dcc": skill.dcc,
                        "state": state,
                    })
                })
                .collect();
            json!({
                "path": tilde(&folder.path),
                "dcc_type": folder.dcc_type,
                "custom_id": folder.custom_id,
                "exists": folder.path.is_dir(),
                "skills": skills,
            })
        })
        .collect();

    json!({ "instances": instances, "skills": skills, "folders": folders })
}

/// `path` with the home folder shown as `~`: readable, and it keeps the user
/// name out of the page (the skill-paths API never sends absolute paths).
fn tilde(path: &Path) -> String {
    match std::env::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf))
    {
        Some(rest) => Path::new("~").join(rest).to_string_lossy().into_owned(),
        None => path.to_string_lossy().into_owned(),
    }
}

/// `~/.meshy-dcc/<app>/skills` for every open app and every app whose folder
/// already exists, then the folders added on the Skills page.
fn user_folders(open: &BTreeSet<String>, custom: Vec<(i64, String)>) -> Vec<UserFolder> {
    let mut apps = open.clone();
    if let Ok(dir) = meshy_dcc_skills::paths::get_local_skills_dir(None)
        && let Some(root) = Path::new(&dir).parent()
        && let Ok(entries) = std::fs::read_dir(root)
    {
        apps.extend(
            entries
                .flatten()
                .filter(|entry| entry.path().join("skills").is_dir())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| name != "marketplace"),
        );
    }
    let mut folders: Vec<UserFolder> = apps
        .into_iter()
        .filter_map(|app| {
            let path =
                PathBuf::from(meshy_dcc_skills::paths::get_local_skills_dir(Some(&app)).ok()?);
            Some(UserFolder {
                skills: scan_folder(&path),
                path,
                dcc_type: Some(app),
                custom_id: None,
            })
        })
        .collect();
    folders.extend(custom.into_iter().map(|(id, path)| {
        let path = PathBuf::from(path);
        UserFolder {
            skills: scan_folder(&path),
            path,
            dcc_type: None,
            custom_id: Some(id),
        }
    }));
    folders
}

/// Marketplace packages by install folder (`<root>/<app>` or `<root>/any`).
fn marketplace_skills() -> BTreeMap<String, Vec<DiskSkill>> {
    let root = meshy_dcc_marketplace::marketplace_root_or_default();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return BTreeMap::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let key = entry.file_name().into_string().ok()?;
            Some((key, scan_folder(&entry.path())))
        })
        .collect()
}

/// Names of the skills installed from the marketplace (usage telemetry sends
/// only these and bundled names).
pub(crate) fn marketplace_skill_names() -> Vec<String> {
    marketplace_skills()
        .into_values()
        .flatten()
        .map(|skill| skill.name)
        .collect()
}

/// `GET /admin/api/skill-library`
pub async fn handle_skill_library(State(s): State<AdminState>) -> Response {
    let per_instance = list_skills_by_instance(&s.gateway).await;
    let open: BTreeSet<String> = per_instance
        .iter()
        .map(|(_, dcc_type, _)| dcc_type.clone())
        .filter(|dcc_type| !dcc_type.starts_with("__"))
        .collect();
    let custom = s
        .admin_sqlite_lane
        .as_ref()
        .map(|lane| lane.reader().list_custom_skill_paths())
        .unwrap_or_default();
    let payload = tokio::task::spawn_blocking(move || {
        let folders = user_folders(&open, custom);
        library_payload(per_instance, &folders, &marketplace_skills())
    })
    .await
    .unwrap_or_else(|err| json!({"error": err.to_string()}));
    Json(payload).into_response()
}

/// `POST /admin/api/skill-library/reload`
pub async fn handle_skill_library_reload(State(s): State<AdminState>) -> Response {
    let reloaded = reload_skill_paths_and_refresh_backends(&s, RefreshReason::ToolsListChanged)
        .await
        .unwrap_or_else(|| json!({"instances": []}));
    let instances = reloaded["instances"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let failed = instances
        .iter()
        .filter(|row| row.get("error").is_some())
        .count();
    Json(json!({
        "ok": failed == 0,
        "reloaded": instances.len() - failed,
        "failed": failed,
        "instances": instances,
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, description: &str, dcc: &str) -> DiskSkill {
        DiskSkill {
            name: name.into(),
            description: description.into(),
            dcc: dcc.into(),
        }
    }

    fn row(name: &str, description: &str, loaded: bool) -> Value {
        json!({"name": name, "description": description, "loaded": loaded, "tool_count": 2, "stage": "scene"})
    }

    #[test]
    fn sources_and_folder_states() {
        let blender = Uuid::from_u128(1);
        let per_instance = vec![(
            blender,
            "blender".to_string(),
            Ok(vec![
                row("blender-scene", "Built-in scene tools", true),
                row("my-import", "Center and scale imports", false),
                row("market-pack", "From the marketplace", false),
            ]),
        )];
        let folders = vec![
            UserFolder {
                path: PathBuf::from("/home/u/.meshy-dcc/blender/skills"),
                dcc_type: Some("blender".into()),
                custom_id: None,
                skills: vec![
                    skill("blender-scene", "My copy of the scene tools", "blender"),
                    skill("my-import", "Center and scale  imports", "blender"),
                    skill("my-new", "Not picked up yet", "blender"),
                ],
            },
            UserFolder {
                path: PathBuf::from("/team/skills"),
                dcc_type: None,
                custom_id: Some(7),
                skills: vec![skill("maya-tool", "For Maya", "maya")],
            },
        ];
        let marketplace = BTreeMap::from([(
            "any".to_string(),
            vec![skill("market-pack", "From the marketplace", "any")],
        )]);

        let payload = library_payload(per_instance, &folders, &marketplace);

        let source = |name: &str| {
            payload["skills"]
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["name"] == name)
                .unwrap()["source"]
                .clone()
        };
        assert_eq!(source("blender-scene"), "builtin");
        assert_eq!(source("my-import"), "mine");
        assert_eq!(source("market-pack"), "marketplace");
        assert_eq!(payload["instances"][0]["loaded"], json!(["blender-scene"]));

        let states: Vec<(String, String)> = payload["folders"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|f| f["skills"].as_array().unwrap().clone())
            .map(|s| {
                (
                    s["name"].as_str().unwrap().into(),
                    s["state"].as_str().unwrap().into(),
                )
            })
            .collect();
        assert_eq!(
            states,
            vec![
                ("blender-scene".into(), "shadowed".into()),
                ("my-import".into(), "active".into()),
                ("my-new".into(), "pending".into()),
                ("maya-tool".into(), "app_closed".into()),
            ]
        );
    }
}
