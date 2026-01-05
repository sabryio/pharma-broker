pub mod beam;
pub mod extract;
pub mod heuristic;
pub mod json;
pub mod lexer;
pub mod llm;
pub mod llm_fallback;
pub(crate) mod parallel_scan;
pub mod pipeline;
pub mod scale;
pub mod schema;
pub mod strict;
pub mod tape;
pub mod types;

pub use llm::{apply_patch_ops_utf8, build_llm_payload_json};
pub use pipeline::{arbiter_parse, parse, parse_bytes};
pub use scale::{SplitPlan, parse_root_array_scale};
pub use types::{Candidate, RepairAction, RepairOptions, RepairResult};
