use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::beam::probabilistic_repair;
use crate::json::{JsonValue, parse_strict_json};
use crate::llm::{apply_patch_ops_utf8, build_llm_payload_json};
use crate::types::{Candidate, RepairAction, RepairOptions};

fn parse_jsonish(s: &str) -> Option<JsonValue> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(v) = parse_strict_json(trimmed) {
        return Some(v);
    }
    // Fallback: try to extract a JSON object/array substring.
    let start_obj = trimmed.find('{');
    let start_arr = trimmed.find('[');
    let start = match (start_obj, start_arr) {
        (None, None) => return None,
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (Some(a), Some(b)) => a.min(b),
    };
    let end_obj = trimmed.rfind('}');
    let end_arr = trimmed.rfind(']');
    let end = match (end_obj, end_arr) {
        (None, None) => return None,
        (Some(a), None) => a + 1,
        (None, Some(b)) => b + 1,
        (Some(a), Some(b)) => (a + 1).max(b + 1),
    };
    if start >= end || end > trimmed.len() {
        return None;
    }
    parse_strict_json(&trimmed[start..end]).ok()
}

fn split_command(cmd: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut chars = cmd.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            '\\' if in_double => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    out.push(cur);
                    cur = String::new();
                }
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn run_llm_command(cmd: &str, input: &str, timeout_ms: u64) -> Result<String, String> {
    let parts = split_command(cmd);
    if parts.is_empty() {
        return Err("llm_command is empty".to_string());
    }
    let program = &parts[0];
    let args = &parts[1..];

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("failed to spawn llm_command: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(input.as_bytes())
            .map_err(|e| format!("failed to write llm stdin: {e}"))?;
    }

    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "failed to capture llm stdout".to_string())?;

    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<u8>, String>>();
    std::thread::spawn(move || {
        let mut buf: Vec<u8> = Vec::new();
        let res = stdout
            .read_to_end(&mut buf)
            .map(|_| buf)
            .map_err(|e| format!("failed to read llm stdout: {e}"));
        let _ = tx.send(res);
    });

    let timeout = Duration::from_millis(timeout_ms);
    let poll = Duration::from_millis(5);
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if timeout_ms > 0 && start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("llm_command timeout".to_string());
                }
                std::thread::sleep(poll);
            }
            Err(e) => return Err(format!("failed to wait for llm_command: {e}")),
        }
    };

    if !status.success() {
        return Err(format!("llm_command exited non-zero: {}", status));
    }

    let out = rx
        .recv()
        .map_err(|_| "failed to receive llm stdout".to_string())??;
    Ok(String::from_utf8_lossy(&out).to_string())
}

fn get_field<'a>(obj: &'a [(String, JsonValue)], key: &str) -> Option<&'a JsonValue> {
    obj.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn trigger_reason(candidates: &[Candidate], opt: &RepairOptions) -> Option<String> {
    if !opt.allow_llm || opt.max_llm_calls_per_doc == 0 {
        return None;
    }
    if candidates.is_empty() {
        return Some("no_candidates".to_string());
    }
    if candidates[0].confidence < opt.llm_min_confidence {
        return Some("low_confidence".to_string());
    }
    None
}

pub fn maybe_llm_rerun(
    repaired_text: &str,
    base_repairs: &[RepairAction],
    candidates: &[Candidate],
    error_pos: Option<usize>,
    opt: &RepairOptions,
) -> Result<(Vec<Candidate>, usize, u128, Option<String>), String> {
    let reason = trigger_reason(candidates, opt);
    if reason.is_none() {
        return Ok((Vec::new(), 0, 0, None));
    }
    let Some(cmd) = opt.llm_command.as_deref() else {
        return Ok((Vec::new(), 0, 0, reason));
    };

    let payload = build_llm_payload_json(
        repaired_text,
        &opt.llm_mode,
        error_pos,
        opt.schema.as_ref(),
        None,
        5,
        1200,
    );
    let payload_str = payload.to_compact_string();

    let max_calls = opt.max_llm_calls_per_doc.max(1);
    let mut llm_calls: usize = 0;
    let mut llm_time_ms: u128 = 0;
    let mut parsed_obj: Option<Vec<(String, JsonValue)>> = None;
    for _ in 0..max_calls {
        llm_calls += 1;
        let t0 = Instant::now();
        let raw_res = run_llm_command(cmd, &payload_str, opt.llm_timeout_ms);
        let ms = t0.elapsed().as_millis();
        llm_time_ms += ms;

        let raw = match raw_res {
            Ok(s) => s,
            Err(_) => return Ok((Vec::new(), llm_calls, llm_time_ms, reason)),
        };
        let parsed = match parse_jsonish(&raw) {
            Some(v) => v,
            None => continue,
        };
        let obj = match parsed {
            JsonValue::Object(o) => o,
            _ => continue,
        };
        let mode = match get_field(&obj, "mode") {
            Some(JsonValue::String(s)) => s.as_str(),
            _ => "",
        };
        if mode != "patch_suggest" {
            continue;
        }
        parsed_obj = Some(obj);
        break;
    }

    let Some(parsed_obj) = parsed_obj else {
        return Ok((Vec::new(), llm_calls, llm_time_ms, reason));
    };

    let patches = match get_field(&parsed_obj, "patches") {
        Some(JsonValue::Array(a)) => a.clone(),
        _ => Vec::new(),
    };

    let mut out: Vec<Candidate> = Vec::new();
    for p in patches.into_iter().take(opt.top_k.max(1)) {
        let pobj = match p {
            JsonValue::Object(o) => o,
            _ => continue,
        };
        let patch_id = match get_field(&pobj, "patch_id") {
            Some(JsonValue::String(s)) => Some(s.clone()),
            _ => None,
        };
        let ops = match get_field(&pobj, "ops") {
            Some(JsonValue::Array(a)) => a.clone(),
            _ => Vec::new(),
        };

        let patched = match apply_patch_ops_utf8(repaired_text, &ops) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let mut patch_action = RepairAction::new("llm_patch_suggest", 1.5);
        patch_action.note = patch_id;
        let mut next_base: Vec<RepairAction> = base_repairs.to_vec();
        next_base.push(patch_action);
        out.extend(probabilistic_repair(&patched, opt, &next_base));
        if out.len() >= opt.top_k {
            break;
        }
    }

    Ok((out, llm_calls, llm_time_ms, reason))
}
