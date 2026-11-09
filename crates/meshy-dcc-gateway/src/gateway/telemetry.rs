//! Anonymous, aggregate usage telemetry (see `docs/telemetry.md`).
//!
//! The CLI only appends a line to `counters.log`; the long-lived gateway rolls
//! the day up from what it already keeps (persisted tool-call traces, live
//! instances, install receipts) and posts it once a day to each of [`SINKS`].
//! Nothing here sends content: every field is a count, a version, an OS/arch,
//! a host type, or a skill name whose folder is inside an installed connector's
//! package folder or that was installed from the marketplace (anything else is
//! sent as `custom`).
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
pub const POSTHOG_HOST: &str = "https://us.i.posthog.com";
/// Meshy's Statsig client SDK key, the one its web app ships to browsers.
/// Public by design; a server secret never goes here.
pub const STATSIG_CLIENT_KEY: &str = "client-R6Mroa01eoB5HcNS9bbwH6WGALoYn1ZVTS0Jn3WkELw";
pub const STATSIG_HOST: &str = "https://events.statsigapi.net";
/// Test-only: send to this local stub instead of every destination.
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

/// Where the events go. Each one keeps its own queue, so a send that fails at
/// one is never repeated at another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sink {
    Posthog,
    Statsig,
}

/// Every destination of the daily send. Stopping one is deleting its entry;
/// what was queued for it is then dropped.
pub const SINKS: &[Sink] = &[Sink::Posthog, Sink::Statsig];

impl Sink {
    pub fn url(self) -> String {
        let (default, path) = match self {
            Sink::Posthog => (POSTHOG_HOST, "/batch/"),
            Sink::Statsig => (STATSIG_HOST, "/v1/log_event"),
        };
        let host = std::env::var(HOST_ENV)
            .ok()
            .filter(|h| !h.trim().is_empty())
            .unwrap_or_else(|| default.to_string());
        format!("{}{path}", host.trim_end_matches('/'))
    }

    fn headers(self, now_ms: u64) -> Value {
        match self {
            Sink::Posthog => json!({}),
            Sink::Statsig => json!({
                "statsig-api-key": STATSIG_CLIENT_KEY,
                "statsig-client-time": now_ms.to_string(),
            }),
        }
    }

    /// The request body for the queued days.
    pub fn body(self, queue: &[Vec<Value>]) -> Value {
        let events: Vec<&Value> = queue.iter().flatten().collect();
        match self {
            Sink::Posthog => json!({ "api_key": PROJECT_KEY, "batch": events }),
            Sink::Statsig => json!({ "events": events }),
        }
    }

    /// Today's events in this destination's shape.
    pub fn events(
        self,
        today: &[(&'static str, Value)],
        install_id: &str,
        now_ms: u64,
    ) -> Vec<Value> {
        today
            .iter()
            .flat_map(|(name, props)| match self {
                Sink::Posthog => vec![event(name, install_id, now_ms, props.clone())],
                Sink::Statsig => statsig_events(name, props, install_id, now_ms),
            })
            .collect()
    }
}

/// Turn telemetry on or off. Off also drops anything not yet sent.
pub fn set_enabled(on: bool) -> anyhow::Result<()> {
    meshy_dcc_updater::settings::set_bool(SETTING, on)?;
    if !on {
        let _ = std::fs::remove_file(dir().join(COUNTERS));
        let _ = std::fs::remove_file(dir().join(SENDING));
        let mut state = State::load();
        state.queues.clear();
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
    state.queues.clear();
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
    /// Connector-shipped or marketplace skill names seen on any instance.
    #[serde(default)]
    pub known_skills: BTreeSet<String>,
    /// Unsent events per destination, one entry per day.
    #[serde(default)]
    pub queues: BTreeMap<Sink, Vec<Vec<Value>>>,
    /// Up to 0.2.3: PostHog's unsent days. Moved into `queues` on load.
    #[serde(default, skip_serializing)]
    queue: Vec<Vec<Value>>,
}

impl State {
    pub fn load() -> Self {
        Self::parse(&std::fs::read(dir().join(STATE)).unwrap_or_default())
    }

    fn parse(bytes: &[u8]) -> Self {
        let mut state: Self = serde_json::from_slice(bytes).unwrap_or_default();
        let legacy = std::mem::take(&mut state.queue);
        if !legacy.is_empty() {
            state
                .queues
                .entry(Sink::Posthog)
                .or_default()
                .extend(legacy);
        }
        state
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

/// Payload version, sent to Statsig as `meta.schema_version`.
const SCHEMA_VERSION: u64 = 1;

/// One event in the shape of Statsig's `log_event`. Metadata values are
/// strings (lists as JSON text, `null` left out). Each `tool_calls` row becomes
/// its own `meshy_dcc_tool_calls` event with its count as `value`, which keeps
/// every event's metadata small.
fn statsig_events(name: &str, props: &Value, install_id: &str, now_ms: u64) -> Vec<Value> {
    let mut props = props.clone();
    let rows = props
        .as_object_mut()
        .and_then(|map| map.remove("tool_calls"))
        .and_then(|rows| rows.as_array().cloned())
        .unwrap_or_default();
    let mut events = vec![statsig_event(name, None, &props, "", install_id, now_ms)];
    for mut row in rows {
        let count = row.as_object_mut().and_then(|map| map.remove("count"));
        let key = format!("{}/{}/{}", row["dcc_type"], row["skill"], row["outcome"]);
        events.push(statsig_event(
            "meshy_dcc_tool_calls",
            count,
            &row,
            &key,
            install_id,
            now_ms,
        ));
    }
    events
}

/// `meta.event_id` is a UUID v5 of the install ID, day, event and row, so a
/// day sent again keeps its IDs and the warehouse can drop the duplicates.
fn statsig_event(
    name: &str,
    value: Option<Value>,
    props: &Value,
    row: &str,
    install_id: &str,
    now_ms: u64,
) -> Value {
    let seed = format!("meshy-dcc:{install_id}:{}:{name}:{row}", day(now_ms));
    let event_id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, seed.as_bytes());
    let mut metadata: serde_json::Map<String, Value> = props
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| {
            let text = v.as_str().map_or_else(|| v.to_string(), str::to_string);
            (k.clone(), Value::String(text))
        })
        .collect();
    metadata.insert("client_type".into(), json!("meshy_dcc"));
    let meta = json!({"event_id": event_id, "schema_version": SCHEMA_VERSION});
    metadata.insert("meta".into(), json!(meta.to_string()));
    let mut event = json!({
        "eventName": name,
        "time": now_ms,
        "metadata": metadata,
        "user": {
            "customIDs": {"dcc_install_id": install_id},
            "appVersion": VERSION,
            "statsigEnvironment": {"tier": "production"},
        },
    });
    if let Some(value) = value {
        event["value"] = value;
    }
    event
}

/// Today's events before they are shaped for a destination: lifecycle
/// (`installed` / `updated`) then the daily rollup.
pub fn today_events(state: &State, input: &RollupInput) -> Vec<(&'static str, Value)> {
    let platform = json!({"server_version": VERSION, "os": std::env::consts::OS, "arch": std::env::consts::ARCH});
    let mut events = Vec::new();
    match state.last_version.as_deref() {
        None => events.push(("meshy_dcc_installed", platform)),
        Some(previous) if previous != VERSION => {
            let mut props = platform;
            props["from_version"] = json!(version_text(previous));
            events.push(("meshy_dcc_updated", props));
        }
        Some(_) => {}
    }
    events.push(("meshy_dcc_daily_usage", daily_properties(input)));
    events
}

/// Queue today's events at every destination as rolled up at `now`, dropping
/// the oldest days past the cap, and return one request per destination
/// (`sink`, `url`, `headers`, `body`). Preview runs this on a copy.
pub fn enqueue(
    st: &mut State,
    today: &[(&'static str, Value)],
    install_id: &str,
    now: u64,
) -> Vec<Value> {
    st.queues.retain(|sink, _| SINKS.contains(sink));
    let requests = SINKS
        .iter()
        .map(|&sink| {
            let queue = st.queues.entry(sink).or_default();
            queue.push(sink.events(today, install_id, now));
            let overflow = queue.len().saturating_sub(MAX_QUEUED_DAYS);
            queue.drain(..overflow);
            json!({"sink": sink, "url": sink.url(), "headers": sink.headers(now), "body": sink.body(queue)})
        })
        .collect();
    st.last_day = Some(day(now));
    st.last_rollup_ms = Some(now);
    st.last_version = Some(VERSION.to_string());
    requests
}

/// POST one request from [`enqueue`] once, with a short timeout. True when it
/// is done with: accepted, or rejected as a request (4xx other than 408/429),
/// which sending again would not change. Any failure is silent; that
/// destination keeps its queue and tries again the next day.
pub async fn post(request: &Value) -> bool {
    let Ok(client) = reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .user_agent(format!("meshy-dcc/{VERSION}"))
        .build()
    else {
        return false;
    };
    let mut builder = client
        .post(request["url"].as_str().unwrap_or_default())
        .json(&request["body"]);
    for (key, value) in request["headers"].as_object().into_iter().flatten() {
        builder = builder.header(key, value.as_str().unwrap_or_default());
    }
    match builder.send().await {
        Ok(response) => {
            let status = response.status();
            status.is_success()
                || (status.is_client_error() && !matches!(status.as_u16(), 408 | 429))
        }
        Err(_) => false,
    }
}

/// Post the requests from [`enqueue`] side by side, so a destination that does
/// not answer never holds up another. A destination's queue is cleared only
/// when its own request is done with.
pub async fn send(st: &mut State, requests: &[Value]) {
    let done = futures::future::join_all(requests.iter().map(post)).await;
    for (sink, done) in SINKS.iter().zip(done) {
        if done {
            st.queues.remove(sink);
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// The CLI's install receipts, as written (`<data dir>/installs/*.json`).
fn receipts() -> Vec<Value> {
    let data = std::env::var_os("MESHY_DCC_DATA_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| dirs::data_dir().map(|d| d.join("meshy-dcc")));
    let Some(Ok(entries)) = data.map(|d| std::fs::read_dir(d.join("installs"))) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| serde_json::from_slice(&std::fs::read(entry.path()).ok()?).ok())
        .collect()
}

/// Installed connectors from the install receipts.
fn installed_connectors() -> Vec<(String, String)> {
    receipts()
        .iter()
        .filter_map(|value| {
            Some((
                value["dcc_type"].as_str()?.to_string(),
                value["adapter_version"].as_str()?.to_string(),
            ))
        })
        .collect()
}

/// Where each installed connector's package was extracted, from the receipts.
/// Connectors the host installs itself (Blender) record none.
fn package_dirs() -> Vec<PathBuf> {
    package_dirs_of(&receipts())
}

fn package_dirs_of(receipts: &[Value]) -> Vec<PathBuf> {
    receipts
        .iter()
        .filter_map(|value| value["package_dir"].as_str().map(PathBuf::from))
        .collect()
}

/// A path as it is compared: symlinks, `..`, Windows `\\?\` and 8.3 names
/// resolved by the file system. Lowercased on Windows and macOS, whose usual
/// file systems ignore case; Linux keeps it. `None` when the path is gone.
fn comparable(path: &Path) -> Option<PathBuf> {
    let real = std::fs::canonicalize(path).ok()?;
    if cfg!(any(windows, target_os = "macos")) {
        Some(PathBuf::from(real.to_string_lossy().to_lowercase()))
    } else {
        Some(real)
    }
}

/// The names (normalized) of the `(name, skill folder)` pairs whose folder is
/// strictly inside one of `package_dirs`. Compared by path component, so a
/// sibling such as `<package>-old` does not count. A folder or package that
/// cannot be resolved does not count either.
pub fn connector_skill_names(
    skills: &[(String, PathBuf)],
    package_dirs: &[PathBuf],
) -> BTreeSet<String> {
    let packages: Vec<PathBuf> = package_dirs.iter().filter_map(|p| comparable(p)).collect();
    skills
        .iter()
        .filter(|(_, dir)| {
            comparable(dir)
                .is_some_and(|dir| packages.iter().any(|p| dir != *p && dir.starts_with(p)))
        })
        .map(|(name, _)| normalize_skill(name))
        .collect()
}

/// The skill folder in the gateway's reply to a targeted `get_skill_info`
/// (`{"instances":[{"result":"<the skill's detail as JSON text>"}]}`). It must
/// be absolute: a relative path cannot be resolved from here.
fn skill_folder_in(reply: &str) -> Option<PathBuf> {
    let reply: Value = serde_json::from_str(reply).ok()?;
    let detail: Value = serde_json::from_str(reply["instances"][0]["result"].as_str()?).ok()?;
    let path = PathBuf::from(detail["skill_path"].as_str()?);
    path.is_absolute().then_some(path)
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

    /// The folder a live instance loaded a skill from, as its own
    /// `get_skill_info` reports it (the registry rows, the capability index and
    /// `list_skills` do not carry it). `None` when the instance does not answer.
    async fn skill_folder(state: &AdminState, instance: uuid::Uuid, name: &str) -> Option<PathBuf> {
        let args = json!({"skill_name": name, "instance_id": instance.to_string()});
        let (reply, is_error) = crate::gateway::aggregator::skill_mgmt_dispatch(
            &state.gateway,
            "get_skill_info",
            &args,
        )
        .await;
        (!is_error).then(|| skill_folder_in(&reply)).flatten()
    }

    /// Skill names that may be sent: marketplace ones, and those a live
    /// instance loaded from inside an installed connector's package folder.
    /// Names in `known` are not asked about again, so after the first pass
    /// only skills that are not connector-shipped cost a call.
    pub async fn current_known_skills(
        state: &AdminState,
        known: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        let mut found: BTreeSet<String> =
            crate::gateway::admin::application::skill_library::marketplace_skill_names()
                .iter()
                .map(|n| normalize_skill(n))
                .collect();
        let packages = package_dirs();
        if packages.is_empty() {
            return found;
        }
        let listed = crate::gateway::aggregator::list_skills_by_instance(&state.gateway).await;
        let asked: BTreeSet<(uuid::Uuid, String)> = listed
            .into_iter()
            .flat_map(|(instance, _, rows)| {
                rows.unwrap_or_default()
                    .into_iter()
                    .filter_map(move |row| Some((instance, row.get("name")?.as_str()?.to_string())))
            })
            .filter(|(_, name)| !known.contains(&normalize_skill(name)))
            .collect();
        let folders = futures::future::join_all(
            asked
                .iter()
                .map(|(instance, name)| skill_folder(state, *instance, name)),
        )
        .await;
        let skills: Vec<(String, PathBuf)> = asked
            .into_iter()
            .zip(folders)
            .filter_map(|((_, name), folder)| Some((name, folder?)))
            .collect();
        found.extend(connector_skill_names(&skills, &packages));
        found
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
    pub async fn preview(state: &AdminState, now: u64) -> Vec<Value> {
        let st = State::load();
        let mut known = st.known_skills.clone();
        known.extend(current_known_skills(state, &st.known_skills).await);
        let input = input(
            state,
            known,
            window_start(&st, now),
            now,
            read_counters(false),
        )
        .await;
        enqueue(
            &mut st.clone(),
            &today_events(&st, &input),
            &install_id(),
            now,
        )
    }

    /// Roll up and send if today has not been rolled up yet. Returns the
    /// requests posted, if any.
    pub async fn tick(state: &AdminState, now: u64) -> Option<Vec<Value>> {
        if !enabled() {
            return None;
        }
        let mut st = State::load();
        let found = current_known_skills(state, &st.known_skills).await;
        st.known_skills.extend(found);
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
        let today = today_events(&st, &input);
        let requests = enqueue(&mut st, &today, &install_id(), now);
        st.save();
        send(&mut st, &requests).await;
        st.save();
        Some(requests)
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

        // The Statsig payload: same content, nothing more, within its limits.
        let id = "11111111-2222-4333-8444-555555555555";
        let today = today_events(&State::default(), &seeded());
        let events = Sink::Statsig.events(&today, id, 1_800_000_000_000);
        let text = serde_json::to_string(&events).unwrap();
        for secret in [
            "artist", "secret", "Users", "scene", "rig", "macbook", "Hero", "9f1c2d3e", "req-",
            "dragon", "/", "\\\\",
        ] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
        let rows = props["tool_calls"].as_array().unwrap();
        assert_eq!(events.len(), 2 + rows.len());
        for e in &events {
            let metadata = e["metadata"].as_object().unwrap();
            assert!(metadata.values().all(Value::is_string), "{e}");
            assert!(serde_json::to_string(metadata).unwrap().len() <= 4096);
            assert!(e["eventName"].as_str().unwrap().len() <= 64);
            assert_eq!(metadata["client_type"], "meshy_dcc");
            let user = e["user"].as_object().unwrap();
            assert_eq!(
                user.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                BTreeSet::from(["appVersion", "customIDs", "statsigEnvironment"])
            );
            assert_eq!(user["customIDs"], json!({"dcc_install_id": id}));
        }
        assert!(
            !events[1]["metadata"]
                .as_object()
                .unwrap()
                .contains_key("tool_calls")
        );
        let calls: Vec<&Value> = events
            .iter()
            .filter(|e| e["eventName"] == "meshy_dcc_tool_calls")
            .collect();
        assert_eq!(calls.len(), rows.len());
        let counted: u64 = calls.iter().map(|e| e["value"].as_u64().unwrap()).sum();
        assert_eq!(counted, 15);
        assert_eq!(
            calls[0]["metadata"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "client_type",
                "dcc_type",
                "meta",
                "outcome",
                "p50",
                "p95",
                "skill"
            ])
        );
    }

    /// A day sent again keeps its event IDs; every event has its own.
    #[test]
    fn statsig_event_ids_are_stable_and_distinct() {
        let today = today_events(&State::default(), &seeded());
        let ids = |now| -> Vec<String> {
            Sink::Statsig
                .events(&today, "id-1", now)
                .iter()
                .map(|e| {
                    let meta: Value =
                        serde_json::from_str(e["metadata"]["meta"].as_str().unwrap()).unwrap();
                    assert_eq!(meta["schema_version"], 1);
                    meta["event_id"].as_str().unwrap().to_string()
                })
                .collect()
        };
        let now = 1_800_000_000_000;
        let first = ids(now);
        assert_eq!(first, ids(now + 60_000));
        assert_eq!(first.iter().collect::<BTreeSet<_>>().len(), first.len());
        assert!(first.iter().all(|id| !ids(now + 86_400_000).contains(id)));
    }

    /// State written by 0.2.3 and earlier still loads; its queue was PostHog's.
    #[test]
    fn old_state_queue_moves_to_posthog() {
        let st = State::parse(br#"{"last_version":"0.2.3","queue":[[{"event":"x"}]]}"#);
        assert_eq!(st.queues[&Sink::Posthog], vec![vec![json!({"event": "x"})]]);
        let saved = serde_json::to_value(&st).unwrap();
        assert!(saved.get("queue").is_none());
        assert_eq!(saved["queues"]["posthog"][0][0]["event"], "x");
        assert_eq!(State::parse(b"not json").queues.len(), 0);
    }

    /// A skill's name leaves the machine only when its folder is inside an
    /// installed connector's package folder; every other folder, whatever
    /// registered it, is `custom`, and no path ever appears in the event.
    #[test]
    fn skill_names_are_sent_only_for_connector_package_folders() {
        let root = tempfile::tempdir().unwrap();
        let dir = |rel: &str| {
            let path = root.path().join(rel);
            std::fs::create_dir_all(&path).unwrap();
            path
        };
        let package = dir("data/adapters/meshy-dcc-maya/0.1.0/pkg");
        let shipped = dir("data/adapters/meshy-dcc-maya/0.1.0/pkg/python/skills/maya-mesh-ops");
        // Registered with `explicit_arg`, as a connector's own skills are.
        let user = dir("home/artist/.meshy-dcc/maya/skills/hero-rig-secret");
        let project = dir("work/dragon-movie/skills/dragon-tools");
        // Shares the package folder's name as a string prefix, not as a folder.
        let sibling = dir("data/adapters/meshy-dcc-maya/0.1.0/pkg-old/skills/lookalike-skill");
        // The package folder itself is not inside itself.
        let itself = package.clone();
        let missing = package.join("python/skills/gone-skill");

        let mut skills = vec![
            ("maya_mesh_ops".to_string(), shipped.clone()),
            ("hero_rig_secret".to_string(), user),
            ("dragon_tools".to_string(), project),
            ("lookalike_skill".to_string(), sibling),
            ("package_itself".to_string(), itself),
            ("gone_skill".to_string(), missing),
            (
                "unresolved_skill".to_string(),
                PathBuf::from("pkg/python/skills/x"),
            ),
        ];
        #[cfg(unix)]
        {
            // A link inside the package that leads out of it does not count.
            let outside = dir("home/artist/linked-secret");
            std::os::unix::fs::symlink(&outside, package.join("python/skills/linked-secret"))
                .unwrap();
            skills.push((
                "linked_secret".to_string(),
                package.join("python/skills/linked-secret"),
            ));
        }
        // The same folder spelled with another case resolves only where the
        // file system ignores case (Windows, macOS); there it must match.
        let upper = root
            .path()
            .join("DATA/adapters/meshy-dcc-maya/0.1.0/pkg/python/skills/maya-mesh-ops");
        skills.push(("upper_case_spelling".to_string(), upper.clone()));

        let packages = vec![
            package.clone(),
            root.path().join("data/adapters/uninstalled"),
        ];
        let known = connector_skill_names(&skills, &packages);
        let mut expected = BTreeSet::from(["maya-mesh-ops".to_string()]);
        if upper.exists() {
            expected.insert("upper-case-spelling".to_string());
        }
        assert_eq!(known, expected);

        // The receipts name the package folders; a host-installed connector
        // (Blender's extension) records none.
        let receipts = [
            json!({"dcc_type": "maya", "adapter_version": "0.1.0", "package_dir": package}),
            json!({"dcc_type": "blender", "adapter_version": "0.1.0"}),
            json!({"dcc_type": "x", "adapter_version": "0.1.0", "package_dir": 7}),
        ];
        assert_eq!(package_dirs_of(&receipts), vec![package.clone()]);

        // The event: the connector skill by name, the rest `custom`, no path.
        let mut input = RollupInput {
            known_skills: known,
            window_hours: 24,
            ..RollupInput::default()
        };
        for (name, _) in &skills {
            input.calls.push(CallSample {
                dcc_type: "maya".into(),
                skill: Some(name.clone()),
                outcome: "ok",
                total_ms: 20,
            });
        }
        let props = daily_properties(&input);
        let rows = props["tool_calls"].as_array().unwrap();
        let named: BTreeSet<&str> = rows.iter().filter_map(|r| r["skill"].as_str()).collect();
        let mut want: BTreeSet<&str> = expected.iter().map(String::as_str).collect();
        want.insert("custom");
        assert_eq!(named, want);
        let text = props.to_string();
        for secret in [
            "hero",
            "secret",
            "dragon",
            "lookalike",
            "artist",
            "gone",
            "itself",
            "unresolved",
            "linked",
            "adapters",
            "pkg",
            "python",
            "/",
            "\\",
        ] {
            assert!(!text.contains(secret), "{secret} leaked: {text}");
        }
        assert!(!text.contains(root.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn the_skill_folder_comes_from_the_get_skill_info_reply() {
        let absolute = if cfg!(windows) {
            r"C:\pkg\skills\a"
        } else {
            "/pkg/skills/a"
        };
        let reply = |path: &str| {
            let detail = json!({"name": "a", "skill_path": path}).to_string();
            json!({"instances": [{"instance_id": "x", "dcc_type": "maya", "result": detail}]})
                .to_string()
        };
        assert_eq!(
            skill_folder_in(&reply(absolute)),
            Some(PathBuf::from(absolute))
        );
        // Relative or empty: it cannot be resolved from the gateway.
        assert_eq!(skill_folder_in(&reply("pkg/skills/a")), None);
        assert_eq!(skill_folder_in(&reply("")), None);
        // The instance does not know the skill, answers with no instance, or is not JSON.
        assert_eq!(
            skill_folder_in(r#"{"instances":[{"result":"Skill 'a' not found"}]}"#),
            None
        );
        assert_eq!(
            skill_folder_in(r#"{"skills":[],"total":0,"instances":[]}"#),
            None
        );
        assert_eq!(skill_folder_in("backend unreachable"), None);
    }

    #[test]
    fn events_carry_the_privacy_properties() {
        let st = State::default();
        let now = 1_800_000_000_000;
        let events = Sink::Posthog.events(&today_events(&st, &seeded()), "id-1", now);
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
        let today = today_events(&updated, &seeded());
        let events = Sink::Posthog.events(&today, "id-1", now);
        let statsig = Sink::Statsig.events(&today, "id-1", now);
        if VERSION != "0.2.1" {
            assert_eq!(events[0]["event"], "meshy_dcc_updated");
            assert_eq!(events[0]["properties"]["from_version"], "0.2.1");
            assert_eq!(statsig[0]["eventName"], "meshy_dcc_updated");
            assert_eq!(statsig[0]["metadata"]["from_version"], "0.2.1");
        }
        for e in &statsig {
            assert_eq!(e["time"], now);
            assert_eq!(e["user"]["appVersion"], VERSION);
            assert_eq!(e["user"]["statsigEnvironment"]["tier"], "production");
            assert!(e["eventName"].as_str().unwrap().starts_with("meshy_dcc_"));
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
    /// Each destination keeps its own queue: one failing never resends to the other.
    #[tokio::test]
    async fn preview_matches_what_is_sent_and_each_sink_is_tracked_alone() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};
        type Seen = Arc<Mutex<Vec<(String, Option<String>, Value)>>>;
        let captured: Seen = Arc::default();
        let statsig_down = Arc::new(AtomicBool::new(true));
        let handler = |path: &'static str, captured: Seen, down: Option<Arc<AtomicBool>>| {
            axum::routing::post(
                move |headers: axum::http::HeaderMap, axum::Json(body): axum::Json<Value>| async move {
                    let key = headers
                        .get("statsig-api-key")
                        .map(|v| v.to_str().unwrap().to_string());
                    captured.lock().unwrap().push((path.into(), key, body));
                    if down.is_some_and(|d| d.load(Ordering::SeqCst)) {
                        axum::http::StatusCode::SERVICE_UNAVAILABLE
                    } else {
                        axum::http::StatusCode::ACCEPTED
                    }
                },
            )
        };
        let app = axum::Router::new()
            .route("/batch/", handler("/batch/", captured.clone(), None))
            .route(
                "/v1/log_event",
                handler(
                    "/v1/log_event",
                    captured.clone(),
                    Some(statsig_down.clone()),
                ),
            )
            .route(
                "/status/{code}",
                axum::routing::post(
                    |axum::extract::Path(code): axum::extract::Path<u16>| async move {
                        axum::http::StatusCode::from_u16(code).unwrap()
                    },
                ),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let host = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let to_stub = |mut requests: Vec<Value>| {
            for r in &mut requests {
                let url = r["url"].as_str().unwrap().to_string();
                assert!(url.starts_with("https://"), "{url}");
                let path = &url[url[8..].find('/').unwrap() + 8..];
                r["url"] = json!(format!("{host}{path}"));
            }
            requests
        };

        let now = 1_800_000_000_000;
        let mut st = State {
            last_version: Some(VERSION.into()),
            queues: BTreeMap::from([(
                Sink::Posthog,
                (0..MAX_QUEUED_DAYS)
                    .map(|d| vec![json!({"event": format!("day{d}")})])
                    .collect(),
            )]),
            ..State::default()
        };
        let input = seeded();
        let today = today_events(&st, &input);
        let preview = enqueue(&mut st.clone(), &today, "id-1", now);
        let requests = enqueue(&mut st, &today, "id-1", now);
        assert_eq!(preview, requests);
        assert_eq!(preview[0]["url"], "https://us.i.posthog.com/batch/");
        assert_eq!(
            preview[1]["url"],
            "https://events.statsigapi.net/v1/log_event"
        );
        assert_eq!(preview[1]["headers"]["statsig-api-key"], STATSIG_CLIENT_KEY);
        assert_eq!(
            preview[1]["headers"]["statsig-client-time"],
            now.to_string()
        );
        // PostHog: the oldest day fell off; three days remain, today last.
        let batch = preview[0]["body"]["batch"].as_array().unwrap();
        assert_eq!(batch.len(), MAX_QUEUED_DAYS);
        assert_eq!(batch[0]["event"], "day1");
        assert_eq!(batch[MAX_QUEUED_DAYS - 1]["event"], "meshy_dcc_daily_usage");
        assert_eq!(preview[0]["body"]["api_key"], PROJECT_KEY);
        let events = preview[1]["body"]["events"].as_array().unwrap();
        assert_eq!(events[0]["eventName"], "meshy_dcc_daily_usage");

        // PostHog accepts, Statsig is down.
        send(&mut st, &to_stub(requests)).await;
        {
            let mut seen = captured.lock().unwrap();
            seen.sort_by(|a, b| a.0.cmp(&b.0));
            assert_eq!(seen.len(), 2);
            assert_eq!(
                seen[0],
                ("/batch/".into(), None, preview[0]["body"].clone())
            );
            assert_eq!(
                seen[1],
                (
                    "/v1/log_event".into(),
                    Some(STATSIG_CLIENT_KEY.into()),
                    preview[1]["body"].clone()
                )
            );
        }
        assert!(!st.queues.contains_key(&Sink::Posthog));
        assert_eq!(st.queues[&Sink::Statsig].len(), 1);

        // The next day PostHog gets only that day; Statsig both, unchanged.
        let next = now + 86_400_000;
        let today = today_events(&st, &input);
        let requests = enqueue(&mut st, &today, "id-1", next);
        let batch = requests[0]["body"]["batch"].as_array().unwrap();
        assert!(batch.iter().all(|e| e["timestamp"] == iso(next)));
        let both = requests[1]["body"]["events"].as_array().unwrap();
        assert_eq!(both.len(), 2 * events.len());
        assert_eq!(&both[..events.len()], events.as_slice());
        statsig_down.store(false, Ordering::SeqCst);
        send(&mut st, &to_stub(requests.clone())).await;
        assert!(st.queues.is_empty());

        // A host that does not answer is a silent failure; a rejected request
        // is done with, except for 408 and 429.
        let at = |path: &str| {
            let mut r = requests[0].clone();
            r["url"] = json!(format!("{host}{path}"));
            r
        };
        let mut dead = requests[0].clone();
        dead["url"] = json!("http://127.0.0.1:9/batch/");
        assert!(!post(&dead).await);
        assert!(post(&at("/status/400")).await);
        assert!(!post(&at("/status/429")).await);
        assert!(!post(&at("/status/503")).await);
    }
}
