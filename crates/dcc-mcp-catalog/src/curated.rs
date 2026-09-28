//! A curated catalog: own entries plus named entries taken from another catalog.
//!
//! meshy-dcc bundles `meshy-dcc-catalog.yml`, whose top-level `upstream` list
//! names entries to take from the upstream dcc-mcp catalog. An item is either a
//! bare name (taken as it is) or a mapping that also re-pins the entry to a
//! newer release: version, artifact url and sha256, minimum core version,
//! install runbook, policy. Everything else still comes from upstream. An own
//! entry that is not installable yet yields to a named upstream entry for the
//! same DCC, so that host stays installable until the own adapter is published.

use serde::Deserialize;

use crate::{CatalogEntry, CatalogError, CatalogPolicy, load_from_str};

#[derive(Deserialize)]
struct Selection {
    #[serde(default)]
    upstream: Vec<Pick>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Pick {
    Name(String),
    Pinned(Pin),
}

/// Fields a curated catalog may re-pin on an upstream entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    name: String,
    version: Option<String>,
    url: Option<String>,
    sha256: Option<String>,
    min_core_version: Option<String>,
    instructions_url: Option<String>,
    policy: Option<CatalogPolicy>,
}

impl Pick {
    fn name(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Pinned(pin) => &pin.name,
        }
    }

    fn apply(&self, mut entry: CatalogEntry) -> CatalogEntry {
        let Self::Pinned(pin) = self else {
            return entry;
        };
        let set = |field: &mut Option<String>, value: &Option<String>| {
            if value.is_some() {
                field.clone_from(value);
            }
        };
        set(&mut entry.version, &pin.version);
        set(&mut entry.min_core_version, &pin.min_core_version);
        if pin.policy.is_some() {
            entry.policy.clone_from(&pin.policy);
        }
        if let Some(install) = entry.install.as_mut() {
            set(&mut install.url, &pin.url);
            set(&mut install.sha256, &pin.sha256);
            set(&mut install.instructions_url, &pin.instructions_url);
        }
        entry
    }
}

fn selection(curated: &str) -> Result<Vec<Pick>, CatalogError> {
    let selection: Selection =
        serde_yaml_ng::from_str(curated).map_err(|e| CatalogError::Parse(e.to_string()))?;
    Ok(selection.upstream)
}

/// Names listed under the catalog's top-level `upstream` key.
pub fn upstream_selection(curated: &str) -> Result<Vec<String>, CatalogError> {
    Ok(selection(curated)?
        .iter()
        .map(|pick| pick.name().to_string())
        .collect())
}

/// Merge `curated` with the entries of `upstream` it names.
///
/// A named entry missing from `upstream` is an error, so a stale selection
/// fails loudly instead of silently dropping a host.
pub fn load_curated(curated: &str, upstream: &str) -> Result<Vec<CatalogEntry>, CatalogError> {
    let own = load_from_str(curated)?;
    let upstream_entries = load_from_str(upstream)?;
    let mut picked = Vec::new();
    for pick in selection(curated)? {
        let entry = upstream_entries
            .iter()
            .find(|entry| entry.name == pick.name())
            .ok_or_else(|| {
                CatalogError::Parse(format!("upstream catalog has no entry '{}'", pick.name()))
            })?;
        picked.push(pick.apply(entry.clone()));
    }

    let mut entries: Vec<CatalogEntry> = own
        .into_iter()
        .filter(|entry| !(unavailable(entry) && picked.iter().any(|up| same_dcc(entry, up))))
        .collect();
    let extra: Vec<CatalogEntry> = picked
        .into_iter()
        .filter(|up| !entries.iter().any(|entry| same_dcc(entry, up)))
        .collect();
    entries.extend(extra);
    Ok(entries)
}

fn unavailable(entry: &CatalogEntry) -> bool {
    entry
        .policy
        .as_ref()
        .is_some_and(|policy| policy.installation == "not_available")
}

fn same_dcc(left: &CatalogEntry, right: &CatalogEntry) -> bool {
    let key = |dcc: &String| {
        dcc.chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase()
    };
    left.dcc
        .iter()
        .any(|a| right.dcc.iter().any(|b| key(a) == key(b)))
}

#[cfg(test)]
mod tests {
    use super::load_curated;

    const UPSTREAM: &str = r#"
entries:
  - { name: up-maya, description: m, dcc: [maya] }
  - { name: up-houdini, description: h, dcc: [houdini] }
  - { name: up-3dsmax, description: x, dcc: ["3ds max", 3dsmax] }
  - { name: up-krita, description: k, dcc: [krita] }
"#;

    fn names(curated: &str) -> Vec<String> {
        load_curated(curated, UPSTREAM)
            .unwrap()
            .into_iter()
            .map(|entry| entry.name)
            .collect()
    }

    #[test]
    fn unpublished_own_entry_yields_to_named_upstream_entry() {
        let curated = r#"
upstream: [up-maya, up-houdini, up-3dsmax]
entries:
  - { name: own-maya, description: m, dcc: [maya], policy: { installation: not_available } }
  - { name: own-3dsmax, description: x, dcc: [3dsmax] }
  - { name: own-roblox, description: r, dcc: [roblox], policy: { installation: not_available } }
"#;
        // Published own entries win; unlisted upstream entries never appear.
        assert_eq!(
            names(curated),
            ["own-3dsmax", "own-roblox", "up-maya", "up-houdini"]
        );
    }

    #[test]
    fn pinned_pick_overrides_only_the_listed_fields() {
        let upstream = r#"
entries:
  - name: up-houdini
    description: h
    dcc: [houdini]
    version: "0.31.5"
    install: { type: pip, pip_package: up-houdini, url: https://old/up_houdini-0.31.5-py3-none-any.whl, sha256: old, entry_point: "up:main" }
"#;
        let curated = r#"
upstream:
  - name: up-houdini
    version: "0.43.1"
    url: https://new/up_houdini-0.43.1-py3-none-any.whl
    sha256: new
entries: []
"#;
        let entry = load_curated(curated, upstream).unwrap().remove(0);
        assert_eq!(entry.version.as_deref(), Some("0.43.1"));
        let install = entry.install.unwrap();
        assert_eq!(
            install.url.as_deref(),
            Some("https://new/up_houdini-0.43.1-py3-none-any.whl")
        );
        assert_eq!(install.sha256.as_deref(), Some("new"));
        assert_eq!(install.entry_point.as_deref(), Some("up:main"));
    }

    #[test]
    fn unknown_upstream_name_is_an_error() {
        let curated = "upstream: [up-missing]\nentries: []\n";
        assert!(load_curated(curated, UPSTREAM).is_err());
    }
}
