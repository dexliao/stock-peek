use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, Once,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use akshare::AkShareClient;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, State,
    WebviewWindow, WindowEvent,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

const MAIN_WINDOW: &str = "main";
const WATCHLIST_FILE: &str = "watchlist.json";
const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_SHARES: u32 = 100;
const DEFAULT_OPACITY: f64 = 0.75;
const MIN_OPACITY: f64 = 0.3;
const MAX_OPACITY: f64 = 0.9;
const DEFAULT_REFRESH_SECONDS: u64 = 10;
const MIN_REFRESH_SECONDS: u64 = 5;
const MAX_REFRESH_SECONDS: u64 = 60;
const EDGE_MARGIN: f64 = 16.0;
const QUOTE_TIMEOUT: Duration = Duration::from_secs(5);
const NORMAL_WIDTH: f64 = 320.0;
const NORMAL_HEIGHT: f64 = 272.0;
const BOSS_SIZE: f64 = 30.0;

static SHOW_ON_FIRST_LOAD: Once = Once::new();

struct WatchlistStore {
    path: PathBuf,
    items: Mutex<Vec<WatchItem>>,
}

struct SettingsStore {
    path: PathBuf,
    settings: Mutex<AppSettings>,
}

struct MarketClient {
    client: AkShareClient,
}

struct WindowUiState {
    boss: Mutex<bool>,
    click_through: AtomicBool,
    normal_bounds: Mutex<Option<SavedBounds>>,
}

#[derive(Debug, Clone, Copy)]
struct SavedBounds {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WindowFlags {
    boss: bool,
    click_through: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AppSettings {
    opacity: f64,
    refresh_seconds: u64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            opacity: DEFAULT_OPACITY,
            refresh_seconds: DEFAULT_REFRESH_SECONDS,
        }
    }
}

impl AppSettings {
    fn clamped(self) -> Self {
        let opacity = if self.opacity.is_finite() {
            ((self.opacity * 100.0).round() / 100.0).clamp(MIN_OPACITY, MAX_OPACITY)
        } else {
            DEFAULT_OPACITY
        };
        let refresh_seconds = if self.refresh_seconds == 0 {
            DEFAULT_REFRESH_SECONDS
        } else {
            self.refresh_seconds
                .clamp(MIN_REFRESH_SECONDS, MAX_REFRESH_SECONDS)
        };
        Self {
            opacity,
            refresh_seconds,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchItem {
    code: String,
    name: String,
    cost_price: f64,
    #[serde(default = "default_shares")]
    shares: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewWatchItem {
    code: String,
    name: String,
    cost_price: f64,
    #[serde(default = "default_shares")]
    shares: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchPatch {
    code: String,
    cost_price: f64,
    shares: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchHit {
    code: String,
    name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteRow {
    code: String,
    name: String,
    cost_price: f64,
    shares: u32,
    price: Option<f64>,
    change_pct: Option<f64>,
    pnl: Option<f64>,
    date: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuoteBatch {
    rows: Vec<QuoteRow>,
    fetched_at: u64,
}

const fn default_shares() -> u32 {
    DEFAULT_SHARES
}

fn normalize_code(input: &str) -> Option<String> {
    let symbol = akshare::normalize_a_share_symbol(input)?;
    Some(symbol.split('.').next()?.to_string())
}

fn is_a_share_stock(code: &str) -> bool {
    code.len() == 6
        && code.bytes().all(|byte| byte.is_ascii_digit())
        && (code.starts_with('6')
            || code.starts_with('0')
            || code.starts_with('3')
            || code.starts_with('8')
            || code.starts_with('4')
            || code.starts_with("92"))
}

fn load_watchlist(path: &Path) -> Vec<WatchItem> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let parsed: Vec<WatchItem> = match serde_json::from_str(&raw) {
        Ok(items) => items,
        Err(error) => {
            eprintln!("failed to parse {}: {error}", path.display());
            return Vec::new();
        }
    };

    let mut seen = HashSet::new();
    parsed
        .into_iter()
        .filter_map(|mut item| {
            item.code = normalize_code(&item.code)?;
            if !is_a_share_stock(&item.code) {
                return None;
            }
            item.name = item.name.trim().to_string();
            if item.name.is_empty() {
                return None;
            }
            if item.shares == 0 {
                item.shares = DEFAULT_SHARES;
            }
            if !seen.insert(item.code.clone()) {
                return None;
            }
            Some(item)
        })
        .collect()
}

fn save_watchlist(path: &Path, items: &[WatchItem]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建数据目录失败: {error}"))?;
    }
    let body = serde_json::to_string_pretty(items)
        .map_err(|error| format!("序列化 watchlist.json 失败: {error}"))?;
    fs::write(path, format!("{body}\n"))
        .map_err(|error| format!("写入 watchlist.json 失败: {error}"))
}

fn load_settings(path: &Path) -> AppSettings {
    let Ok(raw) = fs::read_to_string(path) else {
        return AppSettings::default();
    };
    match serde_json::from_str::<AppSettings>(&raw) {
        Ok(settings) => settings.clamped(),
        Err(error) => {
            eprintln!("failed to parse {}: {error}", path.display());
            AppSettings::default()
        }
    }
}

fn save_settings(path: &Path, settings: AppSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建数据目录失败: {error}"))?;
    }
    let body = serde_json::to_string_pretty(&settings)
        .map_err(|error| format!("序列化 settings.json 失败: {error}"))?;
    fs::write(path, format!("{body}\n"))
        .map_err(|error| format!("写入 settings.json 失败: {error}"))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

async fn fetch_quote_row(client: AkShareClient, item: WatchItem) -> QuoteRow {
    let mut row = QuoteRow {
        code: item.code.clone(),
        name: item.name.clone(),
        cost_price: item.cost_price,
        shares: item.shares,
        price: None,
        change_pct: None,
        pnl: None,
        date: None,
        error: None,
    };

    match client.a_share_quote(&item.code).await {
        Ok(quote) if quote.close > 0.0 => {
            let price = quote.close;
            row.price = Some(price);
            row.date = Some(quote.date);

            if item.cost_price.is_finite() && item.cost_price > 0.0 {
                row.change_pct = Some((price - item.cost_price) / item.cost_price * 100.0);
                row.pnl = Some((price - item.cost_price) * f64::from(item.shares));
            }
        }
        Ok(_) => {
            row.error = Some("暂无有效现价".to_string());
        }
        Err(error) => {
            row.error = Some(error.to_string());
        }
    }

    row
}

#[tauri::command]
fn get_settings(settings: State<'_, SettingsStore>) -> AppSettings {
    settings
        .settings
        .lock()
        .map(|settings| *settings)
        .unwrap_or_default()
}

#[tauri::command]
fn update_settings(
    settings: State<'_, SettingsStore>,
    next: AppSettings,
) -> Result<AppSettings, String> {
    let next = next.clamped();
    {
        let mut current = settings
            .settings
            .lock()
            .map_err(|_| "设置不可用".to_string())?;
        *current = next;
    }
    save_settings(&settings.path, next)?;
    Ok(next)
}

#[tauri::command]
fn get_watchlist(watchlist: State<'_, WatchlistStore>) -> Result<Vec<WatchItem>, String> {
    watchlist
        .items
        .lock()
        .map(|items| items.clone())
        .map_err(|_| "自选列表不可用".to_string())
}

#[tauri::command]
async fn search_stocks(
    query: String,
    market: State<'_, MarketClient>,
) -> Result<Vec<SearchHit>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let results = market
        .client
        .a_share_search(query, Some("A股"), 8)
        .await
        .map_err(|error| error.to_string())?;

    let mut seen = HashSet::new();
    Ok(results
        .into_iter()
        .filter_map(|result| {
            if result.market != "A股" {
                return None;
            }
            let code = normalize_code(&result.symbol)?;
            if !is_a_share_stock(&code) || !seen.insert(code.clone()) {
                return None;
            }
            let name = result.name.trim().to_string();
            if name.is_empty() {
                return None;
            }
            Some(SearchHit { code, name })
        })
        .collect())
}

#[tauri::command]
fn add_watch_item(
    watchlist: State<'_, WatchlistStore>,
    item: NewWatchItem,
) -> Result<(), String> {
    let code = normalize_code(&item.code)
        .filter(|code| is_a_share_stock(code))
        .ok_or_else(|| "请输入有效的 A 股代码".to_string())?;
    let name = item.name.trim().to_string();
    if name.is_empty() {
        return Err("股票名称不能为空".to_string());
    }
    if !item.cost_price.is_finite() || item.cost_price <= 0.0 {
        return Err("成本价必须大于 0".to_string());
    }

    let shares = if item.shares == 0 {
        DEFAULT_SHARES
    } else {
        item.shares
    };

    let mut items = watchlist
        .items
        .lock()
        .map_err(|_| "自选列表不可用".to_string())?;
    if items.iter().any(|existing| existing.code == code) {
        return Err("该股票已在自选列表".to_string());
    }

    items.push(WatchItem {
        code,
        name,
        cost_price: item.cost_price,
        shares,
    });
    save_watchlist(&watchlist.path, items.as_slice())
}

#[tauri::command]
fn update_watch_item(
    watchlist: State<'_, WatchlistStore>,
    patch: WatchPatch,
) -> Result<(), String> {
    let code = normalize_code(&patch.code)
        .filter(|code| is_a_share_stock(code))
        .ok_or_else(|| "无效的 A 股代码".to_string())?;
    if !patch.cost_price.is_finite() || patch.cost_price <= 0.0 {
        return Err("成本价必须大于 0".to_string());
    }
    let shares = if patch.shares == 0 {
        DEFAULT_SHARES
    } else {
        patch.shares
    };

    let mut items = watchlist
        .items
        .lock()
        .map_err(|_| "自选列表不可用".to_string())?;
    let Some(item) = items.iter_mut().find(|item| item.code == code) else {
        return Err("自选列表中没有该股票".to_string());
    };
    item.cost_price = patch.cost_price;
    item.shares = shares;
    save_watchlist(&watchlist.path, items.as_slice())
}

#[tauri::command]
fn remove_watch_item(
    watchlist: State<'_, WatchlistStore>,
    code: String,
) -> Result<(), String> {
    let code = normalize_code(&code)
        .filter(|code| is_a_share_stock(code))
        .ok_or_else(|| "无效的 A 股代码".to_string())?;
    let mut items = watchlist
        .items
        .lock()
        .map_err(|_| "自选列表不可用".to_string())?;
    let before = items.len();
    items.retain(|item| item.code != code);
    if items.len() == before {
        return Err("自选列表中没有该股票".to_string());
    }
    save_watchlist(&watchlist.path, items.as_slice())
}

#[tauri::command]
async fn refresh_quotes(
    watchlist: State<'_, WatchlistStore>,
    market: State<'_, MarketClient>,
) -> Result<QuoteBatch, String> {
    let items = watchlist
        .items
        .lock()
        .map(|items| items.clone())
        .map_err(|_| "自选列表不可用".to_string())?;

    if items.is_empty() {
        return Ok(QuoteBatch {
            rows: Vec::new(),
            fetched_at: now_millis(),
        });
    }

    let client = market.client.clone();
    let rows = join_all(items.into_iter().map(|item| {
        let client = client.clone();
        async move { fetch_quote_row(client, item).await }
    }))
    .await;

    Ok(QuoteBatch {
        rows,
        fetched_at: now_millis(),
    })
}

fn position_bottom_right(window: &WebviewWindow) -> tauri::Result<()> {
    let monitor = match window.current_monitor()? {
        Some(monitor) => monitor,
        None => match window.primary_monitor()? {
            Some(monitor) => monitor,
            None => return Ok(()),
        },
    };

    let work_area = monitor.work_area();
    let window_size = window.outer_size()?;
    let margin = (EDGE_MARGIN * monitor.scale_factor()).round() as i32;

    let x = work_area.position.x + work_area.size.width as i32
        - window_size.width as i32
        - margin;
    let y = work_area.position.y + work_area.size.height as i32
        - window_size.height as i32
        - margin;

    window.set_position(PhysicalPosition::new(x, y))
}

fn read_window_flags(state: &WindowUiState) -> WindowFlags {
    WindowFlags {
        boss: state.boss.lock().map(|boss| *boss).unwrap_or(false),
        click_through: state.click_through.load(Ordering::SeqCst),
    }
}

fn emit_window_flags(app: &AppHandle, state: &WindowUiState) {
    let _ = app.emit("window-mode-changed", read_window_flags(state));
}

fn save_normal_bounds(window: &WebviewWindow, state: &WindowUiState) {
    if let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) {
        if let Ok(mut bounds) = state.normal_bounds.lock() {
            *bounds = Some(SavedBounds { position, size });
        }
    }
}

fn restore_normal_window(window: &WebviewWindow, state: &WindowUiState) -> tauri::Result<()> {
    let bounds = state.normal_bounds.lock().ok().and_then(|bounds| *bounds);
    if let Some(bounds) = bounds {
        window.set_size(bounds.size)?;
        window.set_position(bounds.position)?;
    } else {
        window.set_size(LogicalSize::new(NORMAL_WIDTH, NORMAL_HEIGHT))?;
        position_bottom_right(window)?;
    }
    Ok(())
}

fn enter_boss_mode(window: &WebviewWindow, state: &WindowUiState) -> tauri::Result<()> {
    save_normal_bounds(window, state);

    let scale = window.scale_factor().unwrap_or(1.0);
    let dot_size = (BOSS_SIZE * scale).round().max(1.0) as u32;
    let anchor = match (window.outer_position(), window.outer_size()) {
        (Ok(position), Ok(size)) => Some((
            position.x + size.width as i32 - dot_size as i32,
            position.y + size.height as i32 - dot_size as i32,
        )),
        _ => None,
    };

    window.set_size(LogicalSize::new(BOSS_SIZE, BOSS_SIZE))?;
    if let Some((x, y)) = anchor {
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    Ok(())
}

fn show_main_window(app: &AppHandle, state: &WindowUiState) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    if state.click_through.load(Ordering::SeqCst)
        && window.set_ignore_cursor_events(false).is_ok()
    {
        state.click_through.store(false, Ordering::SeqCst);
    }

    if state.boss.lock().map(|boss| *boss).unwrap_or(false)
        && restore_normal_window(&window, state).is_ok()
    {
        if let Ok(mut boss) = state.boss.lock() {
            *boss = false;
        }
    }

    let _ = window.show();
    let _ = window.set_focus();
    emit_window_flags(app, state);
}

fn toggle_boss_mode(app: &AppHandle, state: &WindowUiState) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    let visible = window.is_visible().unwrap_or(true);
    if !visible || state.click_through.load(Ordering::SeqCst) {
        show_main_window(app, state);
        return;
    }

    let boss = state.boss.lock().map(|boss| *boss).unwrap_or(false);
    if boss {
        match restore_normal_window(&window, state) {
            Ok(()) => {
                if let Ok(mut boss) = state.boss.lock() {
                    *boss = false;
                }
            }
            Err(error) => {
                eprintln!("failed to restore the window: {error}");
                return;
            }
        }
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        if let Err(error) = enter_boss_mode(&window, state) {
            eprintln!("failed to enter boss mode: {error}");
            return;
        }
        if let Ok(mut boss) = state.boss.lock() {
            *boss = true;
        }
        let _ = window.show();
    }

    emit_window_flags(app, state);
}

#[tauri::command]
fn get_window_flags(state: State<'_, WindowUiState>) -> WindowFlags {
    read_window_flags(state.inner())
}

#[tauri::command]
fn set_click_through(
    app: AppHandle,
    state: State<'_, WindowUiState>,
    enabled: bool,
) -> Result<WindowFlags, String> {
    if enabled && state.boss.lock().map(|boss| *boss).unwrap_or(false) {
        return Err("请先恢复窗口再开启鼠标穿透".to_string());
    }

    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return Err("主窗口不存在".to_string());
    };
    window
        .set_ignore_cursor_events(enabled)
        .map_err(|error| error.to_string())?;
    state.click_through.store(enabled, Ordering::SeqCst);

    let flags = read_window_flags(state.inner());
    let _ = app.emit("window-mode-changed", flags.clone());
    Ok(flags)
}

fn create_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("todo.txt")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(state) = app.try_state::<WindowUiState>() {
                    show_main_window(app, state.inner());
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(state) = app.try_state::<WindowUiState>() {
                    show_main_window(app, state.inner());
                }
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    } else {
        eprintln!("tray icon: default window icon is unavailable");
    }

    builder.build(app)?;
    Ok(())
}

fn handle_toggle_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if matches!(event.state(), ShortcutState::Pressed)
        && shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyH)
    {
        if let Some(state) = app.try_state::<WindowUiState>() {
            toggle_boss_mode(app, state.inner());
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_watchlist,
            search_stocks,
            add_watch_item,
            update_watch_item,
            remove_watch_item,
            refresh_quotes,
            get_window_flags,
            set_click_through,
            get_settings,
            update_settings
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                if let Err(error) = position_bottom_right(&window) {
                    eprintln!("failed to position the main window: {error}");
                }
            }

            let app_data_dir = app.path().app_data_dir()?;
            let watchlist_path = app_data_dir.join(WATCHLIST_FILE);
            let items = load_watchlist(&watchlist_path);
            if !watchlist_path.exists() {
                if let Err(error) = save_watchlist(&watchlist_path, &items) {
                    eprintln!("failed to create watchlist.json: {error}");
                }
            }
            app.manage(WatchlistStore {
                path: watchlist_path,
                items: Mutex::new(items),
            });

            let settings_path = app_data_dir.join(SETTINGS_FILE);
            let settings = load_settings(&settings_path);
            if !settings_path.exists() {
                if let Err(error) = save_settings(&settings_path, settings) {
                    eprintln!("failed to create settings.json: {error}");
                }
            }
            app.manage(SettingsStore {
                path: settings_path,
                settings: Mutex::new(settings),
            });

            let client = AkShareClient::builder().timeout(QUOTE_TIMEOUT).build();
            app.manage(MarketClient { client });

            app.manage(WindowUiState {
                boss: Mutex::new(false),
                click_through: AtomicBool::new(false),
                normal_bounds: Mutex::new(None),
            });

            if let Err(error) = create_tray(app) {
                eprintln!("failed to create system tray: {error}");
            }

            let toggle_shortcut =
                Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyH);
            if let Err(error) =
                app.global_shortcut()
                    .on_shortcut(toggle_shortcut, handle_toggle_shortcut)
            {
                eprintln!("failed to register Ctrl+Shift+H: {error}");
            }

            Ok(())
        })
        .on_page_load(|webview, _payload| {
            if webview.label() == MAIN_WINDOW {
                // The window starts hidden to avoid a white flash while the
                // webview boots. Reveal it once the first page has loaded.
                SHOW_ON_FIRST_LOAD.call_once(|| {
                    let _ = webview.window().show();
                });
            }
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running stock-peek");
}
