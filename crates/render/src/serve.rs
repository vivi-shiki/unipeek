//! `serve`: one JSON request per line on stdin, one JSON reply per line on stdout. A host runs it
//! in a child process so a crashing graphics driver cannot take the host down.
//!
//! Requests: `{"id": any, "op": "thumb", "source": "a.unitypackage#Assets/x.prefab", "out": "t.webp",
//! "size": 512, "view": "3q", "background": "transparent"}`, `{"op": "info", "source": ...}` and
//! `{"op": "version"}`. Every reply echoes `id` and has `status`: ok, degraded, unsupported or error.
//! `thumb` and `info` take `"with": ["base.unitypackage", "D:/Assets"]` to find missing GUIDs.
use crate::{
    LoadOptions, Scene,
    gpu::Gpu,
    thumb::{self, Background, RENDERER_VERSION, ThumbOptions, View},
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    path::Path,
};
use unipeek_unity::{Input, Report, Status, prefab::Usage};

#[derive(serde::Deserialize)]
struct Request {
    #[serde(default)]
    id: Value,
    op: String,
    source: Option<String>,
    out: Option<String>,
    size: Option<u32>,
    view: Option<String>,
    background: Option<String>,
    /// Packages or folders to search for GUIDs the source does not contain.
    #[serde(default)]
    with: Vec<String>,
}

/// Serve requests until the input ends. Returns the process exit code (0 unless the output broke).
pub fn serve(input: impl BufRead, mut output: impl Write) -> i32 {
    let mut gpu: Option<Result<Gpu, String>> = None;
    for line in input.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle(req, &mut gpu),
            Err(e) => error(Value::Null, format!("invalid json request: {e}")),
        };
        if writeln!(output, "{reply}").and_then(|_| output.flush()).is_err() {
            return 1;
        }
    }
    0
}

fn handle(req: Request, gpu: &mut Option<Result<Gpu, String>>) -> Value {
    let id = req.id.clone();
    match req.op.as_str() {
        "version" => json!({"id": id, "status": "ok", "version": env!("CARGO_PKG_VERSION"), "renderer_version": RENDERER_VERSION}),
        "info" => {
            let Some(source) = req.source else { return error(id, "info needs a source".into()) };
            match Scene::load(Input::from_location(&source), &LoadOptions { decode_textures: false, resolver: resolver(&req.with), ..Default::default() }) {
                Ok(scene) => {
                    let mut reply = summary(id, &scene.report);
                    reply["report"] = serde_json::to_value(&scene.report).unwrap_or(Value::Null);
                    reply
                }
                Err(e) => failure(id, &e),
            }
        }
        "thumb" => {
            let (Some(source), Some(out)) = (req.source, req.out) else { return error(id, "thumb needs a source and an out path".into()) };
            let view = match req.view.as_deref().map(str::parse::<View>).transpose() {
                Ok(v) => v.unwrap_or(View::ThreeQuarter),
                Err(e) => return error(id, e),
            };
            let background = match req.background.as_deref().map(Background::parse).transpose() {
                Ok(b) => b.unwrap_or(Background::Transparent),
                Err(e) => return error(id, e),
            };
            if thumb::ImageFormat::from_path(&out).is_none() {
                return error(id, format!("{out} must end with .png or .webp"));
            }
            let gpu = match gpu.get_or_insert_with(|| Gpu::headless().map_err(|e| e.to_string())) {
                Ok(g) => g,
                Err(e) => return error(id, e.clone()),
            };
            let opts = ThumbOptions { size: req.size.unwrap_or(512).clamp(16, 4096), view, background };
            match thumb::write_thumbnail_with(gpu, &source, Path::new(&out), &opts, resolver(&req.with)) {
                Ok(report) => {
                    let mut reply = summary(id, &report);
                    reply["out"] = json!(out);
                    reply["backend"] = json!(gpu.describe());
                    reply["fallback"] = json!(gpu.fallback);
                    reply
                }
                Err(e) => failure(id, &e),
            }
        }
        other => error(id, format!("unknown op {other:?}")),
    }
}

fn resolver(with: &[String]) -> Option<std::sync::Arc<dyn unipeek_unity::Resolver>> {
    (!with.is_empty()).then(|| std::sync::Arc::new(unipeek_unity::Sources::new(with, Default::default())) as std::sync::Arc<dyn unipeek_unity::Resolver>)
}

fn summary(id: Value, report: &Report) -> Value {
    let reasons: Vec<Value> = report.degrades.iter().filter_map(|d| serde_json::to_value(d).ok()?.get("kind").cloned()).collect();
    let missing: Vec<String> = report.missing.iter().filter(|m| matches!(m.usage, Usage::Prefab | Usage::Mesh | Usage::Material)).map(|m| m.guid.to_string()).collect();
    json!({
        "id": id,
        "status": report.status,
        "prefab": report.prefab,
        "reasons": reasons,
        "missing_guids": missing,
    })
}

fn failure(id: Value, e: &crate::Error) -> Value {
    let mut v = error(id, e.to_string());
    v["status"] = json!(e.status());
    v
}

fn error(id: Value, message: String) -> Value {
    json!({"id": id, "status": Status::Error, "error": message})
}

/// Why a request to a `Worker` got no reply.
#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("could not start the worker: {0}")]
    Spawn(std::io::Error),
    /// The child exited or closed its output; it is started again on the next request.
    #[error("the worker died: {0}")]
    Died(String),
    /// No reply in time; the child was killed and is started again on the next request.
    #[error("the worker gave no reply within {0:?}")]
    Timeout(std::time::Duration),
}

struct Running {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    lines: std::sync::mpsc::Receiver<std::io::Result<String>>,
}

/// The host side of `serve`: a child process running the line protocol, restarted after a crash.
///
/// `command` builds the child's command line (for Snowcase `snowcase.exe --thumb-worker`); stdin and
/// stdout are piped by the worker. A request whose child dies or hangs returns an error, the
/// child is killed, and the next request starts a fresh one, so one bad package or driver crash
/// costs one thumbnail.
pub struct Worker {
    command: Box<dyn Fn() -> std::process::Command + Send>,
    running: Option<Running>,
    timeout: Option<std::time::Duration>,
    restarts: u32,
}

impl Worker {
    pub fn new(command: impl Fn() -> std::process::Command + Send + 'static) -> Worker {
        Worker { command: Box::new(command), running: None, timeout: None, restarts: 0 }
    }

    /// Kill the child when a reply takes longer than this. Off by default.
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Worker {
        self.timeout = Some(timeout);
        self
    }

    /// How many times a child died or was killed and had to be replaced.
    pub fn restarts(&self) -> u32 {
        self.restarts
    }

    /// Process ID of the running child, if one is running.
    pub fn pid(&self) -> Option<u32> {
        self.running.as_ref().map(|r| r.child.id())
    }

    /// Send one request (a JSON object, see the module docs) and wait for its reply.
    pub fn request(&mut self, request: &Value) -> Result<Value, WorkerError> {
        if self.running.is_none() {
            self.running = Some(self.spawn()?);
        }
        let running = self.running.as_mut().expect("spawned above");
        let line = format!("{request}\n");
        let sent = running.stdin.write_all(line.as_bytes()).and_then(|_| running.stdin.flush());
        let reply = match sent {
            Err(e) => Err(WorkerError::Died(e.to_string())),
            Ok(()) => {
                let received = match self.timeout {
                    Some(t) => running.lines.recv_timeout(t).map_err(|e| match e {
                        std::sync::mpsc::RecvTimeoutError::Timeout => WorkerError::Timeout(t),
                        std::sync::mpsc::RecvTimeoutError::Disconnected => WorkerError::Died("output closed".into()),
                    }),
                    None => running.lines.recv().map_err(|_| WorkerError::Died("output closed".into())),
                };
                received.and_then(|l| l.map_err(|e| WorkerError::Died(e.to_string())))
            }
        };
        match reply {
            Ok(line) => serde_json::from_str(&line).map_err(|e| {
                self.stop();
                WorkerError::Died(format!("unreadable reply: {e}"))
            }),
            Err(e) => {
                let status = self.stop();
                Err(match e {
                    WorkerError::Died(why) => WorkerError::Died(status.map_or(why.clone(), |s| format!("{why} ({s})"))),
                    other => other,
                })
            }
        }
    }

    /// Kill the child, if one is running. The next request starts a new one.
    pub fn kill(&mut self) {
        if self.running.is_some() {
            self.stop();
        }
    }

    fn spawn(&mut self) -> Result<Running, WorkerError> {
        use std::process::Stdio;
        let mut child = (self.command)().stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().map_err(WorkerError::Spawn)?;
        let stdin = child.stdin.take().ok_or_else(|| WorkerError::Spawn(std::io::Error::other("no stdin")))?;
        let stdout = child.stdout.take().ok_or_else(|| WorkerError::Spawn(std::io::Error::other("no stdout")))?;
        let (tx, lines) = std::sync::mpsc::channel();
        // A reader thread lets a hung child be detected with a timeout; it ends when the pipe closes.
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stdout).lines() {
                let failed = line.is_err();
                if tx.send(line).is_err() || failed {
                    break;
                }
            }
        });
        Ok(Running { child, stdin, lines })
    }

    /// Kill and reap the child; returns how it ended.
    fn stop(&mut self) -> Option<String> {
        let mut r = self.running.take()?;
        self.restarts += 1;
        let _ = r.child.kill();
        r.child.wait().ok().map(|s| s.to_string())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(mut r) = self.running.take() {
            // Closing stdin ends the serve loop; kill in case it is stuck.
            drop(r.stdin);
            if r.child.try_wait().ok().flatten().is_none() {
                let _ = r.child.kill();
            }
            let _ = r.child.wait();
        }
    }
}
