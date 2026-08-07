use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio, Child};
use std::sync::{Arc, Mutex};
use tauri::Emitter;

struct TermSession { stdin: Mutex<Box<dyn Write + Send>>, child: Arc<Mutex<Option<Child>>>, }
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
    if let Some(ref dir) = cwd { cmd.current_dir(dir); }
    let mut child = cmd.spawn().map_err(|e| format!("Spawn: {}", e))?;
    let stdout = child.stdout.take().ok_or("No stdout")?;
    let stderr = child.stderr.take().ok_or("No stderr")?;
    let stdin = child.stdin.take().ok_or("No stdin")?;
    let child_arc = Arc::new(Mutex::new(Some(child)));
    let child_clone = child_arc.clone();
    let tid_exit = terminal_id.clone(); let a_exit = app.clone();
    std::thread::spawn(move || { let c = child_clone.lock().unwrap().take(); if let Some(mut ch) = c { let _ = ch.wait(); } let _ = a_exit.emit("terminal-exit", serde_json::json!({"terminalId":tid_exit})); });
    let tid2 = terminal_id.clone(); let a2 = app.clone();
    std::thread::spawn(move || { for line in BufReader::new(stdout).lines().flatten() { let _ = a2.emit("terminal-output", serde_json::json!({"terminalId":tid2,"data":line+"\r\n"})); } });
    let tid3 = terminal_id.clone(); let a3 = app.clone();
    std::thread::spawn(move || { for line in BufReader::new(stderr).lines().flatten() { let _ = a3.emit("terminal-output", serde_json::json!({"terminalId":tid3,"data":line+"\r\n"})); } });
    init_terminals();
    TERMINALS.lock().unwrap().as_mut().unwrap().insert(terminal_id, TermSession{stdin:Mutex::new(Box::new(stdin)), child: child_arc});
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
        if let Ok(mut c) = s.child.lock() {
            if let Some(ref mut ch) = *c { let _ = ch.kill(); }
        }
    }
    Ok(())
}
