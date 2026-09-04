#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 256 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let first = ooxml_cli::fuzzing::refs(data);
    support::assert_teaching_error(&first);
    let second = ooxml_cli::fuzzing::refs(data);
    assert_eq!(first, second, "$ref resolution must be deterministic");
    if let Ok(value) = first {
        let _ = support::stable_json(&value);
    }
});
