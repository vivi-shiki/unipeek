//! `serve`: one JSON request per line on stdin, one JSON reply per line on stdout. A host runs it
//! in a child process so a crashing graphics driver cannot take the host down.
//!
//! Requests: `{"id": any, "op": "thumb", "source": "a.unitypackage#Assets/x.prefab", "out": "t.webp",
//! "size": 512, "view": "3q", "background": "transparent"}`, `{"op": "info", "source": ...}` and
//! `{"op": "version"}`. Every reply echoes `id` and has `status`: ok, degraded, unsupported or error.
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
            match Scene::load(Input::from_location(&source), &LoadOptions::default()) {
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
            match thumb::write_thumbnail(gpu, &source, Path::new(&out), &opts) {
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

fn summary(id: Value, report: &Report) -> Value {
    let reasons: Vec<Value> = report.degrades.iter().filter_map(|d| serde_json::to_value(d).ok()?.get("kind").cloned()).collect();
    let missing: Vec<String> = report.missing.iter().filter(|m| matches!(m.usage, Usage::Prefab | Usage::Mesh)).map(|m| m.guid.to_string()).collect();
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
