use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy)]
struct StrState {
    in_string: bool,
    escape: bool,
}

impl StrState {
    fn from_idx(idx: usize) -> Self {
        Self {
            in_string: (idx & 0b10) != 0,
            escape: (idx & 0b01) != 0,
        }
    }

    fn idx(self) -> usize {
        ((self.in_string as usize) << 1) | (self.escape as usize)
    }

    fn normalized(mut self) -> Self {
        if !self.in_string {
            self.escape = false;
        }
        self
    }
}

#[derive(Clone, Copy)]
struct Trans {
    end_state: StrState,
    delta_brace: i64,
    delta_bracket: i64,
}

#[derive(Clone, Copy)]
struct ChunkTransducer {
    table: [Trans; 4],
}

fn compute_transducer(chunk: &[u8]) -> ChunkTransducer {
    let mut in_string: [bool; 4] = [false; 4];
    let mut escape: [bool; 4] = [false; 4];
    for i in 0..4 {
        let st = StrState::from_idx(i).normalized();
        in_string[i] = st.in_string;
        escape[i] = st.escape;
    }

    let mut delta_brace: [i64; 4] = [0; 4];
    let mut delta_bracket: [i64; 4] = [0; 4];

    for &ch in chunk {
        for i in 0..4 {
            if in_string[i] {
                if escape[i] {
                    escape[i] = false;
                } else if ch == b'\\' {
                    escape[i] = true;
                } else if ch == b'"' {
                    in_string[i] = false;
                    escape[i] = false;
                }
                continue;
            }

            if ch == b'"' {
                in_string[i] = true;
                escape[i] = false;
                continue;
            }

            match ch {
                b'{' => delta_brace[i] += 1,
                b'}' => delta_brace[i] -= 1,
                b'[' => delta_bracket[i] += 1,
                b']' => delta_bracket[i] -= 1,
                _ => {}
            }
        }
    }

    let mut table = [Trans {
        end_state: StrState {
            in_string: false,
            escape: false,
        },
        delta_brace: 0,
        delta_bracket: 0,
    }; 4];
    for i in 0..4 {
        table[i] = Trans {
            end_state: StrState {
                in_string: in_string[i],
                escape: escape[i],
            }
            .normalized(),
            delta_brace: delta_brace[i],
            delta_bracket: delta_bracket[i],
        };
    }
    ChunkTransducer { table }
}

#[derive(Clone, Copy)]
struct ChunkRange {
    start: usize,
    end: usize, // exclusive
}

#[derive(Clone, Copy)]
struct ChunkStart {
    state: StrState,
    brace_depth: i64,
    bracket_depth: i64,
}

fn chunk_ranges(scan_start: usize, scan_end: usize, chunk_bytes: usize) -> Vec<ChunkRange> {
    let mut ranges: Vec<ChunkRange> = Vec::new();
    if scan_end <= scan_start {
        return ranges;
    }
    let chunk_bytes = std::cmp::max(1usize, chunk_bytes);
    let mut s = scan_start;
    while s < scan_end {
        let e = std::cmp::min(scan_end, s + chunk_bytes);
        ranges.push(ChunkRange { start: s, end: e });
        s = e;
    }
    ranges
}

fn scan_chunk_commas(
    data: &[u8],
    range: ChunkRange,
    start: ChunkStart,
    target_brace: i64,
    target_bracket: i64,
) -> Vec<usize> {
    let mut state = start.state.normalized();
    let mut brace_depth = start.brace_depth;
    let mut bracket_depth = start.bracket_depth;
    let mut out: Vec<usize> = Vec::new();

    for (offset, &ch) in data[range.start..range.end].iter().enumerate() {
        let pos = range.start + offset;
        if state.in_string {
            if state.escape {
                state.escape = false;
            } else if ch == b'\\' {
                state.escape = true;
            } else if ch == b'"' {
                state.in_string = false;
                state.escape = false;
            }
            continue;
        }

        if ch == b'"' {
            state.in_string = true;
            state.escape = false;
            continue;
        }

        match ch {
            b'{' => brace_depth += 1,
            b'}' => brace_depth -= 1,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            _ => {}
        }

        if ch == b',' && brace_depth == target_brace && bracket_depth == target_bracket {
            out.push(pos);
        }
    }

    out
}

#[derive(Debug, Clone, Copy)]
struct CommaScanConfig {
    scan_start: usize,
    scan_end: usize,
    initial_brace: i64,
    initial_bracket: i64,
    target_brace: i64,
    target_bracket: i64,
    workers: usize,
    chunk_bytes: usize,
}

fn find_commas(data: &[u8], cfg: CommaScanConfig) -> Result<Vec<usize>, String> {
    let ranges = chunk_ranges(cfg.scan_start, cfg.scan_end, cfg.chunk_bytes);
    if ranges.is_empty() {
        return Ok(Vec::new());
    }

    let workers = std::cmp::max(1usize, cfg.workers).min(ranges.len());

    let transducers: Mutex<Vec<Option<ChunkTransducer>>> = Mutex::new(vec![None; ranges.len()]);
    let next_idx = AtomicUsize::new(0usize);
    let mut first_err: Option<String> = None;

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> Result<(), String> {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= ranges.len() {
                        break;
                    }
                    let r = ranges[idx];
                    let t = compute_transducer(&data[r.start..r.end]);
                    let mut out = transducers
                        .lock()
                        .map_err(|_| "mutex poisoned".to_string())?;
                    out[idx] = Some(t);
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

    let transducers = {
        let mut t = transducers
            .lock()
            .map_err(|_| "mutex poisoned".to_string())?;
        let mut out: Vec<ChunkTransducer> = Vec::with_capacity(ranges.len());
        for slot in t.iter_mut() {
            out.push(
                slot.take()
                    .ok_or_else(|| "missing transducer".to_string())?,
            );
        }
        out
    };

    let mut starts: Vec<ChunkStart> = Vec::with_capacity(ranges.len());
    let mut state = StrState {
        in_string: false,
        escape: false,
    };
    let mut brace_depth = cfg.initial_brace;
    let mut bracket_depth = cfg.initial_bracket;
    for tr in &transducers {
        starts.push(ChunkStart {
            state,
            brace_depth,
            bracket_depth,
        });
        let trans = tr.table[state.idx()];
        state = trans.end_state.normalized();
        brace_depth += trans.delta_brace;
        bracket_depth += trans.delta_bracket;
    }

    let results: Mutex<Vec<Option<Vec<usize>>>> = Mutex::new(vec![None; ranges.len()]);
    let next_idx = AtomicUsize::new(0usize);
    let mut first_err: Option<String> = None;

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            handles.push(scope.spawn(|| -> Result<(), String> {
                loop {
                    let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                    if idx >= ranges.len() {
                        break;
                    }
                    let commas = scan_chunk_commas(
                        data,
                        ranges[idx],
                        starts[idx],
                        cfg.target_brace,
                        cfg.target_bracket,
                    );
                    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
                    r[idx] = Some(commas);
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

    let mut out: Vec<usize> = Vec::new();
    let mut r = results.lock().map_err(|_| "mutex poisoned".to_string())?;
    for slot in r.iter_mut() {
        if let Some(mut v) = slot.take() {
            out.append(&mut v);
        }
    }
    Ok(out)
}

pub(crate) fn find_root_array_commas(
    data: &[u8],
    start: usize,
    end: usize,
    workers: usize,
    chunk_bytes: usize,
) -> Result<Vec<usize>, String> {
    if start >= end
        || data.get(start) != Some(&b'[')
        || data.get(end.saturating_sub(1)) != Some(&b']')
    {
        return Err("not a root array span".to_string());
    }
    let scan_start = start + 1;
    let scan_end = end.saturating_sub(1);
    find_commas(
        data,
        CommaScanConfig {
            scan_start,
            scan_end,
            initial_brace: 0,
            initial_bracket: 1,
            target_brace: 0,
            target_bracket: 1,
            workers,
            chunk_bytes,
        },
    )
}

pub(crate) fn find_root_object_commas(
    data: &[u8],
    start: usize,
    end: usize,
    workers: usize,
    chunk_bytes: usize,
) -> Result<Vec<usize>, String> {
    if start >= end
        || data.get(start) != Some(&b'{')
        || data.get(end.saturating_sub(1)) != Some(&b'}')
    {
        return Err("not a root object span".to_string());
    }
    let scan_start = start + 1;
    let scan_end = end.saturating_sub(1);
    find_commas(
        data,
        CommaScanConfig {
            scan_start,
            scan_end,
            initial_brace: 1,
            initial_bracket: 0,
            target_brace: 1,
            target_bracket: 0,
            workers,
            chunk_bytes,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_root_array_commas_single(data: &[u8], start: usize, end: usize) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        let mut in_string = false;
        let mut escape = false;
        let mut depth_brace: i64 = 0;
        let mut depth_bracket: i64 = 1;

        for (pos, &ch) in data.iter().enumerate().take(end - 1).skip(start + 1) {
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
                out.push(pos);
            }
        }
        out
    }

    fn find_root_object_commas_single(data: &[u8], start: usize, end: usize) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        let mut in_string = false;
        let mut escape = false;
        let mut depth_brace: i64 = 1;
        let mut depth_bracket: i64 = 0;

        for (pos, &ch) in data.iter().enumerate().take(end - 1).skip(start + 1) {
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
                out.push(pos);
            }
        }
        out
    }

    #[test]
    fn parallel_scan_matches_single_root_array() {
        let data = br#"["a,b",{"x":[1,2,3]},"c\\\"d","e\\\\","f}g","h]i"]"#;
        let start = 0;
        let end = data.len();
        let single = find_root_array_commas_single(data, start, end);
        let parallel = find_root_array_commas(data, start, end, 4, 3).expect("parallel scan");
        assert_eq!(single, parallel);
    }

    #[test]
    fn parallel_scan_matches_single_root_object() {
        let data = br#"{"a":"x,y","b":{"c":[1,2,3],"d":"q\\\"w"},"e":"\\\\","f":["]", "}"]}"#;
        let start = 0;
        let end = data.len();
        let single = find_root_object_commas_single(data, start, end);
        let parallel = find_root_object_commas(data, start, end, 4, 5).expect("parallel scan");
        assert_eq!(single, parallel);
    }
}
