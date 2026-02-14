use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, SubmenuBuilder};
use tauri::{Emitter, Manager};

// Cookie names that Cursor uses for session auth
const SESSION_COOKIE_NAMES: &[&str] = &[
    "WorkosCursorSessionToken",
    "__Secure-next-auth.session-token",
    "next-auth.session-token",
];

const CURSOR_DOMAINS: &[&str] = &["cursor.com", "cursor.sh"];

// --- API Response Models (matching CodexBar's CursorUsageSummary) ---

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct CursorUsageSummary {
    billing_cycle_start: Option<String>,
    billing_cycle_end: Option<String>,
    membership_type: Option<String>,
    individual_usage: Option<CursorIndividualUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorIndividualUsage {
    plan: Option<CursorPlanUsage>,
    on_demand: Option<CursorOnDemandUsage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorPlanUsage {
    used: Option<i64>,
    limit: Option<i64>,
    remaining: Option<i64>,
    total_percent_used: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorOnDemandUsage {
    used: Option<i64>,
    limit: Option<i64>,
}

// --- Response sent to the frontend ---

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageData {
    pub percent_used: f64,
    pub used_usd: f64,
    pub limit_usd: f64,
    pub remaining_usd: f64,
    pub on_demand_percent_used: f64,
    pub on_demand_used_usd: f64,
    pub on_demand_limit_usd: Option<f64>,
    pub billing_cycle_end: Option<String>,
    pub membership_type: Option<String>,
}

// --- Settings ---

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub show_plan: bool,
    pub show_on_demand: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_plan: true,
            show_on_demand: true,
        }
    }
}

fn settings_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cursor-juice");
    config_dir.join("settings.json")
}

fn load_settings() -> Settings {
    let path = settings_path();
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

fn save_settings(settings: &Settings) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(&path, json);
    }
}

// --- Cookie Import ---

fn find_cursor_cookie_header() -> Result<String, String> {
    // Try loading cookies from all browsers at once
    let domains: Vec<String> = CURSOR_DOMAINS.iter().map(|d| d.to_string()).collect();
    println!("[cursor-juice] Searching for cookies in domains: {:?}", domains);

    let cookies = rookie::load(Some(domains)).map_err(|e| {
        let msg = format!("Failed to read browser cookies: {}", e);
        eprintln!("[cursor-juice] {}", msg);
        msg
    })?;

    println!(
        "[cursor-juice] Found {} cookies from cursor domains",
        cookies.len()
    );
    for cookie in &cookies {
        println!(
            "[cursor-juice]   cookie: name={}, domain={}",
            cookie.name, cookie.domain
        );
    }

    // Find a session cookie
    for cookie in &cookies {
        if SESSION_COOKIE_NAMES.contains(&cookie.name.as_str()) {
            println!(
                "[cursor-juice] Found session cookie: {}",
                cookie.name
            );
            // Build a cookie header with all cookies from cursor domains
            let cookie_header: String = cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            return Ok(cookie_header);
        }
    }

    let msg =
        "No Cursor session cookie found. Make sure you are logged into cursor.com in your browser."
            .to_string();
    eprintln!("[cursor-juice] {}", msg);
    Err(msg)
}

// --- Tauri Commands ---

#[tauri::command]
async fn fetch_cursor_usage() -> Result<UsageData, String> {
    println!("[cursor-juice] fetch_cursor_usage called");

    let cookie_header = find_cursor_cookie_header()?;
    println!("[cursor-juice] Got cookie header, making API request...");

    let client = reqwest::Client::new();
    let response = client
        .get("https://cursor.com/api/usage-summary")
        .header("Accept", "application/json")
        .header("Cookie", &cookie_header)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| {
            let msg = format!("Network error: {}", e);
            eprintln!("[cursor-juice] {}", msg);
            msg
        })?;

    let status = response.status();
    println!("[cursor-juice] API response status: {}", status);

    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        let msg = "Not logged in. Please log into cursor.com in your browser and try again.";
        eprintln!("[cursor-juice] {}", msg);
        return Err(msg.to_string());
    }

    if !status.is_success() {
        let msg = format!("Cursor API returned HTTP {}", status);
        eprintln!("[cursor-juice] {}", msg);
        return Err(msg);
    }

    // Read raw body first for debugging
    let body = response.text().await.map_err(|e| {
        let msg = format!("Failed to read response body: {}", e);
        eprintln!("[cursor-juice] {}", msg);
        msg
    })?;
    println!("[cursor-juice] Raw API response: {}", &body[..body.len().min(500)]);

    let summary: CursorUsageSummary = serde_json::from_str(&body).map_err(|e| {
        let msg = format!("Failed to parse Cursor API response: {}", e);
        eprintln!("[cursor-juice] {}", msg);
        msg
    })?;

    // Convert cents to USD
    let plan = summary
        .individual_usage
        .as_ref()
        .and_then(|u| u.plan.as_ref());
    let on_demand = summary
        .individual_usage
        .as_ref()
        .and_then(|u| u.on_demand.as_ref());

    let used_cents = plan.and_then(|p| p.used).unwrap_or(0) as f64;
    let limit_cents = plan.and_then(|p| p.limit).unwrap_or(0) as f64;
    let remaining_cents = plan.and_then(|p| p.remaining).unwrap_or(0) as f64;

    // Use dollar-based calculation: used / limit * 100
    let percent_used = if limit_cents > 0.0 {
        (used_cents / limit_cents) * 100.0
    } else {
        0.0
    };

    println!(
        "[cursor-juice] Plan percent: {:.2}% (${:.2} / ${:.2})",
        percent_used, used_cents / 100.0, limit_cents / 100.0
    );

    let od_used_cents = on_demand.and_then(|o| o.used).unwrap_or(0) as f64;
    let od_limit_cents = on_demand.and_then(|o| o.limit);

    let on_demand_percent_used = match od_limit_cents {
        Some(limit) if limit > 0 => (od_used_cents / limit as f64) * 100.0,
        _ => 0.0,
    };

    let result = UsageData {
        percent_used,
        used_usd: used_cents / 100.0,
        limit_usd: limit_cents / 100.0,
        remaining_usd: remaining_cents / 100.0,
        on_demand_percent_used,
        on_demand_used_usd: od_used_cents / 100.0,
        on_demand_limit_usd: od_limit_cents.map(|c| c as f64 / 100.0),
        billing_cycle_end: summary.billing_cycle_end,
        membership_type: summary.membership_type,
    };

    println!(
        "[cursor-juice] Plan: {:.1}% (${:.2} / ${:.2}) | On-demand: {:.1}% (${:.2} / ${}) | membership: {:?}",
        result.percent_used, result.used_usd, result.limit_usd,
        result.on_demand_percent_used, result.on_demand_used_usd,
        result.on_demand_limit_usd.map(|v| format!("{:.2}", v)).unwrap_or("unlimited".to_string()),
        result.membership_type
    );

    Ok(result)
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, Mutex<Settings>>) -> Settings {
    state.lock().unwrap().clone()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(load_settings()))
        .invoke_handler(tauri::generate_handler![fetch_cursor_usage, get_settings])
        .setup(|app| {
            let settings = load_settings();

            let show_plan_item =
                CheckMenuItemBuilder::with_id("show_plan", "Show Plan Usage")
                    .checked(settings.show_plan)
                    .build(app)?;
            let show_od_item =
                CheckMenuItemBuilder::with_id("show_on_demand", "Show On-Demand Usage")
                    .checked(settings.show_on_demand)
                    .build(app)?;

            let view_menu = SubmenuBuilder::new(app, "View")
                .items(&[&show_plan_item, &show_od_item])
                .build()?;

            let menu = MenuBuilder::new(app)
                .items(&[&view_menu])
                .build()?;

            app.set_menu(menu)?;

            app.on_menu_event(move |app_handle, event| {
                let id = event.id().0.as_str();
                match id {
                    "show_plan" | "show_on_demand" => {
                        let state = app_handle.state::<Mutex<Settings>>();
                        let mut settings = state.lock().unwrap();
                        if id == "show_plan" {
                            settings.show_plan =
                                show_plan_item.is_checked().unwrap_or(true);
                        } else {
                            settings.show_on_demand =
                                show_od_item.is_checked().unwrap_or(true);
                        }
                        save_settings(&settings);
                        let _ = app_handle.emit("settings-changed", settings.clone());
                        println!(
                            "[cursor-juice] Settings changed: show_plan={}, show_on_demand={}",
                            settings.show_plan, settings.show_on_demand
                        );
                    }
                    _ => {}
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
