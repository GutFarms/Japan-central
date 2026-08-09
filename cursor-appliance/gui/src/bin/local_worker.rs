//! Lightweight local appliance worker — no Cursor cloud connection.
//!
//! Takes over when the My Machines cloud worker stops: local health endpoints,
//! status heartbeats, and a small on-disk job queue.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Job {
    #[serde(default)]
    id: String,
    kind: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    created_at: String,
}

#[derive(Debug, Clone, Serialize)]
struct Status {
    mode: &'static str,
    name: String,
    worker_dir: String,
    listen: String,
    uptime_secs: u64,
    jobs_done: u64,
    jobs_failed: u64,
    queue_incoming: usize,
    last_job: String,
    cloud: &'static str,
}

struct Shared {
    name: String,
    worker_dir: PathBuf,
    data_dir: PathBuf,
    listen: String,
    started: Instant,
    jobs_done: AtomicU64,
    jobs_failed: AtomicU64,
    last_job: Mutex<String>,
    allow_prefixes: Vec<String>,
    stop: AtomicBool,
}

fn main() {
    let mut listen = env::var("CURSOR_APPLIANCE_LOCAL_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8734".into());
    let mut worker_dir = env::var("CURSOR_APPLIANCE_WORKER_DIR").unwrap_or_default();
    let mut data_dir = env::var("CURSOR_APPLIANCE_DATA_DIR").unwrap_or_default();
    let mut name = env::var("CURSOR_APPLIANCE_NAME").unwrap_or_else(|_| "cursor-appliance".into());

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => {
                if let Some(v) = args.next() {
                    listen = v;
                }
            }
            "--worker-dir" => {
                if let Some(v) = args.next() {
                    worker_dir = v;
                }
            }
            "--data-dir" => {
                if let Some(v) = args.next() {
                    data_dir = v;
                }
            }
            "--name" => {
                if let Some(v) = args.next() {
                    name = v;
                }
            }
            "-h" | "--help" => {
                print_help();
                return;
            }
            other => {
                eprintln!("unknown arg: {other}");
                print_help();
                std::process::exit(2);
            }
        }
    }

    if worker_dir.is_empty() {
        worker_dir = env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".into());
    }
    if data_dir.is_empty() {
        data_dir = PathBuf::from(&worker_dir)
            .join("cursor-appliance")
            .join("data")
            .display()
            .to_string();
        if !Path::new(&data_dir).exists() {
            data_dir = PathBuf::from(&worker_dir).join("data").display().to_string();
        }
    }

    let allow_prefixes = env::var("CURSOR_APPLIANCE_LOCAL_ALLOW")
        .unwrap_or_else(|_| {
            "git status,git diff,git log,git branch,cargo check,cargo test,ls,pwd,uname".into()
        })
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    let data_path = PathBuf::from(&data_dir);
    for sub in ["local-queue/incoming", "local-queue/done", "local-queue/failed"] {
        let _ = fs::create_dir_all(data_path.join(sub));
    }

    let shared = Arc::new(Shared {
        name: name.clone(),
        worker_dir: PathBuf::from(&worker_dir),
        data_dir: data_path.clone(),
        listen: listen.clone(),
        started: Instant::now(),
        jobs_done: AtomicU64::new(0),
        jobs_failed: AtomicU64::new(0),
        last_job: Mutex::new(String::new()),
        allow_prefixes,
        stop: AtomicBool::new(false),
    });

    // Heartbeat + queue processor
    let bg = Arc::clone(&shared);
    thread::spawn(move || background_loop(bg));

    let listener = TcpListener::bind(&listen).unwrap_or_else(|e| {
        eprintln!("error: bind {listen}: {e}");
        std::process::exit(1);
    });
    listener
        .set_nonblocking(false)
        .expect("blocking listener");

    println!("local worker listening on http://{listen}");
    println!("  name:       {name}");
    println!("  worker_dir: {worker_dir}");
    println!("  data_dir:   {data_dir}");
    println!("  mode:       local (no Cursor cloud)");

    write_heartbeat(&shared);

    for conn in listener.incoming() {
        if shared.stop.load(Ordering::Relaxed) {
            break;
        }
        match conn {
            Ok(stream) => {
                let s = Arc::clone(&shared);
                thread::spawn(move || {
                    if let Err(err) = handle_client(stream, &s) {
                        eprintln!("request error: {err}");
                    }
                });
            }
            Err(err) => eprintln!("accept error: {err}"),
        }
    }
}

fn print_help() {
    eprintln!(
        "cursor-local-worker — lightweight offline takeover worker\n\n\
         Usage:\n  cursor-local-worker [--listen ADDR] [--worker-dir DIR] [--data-dir DIR] [--name NAME]\n\n\
         Env:\n  CURSOR_APPLIANCE_LOCAL_ADDR   default 127.0.0.1:8734\n  CURSOR_APPLIANCE_LOCAL_ALLOW  comma-separated shell prefixes\n"
    );
}

fn background_loop(shared: Arc<Shared>) {
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            break;
        }
        write_heartbeat(&shared);
        process_one_job(&shared);
        thread::sleep(Duration::from_millis(750));
    }
}

fn write_heartbeat(shared: &Shared) {
    let path = shared.data_dir.join("local-worker-heartbeat.json");
    let body = json!({
        "ts": Utc::now().to_rfc3339(),
        "name": shared.name,
        "listen": shared.listen,
        "uptime_secs": shared.started.elapsed().as_secs(),
        "mode": "local",
    });
    let _ = fs::write(path, body.to_string());
}

fn incoming_dir(shared: &Shared) -> PathBuf {
    shared.data_dir.join("local-queue/incoming")
}

fn process_one_job(shared: &Shared) {
    let dir = incoming_dir(shared);
    let Ok(mut entries) = fs::read_dir(&dir) else {
        return;
    };
    let mut files = Vec::new();
    while let Some(Ok(entry)) = entries.next() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            files.push(path);
        }
    }
    files.sort();
    let Some(path) = files.into_iter().next() else {
        return;
    };

    let raw = match fs::read_to_string(&path) {
        Ok(v) => v,
        Err(_) => return,
    };
    let _ = fs::remove_file(&path);
    let job: Job = match serde_json::from_str(&raw) {
        Ok(j) => j,
        Err(err) => {
            shared.jobs_failed.fetch_add(1, Ordering::Relaxed);
            let _ = fs::write(
                shared
                    .data_dir
                    .join("local-queue/failed")
                    .join(format!("bad-{}.json", Utc::now().timestamp_millis())),
                json!({"error": err.to_string(), "raw": raw}).to_string(),
            );
            return;
        }
    };

    let result = run_job(shared, &job);
    let dest_dir = if result.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        shared.jobs_done.fetch_add(1, Ordering::Relaxed);
        shared.data_dir.join("local-queue/done")
    } else {
        shared.jobs_failed.fetch_add(1, Ordering::Relaxed);
        shared.data_dir.join("local-queue/failed")
    };
    let dest = dest_dir.join(format!("{}.json", job.id));
    let _ = fs::write(
        dest,
        json!({"job": job, "result": result, "finished_at": Utc::now().to_rfc3339()}).to_string(),
    );
    if let Ok(mut last) = shared.last_job.lock() {
        *last = format!(
            "{} ({})",
            result
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("job"),
            result
                .get("ok")
                .and_then(|v| v.as_bool())
                .map(|ok| if ok { "ok" } else { "fail" })
                .unwrap_or("?")
        );
    }
}

fn run_job(shared: &Shared, job: &Job) -> serde_json::Value {
    match job.kind.as_str() {
        "note" => {
            let journal = shared.data_dir.join("local-journal.log");
            let line = format!(
                "{} [{}] {}\n",
                Utc::now().to_rfc3339(),
                job.id,
                job.note
            );
            if let Ok(mut f) = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&journal)
            {
                let _ = f.write_all(line.as_bytes());
            }
            json!({"ok": true, "summary": format!("note recorded ({})", job.id)})
        }
        "inspect" => {
            let output = Command::new("git")
                .args(["status", "--short", "--branch"])
                .current_dir(&shared.worker_dir)
                .output();
            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    json!({"ok": out.status.success(), "summary": "git status", "stdout": stdout})
                }
                Err(err) => json!({"ok": false, "summary": format!("inspect failed: {err}")}),
            }
        }
        "shell" => {
            let cmd = job.command.trim();
            if cmd.is_empty() {
                return json!({"ok": false, "summary": "empty command"});
            }
            if !shared
                .allow_prefixes
                .iter()
                .any(|p| cmd == p || cmd.starts_with(&format!("{p} ")))
            {
                return json!({
                    "ok": false,
                    "summary": "command rejected by allowlist",
                    "command": cmd,
                    "allow": shared.allow_prefixes,
                });
            }
            let output = if cfg!(windows) {
                Command::new("cmd")
                    .args(["/C", cmd])
                    .current_dir(&shared.worker_dir)
                    .output()
            } else {
                Command::new("bash")
                    .args(["-lc", cmd])
                    .current_dir(&shared.worker_dir)
                    .output()
            };
            match output {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                    json!({
                        "ok": out.status.success(),
                        "summary": format!("shell: {cmd}"),
                        "stdout": stdout,
                        "stderr": stderr,
                        "code": out.status.code(),
                    })
                }
                Err(err) => json!({"ok": false, "summary": format!("spawn failed: {err}")}),
            }
        }
        other => json!({"ok": false, "summary": format!("unknown kind: {other}")}),
    }
}

fn queue_len(shared: &Shared) -> usize {
    fs::read_dir(incoming_dir(shared))
        .map(|it| {
            it.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
                .count()
        })
        .unwrap_or(0)
}

fn handle_client(mut stream: TcpStream, shared: &Shared) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    if n == 0 {
        return Ok(());
    }
    let req = String::from_utf8_lossy(&buf[..n]);
    let mut lines = req.lines();
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("GET");
    let path = parts.next().unwrap_or("/");

    let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
    let body = &req[body_start.min(req.len())..];

    match (method, path) {
        ("GET", "/healthz") | ("GET", "/readyz") => {
            write_response(&mut stream, 200, "text/plain", "ok\n")
        }
        ("GET", "/metrics") => {
            let body = format!(
                "local_worker_up 1\nlocal_worker_uptime_seconds {}\nlocal_worker_jobs_done {}\nlocal_worker_jobs_failed {}\nlocal_worker_queue_incoming {}\n",
                shared.started.elapsed().as_secs(),
                shared.jobs_done.load(Ordering::Relaxed),
                shared.jobs_failed.load(Ordering::Relaxed),
                queue_len(shared),
            );
            write_response(&mut stream, 200, "text/plain; version=0.0.4", &body)
        }
        ("GET", "/status") => {
            let last = shared
                .last_job
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default();
            let status = Status {
                mode: "local",
                name: shared.name.clone(),
                worker_dir: shared.worker_dir.display().to_string(),
                listen: shared.listen.clone(),
                uptime_secs: shared.started.elapsed().as_secs(),
                jobs_done: shared.jobs_done.load(Ordering::Relaxed),
                jobs_failed: shared.jobs_failed.load(Ordering::Relaxed),
                queue_incoming: queue_len(shared),
                last_job: last,
                cloud: "disconnected",
            };
            let body = serde_json::to_string_pretty(&status).unwrap_or_else(|_| "{}".into());
            write_response(&mut stream, 200, "application/json", &body)
        }
        ("POST", "/v1/jobs") => {
            let mut job: Job = match serde_json::from_str(body) {
                Ok(j) => j,
                Err(err) => {
                    let resp = json!({"queued": false, "error": format!("invalid job json: {err}")});
                    return write_response(
                        &mut stream,
                        400,
                        "application/json",
                        &resp.to_string(),
                    );
                }
            };
            if job.id.is_empty() {
                job.id = format!("job-{}", Utc::now().timestamp_millis());
            }
            if job.created_at.is_empty() {
                job.created_at = Utc::now().to_rfc3339();
            }
            let path = incoming_dir(shared).join(format!("{}.json", job.id));
            if let Err(err) = fs::write(
                &path,
                serde_json::to_string_pretty(&job).unwrap_or_default(),
            ) {
                let resp = json!({"queued": false, "error": err.to_string()});
                return write_response(&mut stream, 500, "application/json", &resp.to_string());
            }
            let resp = json!({"queued": true, "id": job.id});
            write_response(
                &mut stream,
                202,
                "application/json",
                &resp.to_string(),
            )
        }
        ("POST", "/v1/shutdown") => {
            shared.stop.store(true, Ordering::Relaxed);
            write_response(&mut stream, 200, "application/json", "{\"stopping\":true}\n")?;
            let _ = stream.shutdown(Shutdown::Both);
            std::process::exit(0);
        }
        _ => write_response(&mut stream, 404, "text/plain", "not found\n"),
    }
}

fn write_response(
    stream: &mut TcpStream,
    code: u16,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    let reason = match code {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let resp = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(resp.as_bytes()).map_err(|e| e.to_string())?;
    Ok(())
}
