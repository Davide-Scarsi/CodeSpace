use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".vscode/tasks.json".into());
    let ws = std::path::Path::new(&path).parent().and_then(|p| p.parent())
        .map(|p| p.to_string_lossy().to_string()).unwrap_or_default();

    let json: serde_json::Value = match std::fs::read_to_string(&path) {
        Ok(c) => serde_json::from_str(&c).unwrap_or_default(),
        Err(e) => { eprintln!("FAIL: {}", e); return; }
    };
    let empty_arr = vec![];
    let tasks = json["tasks"].as_array().unwrap_or(&empty_arr);
    let mut ok = 0; let mut bad = 0;
    println!("=== {} tasks ===\n", tasks.len());

    for t in tasks {
        let label = t["label"].as_str().unwrap_or("?");
        // Skip tasks that are not testable (real dev server, etc.)
        if label.contains("Avvia CodeSpace") { continue; }
        let cmd = t["command"].as_str().unwrap_or("");
        let args: Vec<String> = t["args"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let task_type = t["codeSpace"]["taskType"].as_str().unwrap_or("default");
        let raw_cwd = t["options"]["cwd"].as_str().unwrap_or("");
        let cwd = if raw_cwd.is_empty() { None } else { Some(raw_cwd.replace("${workspaceFolder}", &ws)) };

        let start = Instant::now();
        match run_one(cmd, &args, cwd.as_deref()) {
            Ok(output) => {
                let ms = start.elapsed().as_millis();
                let failures = check(label, &output, task_type);
                if failures.is_empty() {
                    ok += 1;
                    println!("   OK  {}  ({}ms, {}b)", label, ms, output.len());
                } else {
                    bad += 1;
                    println!("  FAIL {}  ({}ms)  missing: {:?}", label, ms, failures);
                    for line in output.lines().take(4) { println!("       {}", line); }
                }
            }
            Err(e) => {
                bad += 1;
                println!("  CRASH {}  {}", label, e);
            }
        }
    }
    println!("\n=== {} OK / {} FAIL / {} total ===", ok, bad, ok + bad);
    std::process::exit(if bad > 0 { 1 } else { 0 });
}

fn check(_label: &str, output: &str, task_type: &str) -> Vec<&'static str> {
    let lo = output.to_lowercase();
    let mut miss = Vec::new();
    let expect = |miss: &mut Vec<_>, cond: bool, msg: &'static str| { if !cond { miss.push(msg); } };

    expect(&mut miss, !lo.contains("parsererror") && !lo.contains("terminatore mancante"), "no-parse-error");
    expect(&mut miss, !lo.contains("percorso non valido") && !lo.contains("non è un percorso valido"), "no-bad-path");
    expect(&mut miss, !lo.contains("non riconosciuto come nome di cmdlet"), "no-cmdlet-err");
    expect(&mut miss, output.len() > 20, "has-output");

    match task_type {
        "sync" => { expect(&mut miss, lo.contains("[ok]"), "has-[OK]"); expect(&mut miss, lo.contains("[1/") || lo.contains("[2/"), "has-progress"); }
        "push" => { expect(&mut miss, lo.contains("push"), "has-push"); expect(&mut miss, lo.contains("[ok]"), "has-[OK]"); }
        "upload" => { expect(&mut miss, lo.contains("[upload]"), "has-[UPLOAD]"); expect(&mut miss, lo.contains("[ok]"), "has-[OK]"); }
        "live-server" => { expect(&mut miss, output.len() > 80, "has-output"); }
        "php-server" => { expect(&mut miss, lo.contains("started") || lo.contains("listening"), "has-started"); }
        "npm" => { expect(&mut miss, lo.contains("npm") || lo.contains("node"), "has-node"); }
        "ftp-mount" => { expect(&mut miss, lo.contains("[ok]") || output.len() > 100, "has-output"); }
        "default" => { expect(&mut miss, output.len() > 40, "has-output"); }
        _ => { expect(&mut miss, output.len() > 40, "has-output"); }
    }
    miss
}

fn run_one(cmd_str: &str, args: &[String], cwd: Option<&str>) -> Result<String, String> {
    let (exe, a): (&str, Vec<String>) = if cmd_str == "echo" {
        let mut v = vec!["/c".into(), "echo".into()]; v.extend(args.iter().cloned()); ("cmd", v)
    } else {
        (cmd_str, args.to_vec())
    };

    let is_ps = exe.eq_ignore_ascii_case("powershell") || exe.eq_ignore_ascii_case("pwsh");
    let (exe_final, final_args) = if is_ps && !a.is_empty() {
        let mut v = a;
        let last_is_flag = v.last().map(|x| x.starts_with('-')).unwrap_or(false);
        let last_is_file = v.windows(2).any(|w| w[0].eq_ignore_ascii_case("-File"));
        if !last_is_flag && !last_is_file {
            let i = v.len() - 1;
            if v[i].ends_with('}') { v[i] = format!("{} *>&1 }}", &v[i][..v[i].len()-1]); }
            else { v[i] = format!("{} *>&1", v[i]); }
        }
        (exe, v)
    } else if a.is_empty() && exe.contains(' ') {
        ("powershell", vec!["-NoProfile".into(), "-Command".into(), format!("{} *>&1", exe)])
    } else {
        (exe, a.to_vec())
    };

    let mut child = Command::new(exe_final);
    for a in &final_args { child.arg(a); }
    child.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    #[cfg(windows)] { child.creation_flags(0x08000000); }
    if let Some(d) = cwd { child.current_dir(d); }

    let mut child = child.spawn().map_err(|e| format!("spawn: {}", e))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel();

    if let Some(out) = stdout {
        let tx = tx.clone();
        thread::spawn(move || { for line in BufReader::new(out).lines().flatten() { if tx.send(line).is_err() { break; } } });
    }
    if let Some(err) = stderr {
        let tx = tx.clone();
        thread::spawn(move || { for line in BufReader::new(err).lines().flatten() { if tx.send(line).is_err() { break; } } });
    }
    drop(tx); // last sender dropped → channel closes when threads finish

    let mut lines = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(line) => lines.push(line),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if child.try_wait().ok().flatten().is_some() { break; }
                if Instant::now() > deadline { break; }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.kill();
    Ok(lines.join("\n"))
}
