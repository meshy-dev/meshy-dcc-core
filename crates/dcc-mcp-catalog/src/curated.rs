//! A curated catalog: own entries plus named entries taken from another catalog.
//!
//! meshy-dcc bundles `meshy-dcc-catalog.yml`, whose top-level `upstream` list
//! names entries to take from the upstream dcc-mcp catalog as they are. An own
//! entry that is not installable yet yields to a named upstream entry for the
//! same DCC, so that host stays installable until the own adapter is published.

use serde::Deserialize;

use crate::{CatalogEntry, CatalogError, load_from_str};

#[derive(Deserialize)]
struct Selection {
    #[serde(default)]
    upstream: Vec<String>,
}

/// Names listed under the catalog's top-level `upstream` key.
pub fn upstream_selection(curated: &str) -> Result<Vec<String>, CatalogError> {
    let selection: Selection =
        serde_yaml_ng::from_str(curated).map_err(|e| CatalogError::Parse(e.to_string()))?;
    Ok(selection.upstream)
}

/// Merge `curated` with the entries of `upstream` it names.
///
/// A named entry missing from `upstream` is an error, so a stale selection
/// fails loudly instead of silently dropping a host.
pub fn load_curated(curated: &str, upstream: &str) -> Result<Vec<CatalogEntry>, CatalogError> {
    let own = load_from_str(curated)?;
    let upstream_entries = load_from_str(upstream)?;
    let mut picked = Vec::new();
    for name in upstream_selection(curated)? {
        let entry = upstream_entries
            .iter()
            .find(|entry| entry.name == name)
            .ok_or_else(|| {
                CatalogError::Parse(format!("upstream catalog has no entry '{name}'"))
            })?;
        picked.push(entry.clone());
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
    fn unknown_upstream_name_is_an_error() {
        let curated = "upstream: [up-missing]\nentries: []\n";
        assert!(load_curated(curated, UPSTREAM).is_err());
    }
}
