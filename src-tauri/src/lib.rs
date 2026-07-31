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
pub struct TreeNode {
    pub name: String,
    pub full_path: String,
    pub is_workspace: bool,
    pub workspace_path: Option<String>,
    pub children: Vec<TreeNode>,
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

fn build_tree(workspaces: &[WorkspaceInfo]) -> Vec<TreeNode> {
    let mut root: HashMap<String, TreeNode> = HashMap::new();

    for ws in workspaces {
        let display = &ws.display_path;
        let path = Path::new(display);

        let components: Vec<String> = path
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .filter(|c| !c.is_empty())
            .collect();

        if components.is_empty() {
            continue;
        }

        let root_key = components[0].clone();
        let root_node = root.entry(root_key.clone()).or_insert_with(|| TreeNode {
            name: root_key.clone(),
            full_path: root_key.clone(),
            is_workspace: false,
            workspace_path: None,
            children: Vec::new(),
        });

        let mut current_children = &mut root_node.children;
        let mut current_path = root_key;

        for (i, comp) in components.iter().enumerate().skip(1) {
            current_path = format!("{}\\{}", current_path, comp);
            let is_last = i == components.len() - 1;

            let idx = current_children.iter().position(|n| n.name == *comp);

            if let Some(pos) = idx {
                current_children = &mut current_children[pos].children;
            } else {
                let new_node = TreeNode {
                    name: comp.clone(),
                    full_path: current_path.clone(),
                    is_workspace: is_last,
                    workspace_path: if is_last {
                        Some(ws.path.clone())
                    } else {
                        None
                    },
                    children: Vec::new(),
                };
                current_children.push(new_node);
                let last = current_children.len() - 1;
                current_children = &mut current_children[last].children;
            }
        }
    }

    let mut result: Vec<TreeNode> = root.into_values().collect();
    sort_tree(&mut result);
    result
}

fn sort_tree(nodes: &mut [TreeNode]) {
    nodes.sort_by(|a, b| {
        if a.is_workspace != b.is_workspace {
            a.is_workspace.cmp(&b.is_workspace)
        } else {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        }
    });
    for node in nodes.iter_mut() {
        sort_tree(&mut node.children);
    }
}

// ── Tauri Commands ─────────────────────────────────────────────

#[tauri::command]
fn scan_workspaces(app: tauri::AppHandle, force_full: bool) -> Vec<TreeNode> {
    let workspaces = if force_full {
        full_scan()
    } else if let Some(cache) = load_cache(&app) {
        quick_scan(&cache)
    } else {
        full_scan()
    };

    save_cache(&app, &workspaces);
    build_tree(&workspaces)
}

#[tauri::command]
fn launch_workspace(path: String) -> Result<(), String> {
    let result = Command::new("code").arg(&path).spawn();

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let fallbacks = vec![
                format!(
                    "{}\\Microsoft VS Code\\Code.exe",
                    std::env::var("LOCALAPPDATA").unwrap_or_default()
                ),
                format!(
                    "{}\\Microsoft VS Code\\Code.exe",
                    std::env::var("ProgramFiles").unwrap_or_default()
                ),
                format!(
                    "{}\\Microsoft VS Code\\Code.exe",
                    std::env::var("ProgramFiles(x86)").unwrap_or_default()
                ),
            ];

            for fb in &fallbacks {
                if Path::new(fb).exists() {
                    return Command::new(fb)
                        .arg(&path)
                        .spawn()
                        .map(|_| ())
                        .map_err(|e2| format!("Failed to launch: {}", e2));
                }
            }

            Err(format!(
                "VS Code not found. Make sure 'code' is in PATH. Error: {}",
                e
            ))
        }
    }
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
            get_scan_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
