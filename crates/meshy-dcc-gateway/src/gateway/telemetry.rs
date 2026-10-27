//! Anonymous, aggregate usage telemetry (see `docs/telemetry.md`).
//!
//! The CLI only appends a line to `counters.log`; the long-lived gateway rolls
//! the day up from what it already keeps (persisted tool-call traces, live
//! instances, install receipts) and posts one PostHog `/batch/` request a day.
//! Nothing here sends content: every field is a count, a version, an OS/arch,
//! a host type, or a skill name that is bundled with a connector or installed
//! from the marketplace (anything else is sent as `custom`).
//!
//! Files live next to `settings.json` (`~/.meshy-dcc/telemetry/`):
//! `install_id`, `counters.log` (CLI appends), `state.json` (gateway only),
//! `notice_cli` / `notice_window` (first-run notice markers).

// The rollup and its loop need the admin state; without it only the switches remain.
#![cfg_attr(not(feature = "admin"), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Meshy's PostHog project. A public client ingestion key, meant to be embedded.
pub const PROJECT_KEY: &str = "phc_h7d2h1ThCh7D4aAjWRXHKw3WkjVWLBES1C7ZJrR29cm";
pub const DEFAULT_HOST: &str = "https://us.i.posthog.com";
/// Test-only: send to a local stub instead of PostHog.
pub const HOST_ENV: &str = "MESHY_DCC_TELEMETRY_HOST";
/// Test-only: seconds the gateway waits after start before its first rollup.
pub const DELAY_ENV: &str = "MESHY_DCC_TELEMETRY_DELAY_SECS";
/// `0` turns telemetry off; `1` turns it on in a debug build.
pub const ENV: &str = "MESHY_DCC_TELEMETRY";
pub const SETTING: &str = "telemetry";
pub const DOCS_URL: &str = "https://docs.meshy.ai/en/webapp/plugins/meshy-dcc/telemetry";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const SEND_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_DELAY: Duration = Duration::from_secs(10 * 60);
const TICK: Duration = Duration::from_secs(60 * 60);
/// Days of unsent rollups kept before the oldest is dropped.
const MAX_QUEUED_DAYS: usize = 3;
const MAX_CALL_GROUPS: usize = 200;
/// Stands in for the client IP: PostHog uses the request IP only when an event
/// has no `$ip`, so the stored value is this placeholder.
const PLACEHOLDER_IP: &str = "0.0.0.0";

pub const NOTICE: &str = "Meshy DCC Core sends anonymous, aggregate usage counts once a day \
(versions, OS, connector types, tool-call counts by skill and outcome) to help improve it. \
It never sends tool arguments, file paths, scene or project names, or prompts. \
Turn it off with `meshy-dcc telemetry off` or DO_NOT_TRACK=1. Details:";

// ── consent ──────────────────────────────────────────────────────────────────

const CI_VARS: &[&str] = &[
    "CI",
    "GITHUB_ACTIONS",
    "GITLAB_CI",
    "BUILDKITE",
    "CIRCLECI",
    "TRAVIS",
    "JENKINS_URL",
    "TEAMCITY_VERSION",
    "TF_BUILD",
    "BITBUCKET_BUILD_NUMBER",
    "CODEBUILD_BUILD_ID",
    "DRONE",
];

fn truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "no" | "off"
    )
}

/// Why telemetry is off, given an environment lookup and the saved setting;
/// `None` when it is on.
pub fn disabled_reason_from(
    env: impl Fn(&str) -> Option<String>,
    setting: bool,
    debug_build: bool,
) -> Option<&'static str> {
    if env("DO_NOT_TRACK").is_some_and(|v| truthy(&v)) {
        return Some("DO_NOT_TRACK");
    }
    let explicit = env(ENV);
    if explicit.as_deref().is_some_and(|v| !truthy(v)) {
        return Some(ENV);
    }
    if CI_VARS
        .iter()
        .any(|var| env(var).is_some_and(|v| truthy(&v)))
    {
        return Some("ci");
    }
    if !setting {
        return Some("setting");
    }
    // Development builds (and every `cargo test`) stay quiet unless asked.
    if debug_build && !explicit.as_deref().is_some_and(truthy) {
        return Some("debug_build");
    }
    None
}

pub fn disabled_reason() -> Option<&'static str> {
    disabled_reason_from(
        |key| std::env::var(key).ok(),
        meshy_dcc_updater::settings::get_bool(SETTING, true),
        cfg!(debug_assertions),
    )
}

pub fn enabled() -> bool {
    disabled_reason().is_none()
}

pub fn host() -> String {
    std::env::var(HOST_ENV)
        .ok()
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_HOST.to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Turn telemetry on or off. Off also drops anything not yet sent.
pub fn set_enabled(on: bool) -> anyhow::Result<()> {
    meshy_dcc_updater::settings::set_bool(SETTING, on)?;
    if !on {
        let _ = std::fs::remove_file(dir().join(COUNTERS));
        let _ = std::fs::remove_file(dir().join(SENDING));
        let mut state = State::load();
        state.queue.clear();
        state.save();
    }
    Ok(())
}

// ── files ────────────────────────────────────────────────────────────────────

const INSTALL_ID: &str = "install_id";
const COUNTERS: &str = "counters.log";
const SENDING: &str = "counters.sending";
const STATE: &str = "state.json";

pub fn dir() -> PathBuf {
    meshy_dcc_updater::settings::settings_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir)
        .join("telemetry")
}

/// The random install ID, created on first use. Never tied to an account.
pub fn install_id() -> String {
    let path = dir().join(INSTALL_ID);
    if let Ok(text) = std::fs::read_to_string(&path)
        && let Ok(id) = uuid::Uuid::parse_str(text.trim())
    {
        return id.to_string();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(&path, &id);
    id
}

/// A new install ID next time; unsent rollups (which carry the old one) and
/// pending counts are dropped.
pub fn reset() -> String {
    let _ = std::fs::remove_file(dir().join(INSTALL_ID));
    let _ = std::fs::remove_file(dir().join(COUNTERS));
    let _ = std::fs::remove_file(dir().join(SENDING));
    let mut state = State::load();
    state.queue.clear();
    state.save();
    install_id()
}

/// Count one event locally (`search`, `bug_report`, `window_opened`,
/// `agent:<id>`). A no-op when telemetry is off. Never blocks on the network.
pub fn bump(name: &str) {
    if !enabled() || name.is_empty() || name.contains(['\n', '\t']) {
        return;
    }
    let _ = std::fs::create_dir_all(dir());
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir().join(COUNTERS))
    {
        let _ = writeln!(file, "{name}\t{VERSION}");
    }
}

/// Show-once marker for a surface (`cli`, `window`). True the first time.
pub fn take_notice(surface: &str) -> bool {
    if !enabled() || notice_seen(surface) {
        return false;
    }
    mark_notice(surface);
    true
}

pub fn notice_seen(surface: &str) -> bool {
    dir().join(format!("notice_{surface}")).exists()
}

pub fn mark_notice(surface: &str) {
    let _ = std::fs::create_dir_all(dir());
    let _ = std::fs::write(dir().join(format!("notice_{surface}")), "");
}

/// Counts from `counters.log` plus the newest CLI version that wrote one.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Counters {
    pub counts: BTreeMap<String, u64>,
    pub cli_version: Option<String>,
}

fn parse_counters(text: &str, into: &mut Counters) {
    for line in text.lines() {
        let mut parts = line.splitn(2, '\t');
        let Some(name) = parts.next().filter(|n| !n.is_empty()) else {
            continue;
        };
        *into.counts.entry(name.to_string()).or_default() += 1;
        if let Some(version) = parts.next().filter(|v| !v.is_empty()) {
            into.cli_version = Some(version.to_string());
        }
    }
}

/// Read the counters. `consume` moves them aside first (atomic rename), so
/// later CLI appends start a new file and nothing is counted twice.
fn read_counters(consume: bool) -> Counters {
    let dir = dir();
    let mut counters = Counters::default();
    if consume {
        let _ = std::fs::rename(dir.join(COUNTERS), dir.join(SENDING));
        if let Ok(text) = std::fs::read_to_string(dir.join(SENDING)) {
            parse_counters(&text, &mut counters);
        }
        let _ = std::fs::remove_file(dir.join(SENDING));
    } else {
        for name in [SENDING, COUNTERS] {
            if let Ok(text) = std::fs::read_to_string(dir.join(name)) {
                parse_counters(&text, &mut counters);
            }
        }
    }
    counters
}

/// Gateway-owned state.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub last_day: Option<String>,
    #[serde(default)]
    pub last_rollup_ms: Option<u64>,
    #[serde(default)]
    pub last_version: Option<String>,
    /// Bundled or marketplace skill names seen on any instance.
    #[serde(default)]
    pub known_skills: BTreeSet<String>,
    /// Unsent events, one entry per day.
    #[serde(default)]
    pub queue: Vec<Vec<Value>>,
}

impl State {
    pub fn load() -> Self {
        std::fs::read(dir().join(STATE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(dir());
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let tmp = dir().join("state.json.tmp");
            if std::fs::write(&tmp, bytes).is_ok() {
                let _ = std::fs::rename(tmp, dir().join(STATE));
            }
        }
    }
}

// ── rollup ───────────────────────────────────────────────────────────────────

/// One finished tool call, already reduced to what may leave the machine.
#[derive(Debug, Clone)]
pub struct CallSample {
    pub dcc_type: String,
    /// Raw skill name from the tool slug; mapped to `custom` unless known.
    pub skill: Option<String>,
    pub outcome: &'static str,
    pub total_ms: u64,
}

/// Everything the daily event is built from.
#[derive(Debug, Clone, Default)]
pub struct RollupInput {
    pub calls: Vec<CallSample>,
    /// `(dcc_type, host version)` per live instance.
    pub instances: Vec<(String, Option<String>)>,
    /// `(dcc_type, adapter version)` per installed connector.
    pub connectors: Vec<(String, String)>,
    pub known_skills: BTreeSet<String>,
    pub counters: Counters,
    pub window_hours: u64,
}

/// A plain identifier (host type, agent id, skill name), or `other`.
fn ident(raw: &str) -> String {
    let value = raw.trim().to_ascii_lowercase().replace('_', "-");
    if !value.is_empty()
        && value.len() <= 48
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
    {
        value
    } else {
        "other".to_string()
    }
}

/// A version string such as `0.2.2` or `2022.3.1f1`, or `other`.
fn version_text(raw: &str) -> String {
    let value = raw.trim();
    if value.starts_with(|c: char| c.is_ascii_digit())
        && value.len() <= 32
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
    {
        value.to_string()
    } else {
        "other".to_string()
    }
}

/// Host types from the bundled catalog; anything else is `other`.
static HOST_TYPES: std::sync::LazyLock<BTreeSet<String>> = std::sync::LazyLock::new(|| {
    meshy_dcc_catalog::load_from_str(include_str!("../../../../meshy-dcc-catalog.yml"))
        .unwrap_or_default()
        .into_iter()
        .flat_map(|entry| entry.dcc)
        .map(|dcc| dcc.to_ascii_lowercase())
        .collect()
});

fn host_type(raw: &str) -> String {
    let value = raw.trim().to_ascii_lowercase();
    if HOST_TYPES.contains(&value) {
        value
    } else {
        "other".to_string()
    }
}

/// Host major version: the leading digits (`4.5.2` → `4`, `2022.3` → `2022`).
fn host_major(raw: Option<&str>) -> String {
    let digits: String = raw
        .unwrap_or_default()
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .take(6)
        .collect();
    if digits.is_empty() {
        "unknown".into()
    } else {
        digits
    }
}

pub fn normalize_skill(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace('_', "-")
}

fn latency_bucket(ms: u64) -> &'static str {
    match ms {
        0..100 => "<100ms",
        100..500 => "100-500ms",
        500..1_000 => "0.5-1s",
        1_000..5_000 => "1-5s",
        5_000..30_000 => "5-30s",
        30_000..120_000 => "30s-2m",
        _ => ">2m",
    }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let index = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// The `meshy_dcc_daily_usage` properties. Pure: everything comes from `input`.
pub fn daily_properties(input: &RollupInput) -> Value {
    let mut groups: BTreeMap<(String, String, &'static str), Vec<u64>> = BTreeMap::new();
    for call in &input.calls {
        let skill = call
            .skill
            .as_deref()
            .map(normalize_skill)
            .filter(|s| input.known_skills.contains(s))
            .map(|s| ident(&s))
            .unwrap_or_else(|| "custom".to_string());
        groups
            .entry((host_type(&call.dcc_type), skill, call.outcome))
            .or_default()
            .push(call.total_ms);
    }
    let mut tool_calls: Vec<Value> = groups
        .into_iter()
        .map(|((dcc_type, skill, outcome), mut ms)| {
            ms.sort_unstable();
            json!({
                "dcc_type": dcc_type,
                "skill": skill,
                "outcome": outcome,
                "count": ms.len(),
                "p50": latency_bucket(percentile(&ms, 0.5)),
                "p95": latency_bucket(percentile(&ms, 0.95)),
            })
        })
        .collect();
    tool_calls.sort_by_key(|row| std::cmp::Reverse(row["count"].as_u64().unwrap_or(0)));
    tool_calls.truncate(MAX_CALL_GROUPS);

    let mut instances: BTreeMap<(String, String), u64> = BTreeMap::new();
    for (dcc_type, version) in &input.instances {
        *instances
            .entry((host_type(dcc_type), host_major(version.as_deref())))
            .or_default() += 1;
    }
    let connectors: BTreeSet<(String, String)> = input
        .connectors
        .iter()
        .map(|(dcc, version)| (host_type(dcc), version_text(version)))
        .collect();
    let count = |name: &str| input.counters.counts.get(name).copied().unwrap_or(0);
    let agents: BTreeSet<String> = input
        .counters
        .counts
        .keys()
        .filter_map(|key| key.strip_prefix("agent:"))
        .map(ident)
        .collect();

    json!({
        "schema": 1,
        "window_hours": input.window_hours,
        "cli_version": input.counters.cli_version.as_deref().map(version_text),
        "server_version": VERSION,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "connectors": connectors.into_iter()
            .map(|(dcc_type, version)| json!({"dcc_type": dcc_type, "version": version}))
            .collect::<Vec<_>>(),
        "instances": instances.into_iter()
            .map(|((dcc_type, major), count)| json!({"dcc_type": dcc_type, "host_major": major, "count": count}))
            .collect::<Vec<_>>(),
        "tool_calls_total": input.calls.len(),
        "tool_calls": tool_calls,
        "searches": count("search"),
        "bug_reports_sent": count("bug_report"),
        "agents": agents,
        "window_opened": count("window_opened") > 0,
    })
}

fn iso(ms: u64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms as i64)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn day(ms: u64) -> String {
    iso(ms)[..10].to_string()
}

/// One PostHog event with the privacy properties every event carries.
pub fn event(name: &str, distinct_id: &str, now_ms: u64, mut properties: Value) -> Value {
    if let Some(map) = properties.as_object_mut() {
        map.insert("$process_person_profile".into(), json!(false));
        map.insert("$geoip_disable".into(), json!(true));
        map.insert("$ip".into(), json!(PLACEHOLDER_IP));
        map.insert("$lib".into(), json!("meshy-dcc"));
        map.insert("$lib_version".into(), json!(VERSION));
    }
    json!({
        "event": name,
        "distinct_id": distinct_id,
        "timestamp": iso(now_ms),
        "properties": properties,
    })
}

/// Today's events: lifecycle (`installed` / `updated`) then the daily rollup.
pub fn today_events(
    state: &State,
    input: &RollupInput,
    distinct_id: &str,
    now_ms: u64,
) -> Vec<Value> {
    let platform = json!({"server_version": VERSION, "os": std::env::consts::OS, "arch": std::env::consts::ARCH});
    let mut events = Vec::new();
    match state.last_version.as_deref() {
        None => events.push(event("meshy_dcc_installed", distinct_id, now_ms, platform)),
        Some(previous) if previous != VERSION => {
            let mut props = platform;
            props["from_version"] = json!(version_text(previous));
            events.push(event("meshy_dcc_updated", distinct_id, now_ms, props));
        }
        Some(_) => {}
    }
    events.push(event(
        "meshy_dcc_daily_usage",
        distinct_id,
        now_ms,
        daily_properties(input),
    ));
    events
}

/// The exact `/batch/` body for the queued days.
pub fn batch_body(queue: &[Vec<Value>]) -> Value {
    let batch: Vec<&Value> = queue.iter().flatten().collect();
    json!({ "api_key": PROJECT_KEY, "batch": batch })
}

/// Queue today's events as rolled up at `now`, dropping the oldest days past
/// the cap, and return the body to post. Preview runs this on a copy.
pub fn enqueue(st: &mut State, today: Vec<Value>, now: u64) -> Value {
    st.queue.push(today);
    let overflow = st.queue.len().saturating_sub(MAX_QUEUED_DAYS);
    st.queue.drain(..overflow);
    st.last_day = Some(day(now));
    st.last_rollup_ms = Some(now);
    st.last_version = Some(VERSION.to_string());
    batch_body(&st.queue)
}

/// POST once with a short timeout. Any failure is silent; the caller keeps the
/// queue and tries again the next day.
pub async fn post(host: &str, body: &Value) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .user_agent(format!("meshy-dcc/{VERSION}"))
        .build()
    else {
        return false;
    };
    match client
        .post(format!("{host}/batch/"))
        .json(body)
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Installed connectors from the CLI's install receipts.
fn installed_connectors() -> Vec<(String, String)> {
    let data = std::env::var_os("MESHY_DCC_DATA_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::data_dir().map(|d| d.join("meshy-dcc")));
    let Some(Ok(entries)) = data.map(|d| std::fs::read_dir(d.join("installs"))) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let value: Value = serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok()?;
            Some((
                value["dcc_type"].as_str()?.to_string(),
                value["adapter_version"].as_str()?.to_string(),
            ))
        })
        .collect()
}

/// Map a finished call to `ok`, `timeout`, `unknown` or `error`.
pub fn outcome(ok: bool, output: Option<&str>) -> &'static str {
    if ok {
        return "ok";
    }
    let Some(text) = output else {
        return "unknown";
    };
    let lower = text.to_ascii_lowercase();
    if lower.contains("timeout") || lower.contains("timed out") {
        "timeout"
    } else {
        "error"
    }
}

/// Skill name from a slug such as `blender.1a2b3c4d.blender_interchange__import_file`.
pub fn skill_of(tool_slug: &str) -> Option<String> {
    let backend = tool_slug.rsplit('.').next().unwrap_or(tool_slug);
    backend.split_once("__").map(|(skill, _)| skill.to_string())
}

// ── gateway side ─────────────────────────────────────────────────────────────

#[cfg(feature = "admin")]
pub use gateway_side::*;

#[cfg(feature = "admin")]
mod gateway_side {
    use super::*;
    use crate::gateway::admin::state::AdminState;

    /// Bundled skills from the live capability index, plus marketplace ones.
    pub fn current_known_skills(state: &AdminState) -> BTreeSet<String> {
        let snapshot = state.gateway.capability_index.snapshot();
        let mut known: BTreeSet<String> = snapshot
            .records
            .iter()
            .filter(|r| r.rank_path_source.as_deref() == Some("bundled"))
            .filter_map(|r| r.skill_name.as_deref().map(normalize_skill))
            .collect();
        known.extend(
            crate::gateway::admin::application::skill_library::marketplace_skill_names()
                .iter()
                .map(|n| normalize_skill(n)),
        );
        known
    }

    async fn input(
        state: &AdminState,
        known: BTreeSet<String>,
        since_ms: u64,
        now: u64,
        counters: Counters,
    ) -> RollupInput {
        let cutoff = UNIX_EPOCH + Duration::from_millis(since_ms);
        let calls = state
            .stats
            .as_ref()
            .map(|stats| stats.traces_since(Some(cutoff)))
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.started_at >= cutoff)
            .filter_map(|t| {
                let slug = t.tool_slug.as_deref()?;
                Some(CallSample {
                    dcc_type: t.dcc_type.clone().unwrap_or_else(|| "unknown".into()),
                    skill: skill_of(slug),
                    outcome: outcome(t.ok, t.output.as_ref().map(|o| o.content.as_str())),
                    total_ms: t.total_ms,
                })
            })
            .collect();
        let instances = state
            .gateway
            .live_instances_async()
            .await
            .into_iter()
            .filter(|e| !e.dcc_type.starts_with("__"))
            .map(|e| (e.dcc_type, e.version))
            .collect();
        RollupInput {
            calls,
            instances,
            connectors: installed_connectors(),
            known_skills: known,
            counters,
            window_hours: now.saturating_sub(since_ms).div_ceil(3_600_000),
        }
    }

    fn window_start(st: &State, now: u64) -> u64 {
        st.last_rollup_ms
            .filter(|&ms| ms < now)
            .unwrap_or(now.saturating_sub(24 * 3_600_000))
    }

    /// What the next send would post, without changing anything.
    pub async fn preview(state: &AdminState, now: u64) -> Value {
        let st = State::load();
        let mut known = st.known_skills.clone();
        known.extend(current_known_skills(state));
        let input = input(
            state,
            known,
            window_start(&st, now),
            now,
            read_counters(false),
        )
        .await;
        let today = today_events(&st, &input, &install_id(), now);
        enqueue(&mut st.clone(), today, now)
    }

    /// Roll up and send if today has not been rolled up yet. Returns the body
    /// posted, if any.
    pub async fn tick(state: &AdminState, now: u64) -> Option<Value> {
        if !enabled() {
            return None;
        }
        let mut st = State::load();
        st.known_skills.extend(current_known_skills(state));
        if st.last_day.as_deref() == Some(day(now).as_str()) {
            st.save();
            return None;
        }
        let input = input(
            state,
            st.known_skills.clone(),
            window_start(&st, now),
            now,
            read_counters(true),
        )
        .await;
        let today = today_events(&st, &input, &install_id(), now);
        let body = enqueue(&mut st, today, now);
        st.save();
        if post(&host(), &body).await {
            st.queue.clear();
            st.save();
        }
        Some(body)
    }

    /// The gateway's background loop: first rollup after a delay, then hourly
    /// checks (one send per UTC day). Stops when the gateway yields its port.
    pub fn spawn(state: AdminState, mut yield_rx: tokio::sync::watch::Receiver<bool>) {
        let delay = std::env::var(DELAY_ENV)
            .ok()
            .and_then(|v| v.parse().ok())
            .map(Duration::from_secs)
            .unwrap_or(DEFAULT_DELAY);
        tokio::spawn(async move {
            let mut wait = delay;
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    changed = yield_rx.changed() => {
                        if changed.is_err() || *yield_rx.borrow() { return; }
                        continue;
                    }
                }
                tick(&state, now_ms()).await;
                wait = TICK;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn opt_outs_and_ci_turn_it_off() {
        assert_eq!(disabled_reason_from(env_of(&[]), true, false), None);
        assert_eq!(
            disabled_reason_from(env_of(&[("DO_NOT_TRACK", "1")]), true, false),
            Some("DO_NOT_TRACK")
        );
        assert_eq!(
            disabled_reason_from(env_of(&[("DO_NOT_TRACK", "0")]), true, false),
            None
        );
        assert_eq!(
            disabled_reason_from(env_of(&[(ENV, "0")]), true, false),
            Some(ENV)
        );
        assert_eq!(
            disabled_reason_from(env_of(&[(ENV, "off")]), true, false),
            Some(ENV)
        );
        for var in [
            "CI",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "BUILDKITE",
            "JENKINS_URL",
            "TF_BUILD",
        ] {
            assert_eq!(
                disabled_reason_from(env_of(&[(var, "true")]), true, false),
                Some("ci"),
                "{var}"
            );
        }
        assert_eq!(
            disabled_reason_from(env_of(&[("CI", "false")]), true, false),
            None
        );
        assert_eq!(
            disabled_reason_from(env_of(&[]), false, false),
            Some("setting")
        );
        // `MESHY_DCC_TELEMETRY=1` does not override the saved setting or CI.
        assert_eq!(
            disabled_reason_from(env_of(&[(ENV, "1")]), false, false),
            Some("setting")
        );
        assert_eq!(
            disabled_reason_from(env_of(&[(ENV, "1"), ("CI", "1")]), true, false),
            Some("ci")
        );
        assert_eq!(
            disabled_reason_from(env_of(&[]), true, true),
            Some("debug_build")
        );
        assert_eq!(
            disabled_reason_from(env_of(&[(ENV, "1")]), true, true),
            None
        );
    }

    fn seeded() -> RollupInput {
        let private = [
            "/Users/artist/secret-project/scene.blend",
            "C:\\Users\\artist\\rig.ma",
            "artist-macbook.local",
            "Hero_Rig_Final",
            "9f1c2d3e-0000-4000-8000-123456789abc",
            "req-123456",
            "make the dragon blue",
        ];
        let mut calls = Vec::new();
        for (i, secret) in private.iter().enumerate() {
            calls.push(CallSample {
                dcc_type: "blender".into(),
                skill: Some((*secret).to_string()),
                outcome: if i % 2 == 0 { "ok" } else { "error" },
                total_ms: 50 * i as u64,
            });
            calls.push(CallSample {
                dcc_type: (*secret).to_string(),
                skill: Some("blender_interchange".into()),
                outcome: "ok",
                total_ms: 10,
            });
        }
        calls.push(CallSample {
            dcc_type: "blender".into(),
            skill: Some("blender_interchange".into()),
            outcome: "timeout",
            total_ms: 40_000,
        });
        let mut counters = Counters::default();
        counters.counts.insert("search".into(), 3);
        counters.counts.insert("bug_report".into(), 1);
        counters.counts.insert("window_opened".into(), 2);
        counters.counts.insert("agent:claude-code".into(), 1);
        counters.counts.insert(format!("agent:{}", private[0]), 1);
        counters.cli_version = Some("0.2.2".into());
        RollupInput {
            calls,
            instances: private
                .iter()
                .map(|s| (s.to_string(), Some(format!("4.5 {s}"))))
                .chain([("blender".to_string(), Some("4.5.2".to_string()))])
                .collect(),
            connectors: private
                .iter()
                .map(|s| (s.to_string(), s.to_string()))
                .chain([("blender".to_string(), "0.3.1".to_string())])
                .collect(),
            known_skills: BTreeSet::from(["blender-interchange".to_string()]),
            counters,
            window_hours: 24,
        }
    }

    #[test]
    fn rollup_carries_counts_but_no_names_paths_or_ids() {
        let props = daily_properties(&seeded());
        let text = props.to_string();
        for secret in [
            "artist", "secret", "Users", "scene", "rig", "macbook", "Hero", "9f1c2d3e", "req-",
            "dragon", "/", "\\",
        ] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
        assert_eq!(props["searches"], 3);
        assert_eq!(props["bug_reports_sent"], 1);
        assert_eq!(props["window_opened"], true);
        assert_eq!(props["agents"], json!(["claude-code", "other"]));
        assert_eq!(props["tool_calls_total"], 15);
        let rows = props["tool_calls"].as_array().unwrap();
        assert!(
            rows.iter()
                .all(|r| ["blender-interchange", "custom"].contains(&r["skill"].as_str().unwrap()))
        );
        assert!(
            rows.iter()
                .any(|r| r["outcome"] == "timeout" && r["p95"] == "30s-2m")
        );
        assert!(
            props["instances"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["dcc_type"] == "blender" && r["host_major"] == "4")
        );
        assert!(
            props["connectors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["version"] == "0.3.1")
        );
    }

    #[test]
    fn events_carry_the_privacy_properties() {
        let st = State::default();
        let events = today_events(&st, &seeded(), "id-1", 1_800_000_000_000);
        assert_eq!(events[0]["event"], "meshy_dcc_installed");
        assert_eq!(events[1]["event"], "meshy_dcc_daily_usage");
        for e in &events {
            assert_eq!(e["properties"]["$process_person_profile"], false);
            assert_eq!(e["properties"]["$geoip_disable"], true);
            assert_eq!(e["properties"]["$ip"], PLACEHOLDER_IP);
            assert!(e["event"].as_str().unwrap().starts_with("meshy_dcc_"));
        }
        let updated = State {
            last_version: Some("0.2.1".into()),
            ..State::default()
        };
        let events = today_events(&updated, &seeded(), "id-1", 1_800_000_000_000);
        if VERSION != "0.2.1" {
            assert_eq!(events[0]["event"], "meshy_dcc_updated");
            assert_eq!(events[0]["properties"]["from_version"], "0.2.1");
        }
    }

    #[test]
    fn outcomes_and_skill_names() {
        assert_eq!(outcome(true, None), "ok");
        assert_eq!(outcome(false, None), "unknown");
        assert_eq!(
            outcome(false, Some(r#"{"error":{"message":"backend timed out"}}"#)),
            "timeout"
        );
        assert_eq!(
            outcome(false, Some(r#"{"error":{"kind":"bad-request"}}"#)),
            "error"
        );
        assert_eq!(
            skill_of("blender.1a2b3c4d.blender_interchange__import_file").as_deref(),
            Some("blender_interchange")
        );
        assert_eq!(skill_of("plain_tool"), None);
        assert_eq!(host_major(Some("2022.3.1f1")), "2022");
        assert_eq!(host_major(None), "unknown");
    }

    #[test]
    fn counters_parse_counts_and_cli_version() {
        let mut c = Counters::default();
        parse_counters(
            "search\t0.2.1\nsearch\t0.2.2\nbug_report\t0.2.2\n\n",
            &mut c,
        );
        assert_eq!(c.counts["search"], 2);
        assert_eq!(c.counts["bug_report"], 1);
        assert_eq!(c.cli_version.as_deref(), Some("0.2.2"));
    }

    /// Preview and send share `enqueue`; the stub captures what `post` sends.
    #[tokio::test]
    async fn preview_matches_what_is_sent_and_queue_is_capped() {
        use std::sync::{Arc, Mutex};
        let captured: Arc<Mutex<Vec<Value>>> = Arc::default();
        let sink = captured.clone();
        let app = axum::Router::new().route(
            "/batch/",
            axum::routing::post(move |axum::Json(body): axum::Json<Value>| {
                let sink = sink.clone();
                async move {
                    sink.lock().unwrap().push(body);
                    "ok"
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let now = 1_800_000_000_000;
        let mut st = State {
            last_version: Some(VERSION.into()),
            queue: (0..MAX_QUEUED_DAYS)
                .map(|d| vec![json!({"event": format!("day{d}")})])
                .collect(),
            ..State::default()
        };
        let input = seeded();
        let preview = enqueue(&mut st.clone(), today_events(&st, &input, "id-1", now), now);
        let today = today_events(&st, &input, "id-1", now);
        let body = enqueue(&mut st, today, now);
        assert!(post(&host, &body).await);
        assert_eq!(
            captured.lock().unwrap().as_slice(),
            std::slice::from_ref(&preview)
        );
        // The oldest day fell off; three days remain, today last.
        let batch = preview["batch"].as_array().unwrap();
        assert_eq!(batch.len(), MAX_QUEUED_DAYS);
        assert_eq!(batch[0]["event"], "day1");
        assert_eq!(batch[MAX_QUEUED_DAYS - 1]["event"], "meshy_dcc_daily_usage");
        assert_eq!(preview["api_key"], PROJECT_KEY);
        // A host that does not answer is a silent failure.
        assert!(!post("http://127.0.0.1:9", &body).await);
    }
}
