use std::env;
use std::fs::File;
use std::io::{self, Read};

use memmap2::{Mmap, MmapOptions};

use json_prob_parser::types::RepairOptions;

fn parse_usize(arg: &str, name: &str) -> usize {
    arg.parse::<usize>()
        .unwrap_or_else(|_| panic!("invalid {name}: {arg}"))
}

fn parse_u64(arg: &str, name: &str) -> u64 {
    arg.parse::<u64>()
        .unwrap_or_else(|_| panic!("invalid {name}: {arg}"))
}

fn parse_f64(arg: &str, name: &str) -> f64 {
    arg.parse::<f64>()
        .unwrap_or_else(|_| panic!("invalid {name}: {arg}"))
}

enum InputData {
    Owned(Vec<u8>),
    Mapped { _file: File, mmap: Mmap },
}

impl InputData {
    fn as_bytes(&self) -> &[u8] {
        match self {
            InputData::Owned(v) => v.as_slice(),
            InputData::Mapped { mmap, .. } => mmap.as_ref(),
        }
    }
}

fn read_input(input_path: Option<&str>, no_mmap: bool) -> io::Result<InputData> {
    match input_path {
        Some("-") | None => {
            let mut buf: Vec<u8> = Vec::new();
            io::stdin().read_to_end(&mut buf)?;
            Ok(InputData::Owned(buf))
        }
        Some(p) => {
            if no_mmap {
                return Ok(InputData::Owned(std::fs::read(p)?));
            }
            let file = File::open(p)?;
            let len = file.metadata()?.len();
            if len == 0 {
                return Ok(InputData::Owned(Vec::new()));
            }
            let mmap = unsafe { MmapOptions::new().map(&file)? };
            Ok(InputData::Mapped { _file: file, mmap })
        }
    }
}

pub fn run() -> i32 {
    let bin = env::args()
        .next()
        .unwrap_or_else(|| "agentjson".to_string());

    let mut mode = "auto".to_string();
    let mut scale_output = "dom".to_string();
    let mut top_k: usize = 5;
    let mut beam_width: usize = 32;
    let mut max_repairs: usize = 20;
    let mut max_deleted_tokens: usize = 3;
    let mut max_close_open_string: usize = 1;
    let mut max_garbage_skip_bytes: usize = 8 * 1024;
    let mut confidence_alpha: f64 = 0.7;
    let mut partial_ok: bool = true;
    let mut debug: bool = false;
    let mut deterministic_seed: u64 = 0;
    let mut allow_llm: bool = false;
    let mut llm_mode: String = "patch_suggest".to_string();
    let mut llm_min_confidence: f64 = 0.2;
    let mut llm_command: Option<String> = None;

    let mut min_elements_for_parallel: usize = 512;
    let mut density_threshold: f64 = 0.001;
    let mut parallel_chunk_bytes: usize = 8 * 1024 * 1024;
    let mut parallel_workers: usize = 0;
    let mut parallel_backend = "process".to_string();

    let mut input_path: Option<String> = None;
    let mut no_mmap: bool = false;

    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "--input" | "-i" => {
                i += 1;
                input_path = Some(args.get(i).expect("missing --input value").to_string());
            }
            "--mode" => {
                i += 1;
                mode = args.get(i).expect("missing --mode value").to_string();
            }
            "--scale-output" => {
                i += 1;
                scale_output = args
                    .get(i)
                    .expect("missing --scale-output value")
                    .to_string();
            }
            "--top-k" => {
                i += 1;
                top_k = parse_usize(args.get(i).expect("missing --top-k value"), "top_k");
            }
            "--beam-width" => {
                i += 1;
                beam_width = parse_usize(
                    args.get(i).expect("missing --beam-width value"),
                    "beam_width",
                );
            }
            "--max-repairs" => {
                i += 1;
                max_repairs = parse_usize(
                    args.get(i).expect("missing --max-repairs value"),
                    "max_repairs",
                );
            }
            "--max-deleted-tokens" => {
                i += 1;
                max_deleted_tokens = parse_usize(
                    args.get(i).expect("missing --max-deleted-tokens value"),
                    "max_deleted_tokens",
                );
            }
            "--max-close-open-string" => {
                i += 1;
                max_close_open_string = parse_usize(
                    args.get(i).expect("missing --max-close-open-string value"),
                    "max_close_open_string",
                );
            }
            "--max-garbage-skip-bytes" => {
                i += 1;
                max_garbage_skip_bytes = parse_usize(
                    args.get(i).expect("missing --max-garbage-skip-bytes value"),
                    "max_garbage_skip_bytes",
                );
            }
            "--confidence-alpha" => {
                i += 1;
                confidence_alpha = parse_f64(
                    args.get(i).expect("missing --confidence-alpha value"),
                    "confidence_alpha",
                );
            }
            "--partial-ok" => partial_ok = true,
            "--no-partial-ok" => partial_ok = false,
            "--debug" => debug = true,
            "--no-debug" => debug = false,
            "--deterministic-seed" => {
                i += 1;
                deterministic_seed = parse_u64(
                    args.get(i).expect("missing --deterministic-seed value"),
                    "deterministic_seed",
                );
            }
            "--allow-llm" => allow_llm = true,
            "--no-allow-llm" => allow_llm = false,
            "--llm-mode" => {
                i += 1;
                llm_mode = args.get(i).expect("missing --llm-mode value").to_string();
            }
            "--llm-min-confidence" => {
                i += 1;
                llm_min_confidence = parse_f64(
                    args.get(i).expect("missing --llm-min-confidence value"),
                    "llm_min_confidence",
                );
            }
            "--llm-command" => {
                i += 1;
                llm_command = Some(
                    args.get(i)
                        .expect("missing --llm-command value")
                        .to_string(),
                );
            }
            "--min-elements-for-parallel" => {
                i += 1;
                min_elements_for_parallel = parse_usize(
                    args.get(i)
                        .expect("missing --min-elements-for-parallel value"),
                    "min_elements_for_parallel",
                );
            }
            "--density-threshold" => {
                i += 1;
                density_threshold = parse_f64(
                    args.get(i).expect("missing --density-threshold value"),
                    "density_threshold",
                );
            }
            "--parallel-chunk-bytes" => {
                i += 1;
                parallel_chunk_bytes = parse_usize(
                    args.get(i).expect("missing --parallel-chunk-bytes value"),
                    "parallel_chunk_bytes",
                );
            }
            "--parallel-workers" => {
                i += 1;
                parallel_workers = parse_usize(
                    args.get(i).expect("missing --parallel-workers value"),
                    "parallel_workers",
                );
            }
            "--parallel-backend" => {
                i += 1;
                parallel_backend = args
                    .get(i)
                    .expect("missing --parallel-backend value")
                    .to_string();
            }
            "--no-mmap" => no_mmap = true,
            "--help" | "-h" => {
                eprintln!(
                    "Usage: {bin} [--input FILE|-] [--mode auto|strict_only|fast_repair|probabilistic|scale_pipeline] ...\n\
                     Reads stdin if no --input.\n\
                     Outputs JSON (pretty)."
                );
                return 0;
            }
            _ => {
                eprintln!("Unknown arg: {a}");
                return 2;
            }
        }
        i += 1;
    }

    let input = match read_input(input_path.as_deref(), no_mmap) {
        Ok(v) => v,
        Err(e) => {
            let p = input_path.as_deref().unwrap_or("-");
            eprintln!("failed to read input ({p}): {e}");
            return 2;
        }
    };

    let opt = RepairOptions {
        mode,
        scale_output,
        top_k,
        beam_width,
        max_repairs,
        max_deleted_tokens,
        max_close_open_string,
        max_garbage_skip_bytes,
        confidence_alpha,
        partial_ok,
        debug,
        deterministic_seed,
        min_elements_for_parallel,
        density_threshold,
        parallel_chunk_bytes,
        parallel_workers: if parallel_workers == 0 {
            None
        } else {
            Some(parallel_workers)
        },
        parallel_backend,
        allow_llm,
        llm_mode,
        llm_min_confidence,
        llm_command,
        ..RepairOptions::default()
    };

    let result = json_prob_parser::parse_bytes(input.as_bytes(), &opt);
    println!("{}", result.to_json_string_pretty(2));
    if result.status == "failed" { 2 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let uniq = std::process::id();
        p.push(format!("json_prob_parser_{uniq}_{name}"));
        p
    }

    #[test]
    fn mmap_and_read_match() {
        let path = tmp_file_path("mmap_test.json");
        let data = br#"{"a":[1,2,3],"b":"x"}"#;
        std::fs::write(&path, data).expect("write temp file");

        let mapped = read_input(Some(path.to_str().unwrap()), false).expect("mmap read");
        let owned = read_input(Some(path.to_str().unwrap()), true).expect("fs read");

        assert_eq!(mapped.as_bytes(), owned.as_bytes());

        let opt = RepairOptions::default();
        let r1 = json_prob_parser::parse_bytes(mapped.as_bytes(), &opt);
        let r2 = json_prob_parser::parse_bytes(owned.as_bytes(), &opt);
        assert_eq!(r1.status, r2.status);

        let _ = std::fs::remove_file(&path);
    }
}
