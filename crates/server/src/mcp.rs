//! `/mcp`: read-only MCP tools over the user's Wellness Lab (streamable HTTP, stateless JSON responses).

use crate::{lock, AppState, VERSION};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    JsonObject, ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::sync::Arc;
use wellness_lab_core::data::Table;
use wellness_lab_core::labels::{LabelKind, ProposalStatus};
use wellness_lab_core::time::{iso, parse_iso};

const NOTE: &str = "Values come from the user's Wellness Lab (their own Garmin wellness data). Dates are YYYY-MM-DD in the user's local calendar; a night belongs to the day the user woke up.";

/// The metrics `get_night` scores against the user's baseline.
pub const NIGHT_Z: [&str; 6] = [
    "hr_min30",
    "ln_rmssd",
    "resp_mean",
    "sleep_stress",
    "tst_h",
    "mid_clock",
];

fn schema(v: Value) -> Arc<JsonObject> {
    Arc::new(v.as_object().cloned().unwrap_or_default())
}

fn tool(name: &'static str, desc: &str, input: Value) -> Tool {
    Tool::new(name, format!("{desc} {NOTE}"), schema(input)).with_annotations(
        ToolAnnotations::from_raw(None, Some(true), Some(false), Some(true), Some(false)),
    )
}

pub fn tools() -> Vec<Tool> {
    let date = json!({"type": "string", "description": "YYYY-MM-DD"});
    vec![
        tool(
            "list_episodes",
            "The user's labelled episodes (illness, alcohol, travel, altitude, ...), optionally of one kind and overlapping a date range.",
            json!({"type": "object", "properties": {"kind": {"type": "string", "description": "label kind, e.g. illness"}, "from": date, "to": date}}),
        ),
        tool(
            "get_night",
            "One night's features, with z-scores against the user's rolling baseline for hr_min30, ln_rmssd, resp_mean, sleep_stress, tst_h and mid_clock.",
            json!({"type": "object", "properties": {"date": date}, "required": ["date"]}),
        ),
        tool(
            "nights",
            "Nightly feature rows between two dates, limited to the named metrics (all when empty).",
            json!({"type": "object", "properties": {"from": date, "to": date, "metrics": {"type": "array", "items": {"type": "string"}}}, "required": ["from", "to"]}),
        ),
        tool(
            "get_proposals",
            "Episodes the lab suspects and may ask the user about, by status (open, confirmed, denied, unsure, snoozed, expired or all).",
            json!({"type": "object", "properties": {"status": {"type": "string", "default": "open"}}}),
        ),
        tool(
            "get_bench",
            "The metric bench: each nightly metric's reliability and smallest worthwhile change for this user.",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "get_hrv_status",
            "The HRV headline for the last night with an HRV value (the HRV deep-dive method).",
            json!({"type": "object", "properties": {}}),
        ),
        tool(
            "event_study",
            "How the named nightly metrics moved around the user's labelled episodes of one kind, against matched control nights.",
            json!({"type": "object", "properties": {"kind": {"type": "string"}, "metrics": {"type": "array", "items": {"type": "string"}}, "window": {"type": "array", "items": {"type": "integer"}, "minItems": 2, "maxItems": 2}}, "required": ["kind", "metrics"]}),
        ),
    ]
}

fn day_arg(v: &Option<String>, name: &str) -> Result<Option<i32>, String> {
    match v {
        None => Ok(None),
        Some(s) => parse_iso(s)
            .map(Some)
            .ok_or_else(|| format!("{name} must be YYYY-MM-DD, got {s:?}")),
    }
}

fn kind_arg(k: &str) -> Result<LabelKind, String> {
    serde_json::from_value(Value::String(k.to_string())).map_err(|_| format!("unknown kind {k:?}"))
}

fn num(x: f64) -> Value {
    if x.is_finite() {
        json!(x)
    } else {
        Value::Null
    }
}

/// Table rows as objects (with `date` beside `day`), only the rows `keep` accepts and the columns in `cols`.
pub fn rows(t: &Table, cols: Option<&[String]>, keep: impl Fn(i32) -> bool) -> Vec<Value> {
    let day = t.col("day");
    let want = |k: &str| k == "day" || cols.map(|c| c.iter().any(|x| x == k)).unwrap_or(true);
    (0..t.n)
        .filter(|&i| day[i].is_finite() && keep(day[i] as i32))
        .map(|i| {
            let mut o = Map::new();
            o.insert("date".into(), json!(iso(day[i] as i32)));
            for (k, v) in t.num.iter().filter(|(k, _)| want(k)) {
                o.insert(k.clone(), num(v[i]));
            }
            for (k, v) in t.txt.iter().filter(|(k, _)| want(k)) {
                o.insert(k.clone(), json!(v[i]));
            }
            Value::Object(o)
        })
        .collect()
}

#[derive(Deserialize, Default)]
struct EpisodesIn {
    kind: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

#[derive(Deserialize)]
struct NightIn {
    date: String,
}

#[derive(Deserialize)]
struct NightsIn {
    from: String,
    to: String,
    #[serde(default)]
    metrics: Vec<String>,
}

#[derive(Deserialize, Default)]
struct ProposalsIn {
    status: Option<String>,
}

#[derive(Deserialize)]
struct EventIn {
    kind: String,
    metrics: Vec<String>,
    #[serde(default)]
    window: Option<[i32; 2]>,
}

fn args<T: DeserializeOwned + Default>(a: &Option<JsonObject>) -> Result<T, String> {
    match a {
        None => Ok(T::default()),
        Some(o) => serde_json::from_value(Value::Object(o.clone())).map_err(|e| e.to_string()),
    }
}

fn req_args<T: DeserializeOwned>(a: &Option<JsonObject>) -> Result<T, String> {
    serde_json::from_value(Value::Object(a.clone().unwrap_or_default())).map_err(|e| e.to_string())
}

/// Run one tool against the state (blocking: it may run engine jobs).
pub fn call(s: &AppState, name: &str, a: &Option<JsonObject>) -> Result<Value, String> {
    match name {
        "list_episodes" => {
            let i: EpisodesIn = args(a)?;
            let kind = i.kind.as_deref().map(kind_arg).transpose()?;
            let (from, to) = (day_arg(&i.from, "from")?, day_arg(&i.to, "to")?);
            let labels = s.store.live_labels().map_err(|e| e.to_string())?;
            Ok(Value::Array(
                labels
                    .into_iter()
                    .filter(|l| kind.map(|k| k == l.kind).unwrap_or(true))
                    .filter(|l| from.map(|f| l.end_day >= f).unwrap_or(true))
                    .filter(|l| to.map(|t| l.start_day <= t).unwrap_or(true))
                    .map(|l| {
                        let mut v = json!(l);
                        v["start_date"] = json!(iso(l.start_day));
                        v["end_date"] = json!(iso(l.end_day));
                        v
                    })
                    .collect(),
            ))
        }
        "get_night" => {
            let i: NightIn = req_args(a)?;
            let day = parse_iso(&i.date).ok_or("date must be YYYY-MM-DD")?;
            let mut lab = lock(&s.lab);
            let row = rows(lab.nights(), None, |d| d == day)
                .pop()
                .ok_or_else(|| format!("no night on {}", i.date))?;
            let mut z = Map::new();
            for m in NIGHT_Z {
                let r = lab.run(&json!({"kind": "baseline", "metric": m}).to_string())?;
                let b: Value = serde_json::from_str(&r.json).map_err(|e| e.to_string())?;
                let days = b["num"]["day"].as_array().cloned().unwrap_or_default();
                let zv = days
                    .iter()
                    .position(|d| d.as_f64() == Some(day as f64))
                    .and_then(|k| b["num"]["z"].get(k).cloned())
                    .unwrap_or(Value::Null);
                z.insert(m.to_string(), zv);
            }
            Ok(json!({"night": row, "z": z}))
        }
        "nights" => {
            let i: NightsIn = req_args(a)?;
            let from = parse_iso(&i.from).ok_or("from must be YYYY-MM-DD")?;
            let to = parse_iso(&i.to).ok_or("to must be YYYY-MM-DD")?;
            let cols = (!i.metrics.is_empty()).then_some(i.metrics.as_slice());
            let mut lab = lock(&s.lab);
            Ok(Value::Array(rows(lab.nights(), cols, |d| {
                d >= from && d <= to
            })))
        }
        "get_proposals" => {
            let i: ProposalsIn = args(a)?;
            let st = i.status.unwrap_or_else(|| "open".into());
            let want: Option<ProposalStatus> = if st == "all" {
                None
            } else {
                Some(
                    serde_json::from_value(Value::String(st.clone()))
                        .map_err(|_| format!("unknown status {st:?}"))?,
                )
            };
            let ps = s.store.proposals().map_err(|e| e.to_string())?;
            Ok(Value::Array(
                ps.into_iter()
                    .filter(|p| want.map(|w| w == p.status).unwrap_or(true))
                    .map(|p| {
                        let mut v = json!(p);
                        v["start_date"] = json!(iso(p.start_day));
                        v["end_date"] = json!(iso(p.end_day));
                        v
                    })
                    .collect(),
            ))
        }
        "get_bench" => {
            let r = lock(&s.lab).run(r#"{"kind":"bench"}"#)?;
            serde_json::from_str(&r.json).map_err(|e| e.to_string())
        }
        "get_hrv_status" => {
            let lab = lock(&s.lab);
            let st = &lab.settings;
            let r =
                wellness_lab_core::hrv4t::run(&lab.data, st.hrv_range_days, st.k_daily, st.k_swc);
            Ok(r.headline)
        }
        "event_study" => {
            let i: EventIn = req_args(a)?;
            let kind = kind_arg(&i.kind)?;
            let mut job = json!({"kind": "event_study", "label_kind": kind, "metrics": i.metrics});
            if let Some(w) = i.window {
                job["window"] = json!(w);
            }
            let r = lock(&s.lab).run(&job.to_string())?;
            serde_json::from_str(&r.json).map_err(|e| e.to_string())
        }
        other => Err(format!("unknown tool {other:?}")),
    }
}

#[derive(Clone)]
pub struct LabMcp {
    s: Arc<AppState>,
}

impl LabMcp {
    pub fn new(s: Arc<AppState>) -> Self {
        LabMcp { s }
    }
}

impl ServerHandler for LabMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("wellness-lab", VERSION))
            .with_instructions(format!(
                "Read-only access to the user's Wellness Lab: labelled episodes, nightly features, proposals, the metric bench and HRV status. {NOTE}"
            ))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        Ok(ListToolsResult::with_all_items(tools()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tools().into_iter().find(|t| t.name == name)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let s = self.s.clone();
        let name = request.name.to_string();
        let a = request.arguments.clone();
        let r = tokio::task::spawn_blocking(move || call(&s, &name, &a))
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(match r {
            Ok(v) => CallToolResult::success(vec![ContentBlock::text(v.to_string())]),
            Err(e) => CallToolResult::error(vec![ContentBlock::text(e)]),
        }
        .into())
    }
}

/// The streamable HTTP service mounted at `/mcp` (behind the same auth as the API).
pub fn service(s: Arc<AppState>) -> StreamableHttpService<LabMcp, LocalSessionManager> {
    let mut cfg = StreamableHttpServerConfig::default().disable_allowed_hosts();
    cfg.legacy_session_mode = false;
    cfg.json_response = true;
    StreamableHttpService::new(
        move || Ok(LabMcp::new(s.clone())),
        Arc::new(LocalSessionManager::default()),
        cfg,
    )
}
