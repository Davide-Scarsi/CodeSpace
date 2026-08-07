#![allow(linker_messages)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;
use tauri::Emitter;
use tauri::Manager;
use walkdir::WalkDir;

// ── Data Structures ────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceInfo {
    pub path: String,
    pub name: String,
    pub display_path: String,
    pub color: Option<String>,
    pub is_open: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CacheEntry {
    pub path: String,
    pub name: String,
    pub modified: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cache {
    pub entries: Vec<CacheEntry>,
    pub last_scan: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TaskInfo {
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub icon: String,
    pub task_type: String,
    pub url: Option<String>,
    pub confirm_before_run: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VsCodeTask {
    label: Option<String>,
    #[serde(rename = "type")]
    task_type: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    options: Option<TaskOptions>,
    #[serde(rename = "codeSpace")]
    code_space: Option<CodeSpaceSettings>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CodeSpaceSettings {
    #[serde(rename = "taskType")]
    task_type: Option<String>,
    url: Option<String>,
    #[serde(rename = "confirmationRequest")]
    confirmation_request: Option<bool>,
}

/// Icon mapping: task-type → Lucide SVG path (24x24 viewBox, stroke-based)
fn get_task_icon(task_type: &str) -> &str {
    match task_type {
        "live-server" => "M6.34277267,4.93867691 C6.73329697,5.3292012 6.73329697,5.96236618 6.34277267,6.35289047 C3.21757171,9.47809143 3.21757171,14.5450433 6.34277267,17.6702443 C6.73329697,18.0607686 6.73329697,18.6939336 6.34277267,19.0844579 C5.95224838,19.4749821 5.3190834,19.4749821 4.92855911,19.0844579 C1.02230957,15.1782083 1.02230957,8.84492646 4.92855911,4.93867691 C5.3190834,4.54815262 5.95224838,4.54815262 6.34277267,4.93867691 Z M19.0743401,4.93867691 C22.9805896,8.84492646 22.9805896,15.1782083 19.0743401,19.0844579 C18.6838158,19.4749821 18.0506508,19.4749821 17.6601265,19.0844579 C17.2696022,18.6939336 17.2696022,18.0607686 17.6601265,17.6702443 C20.7853275,14.5450433 20.7853275,9.47809143 17.6601265,6.35289047 C17.2696022,5.96236618 17.2696022,5.3292012 17.6601265,4.93867691 C18.0506508,4.54815262 18.6838158,4.54815262 19.0743401,4.93867691 Z M9.3094225,7.81205295 C9.69994679,8.20257725 9.69994679,8.83574222 9.3094225,9.22626652 C7.77845993,10.7572291 7.77845993,13.2394099 9.3094225,14.7703724 C9.69994679,15.1608967 9.69994679,15.7940617 9.3094225,16.184586 C8.91889821,16.5751103 8.28573323,16.5751103 7.89520894,16.184586 C5.58319778,13.8725748 5.58319778,10.1240641 7.89520894,7.81205295 C8.28573323,7.42152866 8.91889821,7.42152866 9.3094225,7.81205295 Z M16.267742,7.81205295 C18.5797531,10.1240641 18.5797531,13.8725748 16.267742,16.184586 C15.8772177,16.5751103 15.2440527,16.5751103 14.8535284,16.184586 C14.4630041,15.7940617 14.4630041,15.1608967 14.8535284,14.7703724 C16.384491,13.2394099 16.384491,10.7572291 14.8535284,9.22626652 C14.4630041,8.83574222 14.4630041,8.20257725 14.8535284,7.81205295 C15.2440527,7.42152866 15.8772177,7.42152866 16.267742,7.81205295 Z M12.0814755,10.5814755 C12.9099026,10.5814755 13.5814755,11.2530483 13.5814755,12.0814755 C13.5814755,12.9099026 12.9099026,13.5814755 12.0814755,13.5814755 C11.2530483,13.5814755 10.5814755,12.9099026 10.5814755,12.0814755 C10.5814755,11.2530483 11.2530483,10.5814755 12.0814755,10.5814755 Z",
        "php-server" => "M12 2 2 7l10 5 10-5-10-5z M2 17l10 5 10-5 M2 12l10 5 10-5",
        "npm" => "M12 2l10 5v10l-10 5-10-5V7z M12 22V12 M4 7l8 4 8-4",
        "echo" => "M4 6h16 M4 12h10 M4 18h8",
        "ftp-mount" => "M64 160h896v576H64V160zm160 128h576v256H224V288zm192 448h256v128H416v-128z",
        "upload" => "M256 608h512v128H256v-128zm256-320l-128 160h80v128h96V448h80l-128-160z",
        "add-user" => "M12 12a5 5 0 1 0 0-10 5 5 0 0 0 0 10z M4 20c0-4.42 3.58-8 8-8s8 3.58 8 8",
        "test" => "M8 2v6a6 6 0 0 0 8 0V2 M6 14h12 M10 18h4",
        "powershell" => "M4 17l6-6-6-6 M12 19h8",
        _ => "M9 8L5 11.6923L9 16M15 8L19 11.6923L15 16", // default: generic double-arrow
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskOptions {
    cwd: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TasksJson {
    tasks: Option<Vec<VsCodeTask>>,
}

// ── Helpers ────────────────────────────────────────────────────

fn get_cache_path(app: &tauri::AppHandle) -> PathBuf {
    let app_dir = app
        .path()
        .app_data_dir()
        .expect("failed to resolve app data dir");
    fs::create_dir_all(&app_dir).ok();
    app_dir.join("cache.json")
}

fn get_timestamp(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn should_skip_dir(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_string_lossy();
    name.starts_with('.')
        || name.starts_with('$')
        || name == "node_modules"
        || name == "target"
        || name == "Windows"
        || name == "Program Files"
        || name == "Program Files (x86)"
        || name == "ProgramData"
        || name == "AppData"
        || name == "msys64"
        || name == "build"
        || name == "dist"
        || name == ".git"
        || name == ".svn"
        || name == ".hg"
}

fn scan_drive(drive_root: &Path) -> Vec<WorkspaceInfo> {
    let mut results = Vec::new();

    for entry in WalkDir::new(drive_root)
        .follow_links(false)
        .max_depth(20)
        .into_iter()
        .filter_entry(|e| !should_skip_dir(e))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.file_type().is_file() {
            let path = entry.path();
            if path.extension().map(|e| e == "code-workspace").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();

                let display_path = path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                results.push(WorkspaceInfo {
                    path: path.to_string_lossy().to_string(),
                    name,
                    display_path,
                    color: None,
                    is_open: false,
                });
            }
        }
    }

    results
}

fn quick_scan(cache: &Cache) -> Vec<WorkspaceInfo> {
    let mut results = Vec::new();
    let mut cached_paths: HashMap<String, bool> = HashMap::new();

    // Pre-check: verify cached entries still exist
    for entry in &cache.entries {
        let path = Path::new(&entry.path);
        if path.exists() {
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let display_path = path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            results.push(WorkspaceInfo {
                path: entry.path.clone(),
                name,
                display_path,
                color: None,
                is_open: false,
            });
            cached_paths.insert(entry.path.clone(), true);
        }
    }

    // Collect parent directories from cache for incremental scan
    let mut parent_dirs: HashMap<String, bool> = HashMap::new();
    for entry in &cache.entries {
        if let Some(parent) = Path::new(&entry.path).parent() {
            let mut current = parent.to_path_buf();
            for _ in 0..3 {
                if let Some(p) = current.parent() {
                    current = p.to_path_buf();
                }
            }
            parent_dirs.insert(current.to_string_lossy().to_string(), true);
        }
    }

    // Walk known parent areas for new .code-workspace files
    for parent_dir in parent_dirs.keys() {
        let parent = Path::new(parent_dir);
        if !parent.exists() {
            continue;
        }
        for entry in WalkDir::new(parent)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !should_skip_dir(e))
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if entry.file_type().is_file() {
                let path = entry.path();
                if path.extension().map(|e| e == "code-workspace").unwrap_or(false) {
                    let path_str = path.to_string_lossy().to_string();
                    if !cached_paths.contains_key(&path_str) {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let display_path = path
                            .parent()
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_default();
                        results.push(WorkspaceInfo {
                            path: path_str,
                            name,
                            display_path,
                            color: None,
                            is_open: false,
                        });
                    }
                }
            }
        }
    }

    results
}

fn full_scan() -> Vec<WorkspaceInfo> {
    let mut results = Vec::new();

    for letter in b'A'..=b'Z' {
        let drive = format!("{}:\\", letter as char);
        let drive_path = Path::new(&drive);
        if drive_path.exists() {
            results.extend(scan_drive(drive_path));
        }
    }

    results
}

fn save_cache(app: &tauri::AppHandle, workspaces: &[WorkspaceInfo]) {
    let cache_path = get_cache_path(app);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entries: Vec<CacheEntry> = workspaces
        .iter()
        .map(|w| {
            let modified = fs::metadata(&w.path)
                .map(|m| get_timestamp(&m))
                .unwrap_or(0);
            CacheEntry {
                path: w.path.clone(),
                name: w.name.clone(),
                modified,
            }
        })
        .collect();

    let cache = Cache {
        entries,
        last_scan: now,
    };

    if let Ok(json) = serde_json::to_string_pretty(&cache) {
        fs::write(&cache_path, json).ok();
    }
}

fn load_cache(app: &tauri::AppHandle) -> Option<Cache> {
    let cache_path = get_cache_path(app);
    if cache_path.exists() {
        if let Ok(content) = fs::read_to_string(&cache_path) {
            return serde_json::from_str(&content).ok();
        }
    }
    None
}

// ── JSONC support ────────────────────────────────────────────

/// Strip // line comments from JSONC content so serde_json can parse it.
fn strip_json_comments(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read a .code-workspace file and parse it as JSON (strips comments first).
fn read_workspace_json(ws_file: &Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(ws_file).ok()?;
    let cleaned = strip_json_comments(&content);
    serde_json::from_str::<serde_json::Value>(&cleaned).ok()
}

/// Write JSON back to a .code-workspace file (always valid JSON, no comments).
fn write_workspace_json(ws_file: &Path, json: &serde_json::Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(json)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    fs::write(ws_file, &content)
        .map_err(|e| format!("Cannot write workspace file: {}", e))
}

// ── Peacock Color ─────────────────────────────────────────────

/// Read workspace color from .code-workspace file
fn read_workspace_color(workspace_path: &str) -> Option<String> {
    let ws_file = Path::new(workspace_path);

    // 2. .code-workspace file
    if let Some(json) = read_workspace_json(ws_file) {
        if let Some(settings) = json.get("settings") {
            if let Some(wb) = settings.get("workbench.colorCustomizations") {
                if let Some(color) = extract_color_from_customizations(wb) {
                    return Some(color);
                }
            }
        }
    }

    None
}

fn extract_color_from_customizations(customizations: &serde_json::Value) -> Option<String> {
    for key in &["titleBar.activeBackground", "activityBar.background", "statusBar.background"] {
        if let Some(color) = customizations.get(*key).and_then(|v| v.as_str()) {
            return Some(color.to_string());
        }
    }
    if let Some(obj) = customizations.as_object() {
        for (_key, val) in obj {
            if let Some(c) = val.as_str() {
                if c.starts_with('#') {
                    return Some(c.to_string());
                }
            }
        }
    }
    None
}

fn populate_colors(workspaces: &mut [WorkspaceInfo]) {
    for ws in workspaces.iter_mut() {
        ws.color = read_workspace_color(&ws.path);
    }
}

fn populate_open_status(workspaces: &mut [WorkspaceInfo]) {
    let open_names = get_open_workspace_names();
    for ws in workspaces.iter_mut() {
        let name = Path::new(&ws.path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        ws.is_open = open_names.iter().any(|n| n == &name);
    }
}

/// Extract workspace name from a VS Code window title.
/// VS Code titles look like: "file.ts ● WorkspaceName (Workspace) - Visual Studio Code"
fn extract_ws_name_from_title(title: &str) -> Option<String> {
    if let Some(ws_pos) = title.find(" (Workspace)") {
        let before = &title[..ws_pos];
        // Try " - " separator first (file open: "file.ts - WorkspaceName (Workspace)")
        let name = if let Some(dash) = before.rfind(" - ") {
            before[dash + 3..].trim().to_string()
        } else {
            // Try "●" separator (no file open: "● WorkspaceName (Workspace)")
            if let Some(dot) = before.rfind('●') {
                before[dot + '●'.len_utf8()..].trim().to_string()
            } else {
                before.trim().to_string()
            }
        };
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

fn get_open_workspace_names() -> Vec<String> {
    let mut names = Vec::new();
    let names_ptr = &mut names as *mut Vec<String>;

    unsafe extern "system" fn enum_callback(hwnd: isize, lparam: isize) -> i32 {
        let names = &mut *(lparam as *mut Vec<String>);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len > 0 {
            let title = String::from_utf16_lossy(&buf[..len as usize]);
            // eprintln!("[DEBUG] Window: \"{}\"", title);
            if let Some(name) = extract_ws_name_from_title(&title) {
                // eprintln!("[DEBUG]   -> extracted name: \"{}\"", name);
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        1
    }

    unsafe { EnumWindows(enum_callback, names_ptr as isize) };
    // eprintln!("[DEBUG] open workspace names: {:?}", names);
    names
}

/// Get the active (foreground) workspace name by checking the foreground window
fn get_active_workspace_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len > 0 {
            let title = String::from_utf16_lossy(&buf[..len as usize]);
            // eprintln!("[DEBUG] Foreground window: \"{}\"", title);
            extract_ws_name_from_title(&title)
        } else {
            None
        }
    }
}

#[cfg(windows)]
extern "system" {
    fn EnumWindows(cb: unsafe extern "system" fn(hwnd: isize, lparam: isize) -> i32, lparam: isize) -> i32;
    fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn GetForegroundWindow() -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    #[allow(dead_code)]
    fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
}

#[cfg(windows)]
#[repr(C)]
struct Rect { left: i32, top: i32, right: i32, bottom: i32 }

#[cfg(not(windows))]
fn get_open_workspace_names() -> Vec<String> {
    Vec::new()
}

// ── Tauri Commands ─────────────────────────────────────────────

#[tauri::command]
fn scan_workspaces(app: tauri::AppHandle, force_full: bool) -> Vec<WorkspaceInfo> {
    let mut workspaces = if force_full {
        full_scan()
    } else if let Some(cache) = load_cache(&app) {
        quick_scan(&cache)
    } else {
        full_scan()
    };

    // Sort alphabetically by name
    workspaces.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Deduplicate by path (quick_scan may find same workspace from multiple sources)
    let mut seen = HashMap::new();
    workspaces.retain(|w| seen.insert(w.path.clone(), true).is_none());

    populate_colors(&mut workspaces);
    populate_open_status(&mut workspaces);
    save_cache(&app, &workspaces);
    workspaces
}

#[tauri::command]
fn launch_workspace(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let ws_name = Path::new(&path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // Use Windows shell "start" to open .code-workspace with associated program
    Command::new("cmd")
        .args(["/c", "start", "", &path])
        .spawn()
        .map_err(|e| format!("Failed to launch: {}", e))?;

    // Spawn background thread to poll for the VS Code window (non-blocking)
    #[cfg(windows)]
    {
        let title_pattern = format!("{} (Workspace) - Visual Studio Code", ws_name);
        let ws_name_clone = ws_name.clone();
        std::thread::spawn(move || {
            for _ in 0..150 {
                // 15 seconds (150 * 100ms)
                std::thread::sleep(std::time::Duration::from_millis(100));
                unsafe {
                    let hwnd = find_window_by_title(&title_pattern);
                    if hwnd != 0 {
                        ShowWindow(hwnd, 3); // SW_MAXIMIZE
                        SetForegroundWindow(hwnd);
                        let _ = app.emit("workspace-launched", &ws_name_clone);
                        return;
                    }
                }
            }
            // Timeout
            let _ = app.emit("workspace-launch-failed", &ws_name_clone);
        });
    }

    Ok(())
}

#[tauri::command]
fn get_workspace_color(workspace_path: String) -> Option<String> {
    read_workspace_color(&workspace_path)
}

#[tauri::command]
fn set_workspace_color(workspace_path: String, color: String) -> Result<(), String> {
    let ws_file = Path::new(&workspace_path);

    // ── Write to .code-workspace file ──
    if let Some(mut ws_json) = read_workspace_json(ws_file) {
        if ws_json.get("settings").is_none() {
            ws_json["settings"] = serde_json::json!({});
        }
        let s = &mut ws_json["settings"];
        write_full_color_overrides(s, &color);
        write_workspace_json(ws_file, &ws_json)?;
    }

    Ok(())
}

/// Write the full set of workbench.colorCustomizations (VS Code native).
/// Removes any old `peacock.color` key.
fn write_full_color_overrides(settings: &mut serde_json::Value, color: &str) {
    // Remove old Peacock key
    settings.as_object_mut().map(|o| o.remove("peacock.color"));

    let obj = settings["workbench.colorCustomizations"]
        .as_object_mut()
        .map(|o| {
            // Clear all existing color keys, keep non-color ones if any
            o.clear();
        });

    if obj.is_none() {
        settings["workbench.colorCustomizations"] = serde_json::json!({});
    }

    let wb = &mut settings["workbench.colorCustomizations"];
    wb["titleBar.activeBackground"] = serde_json::Value::String(color.to_string());
    wb["titleBar.activeForeground"] = serde_json::Value::String("#ffffff".into());
    wb["titleBar.inactiveBackground"] = serde_json::Value::String(format!("{}99", color));
    wb["titleBar.inactiveForeground"] = serde_json::Value::String("#ffffff99".into());
    wb["activityBar.activeBackground"] = serde_json::Value::String(color.to_string());
    wb["activityBar.background"] = serde_json::Value::String(color.to_string());
    wb["activityBar.foreground"] = serde_json::Value::String("#ffffff".into());
    wb["activityBar.inactiveForeground"] = serde_json::Value::String("#ffffff99".into());
    wb["activityBarBadge.background"] = serde_json::Value::String(color.to_string());
    wb["activityBarBadge.foreground"] = serde_json::Value::String("#ffffff".into());
    wb["statusBar.background"] = serde_json::Value::String(color.to_string());
    wb["statusBar.foreground"] = serde_json::Value::String("#ffffff".into());
    wb["statusBar.debuggingBackground"] = serde_json::Value::String(color.to_string());
    wb["statusBar.debuggingForeground"] = serde_json::Value::String("#ffffff".into());
    wb["statusBarItem.hoverBackground"] = serde_json::Value::String(darken(color));
    wb["statusBarItem.remoteBackground"] = serde_json::Value::String(color.to_string());
    wb["statusBarItem.remoteForeground"] = serde_json::Value::String("#ffffff".into());
    wb["sash.hoverBorder"] = serde_json::Value::String(color.to_string());
    wb["commandCenter.border"] = serde_json::Value::String("#ffffff99".into());
    wb["commandCenter.foreground"] = serde_json::Value::String("#ffffff".into());
}

/// Darken a hex color by ~20%
fn darken(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return format!("#{}", hex);
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    format!("#{:02x}{:02x}{:02x}",
        r.saturating_mul(8).saturating_div(10),
        g.saturating_mul(8).saturating_div(10),
        b.saturating_mul(8).saturating_div(10))
}

#[tauri::command]
fn remove_workspace_color(workspace_path: String) -> Result<(), String> {
    let ws_file = Path::new(&workspace_path);

    // 1. Remove from .code-workspace file
    if let Some(mut ws_json) = read_workspace_json(ws_file) {
        if let Some(settings) = ws_json.get_mut("settings") {
            settings.as_object_mut().map(|o| { o.remove("peacock.color"); o.remove("workbench.colorCustomizations"); });
            write_workspace_json(ws_file, &ws_json)?;
        }
    }

    Ok(())
}

#[tauri::command]
async fn check_update() -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let res = client
        .get("https://api.github.com/repos/Davide-Scarsi/CodeSpace/releases/latest")
        .header("User-Agent", "CodeSpace-Updater")
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let json: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Parse failed: {}", e))?;

    Ok(json)
}

#[tauri::command]
async fn download_and_install(url: String) -> Result<(), String> {
    // Get current exe path
    let current = std::env::current_exe().map_err(|e| format!("{}", e))?;
    let dir = current.parent().ok_or("No parent dir")?;
    let new_exe = dir.join("CodeSpace_new.exe");
    let bat = dir.join("_update.bat");

    // Download the new exe
    let client = reqwest::Client::new();
    let bytes = client
        .get(&url)
        .header("User-Agent", "CodeSpace-Updater")
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?
        .bytes()
        .await
        .map_err(|e| format!("Read failed: {}", e))?;

    fs::write(&new_exe, &bytes).map_err(|e| format!("Save failed: {}", e))?;

    // Write batch script that replaces and restarts
    let bat_content = format!(
        "@echo off\r\n\
         timeout /t 2 /nobreak >nul\r\n\
         del /f \"{}\"\r\n\
         move /y \"{}\" \"{}\"\r\n\
         start \"\" \"{}\"\r\n\
         del /f \"%~f0\"\r\n",
        current.display(),
        new_exe.display(),
        current.display(),
        current.display()
    );
    fs::write(&bat, bat_content).map_err(|e| format!("Bat failed: {}", e))?;

    // Run the script and exit
    Command::new("cmd")
        .args(["/c", bat.to_str().unwrap_or("")])
        .spawn()
        .map_err(|e| format!("Launch failed: {}", e))?;

    std::process::exit(0);
}

#[tauri::command]
fn focus_workspace(name: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        let title_pattern = format!("{} (Workspace) - Visual Studio Code", name);

        unsafe {
            // Find target window
            let target = find_window_by_title(&title_pattern);
            if target == 0 {
                return Err(format!("Window not found for: {}", name));
            }

            // Minimize others FIRST, then maximize & focus target
            minimize_other_vscode_windows(target);
            ShowWindow(target, 3); // SW_MAXIMIZE
            SetForegroundWindow(target);
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("focus_workspace is only supported on Windows".into())
    }
}

#[tauri::command]
fn minimize_workspace(name: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        let title_pattern = format!("{} (Workspace) - Visual Studio Code", name);
        unsafe {
            let hwnd = find_window_by_title(&title_pattern);
            if hwnd == 0 {
                return Err(format!("Window not found for: {}", name));
            }
            ShowWindow(hwnd, 6); // SW_MINIMIZE
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("minimize_workspace is only supported on Windows".into())
    }
}

/// Minimize all visible VS Code windows except the given hwnd.
#[cfg(windows)]
unsafe fn minimize_other_vscode_windows(exclude: isize) {
    struct Ctx {
        exclude: isize,
    }

    unsafe extern "system" fn enum_cb(hwnd: isize, lparam: isize) -> i32 {
        let ctx = &*(lparam as *const Ctx);
        if hwnd == ctx.exclude {
            return 1;
        }
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len > 0 {
            let title = String::from_utf16_lossy(&buf[..len as usize]);
            if title.contains("Visual Studio Code") {
                ShowWindow(hwnd, 6); // SW_MINIMIZE
            }
        }
        1
    }

    let ctx = Ctx { exclude };
    EnumWindows(enum_cb, &ctx as *const Ctx as isize);
}

#[cfg(windows)]
unsafe fn find_window_by_title(partial_title: &str) -> isize {
    struct FocusCtx {
        partial: String,
        found: isize,
    }

    unsafe extern "system" fn enum_cb(hwnd: isize, lparam: isize) -> i32 {
        let ctx = &mut *(lparam as *mut FocusCtx);
        if IsWindowVisible(hwnd) == 0 {
            return 1;
        }
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        if len > 0 {
            let title = String::from_utf16_lossy(&buf[..len as usize]);
            if title.contains(&ctx.partial) && ctx.found == 0 {
                ctx.found = hwnd;
                return 0;
            }
        }
        1
    }

    let mut ctx = FocusCtx {
        partial: partial_title.to_string(),
        found: 0,
    };

    EnumWindows(enum_cb, &mut ctx as *mut FocusCtx as isize);
    ctx.found
}

#[tauri::command]
fn check_open_status(paths: Vec<String>) -> Vec<bool> {
    let open_names = get_open_workspace_names();
    paths.iter().map(|p| {
        let name = Path::new(p)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        open_names.iter().any(|o| o == &name)
    }).collect()
}

/// Payload emitted on "workspace-changed" events
#[derive(Debug, Clone, Serialize)]
struct MonitorPayload {
    open_names: Vec<String>,
    active_name: Option<String>,
    live_terminals: HashMap<String, Vec<isize>>,
}

/// Shared flag to signal the background monitor to stop
static MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

/// Tracks live-server terminal windows: workspace_name -> Vec<HWND>
static LIVE_TERMINALS: Mutex<Option<HashMap<String, Vec<isize>>>> = Mutex::new(None);

fn init_live_terminals() {
    let mut guard = LIVE_TERMINALS.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashMap::new());
    }
}

/// Spawns a background thread that monitors VS Code windows via Win32 API.
/// Emits "workspace-changed" events with the list of open workspace names
/// plus the currently active (foreground) workspace.
#[tauri::command]
fn start_workspace_monitor(app: tauri::AppHandle) {
    if MONITOR_RUNNING.swap(true, Ordering::SeqCst) {
        eprintln!("[DEBUG] start_workspace_monitor: already running, skipping");
        return;
    }
    // eprintln!("[DEBUG] start_workspace_monitor called");
    std::thread::spawn(move || {
        let mut last_payload: Option<MonitorPayload> = None;
        while MONITOR_RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if !MONITOR_RUNNING.load(Ordering::SeqCst) {
                break;
            }
            let names = get_open_workspace_names();
            let active = get_active_workspace_name();

            // Clean dead terminal HWNDs
            init_live_terminals();
            let mut terminals = LIVE_TERMINALS.lock().unwrap();
            let term_map = terminals.as_mut().unwrap();
            for hwnds in term_map.values_mut() {
                #[cfg(windows)]
                hwnds.retain(|&h| unsafe { IsWindow(h) != 0 });
            }
            term_map.retain(|_, hwnds| !hwnds.is_empty());
            let live_terms = term_map.clone();
            drop(terminals);

            let payload = MonitorPayload {
                open_names: names,
                active_name: active,
                live_terminals: live_terms,
            };
            let changed = match &last_payload {
                Some(p) => p.open_names != payload.open_names || p.active_name != payload.active_name || p.live_terminals != payload.live_terminals,
                None => true,
            };
            // eprintln!("[DEBUG] monitor tick: open={:?}, active={:?}", payload.open_names, payload.active_name);
            if changed {
                // eprintln!("[DEBUG] emitting workspace-changed: {:?}", payload);
                match app.emit("workspace-changed", &payload) {
                    Ok(_) => {} // eprintln!("[DEBUG] emit OK")
                    Err(e) => eprintln!("[DEBUG] emit ERROR: {}", e),
                }
                last_payload = Some(payload);
            }
        }
        // eprintln!("[DEBUG] monitor thread exiting");
    });
}

/// Stop the background workspace monitor
#[tauri::command]
fn stop_workspace_monitor() {
    // eprintln!("[DEBUG] stop_workspace_monitor called");
    MONITOR_RUNNING.store(false, Ordering::SeqCst);
}

/// Toggle a live-server terminal window (minimize/restore)
#[tauri::command]
fn toggle_live_terminal(hwnds: Vec<isize>) -> Result<(), String> {
    #[cfg(windows)]
    unsafe {
        for &h in &hwnds {
            if IsWindow(h) != 0 {
                if IsIconic(h) != 0 {
                    ShowWindow(h, 9);
                } else {
                    ShowWindow(h, 6);
                }
                SetForegroundWindow(h);
            }
        }
    }
    let _ = hwnds;
    Ok(())
}

#[tauri::command]
fn create_workspace(folder_path: String) -> Result<String, String> {
    let folder = Path::new(&folder_path);
    if !folder.is_dir() {
        return Err("Not a directory".into());
    }
    let name = folder
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "workspace".into());

    let ws_path = folder.join(format!("{}.code-workspace", name));
    let ws_json = serde_json::json!({
        "folders": [{ "path": "." }],
        "settings": {}
    });
    let content = serde_json::to_string_pretty(&ws_json)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    fs::write(&ws_path, content)
        .map_err(|e| format!("Cannot write: {}", e))?;

    Ok(ws_path.to_string_lossy().to_string())
}

#[tauri::command]
fn get_scan_info(app: tauri::AppHandle) -> serde_json::Value {
    if let Some(cache) = load_cache(&app) {
        serde_json::json!({
            "has_cache": true,
            "count": cache.entries.len(),
            "last_scan": cache.last_scan,
        })
    } else {
        serde_json::json!({
            "has_cache": false,
            "count": 0,
            "last_scan": 0,
        })
    }
}

#[tauri::command]
fn get_workspace_tasks(workspace_path: String) -> Result<Vec<TaskInfo>, String> {
    let ws_file = Path::new(&workspace_path);
    let project_dir = ws_file.parent().unwrap_or(Path::new("."));

    // Resolve the first folder from the workspace to find the actual project root
    let root = if let Some(json) = read_workspace_json(ws_file) {
        if let Some(folders) = json.get("folders").and_then(|v| v.as_array()) {
            if let Some(first) = folders.first() {
                if let Some(folder_path) = first.get("path").and_then(|v| v.as_str()) {
                    project_dir.join(folder_path)
                } else {
                    project_dir.to_path_buf()
                }
            } else {
                project_dir.to_path_buf()
            }
        } else {
            project_dir.to_path_buf()
        }
    } else {
        project_dir.to_path_buf()
    };

    let tasks_path = root.join(".vscode").join("tasks.json");
    if !tasks_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&tasks_path)
        .map_err(|e| format!("Cannot read tasks.json: {}", e))?;
    let tasks_json: TasksJson = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid tasks.json: {}", e))?;

    let mut result = Vec::new();
    if let Some(tasks) = tasks_json.tasks {
        for t in tasks {
            if t.task_type.as_deref() == Some("shell") {
                if let (Some(label), Some(cmd_name)) = (t.label, t.command) {
                    let args = t.args.unwrap_or_default();
                    let cwd = t.options.and_then(|o| o.cwd)
                        .map(|d| {
                            let resolved = d.replace("${workspaceFolder}", &root.to_string_lossy().to_string());
                            let p = Path::new(&resolved);
                            if p.is_relative() {
                                root.join(p).to_string_lossy().to_string()
                            } else {
                                resolved
                            }
                        })
                        .or_else(|| Some(root.to_string_lossy().to_string()));
                    let task_type = t.code_space.as_ref().and_then(|cs| cs.task_type.clone()).unwrap_or_default();
                    let url = t.code_space.as_ref().and_then(|cs| cs.url.clone());
                    let confirm = t.code_space.as_ref().and_then(|cs| cs.confirmation_request);
                    let icon = get_task_icon(&task_type).to_string();
                    result.push(TaskInfo { label, command: cmd_name, args, cwd, icon, task_type, url, confirm_before_run: confirm });
                }
            }
        }
    }

    Ok(result)
}

#[tauri::command]
fn run_task(command: String, args: Vec<String>, cwd: Option<String>, task_type: String, workspace_name: String, url: Option<String>) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x00000010;

    // Save CodeSpace position
    let cs_hwnd = unsafe { GetForegroundWindow() };
    let mut cs_rect = Rect { left: 0, top: 0, right: 800, bottom: 600 };
    unsafe { GetWindowRect(cs_hwnd, &mut cs_rect); }

    let task_type_clone = task_type.clone();
    let workspace_name_clone = workspace_name.clone();
    let url_clone = url.clone();
    let spawn = move || -> Result<(), String> {
        if args.is_empty() && command.contains(' ') {
            let mut ps = Command::new("powershell");
            ps.args(["-NoExit", "-Command", &command]);
            ps.creation_flags(CREATE_NEW_CONSOLE);
            if let Some(dir) = &cwd {
                let resolved = dir.replace("${workspaceFolder}", &std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
                ps.current_dir(&resolved);
            }
            ps.spawn().map_err(|e| format!("Failed: {}", e))?;
        } else {
            let mut cmd = Command::new("cmd");
            cmd.arg("/k");
            cmd.arg(&command);
            for a in &args { cmd.arg(a); }
            cmd.creation_flags(CREATE_NEW_CONSOLE);
            if let Some(dir) = &cwd {
                let resolved = dir.replace("${workspaceFolder}", &std::env::current_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default());
                cmd.current_dir(&resolved);
            }
            cmd.spawn().map_err(|e| format!("Failed: {}", e))?;
        }
        Ok(())
    };

    spawn()?;

    // In background, find the new console window and position it to the right
    std::thread::spawn(move || {
        for _i in 0..40 { // 40 * 150ms = 6 seconds
            std::thread::sleep(std::time::Duration::from_millis(150));
            unsafe {
                let hwnd = GetForegroundWindow();
                if hwnd != 0 && hwnd != cs_hwnd {
                    let mut r = Rect { left: 0, top: 0, right: 0, bottom: 0 };
                    GetWindowRect(hwnd, &mut r);
                    let w = r.right - r.left;
                    // Only position if it looks like a console (~same height or smaller)
                    if w > 200 {
                        let cs_w = cs_rect.right - cs_rect.left;
                        let cs_h = cs_rect.bottom - cs_rect.top;
                        let term_w = cs_w * 75 / 100;
                        SetWindowPos(hwnd, 0, cs_rect.right - 7, cs_rect.top, term_w, cs_h, 0x0014);
                        // Track live-server terminals
                        if task_type_clone == "live-server" {
                            init_live_terminals();
                            let mut terms = LIVE_TERMINALS.lock().unwrap();
                            terms.as_mut().unwrap().entry(workspace_name_clone.clone()).or_default().push(hwnd);
                        }
                        // Open browser if URL is specified
                        if let Some(ref open_url) = url_clone {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            let _ = std::process::Command::new("cmd")
                                .args(["/c", "start", open_url])
                                .spawn();
                        }
                        break;
                    }
                }
            }
        }
    });

    Ok(())
}


#[tauri::command]
fn check_prompts_folder(workspace_path: String) -> Result<bool, String> {
    let ws_file = Path::new(&workspace_path);
    let content = fs::read_to_string(ws_file)
        .map_err(|e| format!("Cannot read: {}", e))?;
    let json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    let folders = json["folders"].as_array()
        .ok_or("No folders array")?;

    let has_prompts = folders.iter().any(|f| {
        f.get("name").and_then(|n| n.as_str()) == Some("prompts")
    });

    Ok(has_prompts)
}

#[tauri::command]
fn toggle_prompts_folder(workspace_path: String) -> Result<bool, String> {
    // Build the user-specific prompts path: %APPDATA%\Code\User\prompts
    let appdata = std::env::var("APPDATA")
        .map_err(|e| format!("Cannot get APPDATA: {}", e))?;
    let prompts_path = Path::new(&appdata)
        .join("Code")
        .join("User")
        .join("prompts");
    let prompts_path_str = prompts_path.to_string_lossy().to_string();

    let ws_file = Path::new(&workspace_path);
    let content = fs::read_to_string(ws_file)
        .map_err(|e| format!("Cannot read: {}", e))?;
    let mut json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    let folders = json["folders"].as_array_mut()
        .ok_or("No folders array")?;

    // Check if prompts folder already exists
    let existing = folders.iter().position(|f| {
        f.get("name").and_then(|n| n.as_str()) == Some("prompts")
    });

    if let Some(idx) = existing {
        // Remove it
        folders.remove(idx);
        let new_content = serde_json::to_string_pretty(&json)
            .map_err(|e| format!("Cannot serialize: {}", e))?;
        fs::write(ws_file, new_content)
            .map_err(|e| format!("Cannot write: {}", e))?;
        Ok(false)
    } else {
        // Add it
        folders.push(serde_json::json!({
            "name": "prompts",
            "path": prompts_path_str
        }));
        let new_content = serde_json::to_string_pretty(&json)
            .map_err(|e| format!("Cannot serialize: {}", e))?;
        fs::write(ws_file, new_content)
            .map_err(|e| format!("Cannot write: {}", e))?;
        Ok(true)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            scan_workspaces,
            launch_workspace,
            get_workspace_color,
            set_workspace_color,
            remove_workspace_color,
            create_workspace,
            check_update,
            download_and_install,
            check_open_status,
            start_workspace_monitor,
            stop_workspace_monitor,
            get_scan_info,
            focus_workspace,
            minimize_workspace,
            check_prompts_folder,
            toggle_prompts_folder,
            get_workspace_tasks,
            run_task,
            toggle_live_terminal,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
