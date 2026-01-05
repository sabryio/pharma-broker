use json_prob_parser::scale::parse_root_array_scale_tape;
use json_prob_parser::tape::parse_strict_tape;
use json_prob_parser::types::RepairOptions;

#[test]
fn recursive_scale_tape_matches_strict_tape_entries() {
    let data = br#"  { "corpus": [ [1,2,3], [4,5,6], {"x":[7,8,9]} ], "y": {"z": [10,11]} }  "#;
    let opt = RepairOptions {
        mode: "scale_pipeline".to_string(),
        scale_output: "tape".to_string(),
        allow_parallel: "true".to_string(),
        parallel_workers: Some(4),
        parallel_threshold_bytes: 0,
        min_elements_for_parallel: 1,
        density_threshold: 0.0,
        ..RepairOptions::default()
    };

    let (scale_tape, _plan) = parse_root_array_scale_tape(data, &opt).expect("scale tape");
    let strict_tape = parse_strict_tape(data, 0).expect("strict tape");

    assert_eq!(scale_tape.root_index, strict_tape.root_index);
    assert_eq!(scale_tape.entries, strict_tape.entries);
}
