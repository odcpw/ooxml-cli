#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 512 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let first = ooxml_cli::fuzzing::build_spec(data);
    support::assert_teaching_error(&first);
    let second = ooxml_cli::fuzzing::build_spec(data);
    assert_eq!(first, second, "build-spec parsing must be deterministic");
    if let Ok(value) = first {
        assert!(value["family"].is_string());
        assert!(value["document"].is_object());
        assert!(value["operations"].is_array());
        let _ = support::stable_json(&value);
    }
});
