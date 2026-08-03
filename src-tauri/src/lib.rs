use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
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

// ── Peacock Color ─────────────────────────────────────────────

/// Reads the Peacock color from a workspace. Tries in order:
// ── Color Store (local cache, no Peacock dependency) ──────────

fn colors_cache_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    Path::new(&appdata).join("CodeSpace").join("colors.json")
}

fn load_colors_cache() -> HashMap<String, String> {
    let path = colors_cache_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content) {
            return map;
        }
    }
    HashMap::new()
}

fn save_colors_cache(cache: &HashMap<String, String>) {
    let path = colors_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, serde_json::to_string_pretty(cache).unwrap_or_default());
}

/// Read workspace color: local cache → .vscode/settings.json → .code-workspace
fn read_workspace_color(workspace_path: &str) -> Option<String> {
    // 1. Local CodeSpace cache
    let cache = load_colors_cache();
    if let Some(color) = cache.get(workspace_path) {
        return Some(color.clone());
    }

    let ws_file = Path::new(workspace_path);

    // 2. .vscode/settings.json workbench.colorCustomizations
    let project_dir = resolve_project_dir(ws_file);
    let settings_path = project_dir.join(".vscode").join("settings.json");
    if let Ok(content) = fs::read_to_string(&settings_path) {
        if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(wb) = settings.get("workbench.colorCustomizations") {
                if let Some(color) = extract_color_from_customizations(wb) {
                    return Some(color);
                }
            }
        }
    }

    // 3. .code-workspace file (backward compat with Peacock)
    if let Ok(content) = fs::read_to_string(ws_file) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(settings) = json.get("settings") {
                if let Some(wb) = settings.get("workbench.colorCustomizations") {
                    if let Some(color) = extract_color_from_customizations(wb) {
                        return Some(color);
                    }
                }
            }
        }
    }

    None
}

fn resolve_project_dir(ws_file: &Path) -> PathBuf {
    if let Ok(content) = fs::read_to_string(ws_file) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(folders) = json.get("folders").and_then(|v| v.as_array()) {
                if let Some(first) = folders.first() {
                    if let Some(folder_path) = first.get("path").and_then(|v| v.as_str()) {
                        let base = ws_file.parent().unwrap_or(Path::new("."));
                        return base.join(folder_path);
                    }
                }
            }
        }
    }
    ws_file.parent().unwrap_or(Path::new(".")).to_path_buf()
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
    // Batch-check all workspaces with a single wmic call
    let open_paths = get_open_workspace_paths();
    for ws in workspaces.iter_mut() {
        ws.is_open = open_paths.iter().any(|p| p == &ws.path);
    }
}

fn get_open_workspace_paths() -> Vec<String> {
    let output = Command::new("wmic")
        .args(["process", "where", "name='Code.exe'", "get", "commandline", "/format:csv"])
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .filter(|line| line.contains(".code-workspace"))
                .filter_map(|line| {
                    // Extract the .code-workspace path from the command line
                    if let Some(start) = line.find('"') {
                        let rest = &line[start + 1..];
                        if let Some(end) = rest.find(".code-workspace") {
                            return Some(format!("{}{}", &rest[..end], ".code-workspace"));
                        }
                    }
                    None
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
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
fn launch_workspace(path: String) -> Result<(), String> {
    // Use Windows shell "start" to open .code-workspace with associated program
    let result = Command::new("cmd")
        .args(["/c", "start", "", &path])
        .spawn();

    match result {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Failed to launch: {}", e)),
    }
}

#[tauri::command]
fn get_workspace_color(workspace_path: String) -> Option<String> {
    read_workspace_color(&workspace_path)
}

#[tauri::command]
fn set_workspace_color(workspace_path: String, color: String) -> Result<(), String> {
    let ws_file = Path::new(&workspace_path);
    let project_dir = resolve_project_dir(ws_file);

    // ── Write to .vscode/settings.json ──
    let vscode_dir = project_dir.join(".vscode");
    fs::create_dir_all(&vscode_dir)
        .map_err(|e| format!("Cannot create .vscode dir: {}", e))?;
    let settings_path = vscode_dir.join("settings.json");
    update_color_in_json_file(&settings_path, &color)?;

    // ── Write to .code-workspace file ──
    if let Ok(content) = fs::read_to_string(ws_file) {
        if let Ok(mut ws_json) = serde_json::from_str::<serde_json::Value>(&content) {
            if ws_json.get("settings").is_none() {
                ws_json["settings"] = serde_json::json!({});
            }
            let s = &mut ws_json["settings"];
            write_full_color_overrides(s, &color);
            let ws_content = serde_json::to_string_pretty(&ws_json)
                .map_err(|e| format!("Cannot serialize: {}", e))?;
            fs::write(ws_file, ws_content)
                .map_err(|e| format!("Cannot write workspace file: {}", e))?;
        }
    }

    // ── Save to local cache ──
    let mut cache = load_colors_cache();
    cache.insert(workspace_path, color);
    save_colors_cache(&cache);

    Ok(())
}

fn update_color_in_json_file(path: &Path, color: &str) -> Result<(), String> {
    let mut json: serde_json::Value = if path.exists() {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {}", path.display(), e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    write_full_color_overrides(&mut json, color);
    let out = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    fs::write(path, out)
        .map_err(|e| format!("Cannot write {}: {}", path.display(), e))
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
    let project_dir = resolve_project_dir(ws_file);

    // 1. Remove from .vscode/settings.json
    let settings_path = project_dir.join(".vscode").join("settings.json");
    if settings_path.exists() {
        if let Ok(content) = fs::read_to_string(&settings_path) {
            if let Ok(mut settings) = serde_json::from_str::<serde_json::Value>(&content) {
                settings.as_object_mut().map(|o| { o.remove("peacock.color"); o.remove("workbench.colorCustomizations"); });
                let json = serde_json::to_string_pretty(&settings)
                    .map_err(|e| format!("Cannot serialize: {}", e))?;
                fs::write(&settings_path, json)
                    .map_err(|e| format!("Cannot write settings.json: {}", e))?;
            }
        }
    }

    // 2. Remove from .code-workspace file
    if let Ok(content) = fs::read_to_string(ws_file) {
        if let Ok(mut ws_json) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(settings) = ws_json.get_mut("settings") {
                settings.as_object_mut().map(|o| { o.remove("peacock.color"); o.remove("workbench.colorCustomizations"); });
                let ws_content = serde_json::to_string_pretty(&ws_json)
                    .map_err(|e| format!("Cannot serialize: {}", e))?;
                fs::write(ws_file, ws_content)
                    .map_err(|e| format!("Cannot write workspace file: {}", e))?;
            }
        }
    }

    // 3. Remove from local cache
    let mut cache = load_colors_cache();
    cache.remove(&workspace_path);
    save_colors_cache(&cache);

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            scan_workspaces,
            launch_workspace,
            get_workspace_color,
            set_workspace_color,
            remove_workspace_color,
            create_workspace,
            check_update,
            download_and_install,
            get_scan_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
