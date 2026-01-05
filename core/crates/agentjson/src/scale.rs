use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::json::{JsonValue, parse_strict_json};
use crate::parallel_scan;
use crate::tape::{
    Tape, TapeEntry, TapeTokenType, append_segment, parse_object_pair_segment, parse_strict_tape,
};
use crate::types::RepairOptions;

pub const SPLIT_NO_SPLIT: &str = "NO_SPLIT";
pub const SPLIT_ROOT_ARRAY_ELEMENTS: &str = "ROOT_ARRAY_ELEMENTS";
pub const SPLIT_ROOT_OBJECT_PAIRS: &str = "ROOT_OBJECT_PAIRS";

#[derive(Debug, Clone, PartialEq)]
pub struct SplitPlan {
    pub mode: String,
    pub elements: usize,
    pub structural_density: f64,
    pub chunk_count: usize,
}

fn is_ws(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\r' | b' ')
}

fn trim_ws(data: &[u8]) -> (usize, usize) {
    let mut start = 0usize;
    let mut end = data.len();
    // UTF-8 BOM
    if end >= 3 && &data[..3] == b"\xEF\xBB\xBF" {
        start = 3;
    }
    while start < end && is_ws(data[start]) {
        start += 1;
    }
    while end > start && is_ws(data[end - 1]) {
        end -= 1;
    }
    (start, end)
}

fn trim_span(data: &[u8], start: usize, end: usize) -> (usize, usize) {
    let mut s = start;
    let mut e = end;
    while s < e && is_ws(data[s]) {
        s += 1;
    }
    while e > s && is_ws(data[e - 1]) {
        e -= 1;
    }
    (s, e)
}

fn iter_root_array_element_spans_single(
    data: &[u8],
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    if start >= end || data.get(start) != Some(&b'[') || data.get(end - 1) != Some(&b']') {
        return spans;
    }

    let mut i = start + 1;
    while i < end && is_ws(data[i]) {
        i += 1;
    }
    if i >= end.saturating_sub(1) {
        return spans; // empty array
    }

    let mut elem_start = i;
    let mut in_string = false;
    let mut escape = false;
    let mut depth_brace: i64 = 0;
    let mut depth_bracket: i64 = 1; // root '[' already entered

    for i in (start + 1)..(end - 1) {
        let ch = data[i];
        if in_string {
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            continue;
        }

        if ch == b'"' {
            in_string = true;
            continue;
        }

        match ch {
            b'{' => depth_brace += 1,
            b'}' => depth_brace -= 1,
            b'[' => depth_bracket += 1,
            b']' => depth_bracket -= 1,
            _ => {}
        }

        if ch == b',' && depth_brace == 0 && depth_bracket == 1 {
            let elem_end = i;
            // trim whitespace around element
            let mut s = elem_start;
            let mut e = elem_end;
            while s < e && is_ws(data[s]) {
                s += 1;
            }
            while e > s && is_ws(data[e - 1]) {
                e -= 1;
            }
            if e > s {
                spans.push((s, e));
            }
            elem_start = i + 1;
        }
    }

    // last element
    let mut s = elem_start;
    let mut e = end - 1;
    while s < e && is_ws(data[s]) {
        s += 1;
    }
    while e > s && is_ws(data[e - 1]) {
        e -= 1;
    }
    if e > s {
        spans.push((s, e));
    }

    spans
}

fn iter_root_object_pair_spans_single(
    data: &[u8],
    start: usize,
    end: usize,
) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    if start >= end || data.get(start) != Some(&b'{') || data.get(end - 1) != Some(&b'}') {
        return spans;
    }

    let mut i = start + 1;
    while i < end && is_ws(data[i]) {
        i += 1;
    }
    if i >= end.saturating_sub(1) {
        return spans; // empty object
    }

    let mut pair_start = i;
    let mut in_string = false;
    let mut escape = false;
    let mut depth_brace: i64 = 1; // root '{' already entered
    let mut depth_bracket: i64 = 0;

    for i in (start + 1)..(end - 1) {
        let ch = data[i];
        if in_string {
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            continue;
        }

        if ch == b'"' {
            in_string = true;
            continue;
        }

        match ch {
            b'{' => depth_brace += 1,
            b'}' => depth_brace -= 1,
            b'[' => depth_bracket += 1,
            b']' => depth_bracket -= 1,
            _ => {}
        }

        if ch == b',' && depth_brace == 1 && depth_bracket == 0 {
            let (s, e) = trim_span(data, pair_start, i);
            if e > s {
                spans.push((s, e));
            }
            pair_start = i + 1;
        }
    }

    let (s, e) = trim_span(data, pair_start, end - 1);
    if e > s {
        spans.push((s, e));
    }

    spans
}

fn spans_from_commas(
    data: &[u8],
    start: usize,
    end: usize,
    commas: &[usize],
) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = start + 1;
    while i < end && is_ws(data[i]) {
        i += 1;
    }
    if i >= end.saturating_sub(1) {
        return spans;
    }
    let mut cur_start = i;
    for &comma_pos in commas {
        let (s, e) = trim_span(data, cur_start, comma_pos);
        if e > s {
            spans.push((s, e));
        }
        cur_start = comma_pos + 1;
    }
    let (s, e) = trim_span(data, cur_start, end - 1);
    if e > s {
        spans.push((s, e));
    }
    spans
}

fn parallel_workers(opt: &RepairOptions) -> usize {
    opt.parallel_workers.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
    })
}

fn iter_root_array_element_spans(
    data: &[u8],
    start: usize,
    end: usize,
    opt: &RepairOptions,
    force_single: bool,
) -> (Vec<(usize, usize)>, bool) {
    if start >= end || data.get(start) != Some(&b'[') || data.get(end - 1) != Some(&b']') {
        return (Vec::new(), false);
    }

    if force_single {
        return (
            iter_root_array_element_spans_single(data, start, end),
            false,
        );
    }

    let allow = allow_parallel_bool(opt);
    let workers = std::cmp::max(1usize, parallel_workers(opt));
    if workers < 2 || allow == Some(false) {
        return (
            iter_root_array_element_spans_single(data, start, end),
            false,
        );
    }

    let force = allow == Some(true);
    if !force && (end - start) < opt.parallel_threshold_bytes {
        return (
            iter_root_array_element_spans_single(data, start, end),
            false,
        );
    }

    let chunk_bytes = std::cmp::max(1usize, opt.parallel_chunk_bytes);
    match parallel_scan::find_root_array_commas(data, start, end, workers, chunk_bytes) {
        Ok(commas) => (spans_from_commas(data, start, end, &commas), true),
        Err(_) => (
            iter_root_array_element_spans_single(data, start, end),
            false,
        ),
    }
}

fn iter_root_object_pair_spans(
    data: &[u8],
    start: usize,
    end: usize,
    opt: &RepairOptions,
    force_single: bool,
) -> (Vec<(usize, usize)>, bool) {
    if start >= end || data.get(start) != Some(&b'{') || data.get(end - 1) != Some(&b'}') {
        return (Vec::new(), false);
    }

    if force_single {
        return (iter_root_object_pair_spans_single(data, start, end), false);
    }

    let allow = allow_parallel_bool(opt);
    let workers = std::cmp::max(1usize, parallel_workers(opt));
    if workers < 2 || allow == Some(false) {
        return (iter_root_object_pair_spans_single(data, start, end), false);
    }

    let force = allow == Some(true);
    if !force && (end - start) < opt.parallel_threshold_bytes {
        return (iter_root_object_pair_spans_single(data, start, end), false);
    }

    let chunk_bytes = std::cmp::max(1usize, opt.parallel_chunk_bytes);
    match parallel_scan::find_root_object_commas(data, start, end, workers, chunk_bytes) {
        Ok(commas) => (spans_from_commas(data, start, end, &commas), true),
        Err(_) => (iter_root_object_pair_spans_single(data, start, end), false),
    }
}

fn extract_object_key_span_and_value_span(
    data: &[u8],
    pair_span: (usize, usize),
) -> Option<((usize, usize), (usize, usize))> {
    let (mut i, end) = pair_span;
    while i < end && is_ws(data[i]) {
        i += 1;
    }
    if i >= end || data[i] != b'"' {
        return None;
    }
    let key_start = i;
    i += 1;
    let mut escape = false;
    let mut closed = false;
    while i < end {
        let ch = data[i];
        if escape {
            escape = false;
        } else if ch == b'\\' {
            escape = true;
        } else if ch == b'"' {
            i += 1;
            closed = true;
            break;
        }
        i += 1;
    }
    if !closed {
        return None;
    }
    let key_span = (key_start, i);

    while i < end && is_ws(data[i]) {
        i += 1;
    }
    if i >= end || data[i] != b':' {
        return None;
    }
    i += 1;
    let (vs, ve) = trim_span(data, i, end);
    if ve <= vs {
        return None;
    }
    Some((key_span, (vs, ve)))
}

fn extract_object_key_and_value_span(
    data: &[u8],
    pair_span: (usize, usize),
) -> Option<(String, (usize, usize))> {
    let (key_span, value_span) = extract_object_key_span_and_value_span(data, pair_span)?;
    let key_json = std::str::from_utf8(&data[key_span.0..key_span.1]).ok()?;
    let key_v = parse_strict_json(key_json).ok()?;
    let key = match key_v {
        JsonValue::String(s) => s,
        _ => return None,
    };
    Some((key, value_span))
}

fn parse_task_bytes(data: &[u8], spans: &[(usize, usize)]) -> Result<Vec<JsonValue>, String> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(b'[');
    for (idx, (s, e)) in spans.iter().enumerate() {
        if idx > 0 {
            payload.push(b',');
        }
        payload.extend_from_slice(&data[*s..*e]);
    }
    payload.push(b']');
    let s =
        std::str::from_utf8(&payload).map_err(|e| format!("invalid utf-8 in task payload: {e}"))?;
    let v = parse_strict_json(s).map_err(|e| {
        format!(
            "strict parse failed in task payload: {} at {}",
            e.message, e.pos
        )
    })?;
    match v {
        JsonValue::Array(a) => Ok(a),
        _ => Err("task payload did not parse to array".to_string()),
    }
}

fn parse_object_pair_task_bytes(
    data: &[u8],
    spans: &[(usize, usize)],
) -> Result<Vec<(String, JsonValue)>, String> {
    let mut payload: Vec<u8> = Vec::new();
    payload.push(b'{');
    for (idx, (s, e)) in spans.iter().enumerate() {
        if idx > 0 {
            payload.push(b',');
        }
        payload.extend_from_slice(&data[*s..*e]);
    }
    payload.push(b'}');
    let s =
        std::str::from_utf8(&payload).map_err(|e| format!("invalid utf-8 in task payload: {e}"))?;
    let v = parse_strict_json(s).map_err(|e| {
        format!(
            "strict parse failed in task payload: {} at {}",
            e.message, e.pos
        )
    })?;
    match v {
        JsonValue::Object(obj) => Ok(obj),
        _ => Err("task payload did not parse to object".to_string()),
    }
}

fn parse_array_tasks_parallel(
    data: &[u8],
    tasks: &[Vec<(usize, usize)>],
    workers: usize,
) -> Result<Vec<JsonValue>, String> {
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    let workers = std::cmp::max(1usize, workers).min(tasks.len());
    let results: Mutex<Vec<Option<Vec<JsonValue>>>> = Mutex::new(vec![None; tasks.len()]);
    let next_idx = AtomicUsize::new(0usize);

    let mut first_err: Option<String> = None;
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> Result<(), String> {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let chunk = parse_task_bytes(data, &tasks[idx])?;
                    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
                    r[idx] = Some(chunk);
                }
                Ok(())
            }));
        }

        for h in handles {
            match h.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
                Err(_) => {
                    if first_err.is_none() {
                        first_err = Some("worker panicked".to_string());
                    }
                }
            }
        }
    });
    if let Some(e) = first_err {
        return Err(e);
    }

    let mut out: Vec<JsonValue> = Vec::new();
    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
    for slot in r.iter_mut() {
        if let Some(vs) = slot.take() {
            out.extend(vs);
        }
    }
    Ok(out)
}

fn parse_object_pair_tasks_parallel(
    data: &[u8],
    tasks: &[Vec<(usize, usize)>],
    workers: usize,
) -> Result<Vec<(String, JsonValue)>, String> {
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    let workers = std::cmp::max(1usize, workers).min(tasks.len());
    type ObjectPairChunk = Vec<(String, JsonValue)>;
    let results: Mutex<Vec<Option<ObjectPairChunk>>> = Mutex::new(vec![None; tasks.len()]);
    let next_idx = AtomicUsize::new(0usize);

    let mut first_err: Option<String> = None;
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> Result<(), String> {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let chunk = parse_object_pair_task_bytes(data, &tasks[idx])?;
                    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
                    r[idx] = Some(chunk);
                }
                Ok(())
            }));
        }

        for h in handles {
            match h.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
                Err(_) => {
                    if first_err.is_none() {
                        first_err = Some("worker panicked".to_string());
                    }
                }
            }
        }
    });
    if let Some(e) = first_err {
        return Err(e);
    }

    let mut out: Vec<(String, JsonValue)> = Vec::new();
    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
    for slot in r.iter_mut() {
        if let Some(chunk) = slot.take() {
            out.extend(chunk);
        }
    }
    Ok(out)
}

fn parse_tape_entries_strict(
    data: &[u8],
    start: usize,
    end: usize,
) -> Result<Vec<TapeEntry>, String> {
    parse_strict_tape(&data[start..end], start)
        .map(|t| t.entries)
        .map_err(|e| format!("tape parse failed: {} at {}", e.message, e.pos))
}

fn structural_density_outside_strings(data: &[u8], start: usize, end: usize) -> f64 {
    let mut structural: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    for &ch in &data[start..end] {
        if in_string {
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            continue;
        }
        if ch == b'"' {
            in_string = true;
            continue;
        }
        if matches!(ch, b'{' | b'}' | b'[' | b']' | b',' | b':') {
            structural += 1;
        }
    }
    (structural as f64) / ((end - start).max(1) as f64)
}

fn parse_object_pair_segment_scale_tape(
    data: &[u8],
    pair_span: (usize, usize),
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<TapeEntry>, String> {
    let ((ks, ke), (vs, ve)) = extract_object_key_span_and_value_span(data, pair_span)
        .ok_or_else(|| "failed to extract object pair spans".to_string())?;
    let key_entries = parse_tape_entries_strict(data, ks, ke)?;
    let value_entries = parse_value_scale_tape(data, vs, ve, opt, depth)
        .or_else(|_| parse_tape_entries_strict(data, vs, ve))?;

    let mut out: Vec<TapeEntry> = Vec::new();
    append_segment(&mut out, &key_entries);
    append_segment(&mut out, &value_entries);
    Ok(out)
}

const MAX_SCALE_TAPE_RECURSION_DEPTH: usize = 8;

fn should_recurse_span(
    data: &[u8],
    start: usize,
    end: usize,
    opt: &RepairOptions,
    depth: usize,
) -> bool {
    if end <= start {
        return false;
    }
    if depth >= MAX_SCALE_TAPE_RECURSION_DEPTH {
        return false;
    }
    let allow = allow_parallel_bool(opt);
    if allow == Some(false) {
        return false;
    }
    let force = allow == Some(true);
    if !force && (end - start) < opt.parallel_threshold_bytes {
        return false;
    }
    matches!((data[start], data[end - 1]), (b'[', b']') | (b'{', b'}'))
}

fn should_recurse_pair_value(
    data: &[u8],
    pair_span: (usize, usize),
    opt: &RepairOptions,
    depth: usize,
) -> bool {
    let Some((_key_span, (vs, ve))) = extract_object_key_span_and_value_span(data, pair_span)
    else {
        return false;
    };
    should_recurse_span(data, vs, ve, opt, depth)
}

fn parse_value_scale_tape(
    data: &[u8],
    start: usize,
    end: usize,
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<TapeEntry>, String> {
    let (s0, e0) = trim_span(data, start, end);
    if e0 <= s0 {
        return Err("empty value span".to_string());
    }

    let strict_fallback = || parse_tape_entries_strict(data, s0, e0);

    if depth >= MAX_SCALE_TAPE_RECURSION_DEPTH {
        return strict_fallback();
    }

    let allow = allow_parallel_bool(opt);
    if allow == Some(false) {
        return strict_fallback();
    }
    let force = allow == Some(true);
    if !force && (e0 - s0) < opt.parallel_threshold_bytes {
        return strict_fallback();
    }

    let first = data[s0];
    let last = data[e0 - 1];
    if first == b'[' && last == b']' {
        return parse_array_value_scale_tape(data, s0, e0, opt, depth)
            .or_else(|_| strict_fallback());
    }
    if first == b'{' && last == b'}' {
        return parse_object_value_scale_tape(data, s0, e0, opt, depth)
            .or_else(|_| strict_fallback());
    }

    strict_fallback()
}

fn parse_array_value_scale_tape(
    data: &[u8],
    s0: usize,
    e0: usize,
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<TapeEntry>, String> {
    let (plan, tasks, used_parallel_indexer) = root_array_split_plan(data, s0, e0, opt, false);
    let workers = std::cmp::max(1usize, parallel_workers(opt));
    let can_parallel = plan.mode != SPLIT_NO_SPLIT && workers >= 2 && tasks.len() > 1;

    if !can_parallel {
        let child_depth = depth + 1;
        let needs_recursive_child = tasks
            .iter()
            .flatten()
            .any(|(s, e)| should_recurse_span(data, *s, *e, opt, child_depth));
        if !needs_recursive_child {
            return parse_tape_entries_strict(data, s0, e0);
        }
    }

    let task_segs = if can_parallel {
        match parse_array_tape_tasks_parallel(data, &tasks, workers, opt, depth) {
            Ok(v) => v,
            Err(e) => {
                if !used_parallel_indexer {
                    return Err(e);
                }
                let (plan2, tasks2, _) = root_array_split_plan(data, s0, e0, opt, true);
                let can_parallel2 =
                    plan2.mode != SPLIT_NO_SPLIT && workers >= 2 && tasks2.len() > 1;
                if can_parallel2 {
                    parse_array_tape_tasks_parallel(data, &tasks2, workers, opt, depth)?
                } else {
                    parse_array_tape_tasks_sequential(data, &tasks2, opt, depth)?
                }
            }
        }
    } else {
        parse_array_tape_tasks_sequential(data, &tasks, opt, depth)?
    };

    Ok(build_root_array_tape(s0, e0, &task_segs).entries)
}

fn parse_array_tape_tasks_sequential(
    data: &[u8],
    tasks: &[Vec<(usize, usize)>],
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<Vec<Vec<TapeEntry>>>, String> {
    let mut out: Vec<Vec<Vec<TapeEntry>>> = Vec::with_capacity(tasks.len());
    for task in tasks {
        let mut segs: Vec<Vec<TapeEntry>> = Vec::with_capacity(task.len());
        for (s, e) in task {
            let entries = parse_value_scale_tape(data, *s, *e, opt, depth + 1)
                .or_else(|_| parse_tape_entries_strict(data, *s, *e))?;
            segs.push(entries);
        }
        out.push(segs);
    }
    Ok(out)
}

fn parse_object_value_scale_tape(
    data: &[u8],
    s0: usize,
    e0: usize,
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<TapeEntry>, String> {
    let (spans, used_parallel_indexer) = iter_root_object_pair_spans(data, s0, e0, opt, false);
    let elements = spans.len();
    let structural_density = structural_density_outside_strings(data, s0, e0);

    let do_parallel = match allow_parallel_bool(opt) {
        None => {
            (e0 - s0) >= opt.parallel_threshold_bytes
                && elements >= opt.min_elements_for_parallel
                && structural_density >= opt.density_threshold
        }
        Some(v) => v,
    };

    let workers = std::cmp::max(1usize, parallel_workers(opt));
    let can_parallel = do_parallel && workers >= 2 && elements > 1;

    let target = std::cmp::max(1_000_000usize, opt.parallel_chunk_bytes);
    let mut tasks: Vec<Vec<(usize, usize)>> = Vec::new();
    if can_parallel {
        let mut cur: Vec<(usize, usize)> = Vec::new();
        let mut cur_bytes: usize = 0;
        for (s, e) in spans {
            cur.push((s, e));
            cur_bytes += e - s;
            if !cur.is_empty() && cur_bytes >= target {
                tasks.push(cur);
                cur = Vec::new();
                cur_bytes = 0;
            }
        }
        if !cur.is_empty() {
            tasks.push(cur);
        }
    } else {
        tasks.push(spans);
    }

    if !can_parallel {
        let child_depth = depth + 1;
        let needs_recursive_value = tasks
            .iter()
            .flatten()
            .any(|&span| should_recurse_pair_value(data, span, opt, child_depth));
        if !needs_recursive_value {
            return parse_tape_entries_strict(data, s0, e0);
        }
    }

    let task_segs = if can_parallel {
        match parse_object_pair_tape_tasks_parallel(data, &tasks, workers, opt, depth) {
            Ok(v) => v,
            Err(e) => {
                if !used_parallel_indexer {
                    return Err(e);
                }
                let (spans2, _) = iter_root_object_pair_spans(data, s0, e0, opt, true);
                let mut tasks2: Vec<Vec<(usize, usize)>> = Vec::new();
                let mut cur: Vec<(usize, usize)> = Vec::new();
                let mut cur_bytes: usize = 0;
                for (s, e) in spans2 {
                    cur.push((s, e));
                    cur_bytes += e - s;
                    if !cur.is_empty() && cur_bytes >= target {
                        tasks2.push(cur);
                        cur = Vec::new();
                        cur_bytes = 0;
                    }
                }
                if !cur.is_empty() {
                    tasks2.push(cur);
                }
                parse_object_pair_tape_tasks_parallel(data, &tasks2, workers, opt, depth)?
            }
        }
    } else {
        parse_object_pair_tape_tasks_sequential(data, &tasks, opt, depth)?
    };

    Ok(build_root_object_tape(s0, e0, &task_segs).entries)
}

fn parse_object_pair_tape_tasks_sequential(
    data: &[u8],
    tasks: &[Vec<(usize, usize)>],
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<Vec<Vec<TapeEntry>>>, String> {
    let mut out: Vec<Vec<Vec<TapeEntry>>> = Vec::with_capacity(tasks.len());
    for task in tasks {
        let mut segs: Vec<Vec<TapeEntry>> = Vec::with_capacity(task.len());
        for &span in task {
            let child_depth = depth + 1;
            let want_recursive_value = should_recurse_pair_value(data, span, opt, child_depth);
            let seg = if want_recursive_value {
                match parse_object_pair_segment_scale_tape(data, span, opt, child_depth) {
                    Ok(v) => v,
                    Err(_) => parse_object_pair_segment(&data[span.0..span.1], span.0)
                        .map_err(|e| format!("tape parse failed: {} at {}", e.message, e.pos))?,
                }
            } else {
                match parse_object_pair_segment(&data[span.0..span.1], span.0) {
                    Ok(v) => v,
                    Err(_) => parse_object_pair_segment_scale_tape(data, span, opt, child_depth)?,
                }
            };
            segs.push(seg);
        }
        out.push(segs);
    }
    Ok(out)
}

fn parse_array_tape_tasks_parallel(
    data: &[u8],
    tasks: &[Vec<(usize, usize)>],
    workers: usize,
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<Vec<Vec<TapeEntry>>>, String> {
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    let workers = std::cmp::max(1usize, workers).min(tasks.len());
    let results: Mutex<Vec<Option<Vec<Vec<TapeEntry>>>>> = Mutex::new(vec![None; tasks.len()]);
    let next_idx = AtomicUsize::new(0usize);

    let mut first_err: Option<String> = None;
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> Result<(), String> {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let mut segs: Vec<Vec<TapeEntry>> = Vec::with_capacity(tasks[idx].len());
                    for (s, e) in &tasks[idx] {
                        let entries = parse_value_scale_tape(data, *s, *e, opt, depth + 1)
                            .or_else(|_| parse_tape_entries_strict(data, *s, *e))?;
                        segs.push(entries);
                    }
                    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
                    r[idx] = Some(segs);
                }
                Ok(())
            }));
        }

        for h in handles {
            match h.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
                Err(_) => {
                    if first_err.is_none() {
                        first_err = Some("worker panicked".to_string());
                    }
                }
            }
        }
    });
    if let Some(e) = first_err {
        return Err(e);
    }

    let mut out: Vec<Vec<Vec<TapeEntry>>> = Vec::with_capacity(tasks.len());
    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
    for slot in r.iter_mut() {
        if let Some(segs) = slot.take() {
            out.push(segs);
        }
    }
    Ok(out)
}

fn parse_object_pair_tape_tasks_parallel(
    data: &[u8],
    tasks: &[Vec<(usize, usize)>],
    workers: usize,
    opt: &RepairOptions,
    depth: usize,
) -> Result<Vec<Vec<Vec<TapeEntry>>>, String> {
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    let workers = std::cmp::max(1usize, workers).min(tasks.len());
    let results: Mutex<Vec<Option<Vec<Vec<TapeEntry>>>>> = Mutex::new(vec![None; tasks.len()]);
    let next_idx = AtomicUsize::new(0usize);

    let mut first_err: Option<String> = None;
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> Result<(), String> {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let mut segs: Vec<Vec<TapeEntry>> = Vec::with_capacity(tasks[idx].len());
                    for (s, e) in &tasks[idx] {
                        let span = (*s, *e);
                        let child_depth = depth + 1;
                        let want_recursive_value =
                            should_recurse_pair_value(data, span, opt, child_depth);
                        let seg = if want_recursive_value {
                            match parse_object_pair_segment_scale_tape(data, span, opt, child_depth)
                            {
                                Ok(v) => v,
                                Err(_) => {
                                    parse_object_pair_segment(&data[*s..*e], *s).map_err(|e| {
                                        format!("tape parse failed: {} at {}", e.message, e.pos)
                                    })?
                                }
                            }
                        } else {
                            match parse_object_pair_segment(&data[*s..*e], *s) {
                                Ok(v) => v,
                                Err(_) => parse_object_pair_segment_scale_tape(
                                    data,
                                    span,
                                    opt,
                                    child_depth,
                                )?,
                            }
                        };
                        segs.push(seg);
                    }
                    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
                    r[idx] = Some(segs);
                }
                Ok(())
            }));
        }

        for h in handles {
            match h.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
                Err(_) => {
                    if first_err.is_none() {
                        first_err = Some("worker panicked".to_string());
                    }
                }
            }
        }
    });
    if let Some(e) = first_err {
        return Err(e);
    }

    let mut out: Vec<Vec<Vec<TapeEntry>>> = Vec::with_capacity(tasks.len());
    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
    for slot in r.iter_mut() {
        if let Some(segs) = slot.take() {
            out.push(segs);
        }
    }
    Ok(out)
}

fn build_root_array_tape(s0: usize, e0: usize, task_segs: &[Vec<Vec<TapeEntry>>]) -> Tape {
    let mut entries: Vec<TapeEntry> = Vec::new();
    let start_idx = entries.len();
    entries.push(TapeEntry::new(TapeTokenType::ArrayStart, s0, 1));
    for segs in task_segs {
        for seg in segs {
            append_segment(&mut entries, seg);
        }
    }
    let end_idx = entries.len();
    entries.push(TapeEntry::new(TapeTokenType::ArrayEnd, e0 - 1, 1));
    entries[start_idx].payload = end_idx as u64;
    Tape {
        root_index: start_idx,
        data_span: (s0, e0),
        entries,
    }
}

fn build_root_object_tape(s0: usize, e0: usize, task_segs: &[Vec<Vec<TapeEntry>>]) -> Tape {
    let mut entries: Vec<TapeEntry> = Vec::new();
    let start_idx = entries.len();
    entries.push(TapeEntry::new(TapeTokenType::ObjectStart, s0, 1));
    for segs in task_segs {
        for seg in segs {
            append_segment(&mut entries, seg);
        }
    }
    let end_idx = entries.len();
    entries.push(TapeEntry::new(TapeTokenType::ObjectEnd, e0 - 1, 1));
    entries[start_idx].payload = end_idx as u64;
    Tape {
        root_index: start_idx,
        data_span: (s0, e0),
        entries,
    }
}

fn allow_parallel_bool(opt: &RepairOptions) -> Option<bool> {
    let s = opt.allow_parallel.trim().to_ascii_lowercase();
    if s == "auto" {
        None
    } else if s == "true" || s == "1" || s == "yes" {
        Some(true)
    } else {
        Some(false)
    }
}

fn try_nested_target_split(
    data: &[u8],
    spans: &[(usize, usize)],
    target_keys: &[String],
    opt: &RepairOptions,
) -> Option<(JsonValue, SplitPlan)> {
    let mut target_span: Option<(usize, usize)> = None;
    let mut target_key: Option<String> = None;
    let mut target_value: Option<JsonValue> = None;
    let mut inner_plan: Option<SplitPlan> = None;

    for &span in spans {
        let (key, (vs, ve)) = extract_object_key_and_value_span(data, span)?;
        if !target_keys.iter().any(|k| k == &key) {
            continue;
        }
        if !matches!(data.get(vs), Some(b'[') | Some(b'{')) {
            continue;
        }
        let mut opt2 = opt.clone();
        opt2.scale_target_keys = None;
        match parse_root_array_scale(&data[vs..ve], &opt2) {
            Ok((v, plan)) => {
                target_span = Some(span);
                target_key = Some(key);
                target_value = Some(v);
                inner_plan = Some(plan);
                break;
            }
            Err(_) => return None,
        }
    }

    let (target_span, target_key, inner_plan) = match (target_span, target_key, inner_plan) {
        (Some(s), Some(k), Some(p)) => (s, k, p),
        _ => return None,
    };

    let mut target_value = target_value?;
    let mut out: Vec<(String, JsonValue)> = Vec::with_capacity(spans.len());
    for &span in spans {
        if span == target_span {
            out.push((target_key.clone(), target_value));
            // make sure we only insert once
            target_value = JsonValue::Null;
            continue;
        }
        let chunk = parse_object_pair_task_bytes(data, std::slice::from_ref(&span)).ok()?;
        out.extend(chunk);
    }

    Some((
        JsonValue::Object(out),
        SplitPlan {
            mode: format!("NESTED_KEY({}).{}", target_key, inner_plan.mode),
            elements: inner_plan.elements,
            structural_density: inner_plan.structural_density,
            chunk_count: inner_plan.chunk_count,
        },
    ))
}

fn root_array_split_plan(
    data: &[u8],
    start: usize,
    end: usize,
    opt: &RepairOptions,
    force_single_indexer: bool,
) -> (SplitPlan, Vec<Vec<(usize, usize)>>, bool) {
    let (spans, used_parallel_indexer) =
        iter_root_array_element_spans(data, start, end, opt, force_single_indexer);
    let elements = spans.len();

    // Structural density approximation (outside strings is expensive to recompute);
    // use a small scan for delimiters.
    let mut structural: usize = 0;
    let mut in_string = false;
    let mut escape = false;
    for &ch in &data[start..end] {
        if in_string {
            if escape {
                escape = false;
            } else if ch == b'\\' {
                escape = true;
            } else if ch == b'"' {
                in_string = false;
            }
            continue;
        }
        if ch == b'"' {
            in_string = true;
            continue;
        }
        if matches!(ch, b'{' | b'}' | b'[' | b']' | b',' | b':') {
            structural += 1;
        }
    }
    let structural_density = (structural as f64) / ((end - start).max(1) as f64);

    let do_parallel = match allow_parallel_bool(opt) {
        None => {
            (end - start) >= opt.parallel_threshold_bytes
                && elements >= opt.min_elements_for_parallel
                && structural_density >= opt.density_threshold
        }
        Some(v) => v,
    };

    if !do_parallel || elements <= 1 {
        return (
            SplitPlan {
                mode: SPLIT_NO_SPLIT.to_string(),
                elements,
                structural_density,
                chunk_count: 1,
            },
            vec![spans],
            used_parallel_indexer,
        );
    }

    let target = std::cmp::max(1_000_000usize, opt.parallel_chunk_bytes);
    let mut tasks: Vec<Vec<(usize, usize)>> = Vec::new();
    let mut cur: Vec<(usize, usize)> = Vec::new();
    let mut cur_bytes: usize = 0;
    for (s, e) in spans {
        cur.push((s, e));
        cur_bytes += e - s;
        if !cur.is_empty() && cur_bytes >= target {
            tasks.push(cur);
            cur = Vec::new();
            cur_bytes = 0;
        }
    }
    if !cur.is_empty() {
        tasks.push(cur);
    }

    (
        SplitPlan {
            mode: SPLIT_ROOT_ARRAY_ELEMENTS.to_string(),
            elements,
            structural_density,
            chunk_count: tasks.len(),
        },
        tasks,
        used_parallel_indexer,
    )
}

pub fn parse_root_array_scale(
    data: &[u8],
    opt: &RepairOptions,
) -> Result<(JsonValue, SplitPlan), String> {
    let (s0, e0) = trim_ws(data);
    if data.get(s0) == Some(&b'[') && data.get(e0.saturating_sub(1)) == Some(&b']') {
        let (plan, tasks, used_parallel_indexer) = root_array_split_plan(data, s0, e0, opt, false);
        if plan.mode == SPLIT_NO_SPLIT {
            let s =
                std::str::from_utf8(&data[s0..e0]).map_err(|e| format!("invalid utf-8: {e}"))?;
            let value = parse_strict_json(s)
                .map_err(|e| format!("strict parse failed: {} at {}", e.message, e.pos))?;
            return Ok((value, plan));
        }

        let workers = std::cmp::max(1usize, parallel_workers(opt));
        match parse_array_tasks_parallel(data, &tasks, workers) {
            Ok(out) => return Ok((JsonValue::Array(out), plan)),
            Err(e) => {
                if !used_parallel_indexer {
                    return Err(e);
                }
                let (plan2, tasks2, _) = root_array_split_plan(data, s0, e0, opt, true);
                if plan2.mode == SPLIT_NO_SPLIT {
                    let s = std::str::from_utf8(&data[s0..e0])
                        .map_err(|e| format!("invalid utf-8: {e}"))?;
                    let value = parse_strict_json(s)
                        .map_err(|e| format!("strict parse failed: {} at {}", e.message, e.pos))?;
                    return Ok((value, plan2));
                }
                let out2 = parse_array_tasks_parallel(data, &tasks2, workers)?;
                return Ok((JsonValue::Array(out2), plan2));
            }
        }
    }

    if data.get(s0) == Some(&b'{') && data.get(e0.saturating_sub(1)) == Some(&b'}') {
        let (spans, used_parallel_indexer) = iter_root_object_pair_spans(data, s0, e0, opt, false);
        let elements = spans.len();

        if let Some(keys) = opt.scale_target_keys.as_ref()
            && !keys.is_empty()
            && let Some((v, plan)) = try_nested_target_split(data, &spans, keys, opt)
        {
            return Ok((v, plan));
        }

        let mut structural: usize = 0;
        let mut in_string = false;
        let mut escape = false;
        for &ch in &data[s0..e0] {
            if in_string {
                if escape {
                    escape = false;
                } else if ch == b'\\' {
                    escape = true;
                } else if ch == b'"' {
                    in_string = false;
                }
                continue;
            }
            if ch == b'"' {
                in_string = true;
                continue;
            }
            if matches!(ch, b'{' | b'}' | b'[' | b']' | b',' | b':') {
                structural += 1;
            }
        }
        let structural_density = (structural as f64) / ((e0 - s0).max(1) as f64);

        let do_parallel = match allow_parallel_bool(opt) {
            None => {
                (e0 - s0) >= opt.parallel_threshold_bytes
                    && elements >= opt.min_elements_for_parallel
                    && structural_density >= opt.density_threshold
            }
            Some(v) => v,
        };

        if !do_parallel || elements <= 1 {
            let s =
                std::str::from_utf8(&data[s0..e0]).map_err(|e| format!("invalid utf-8: {e}"))?;
            let value = parse_strict_json(s)
                .map_err(|e| format!("strict parse failed: {} at {}", e.message, e.pos))?;
            return Ok((
                value,
                SplitPlan {
                    mode: SPLIT_NO_SPLIT.to_string(),
                    elements,
                    structural_density,
                    chunk_count: 1,
                },
            ));
        }

        let target = std::cmp::max(1_000_000usize, opt.parallel_chunk_bytes);
        let mut tasks: Vec<Vec<(usize, usize)>> = Vec::new();
        let mut cur: Vec<(usize, usize)> = Vec::new();
        let mut cur_bytes: usize = 0;
        for (s, e) in spans {
            cur.push((s, e));
            cur_bytes += e - s;
            if !cur.is_empty() && cur_bytes >= target {
                tasks.push(cur);
                cur = Vec::new();
                cur_bytes = 0;
            }
        }
        if !cur.is_empty() {
            tasks.push(cur);
        }

        let plan = SplitPlan {
            mode: SPLIT_ROOT_OBJECT_PAIRS.to_string(),
            elements,
            structural_density,
            chunk_count: tasks.len(),
        };

        let workers = std::cmp::max(1usize, parallel_workers(opt));
        match parse_object_pair_tasks_parallel(data, &tasks, workers) {
            Ok(out) => return Ok((JsonValue::Object(out), plan)),
            Err(e) => {
                if !used_parallel_indexer {
                    return Err(e);
                }

                let (spans2, _) = iter_root_object_pair_spans(data, s0, e0, opt, true);
                let elements2 = spans2.len();
                let do_parallel2 = match allow_parallel_bool(opt) {
                    None => {
                        (e0 - s0) >= opt.parallel_threshold_bytes
                            && elements2 >= opt.min_elements_for_parallel
                            && structural_density >= opt.density_threshold
                    }
                    Some(v) => v,
                };

                if !do_parallel2 || elements2 <= 1 {
                    let s = std::str::from_utf8(&data[s0..e0])
                        .map_err(|e| format!("invalid utf-8: {e}"))?;
                    let value = parse_strict_json(s)
                        .map_err(|e| format!("strict parse failed: {} at {}", e.message, e.pos))?;
                    return Ok((
                        value,
                        SplitPlan {
                            mode: SPLIT_NO_SPLIT.to_string(),
                            elements: elements2,
                            structural_density,
                            chunk_count: 1,
                        },
                    ));
                }

                let mut tasks2: Vec<Vec<(usize, usize)>> = Vec::new();
                let mut cur: Vec<(usize, usize)> = Vec::new();
                let mut cur_bytes: usize = 0;
                for (s, e) in spans2 {
                    cur.push((s, e));
                    cur_bytes += e - s;
                    if !cur.is_empty() && cur_bytes >= target {
                        tasks2.push(cur);
                        cur = Vec::new();
                        cur_bytes = 0;
                    }
                }
                if !cur.is_empty() {
                    tasks2.push(cur);
                }

                let plan2 = SplitPlan {
                    mode: SPLIT_ROOT_OBJECT_PAIRS.to_string(),
                    elements: elements2,
                    structural_density,
                    chunk_count: tasks2.len(),
                };
                let out2 = parse_object_pair_tasks_parallel(data, &tasks2, workers)?;
                return Ok((JsonValue::Object(out2), plan2));
            }
        }
    }

    let s = std::str::from_utf8(&data[s0..e0]).map_err(|e| format!("invalid utf-8: {e}"))?;
    let value = parse_strict_json(s)
        .map_err(|e| format!("strict parse failed: {} at {}", e.message, e.pos))?;
    Ok((
        value,
        SplitPlan {
            mode: SPLIT_NO_SPLIT.to_string(),
            elements: 0,
            structural_density: 0.0,
            chunk_count: 1,
        },
    ))
}

pub fn parse_root_array_scale_tape(
    data: &[u8],
    opt: &RepairOptions,
) -> Result<(Tape, SplitPlan), String> {
    let (s0, e0) = trim_ws(data);

    if data.get(s0) == Some(&b'[') && data.get(e0.saturating_sub(1)) == Some(&b']') {
        let (plan, tasks, used_parallel_indexer) = root_array_split_plan(data, s0, e0, opt, false);
        if plan.mode == SPLIT_NO_SPLIT {
            let entries = parse_value_scale_tape(data, s0, e0, opt, 0)?;
            return Ok((
                Tape {
                    root_index: 0,
                    data_span: (s0, e0),
                    entries,
                },
                plan,
            ));
        }

        let workers = std::cmp::max(1usize, parallel_workers(opt));
        let task_segs = match parse_array_tape_tasks_parallel(data, &tasks, workers, opt, 0) {
            Ok(v) => v,
            Err(e) => {
                if !used_parallel_indexer {
                    return Err(e);
                }
                let (plan2, tasks2, _) = root_array_split_plan(data, s0, e0, opt, true);
                if plan2.mode == SPLIT_NO_SPLIT {
                    let entries = parse_value_scale_tape(data, s0, e0, opt, 0)?;
                    return Ok((
                        Tape {
                            root_index: 0,
                            data_span: (s0, e0),
                            entries,
                        },
                        plan2,
                    ));
                }
                let v = parse_array_tape_tasks_parallel(data, &tasks2, workers, opt, 0)?;
                return Ok((build_root_array_tape(s0, e0, &v), plan2));
            }
        };

        return Ok((build_root_array_tape(s0, e0, &task_segs), plan));
    }

    if data.get(s0) == Some(&b'{') && data.get(e0.saturating_sub(1)) == Some(&b'}') {
        let (spans, used_parallel_indexer) = iter_root_object_pair_spans(data, s0, e0, opt, false);
        let elements = spans.len();

        let mut structural: usize = 0;
        let mut in_string = false;
        let mut escape = false;
        for &ch in &data[s0..e0] {
            if in_string {
                if escape {
                    escape = false;
                } else if ch == b'\\' {
                    escape = true;
                } else if ch == b'"' {
                    in_string = false;
                }
                continue;
            }
            if ch == b'"' {
                in_string = true;
                continue;
            }
            if matches!(ch, b'{' | b'}' | b'[' | b']' | b',' | b':') {
                structural += 1;
            }
        }
        let structural_density = (structural as f64) / ((e0 - s0).max(1) as f64);

        let do_parallel = match allow_parallel_bool(opt) {
            None => {
                (e0 - s0) >= opt.parallel_threshold_bytes
                    && elements >= opt.min_elements_for_parallel
                    && structural_density >= opt.density_threshold
            }
            Some(v) => v,
        };

        if !do_parallel || elements <= 1 {
            let entries = parse_value_scale_tape(data, s0, e0, opt, 0)?;
            return Ok((
                Tape {
                    root_index: 0,
                    data_span: (s0, e0),
                    entries,
                },
                SplitPlan {
                    mode: SPLIT_NO_SPLIT.to_string(),
                    elements,
                    structural_density,
                    chunk_count: 1,
                },
            ));
        }

        let target = std::cmp::max(1_000_000usize, opt.parallel_chunk_bytes);
        let mut tasks: Vec<Vec<(usize, usize)>> = Vec::new();
        let mut cur: Vec<(usize, usize)> = Vec::new();
        let mut cur_bytes: usize = 0;
        for (s, e) in spans {
            cur.push((s, e));
            cur_bytes += e - s;
            if !cur.is_empty() && cur_bytes >= target {
                tasks.push(cur);
                cur = Vec::new();
                cur_bytes = 0;
            }
        }
        if !cur.is_empty() {
            tasks.push(cur);
        }

        let plan = SplitPlan {
            mode: SPLIT_ROOT_OBJECT_PAIRS.to_string(),
            elements,
            structural_density,
            chunk_count: tasks.len(),
        };

        let workers = std::cmp::max(1usize, parallel_workers(opt));
        let task_segs = match parse_object_pair_tape_tasks_parallel(data, &tasks, workers, opt, 0) {
            Ok(v) => v,
            Err(e) => {
                if !used_parallel_indexer {
                    return Err(e);
                }
                let (spans2, _) = iter_root_object_pair_spans(data, s0, e0, opt, true);
                let elements2 = spans2.len();
                let do_parallel2 = match allow_parallel_bool(opt) {
                    None => {
                        (e0 - s0) >= opt.parallel_threshold_bytes
                            && elements2 >= opt.min_elements_for_parallel
                            && structural_density >= opt.density_threshold
                    }
                    Some(v) => v,
                };
                if !do_parallel2 || elements2 <= 1 {
                    let entries = parse_value_scale_tape(data, s0, e0, opt, 0)?;
                    return Ok((
                        Tape {
                            root_index: 0,
                            data_span: (s0, e0),
                            entries,
                        },
                        SplitPlan {
                            mode: SPLIT_NO_SPLIT.to_string(),
                            elements: elements2,
                            structural_density,
                            chunk_count: 1,
                        },
                    ));
                }
                let mut tasks2: Vec<Vec<(usize, usize)>> = Vec::new();
                let mut cur: Vec<(usize, usize)> = Vec::new();
                let mut cur_bytes: usize = 0;
                for (s, e) in spans2 {
                    cur.push((s, e));
                    cur_bytes += e - s;
                    if !cur.is_empty() && cur_bytes >= target {
                        tasks2.push(cur);
                        cur = Vec::new();
                        cur_bytes = 0;
                    }
                }
                if !cur.is_empty() {
                    tasks2.push(cur);
                }
                let plan2 = SplitPlan {
                    mode: SPLIT_ROOT_OBJECT_PAIRS.to_string(),
                    elements: elements2,
                    structural_density,
                    chunk_count: tasks2.len(),
                };
                let v2 = parse_object_pair_tape_tasks_parallel(data, &tasks2, workers, opt, 0)?;
                return Ok((build_root_object_tape(s0, e0, &v2), plan2));
            }
        };

        return Ok((build_root_object_tape(s0, e0, &task_segs), plan));
    }

    let tape = parse_strict_tape(&data[s0..e0], s0)
        .map_err(|e| format!("tape parse failed: {} at {}", e.message, e.pos))?;
    Ok((
        tape,
        SplitPlan {
            mode: SPLIT_NO_SPLIT.to_string(),
            elements: 0,
            structural_density: 0.0,
            chunk_count: 1,
        },
    ))
}
