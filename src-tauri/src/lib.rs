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
/// 1. .code-workspace file's "settings.peacock.color"
/// 2. .code-workspace file's "settings.workbench.colorCustomizations"
/// 3. .vscode/settings.json (from workspace file parent or resolved folder)
fn read_peacock_color(workspace_path: &str) -> Option<String> {
    let ws_file = Path::new(workspace_path);

    // --- Step 1 & 2: Read the .code-workspace file itself ---
    if let Ok(content) = fs::read_to_string(ws_file) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            // Try peacock.color in workspace settings
            if let Some(settings) = json.get("settings") {
                if let Some(color) = settings.get("peacock.color").and_then(|v| v.as_str()) {
                    let _ = log_debug(ws_file, &format!("found in .code-workspace settings.peacock.color: {}", color));
                    return Some(color.to_string());
                }
                if let Some(wb) = settings.get("workbench.colorCustomizations") {
                    if let Some(color) = extract_color_from_customizations(wb) {
                        let _ = log_debug(ws_file, &format!("found in .code-workspace settings.workbench: {}", color));
                        return Some(color);
                    }
                }
            }

            // Resolve project folder from "folders" array
            let project_dir = if let Some(folders) = json.get("folders").and_then(|v| v.as_array()) {
                if let Some(first) = folders.first() {
                    if let Some(folder_path) = first.get("path").and_then(|v| v.as_str()) {
                        let base = ws_file.parent().unwrap_or(Path::new("."));
                        let resolved = base.join(folder_path);
                        let _ = log_debug(ws_file, &format!("resolved folder from .code-workspace: {:?}", resolved));
                        resolved
                    } else {
                        ws_file.parent()?.to_path_buf()
                    }
                } else {
                    ws_file.parent()?.to_path_buf()
                }
            } else {
                ws_file.parent()?.to_path_buf()
            };

            // --- Step 3: .vscode/settings.json ---
            let settings_path = project_dir.join(".vscode").join("settings.json");
            let _ = log_debug(ws_file, &format!("checking settings.json: {:?}", settings_path));
            if let Ok(content) = fs::read_to_string(&settings_path) {
                if let Ok(settings) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(color) = settings.get("peacock.color").and_then(|v| v.as_str()) {
                        let _ = log_debug(ws_file, &format!("found in settings.json peacock.color: {}", color));
                        return Some(color.to_string());
                    }
                    if let Some(wb) = settings.get("workbench.colorCustomizations") {
                        if let Some(color) = extract_color_from_customizations(wb) {
                            let _ = log_debug(ws_file, &format!("found in settings.json workbench: {}", color));
                            return Some(color);
                        }
                    }
                }
            }
        }
    }

    let _ = log_debug(ws_file, "no color found");
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

fn log_debug(ws_file: &Path, msg: &str) -> std::io::Result<()> {
    let log_path = std::env::temp_dir().join("codespace_debug.log");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    use std::io::Write;
    writeln!(file, "[{}] {:?}: {}", chrono::Local::now().format("%H:%M:%S"), ws_file, msg)
}

fn populate_colors(workspaces: &mut [WorkspaceInfo]) {
    for ws in workspaces.iter_mut() {
        ws.color = read_peacock_color(&ws.path);
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

    populate_colors(&mut workspaces);
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
    read_peacock_color(&workspace_path)
}

#[tauri::command]
fn set_workspace_color(workspace_path: String, color: String) -> Result<(), String> {
    let ws_file = Path::new(&workspace_path);
    let project_dir = ws_file.parent().ok_or("Invalid workspace path")?;
    let vscode_dir = project_dir.join(".vscode");
    let settings_path = vscode_dir.join("settings.json");

    // Read existing settings or start fresh
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .map_err(|e| format!("Cannot read settings.json: {}", e))?;
        serde_json::from_str(&content).unwrap_or(serde_json::json!({}))
    } else {
        fs::create_dir_all(&vscode_dir)
            .map_err(|e| format!("Cannot create .vscode dir: {}", e))?;
        serde_json::json!({})
    };

    // Set peacock.color
    settings["peacock.color"] = serde_json::Value::String(color.clone());

    // Also set workbench colors like Peacock does
    if settings.get("workbench.colorCustomizations").is_none() {
        settings["workbench.colorCustomizations"] = serde_json::json!({});
    }
    let wb = &mut settings["workbench.colorCustomizations"];

    wb["titleBar.activeBackground"] = serde_json::Value::String(color.clone());
    wb["titleBar.activeForeground"] = serde_json::Value::String("#ffffff".into());
    wb["activityBar.background"] = serde_json::Value::String(color.clone());
    wb["activityBar.foreground"] = serde_json::Value::String("#ffffff".into());
    wb["statusBar.background"] = serde_json::Value::String(color.clone());
    wb["statusBar.foreground"] = serde_json::Value::String("#ffffff".into());

    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Cannot serialize: {}", e))?;
    fs::write(&settings_path, json)
        .map_err(|e| format!("Cannot write settings.json: {}", e))?;

    Ok(())
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
        .invoke_handler(tauri::generate_handler![
            scan_workspaces,
            launch_workspace,
            get_workspace_color,
            set_workspace_color,
            get_scan_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
