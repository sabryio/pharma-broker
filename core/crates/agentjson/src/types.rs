use crate::json::JsonValue;

#[derive(Debug, Clone, PartialEq)]
pub struct RepairAction {
    pub op: String,
    pub span: Option<(usize, usize)>,
    pub at: Option<usize>,
    pub token: Option<String>,
    pub cost_delta: f64,
    pub note: Option<String>,
}

impl RepairAction {
    pub fn new(op: &str, cost_delta: f64) -> Self {
        Self {
            op: op.to_string(),
            span: None,
            at: None,
            token: None,
            cost_delta,
            note: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateValidations {
    pub strict_json_parse: bool,
    pub schema_match: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CandidateDiagnostics {
    pub garbage_skipped_bytes: usize,
    pub deleted_tokens: usize,
    pub inserted_tokens: usize,
    pub close_open_string_count: usize,
    pub beam_width: Option<usize>,
    pub max_repairs: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub candidate_id: usize,
    pub value: Option<JsonValue>,
    pub normalized_json: Option<String>,
    pub ir: Option<JsonValue>,
    pub confidence: f64,
    pub cost: f64,
    pub repairs: Vec<RepairAction>,
    pub validations: CandidateValidations,
    pub diagnostics: CandidateDiagnostics,
    pub dropped_spans: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InputStats {
    pub input_bytes: usize,
    pub extracted_span: (usize, usize),
    pub prefix_skipped_bytes: usize,
    pub suffix_skipped_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartialResult {
    pub extracted: Option<JsonValue>,
    pub dropped_spans: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub kind: String,
    pub at: Option<usize>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Metrics {
    pub mode_used: String,
    pub elapsed_ms: u128,
    pub beam_width: usize,
    pub max_repairs: usize,
    pub llm_calls: usize,
    pub llm_time_ms: u128,
    pub llm_trigger: Option<String>,
    pub split_mode: String,
    pub parallel_workers: usize,
    pub elements: usize,
    pub structural_density: f64,
}

impl Metrics {
    pub fn new(mode_used: &str) -> Self {
        Self {
            mode_used: mode_used.to_string(),
            elapsed_ms: 0,
            beam_width: 0,
            max_repairs: 0,
            llm_calls: 0,
            llm_time_ms: 0,
            llm_trigger: None,
            split_mode: "".to_string(),
            parallel_workers: 0,
            elements: 0,
            structural_density: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepairOptions {
    pub mode: String, // auto|strict_only|fast_repair|probabilistic|scale_pipeline
    pub top_k: usize,
    pub beam_width: usize,
    pub max_repairs: usize,
    pub max_deleted_tokens: usize,
    pub max_close_open_string: usize,
    pub max_garbage_skip_bytes: usize,
    pub min_elements_for_parallel: usize,
    pub density_threshold: f64,
    pub parallel_chunk_bytes: usize,
    pub parallel_workers: Option<usize>,
    pub parallel_backend: String, // process|thread
    pub scale_output: String,     // dom|tape
    pub scale_target_keys: Option<Vec<String>>,
    pub partial_ok: bool,
    pub allow_single_quotes: bool,
    pub allow_unquoted_keys: bool,
    pub allow_unquoted_values: bool,
    pub allow_comments: bool,
    pub allow_python_literals: bool,
    pub allow_parallel: String, // auto|true|false
    pub parallel_threshold_bytes: usize,
    pub allow_llm: bool,
    pub max_llm_calls_per_doc: usize,
    pub llm_timeout_ms: u64,
    pub llm_mode: String, // patch_suggest|token_suggest
    pub llm_min_confidence: f64,
    pub llm_command: Option<String>,
    pub confidence_alpha: f64,
    pub schema: Option<JsonValue>,
    pub deterministic_seed: u64,
    pub debug: bool,
}

impl Default for RepairOptions {
    fn default() -> Self {
        Self {
            mode: "auto".to_string(),
            top_k: 5,
            beam_width: 32,
            max_repairs: 20,
            max_deleted_tokens: 3,
            max_close_open_string: 1,
            max_garbage_skip_bytes: 8 * 1024,
            min_elements_for_parallel: 512,
            density_threshold: 0.001,
            parallel_chunk_bytes: 8 * 1024 * 1024,
            parallel_workers: None,
            parallel_backend: "process".to_string(),
            scale_output: "dom".to_string(),
            scale_target_keys: None,
            partial_ok: true,
            allow_single_quotes: true,
            allow_unquoted_keys: true,
            allow_unquoted_values: true,
            allow_comments: true,
            allow_python_literals: true,
            allow_parallel: "auto".to_string(),
            parallel_threshold_bytes: 1_000_000_000,
            allow_llm: false,
            max_llm_calls_per_doc: 2,
            llm_timeout_ms: 5000,
            llm_mode: "patch_suggest".to_string(),
            llm_min_confidence: 0.2,
            llm_command: None,
            confidence_alpha: 0.7,
            schema: None,
            deterministic_seed: 0,
            debug: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepairResult {
    pub status: String, // strict_ok|repaired|partial|failed
    pub best_index: Option<usize>,
    pub input_stats: InputStats,
    pub candidates: Vec<Candidate>,
    pub partial: Option<PartialResult>,
    pub errors: Vec<ParseError>,
    pub metrics: Metrics,
    pub debug: Option<JsonValue>,
}

impl RepairResult {
    pub fn best(&self) -> Option<&Candidate> {
        self.best_index.and_then(|i| self.candidates.get(i))
    }

    pub fn to_json_string_pretty(&self, indent: usize) -> String {
        crate::json::pretty::to_pretty_json_string(&self.to_json_value(), indent)
    }

    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("status".to_string(), JsonValue::String(self.status.clone())),
            (
                "best_index".to_string(),
                self.best_index
                    .map(|i| JsonValue::NumberU64(i as u64))
                    .unwrap_or(JsonValue::Null),
            ),
            ("input_stats".to_string(), self.input_stats.to_json_value()),
            (
                "candidates".to_string(),
                JsonValue::Array(self.candidates.iter().map(|c| c.to_json_value()).collect()),
            ),
            (
                "partial".to_string(),
                self.partial
                    .as_ref()
                    .map(|p| p.to_json_value())
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "errors".to_string(),
                JsonValue::Array(self.errors.iter().map(|e| e.to_json_value()).collect()),
            ),
            ("metrics".to_string(), self.metrics.to_json_value()),
            (
                "debug".to_string(),
                self.debug.clone().unwrap_or(JsonValue::Null),
            ),
        ])
    }
}

impl InputStats {
    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "input_bytes".to_string(),
                JsonValue::NumberU64(self.input_bytes as u64),
            ),
            (
                "extracted_span".to_string(),
                JsonValue::Array(vec![
                    JsonValue::NumberU64(self.extracted_span.0 as u64),
                    JsonValue::NumberU64(self.extracted_span.1 as u64),
                ]),
            ),
            (
                "prefix_skipped_bytes".to_string(),
                JsonValue::NumberU64(self.prefix_skipped_bytes as u64),
            ),
            (
                "suffix_skipped_bytes".to_string(),
                JsonValue::NumberU64(self.suffix_skipped_bytes as u64),
            ),
        ])
    }
}

impl PartialResult {
    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "extracted".to_string(),
                self.extracted.clone().unwrap_or(JsonValue::Null),
            ),
            (
                "dropped_spans".to_string(),
                JsonValue::Array(
                    self.dropped_spans
                        .iter()
                        .map(|(s, e)| {
                            JsonValue::Array(vec![
                                JsonValue::NumberU64(*s as u64),
                                JsonValue::NumberU64(*e as u64),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

impl ParseError {
    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("kind".to_string(), JsonValue::String(self.kind.clone())),
            (
                "at".to_string(),
                self.at
                    .map(|v| JsonValue::NumberU64(v as u64))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "message".to_string(),
                self.message
                    .clone()
                    .map(JsonValue::String)
                    .unwrap_or(JsonValue::Null),
            ),
        ])
    }
}

impl Metrics {
    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "mode_used".to_string(),
                JsonValue::String(self.mode_used.clone()),
            ),
            (
                "elapsed_ms".to_string(),
                JsonValue::NumberU64(self.elapsed_ms as u64),
            ),
            (
                "beam_width".to_string(),
                JsonValue::NumberU64(self.beam_width as u64),
            ),
            (
                "max_repairs".to_string(),
                JsonValue::NumberU64(self.max_repairs as u64),
            ),
            (
                "llm_calls".to_string(),
                JsonValue::NumberU64(self.llm_calls as u64),
            ),
            (
                "llm_time_ms".to_string(),
                JsonValue::NumberU64(self.llm_time_ms as u64),
            ),
            (
                "llm_trigger".to_string(),
                self.llm_trigger
                    .clone()
                    .map(JsonValue::String)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "split_mode".to_string(),
                JsonValue::String(self.split_mode.clone()),
            ),
            (
                "parallel_workers".to_string(),
                JsonValue::NumberU64(self.parallel_workers as u64),
            ),
            (
                "elements".to_string(),
                JsonValue::NumberU64(self.elements as u64),
            ),
            (
                "structural_density".to_string(),
                JsonValue::NumberF64(self.structural_density),
            ),
        ])
    }
}

impl Candidate {
    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "candidate_id".to_string(),
                JsonValue::NumberU64(self.candidate_id as u64),
            ),
            (
                "value".to_string(),
                self.value.clone().unwrap_or(JsonValue::Null),
            ),
            (
                "normalized_json".to_string(),
                self.normalized_json
                    .clone()
                    .map(JsonValue::String)
                    .unwrap_or(JsonValue::Null),
            ),
            ("ir".to_string(), self.ir.clone().unwrap_or(JsonValue::Null)),
            (
                "confidence".to_string(),
                JsonValue::NumberF64(self.confidence),
            ),
            ("cost".to_string(), JsonValue::NumberF64(self.cost)),
            (
                "repairs".to_string(),
                JsonValue::Array(self.repairs.iter().map(|r| r.to_json_value()).collect()),
            ),
            ("validations".to_string(), self.validations.to_json_value()),
            ("diagnostics".to_string(), self.diagnostics.to_json_value()),
            (
                "dropped_spans".to_string(),
                JsonValue::Array(
                    self.dropped_spans
                        .iter()
                        .map(|(s, e)| {
                            JsonValue::Array(vec![
                                JsonValue::NumberU64(*s as u64),
                                JsonValue::NumberU64(*e as u64),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

impl CandidateValidations {
    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "strict_json_parse".to_string(),
                JsonValue::Bool(self.strict_json_parse),
            ),
            (
                "schema_match".to_string(),
                self.schema_match
                    .map(JsonValue::NumberF64)
                    .unwrap_or(JsonValue::Null),
            ),
        ])
    }
}

impl CandidateDiagnostics {
    pub fn to_json_value(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "garbage_skipped_bytes".to_string(),
                JsonValue::NumberU64(self.garbage_skipped_bytes as u64),
            ),
            (
                "deleted_tokens".to_string(),
                JsonValue::NumberU64(self.deleted_tokens as u64),
            ),
            (
                "inserted_tokens".to_string(),
                JsonValue::NumberU64(self.inserted_tokens as u64),
            ),
            (
                "close_open_string_count".to_string(),
                JsonValue::NumberU64(self.close_open_string_count as u64),
            ),
            (
                "beam_width".to_string(),
                self.beam_width
                    .map(|v| JsonValue::NumberU64(v as u64))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "max_repairs".to_string(),
                self.max_repairs
                    .map(|v| JsonValue::NumberU64(v as u64))
                    .unwrap_or(JsonValue::Null),
            ),
        ])
    }
}

impl RepairAction {
    pub fn to_json_value(&self) -> JsonValue {
        let span_v = self.span.map(|(s, e)| {
            JsonValue::Array(vec![
                JsonValue::NumberU64(s as u64),
                JsonValue::NumberU64(e as u64),
            ])
        });
        JsonValue::Object(vec![
            ("op".to_string(), JsonValue::String(self.op.clone())),
            ("span".to_string(), span_v.unwrap_or(JsonValue::Null)),
            (
                "at".to_string(),
                self.at
                    .map(|v| JsonValue::NumberU64(v as u64))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "token".to_string(),
                self.token
                    .clone()
                    .map(JsonValue::String)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "cost_delta".to_string(),
                JsonValue::NumberF64(self.cost_delta),
            ),
            (
                "note".to_string(),
                self.note
                    .clone()
                    .map(JsonValue::String)
                    .unwrap_or(JsonValue::Null),
            ),
        ])
    }
}
