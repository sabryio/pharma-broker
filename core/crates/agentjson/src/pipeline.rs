use std::time::Instant;

use crate::beam::probabilistic_repair;
use crate::extract::extract_json_candidate;
use crate::heuristic::heuristic_repair;
use crate::json::JsonValue;
use crate::llm_fallback::maybe_llm_rerun;
use crate::scale::{parse_root_array_scale, parse_root_array_scale_tape};
use crate::schema::schema_match_score;
use crate::strict::strict_parse;
use crate::types::{
    Candidate, CandidateDiagnostics, CandidateValidations, InputStats, Metrics, ParseError,
    PartialResult, RepairAction, RepairOptions, RepairResult,
};

fn extraction_debug_json(
    extracted_span: (usize, usize),
    truncated: bool,
    method: &str,
    repairs: &[RepairAction],
) -> JsonValue {
    JsonValue::Object(vec![
        ("method".to_string(), JsonValue::String(method.to_string())),
        (
            "span".to_string(),
            JsonValue::Array(vec![
                JsonValue::NumberU64(extracted_span.0 as u64),
                JsonValue::NumberU64(extracted_span.1 as u64),
            ]),
        ),
        ("truncated".to_string(), JsonValue::Bool(truncated)),
        (
            "repairs".to_string(),
            JsonValue::Array(repairs.iter().map(|r| r.to_json_value()).collect()),
        ),
    ])
}

fn is_ws_byte(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | b'\r' | b' ')
}

fn trim_ws_bytes(data: &[u8]) -> (usize, usize) {
    let mut start = 0usize;
    let mut end = data.len();
    // UTF-8 BOM
    if end >= 3 && &data[..3] == b"\xEF\xBB\xBF" {
        start = 3;
    }
    while start < end && is_ws_byte(data[start]) {
        start += 1;
    }
    while end > start && is_ws_byte(data[end - 1]) {
        end -= 1;
    }
    (start, end)
}

fn allow_parallel_is_false(s: &str) -> bool {
    let v = s.trim().to_ascii_lowercase();
    v == "false" || v == "0" || v == "no"
}

fn sum_cost(repairs: &[RepairAction]) -> f64 {
    repairs.iter().map(|r| r.cost_delta).sum()
}

fn rank_candidates(mut candidates: Vec<Candidate>) -> Vec<Candidate> {
    fn dropped_bytes(c: &Candidate) -> usize {
        c.dropped_spans
            .iter()
            .map(|(s, e)| e.saturating_sub(*s))
            .sum()
    }

    candidates.sort_by(|a, b| {
        let schema_a = a.validations.schema_match.unwrap_or(0.0);
        let schema_b = b.validations.schema_match.unwrap_or(0.0);
        let ord = schema_b.total_cmp(&schema_a);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = b.confidence.total_cmp(&a.confidence);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = a.cost.total_cmp(&b.cost);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = a
            .diagnostics
            .deleted_tokens
            .cmp(&b.diagnostics.deleted_tokens);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = a
            .diagnostics
            .close_open_string_count
            .cmp(&b.diagnostics.close_open_string_count);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = dropped_bytes(a).cmp(&dropped_bytes(b));
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let norm_len_a = a.normalized_json.as_ref().map(|s| s.len()).unwrap_or(0);
        let norm_len_b = b.normalized_json.as_ref().map(|s| s.len()).unwrap_or(0);
        let ord = norm_len_b.cmp(&norm_len_a);
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        let ord = a.repairs.len().cmp(&b.repairs.len());
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
        a.candidate_id.cmp(&b.candidate_id)
    });

    for (i, c) in candidates.iter_mut().enumerate() {
        c.candidate_id = i;
    }
    candidates
}

pub fn arbiter_parse(
    input_text_or_bytes: impl AsRef<[u8]>,
    options: Option<&RepairOptions>,
) -> RepairResult {
    let opt = options.cloned().unwrap_or_else(RepairOptions::default);
    parse_bytes(input_text_or_bytes.as_ref(), &opt)
}

pub fn parse(input_text: &str, options: &RepairOptions) -> RepairResult {
    parse_bytes(input_text.as_bytes(), options)
}

pub fn parse_bytes(input_bytes: &[u8], options: &RepairOptions) -> RepairResult {
    let t0 = Instant::now();
    let input_size = input_bytes.len();

    if options.mode == "auto"
        && !allow_parallel_is_false(&options.allow_parallel)
        && input_size >= options.parallel_threshold_bytes
    {
        let (s0, e0) = trim_ws_bytes(input_bytes);
        if matches!(input_bytes.get(s0), Some(b'[') | Some(b'{')) && e0 > s0 {
            if options.scale_output == "tape" {
                if let Ok((tape, plan)) = parse_root_array_scale_tape(input_bytes, options) {
                    let elapsed = t0.elapsed().as_millis();
                    let mut ir_pairs = vec![
                        (
                            "split_mode".to_string(),
                            JsonValue::String(plan.mode.to_string()),
                        ),
                        (
                            "chunks".to_string(),
                            JsonValue::NumberU64(plan.chunk_count as u64),
                        ),
                        (
                            "elements".to_string(),
                            JsonValue::NumberU64(plan.elements as u64),
                        ),
                    ];
                    ir_pairs.push((
                        "tape".to_string(),
                        tape.to_json_value(if options.debug { Some(10_000) } else { None }),
                    ));
                    let candidate = Candidate {
                        candidate_id: 0,
                        value: None,
                        normalized_json: None,
                        ir: Some(JsonValue::Object(ir_pairs)),
                        confidence: 1.0,
                        cost: 0.0,
                        repairs: Vec::new(),
                        validations: CandidateValidations {
                            strict_json_parse: true,
                            schema_match: None,
                        },
                        diagnostics: CandidateDiagnostics {
                            beam_width: Some(0),
                            max_repairs: Some(0),
                            ..CandidateDiagnostics::default()
                        },
                        dropped_spans: Vec::new(),
                    };
                    let mut metrics = Metrics::new("auto_scale");
                    metrics.elapsed_ms = elapsed;
                    metrics.split_mode = plan.mode.to_string();
                    metrics.parallel_workers = options.parallel_workers.unwrap_or(0);
                    metrics.elements = plan.elements;
                    metrics.structural_density = plan.structural_density;

                    return RepairResult {
                        status: "strict_ok".to_string(),
                        best_index: Some(0),
                        input_stats: InputStats {
                            input_bytes: input_size,
                            extracted_span: (0, input_size),
                            prefix_skipped_bytes: 0,
                            suffix_skipped_bytes: 0,
                        },
                        candidates: vec![candidate],
                        partial: None,
                        errors: Vec::new(),
                        metrics,
                        debug: None,
                    };
                }
            } else if let Ok((value, plan)) = parse_root_array_scale(input_bytes, options) {
                let elapsed = t0.elapsed().as_millis();
                let candidate = Candidate {
                    candidate_id: 0,
                    value: Some(value),
                    normalized_json: None,
                    ir: Some(JsonValue::Object(vec![
                        (
                            "split_mode".to_string(),
                            JsonValue::String(plan.mode.to_string()),
                        ),
                        (
                            "chunks".to_string(),
                            JsonValue::NumberU64(plan.chunk_count as u64),
                        ),
                        (
                            "elements".to_string(),
                            JsonValue::NumberU64(plan.elements as u64),
                        ),
                    ])),
                    confidence: 1.0,
                    cost: 0.0,
                    repairs: Vec::new(),
                    validations: CandidateValidations {
                        strict_json_parse: true,
                        schema_match: None,
                    },
                    diagnostics: CandidateDiagnostics {
                        beam_width: Some(0),
                        max_repairs: Some(0),
                        ..CandidateDiagnostics::default()
                    },
                    dropped_spans: Vec::new(),
                };
                let mut metrics = Metrics::new("auto_scale");
                metrics.elapsed_ms = elapsed;
                metrics.split_mode = plan.mode.to_string();
                metrics.parallel_workers = options.parallel_workers.unwrap_or(0);
                metrics.elements = plan.elements;
                metrics.structural_density = plan.structural_density;

                return RepairResult {
                    status: "strict_ok".to_string(),
                    best_index: Some(0),
                    input_stats: InputStats {
                        input_bytes: input_size,
                        extracted_span: (0, input_size),
                        prefix_skipped_bytes: 0,
                        suffix_skipped_bytes: 0,
                    },
                    candidates: vec![candidate],
                    partial: None,
                    errors: Vec::new(),
                    metrics,
                    debug: None,
                };
            }
        }
    }

    if options.mode == "scale_pipeline" {
        if options.scale_output == "tape" {
            match parse_root_array_scale_tape(input_bytes, options) {
                Ok((tape, plan)) => {
                    let elapsed = t0.elapsed().as_millis();
                    let mut ir_pairs = vec![
                        (
                            "split_mode".to_string(),
                            JsonValue::String(plan.mode.to_string()),
                        ),
                        (
                            "chunks".to_string(),
                            JsonValue::NumberU64(plan.chunk_count as u64),
                        ),
                        (
                            "elements".to_string(),
                            JsonValue::NumberU64(plan.elements as u64),
                        ),
                    ];
                    ir_pairs.push((
                        "tape".to_string(),
                        tape.to_json_value(if options.debug { Some(10_000) } else { None }),
                    ));
                    let candidate = Candidate {
                        candidate_id: 0,
                        value: None,
                        normalized_json: None,
                        ir: Some(JsonValue::Object(ir_pairs)),
                        confidence: 1.0,
                        cost: 0.0,
                        repairs: Vec::new(),
                        validations: CandidateValidations {
                            strict_json_parse: true,
                            schema_match: None,
                        },
                        diagnostics: CandidateDiagnostics {
                            beam_width: Some(0),
                            max_repairs: Some(0),
                            ..CandidateDiagnostics::default()
                        },
                        dropped_spans: Vec::new(),
                    };
                    let mut metrics = Metrics::new("scale_pipeline");
                    metrics.elapsed_ms = elapsed;
                    metrics.split_mode = plan.mode.to_string();
                    metrics.parallel_workers = options.parallel_workers.unwrap_or(0);
                    metrics.elements = plan.elements;
                    metrics.structural_density = plan.structural_density;

                    return RepairResult {
                        status: "strict_ok".to_string(),
                        best_index: Some(0),
                        input_stats: InputStats {
                            input_bytes: input_size,
                            extracted_span: (0, input_size),
                            prefix_skipped_bytes: 0,
                            suffix_skipped_bytes: 0,
                        },
                        candidates: vec![candidate],
                        partial: None,
                        errors: Vec::new(),
                        metrics,
                        debug: None,
                    };
                }
                Err(e) => {
                    let elapsed = t0.elapsed().as_millis();
                    return RepairResult {
                        status: "failed".to_string(),
                        best_index: None,
                        input_stats: InputStats {
                            input_bytes: input_size,
                            extracted_span: (0, input_size),
                            prefix_skipped_bytes: 0,
                            suffix_skipped_bytes: 0,
                        },
                        candidates: Vec::new(),
                        partial: None,
                        errors: vec![ParseError {
                            kind: "ScalePipelineError".to_string(),
                            at: None,
                            message: Some(e),
                        }],
                        metrics: Metrics {
                            elapsed_ms: elapsed,
                            ..Metrics::new("scale_pipeline")
                        },
                        debug: None,
                    };
                }
            }
        }
        match parse_root_array_scale(input_bytes, options) {
            Ok((value, plan)) => {
                let elapsed = t0.elapsed().as_millis();
                let candidate = Candidate {
                    candidate_id: 0,
                    value: Some(value),
                    normalized_json: None,
                    ir: Some(JsonValue::Object(vec![
                        (
                            "split_mode".to_string(),
                            JsonValue::String(plan.mode.to_string()),
                        ),
                        (
                            "chunks".to_string(),
                            JsonValue::NumberU64(plan.chunk_count as u64),
                        ),
                        (
                            "elements".to_string(),
                            JsonValue::NumberU64(plan.elements as u64),
                        ),
                    ])),
                    confidence: 1.0,
                    cost: 0.0,
                    repairs: Vec::new(),
                    validations: CandidateValidations {
                        strict_json_parse: true,
                        schema_match: None,
                    },
                    diagnostics: CandidateDiagnostics {
                        beam_width: Some(0),
                        max_repairs: Some(0),
                        ..CandidateDiagnostics::default()
                    },
                    dropped_spans: Vec::new(),
                };
                let mut metrics = Metrics::new("scale_pipeline");
                metrics.elapsed_ms = elapsed;
                metrics.split_mode = plan.mode.to_string();
                metrics.parallel_workers = options.parallel_workers.unwrap_or(0);
                metrics.elements = plan.elements;
                metrics.structural_density = plan.structural_density;

                return RepairResult {
                    status: "strict_ok".to_string(),
                    best_index: Some(0),
                    input_stats: InputStats {
                        input_bytes: input_size,
                        extracted_span: (0, input_size),
                        prefix_skipped_bytes: 0,
                        suffix_skipped_bytes: 0,
                    },
                    candidates: vec![candidate],
                    partial: None,
                    errors: Vec::new(),
                    metrics,
                    debug: None,
                };
            }
            Err(e) => {
                let elapsed = t0.elapsed().as_millis();
                return RepairResult {
                    status: "failed".to_string(),
                    best_index: None,
                    input_stats: InputStats {
                        input_bytes: input_size,
                        extracted_span: (0, input_size),
                        prefix_skipped_bytes: 0,
                        suffix_skipped_bytes: 0,
                    },
                    candidates: Vec::new(),
                    partial: None,
                    errors: vec![ParseError {
                        kind: "ScalePipelineError".to_string(),
                        at: None,
                        message: Some(e),
                    }],
                    metrics: Metrics {
                        elapsed_ms: elapsed,
                        ..Metrics::new("scale_pipeline")
                    },
                    debug: None,
                };
            }
        }
    }

    let text = String::from_utf8_lossy(input_bytes).to_string();
    let extraction = extract_json_candidate(&text);
    let extracted = extraction.extracted.clone();
    let input_stats = InputStats {
        input_bytes: input_size,
        extracted_span: extraction.span,
        prefix_skipped_bytes: extraction.span.0,
        suffix_skipped_bytes: text.len().saturating_sub(extraction.span.1),
    };
    let extraction_repairs = extraction.repairs.clone();

    let strict_res = strict_parse(&extracted);
    if let Ok(value) = strict_res {
        let normalized = value.to_compact_string();
        let cost = sum_cost(&extraction_repairs);
        let confidence = if cost <= 0.0 {
            1.0
        } else {
            (-options.confidence_alpha * cost).exp()
        };
        let status = if extraction_repairs.is_empty() {
            "strict_ok".to_string()
        } else {
            "repaired".to_string()
        };
        let schema = schema_match_score(&value, options.schema.as_ref());
        let candidate = Candidate {
            candidate_id: 0,
            value: Some(value),
            normalized_json: Some(normalized),
            ir: None,
            confidence,
            cost,
            repairs: extraction_repairs,
            validations: CandidateValidations {
                strict_json_parse: true,
                schema_match: schema,
            },
            diagnostics: CandidateDiagnostics {
                beam_width: Some(0),
                max_repairs: Some(0),
                ..CandidateDiagnostics::default()
            },
            dropped_spans: Vec::new(),
        };
        let elapsed = t0.elapsed().as_millis();
        return RepairResult {
            status,
            best_index: Some(0),
            input_stats,
            candidates: vec![candidate],
            partial: None,
            errors: Vec::new(),
            metrics: Metrics {
                elapsed_ms: elapsed,
                ..Metrics::new("strict")
            },
            debug: if options.debug {
                Some(JsonValue::Object(vec![(
                    "extraction".to_string(),
                    extraction_debug_json(
                        extraction.span,
                        extraction.truncated,
                        &extraction.method,
                        &extraction.repairs,
                    ),
                )]))
            } else {
                None
            },
        };
    }

    let mut last_err = strict_res.err();

    if options.mode == "strict_only" {
        let elapsed = t0.elapsed().as_millis();
        return RepairResult {
            status: "failed".to_string(),
            best_index: None,
            input_stats,
            candidates: Vec::new(),
            partial: None,
            errors: vec![ParseError {
                kind: "JSONDecodeError".to_string(),
                at: last_err.as_ref().map(|e| e.pos),
                message: last_err.as_ref().map(|e| e.message.clone()),
            }],
            metrics: Metrics {
                elapsed_ms: elapsed,
                ..Metrics::new("strict_only")
            },
            debug: if options.debug {
                Some(JsonValue::Object(vec![(
                    "extraction".to_string(),
                    extraction_debug_json(
                        extraction.span,
                        extraction.truncated,
                        &extraction.method,
                        &extraction.repairs,
                    ),
                )]))
            } else {
                None
            },
        };
    }

    let (repaired_text, heuristic_repairs) = heuristic_repair(&extracted, options);
    let mut base_repairs: Vec<RepairAction> = Vec::new();
    base_repairs.extend_from_slice(&extraction_repairs);
    base_repairs.extend_from_slice(&heuristic_repairs);

    if repaired_text != extracted {
        match strict_parse(&repaired_text) {
            Ok(value2) => {
                let normalized2 = value2.to_compact_string();
                let cost = sum_cost(&base_repairs);
                let confidence = if cost <= 0.0 {
                    1.0
                } else {
                    (-options.confidence_alpha * cost).exp()
                };
                let schema = schema_match_score(&value2, options.schema.as_ref());
                let candidate2 = Candidate {
                    candidate_id: 0,
                    value: Some(value2),
                    normalized_json: Some(normalized2),
                    ir: None,
                    confidence,
                    cost,
                    repairs: base_repairs,
                    validations: CandidateValidations {
                        strict_json_parse: true,
                        schema_match: schema,
                    },
                    diagnostics: CandidateDiagnostics {
                        beam_width: Some(0),
                        max_repairs: Some(0),
                        ..CandidateDiagnostics::default()
                    },
                    dropped_spans: Vec::new(),
                };
                let elapsed = t0.elapsed().as_millis();
                return RepairResult {
                    status: "repaired".to_string(),
                    best_index: Some(0),
                    input_stats,
                    candidates: vec![candidate2],
                    partial: None,
                    errors: Vec::new(),
                    metrics: Metrics {
                        elapsed_ms: elapsed,
                        ..Metrics::new("fast_repair")
                    },
                    debug: if options.debug {
                        Some(JsonValue::Object(vec![(
                            "extraction".to_string(),
                            extraction_debug_json(
                                extraction.span,
                                extraction.truncated,
                                &extraction.method,
                                &extraction.repairs,
                            ),
                        )]))
                    } else {
                        None
                    },
                };
            }
            Err(e2) => {
                last_err = Some(e2);
            }
        }
    }

    if options.mode == "fast_repair" {
        let elapsed = t0.elapsed().as_millis();
        return RepairResult {
            status: "failed".to_string(),
            best_index: None,
            input_stats,
            candidates: Vec::new(),
            partial: None,
            errors: vec![ParseError {
                kind: "JSONDecodeError".to_string(),
                at: last_err.as_ref().map(|e| e.pos),
                message: last_err.as_ref().map(|e| e.message.clone()),
            }],
            metrics: Metrics {
                elapsed_ms: elapsed,
                ..Metrics::new("fast_repair")
            },
            debug: if options.debug {
                Some(JsonValue::Object(vec![(
                    "extraction".to_string(),
                    extraction_debug_json(
                        extraction.span,
                        extraction.truncated,
                        &extraction.method,
                        &extraction.repairs,
                    ),
                )]))
            } else {
                None
            },
        };
    }

    // Probabilistic repair (Top-K). Run on the heuristic-normalized text to reduce search space.
    let mut beam_candidates = probabilistic_repair(&repaired_text, options, &base_repairs);
    if let Some(schema) = options.schema.as_ref() {
        for c in beam_candidates.iter_mut() {
            if let Some(v) = c.value.as_ref() {
                c.validations.schema_match = schema_match_score(v, Some(schema));
            }
        }
    }
    beam_candidates = rank_candidates(beam_candidates);

    let mut llm_calls: usize = 0;
    let mut llm_time_ms: u128 = 0;
    let mut llm_trigger: Option<String> = None;
    if options.allow_llm {
        match maybe_llm_rerun(
            &repaired_text,
            &base_repairs,
            &beam_candidates,
            last_err.as_ref().map(|e| e.pos),
            options,
        ) {
            Ok((mut llm_candidates, calls, ms, trigger)) => {
                llm_calls += calls;
                llm_time_ms += ms;
                llm_trigger = trigger;
                if let Some(schema) = options.schema.as_ref() {
                    for c in llm_candidates.iter_mut() {
                        if let Some(v) = c.value.as_ref() {
                            c.validations.schema_match = schema_match_score(v, Some(schema));
                        }
                    }
                }
                if !llm_candidates.is_empty() {
                    beam_candidates.extend(llm_candidates);
                    beam_candidates = rank_candidates(beam_candidates);
                }
            }
            Err(_) => {
                // Best-effort: ignore LLM errors and keep original candidates.
            }
        }
    }

    let elapsed = t0.elapsed().as_millis();
    if beam_candidates.is_empty() {
        let mut metrics = Metrics::new("probabilistic");
        metrics.elapsed_ms = elapsed;
        metrics.beam_width = options.beam_width;
        metrics.max_repairs = options.max_repairs;
        metrics.llm_calls = llm_calls;
        metrics.llm_time_ms = llm_time_ms;
        metrics.llm_trigger = llm_trigger.clone();
        return RepairResult {
            status: "failed".to_string(),
            best_index: None,
            input_stats,
            candidates: Vec::new(),
            partial: None,
            errors: vec![ParseError {
                kind: "UnrepairableJSON".to_string(),
                at: last_err.as_ref().map(|e| e.pos),
                message: last_err.as_ref().map(|e| e.message.clone()),
            }],
            metrics,
            debug: if options.debug {
                Some(JsonValue::Object(vec![(
                    "extraction".to_string(),
                    extraction_debug_json(
                        extraction.span,
                        extraction.truncated,
                        &extraction.method,
                        &extraction.repairs,
                    ),
                )]))
            } else {
                None
            },
        };
    }

    let best = beam_candidates[0].clone();
    let mut status = "repaired".to_string();
    let mut partial: Option<PartialResult> = None;
    if extraction.truncated || !best.dropped_spans.is_empty() {
        status = "partial".to_string();
        if options.partial_ok {
            partial = Some(PartialResult {
                extracted: best.value.clone(),
                dropped_spans: best.dropped_spans.clone(),
            });
        }
    }

    let mut metrics = Metrics::new("probabilistic");
    metrics.elapsed_ms = elapsed;
    metrics.beam_width = options.beam_width;
    metrics.max_repairs = options.max_repairs;
    metrics.llm_calls = llm_calls;
    metrics.llm_time_ms = llm_time_ms;
    metrics.llm_trigger = llm_trigger;

    beam_candidates.truncate(options.top_k);
    RepairResult {
        status,
        best_index: Some(0),
        input_stats,
        candidates: beam_candidates,
        partial,
        errors: Vec::new(),
        metrics,
        debug: if options.debug {
            Some(JsonValue::Object(vec![(
                "extraction".to_string(),
                extraction_debug_json(
                    extraction.span,
                    extraction.truncated,
                    &extraction.method,
                    &extraction.repairs,
                ),
            )]))
        } else {
            None
        },
    }
}
