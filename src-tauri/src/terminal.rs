use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::Emitter;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

struct TermSession { stdin: Mutex<Box<dyn Write + Send>>, child_pid: u32, }
static TERMINALS: Mutex<Option<HashMap<String, TermSession>>> = Mutex::new(None);
fn init_terminals() { let mut g = TERMINALS.lock().unwrap(); if g.is_none() { *g = Some(HashMap::new()); } }

#[tauri::command]
pub fn terminal_spawn(app: tauri::AppHandle, terminal_id: String, command: String, mut args: Vec<String>, cwd: Option<String>) -> Result<(), String> {
    let is_ps = command.eq_ignore_ascii_case("powershell") || command.eq_ignore_ascii_case("pwsh");
    let (cmd_name, final_args): (String, Vec<String>) = if args.is_empty() && command.contains(' ') {
        ("powershell".into(), vec!["-NoProfile".into(), "-Command".into(), format!("{} *>&1", command)])
    } else {
        if is_ps && !args.is_empty() { let last = args.len() - 1; args[last] = format!("{} *>&1", args[last]); }
        (command, args)
    };
    let mut cmd = Command::new(&cmd_name);
    for a in &final_args { cmd.arg(a); }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::piped());
    #[cfg(windows)]
    { cmd.creation_flags(0x08000000); } // CREATE_NO_WINDOW
    if let Some(ref dir) = cwd { cmd.current_dir(dir); }
    let mut child = cmd.spawn().map_err(|e| format!("Spawn: {}", e))?;
    let pid = child.id();
    let stdout = child.stdout.take().ok_or("No stdout")?;
    let stderr = child.stderr.take().ok_or("No stderr")?;
    let stdin = child.stdin.take().ok_or("No stdin")?;
    let tid = terminal_id.clone(); let a = app.clone();
    std::thread::spawn(move || { let _ = child.wait(); let _ = a.emit("terminal-exit", serde_json::json!({"terminalId":tid})); });
    let tid2 = terminal_id.clone(); let a2 = app.clone();
    std::thread::spawn(move || { for line in BufReader::new(stdout).lines().flatten() { let _ = a2.emit("terminal-output", serde_json::json!({"terminalId":tid2,"data":line+"\r\n"})); } });
    let tid3 = terminal_id.clone(); let a3 = app.clone();
    std::thread::spawn(move || { for line in BufReader::new(stderr).lines().flatten() { let _ = a3.emit("terminal-output", serde_json::json!({"terminalId":tid3,"data":line+"\r\n"})); } });
    init_terminals();
    TERMINALS.lock().unwrap().as_mut().unwrap().insert(terminal_id, TermSession{stdin:Mutex::new(Box::new(stdin)), child_pid: pid});
    Ok(())
}

#[tauri::command]
pub fn terminal_write(terminal_id: String, data: String) -> Result<(), String> {
    let g = TERMINALS.lock().unwrap(); let t = g.as_ref().ok_or("No terminals")?;
    let s = t.get(&terminal_id).ok_or("Not found")?;
    s.stdin.lock().unwrap().write_all(data.as_bytes()).map_err(|e| format!("Write: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn terminal_kill(terminal_id: String) -> Result<(), String> {
    init_terminals();
    let mut g = TERMINALS.lock().unwrap();
    if let Some(s) = g.as_mut().unwrap().remove(&terminal_id) {
        // Kill entire process tree (e.g. PowerShell + child PHP)
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &s.child_pid.to_string()])
            .stdout(Stdio::null()).stderr(Stdio::null())
            .spawn();
    }
    Ok(())
}
