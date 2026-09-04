#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 256 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let first = ooxml_cli::fuzzing::markdown(data);
    support::assert_teaching_error(&first);
    let second = ooxml_cli::fuzzing::markdown(data);
    assert_eq!(first, second, "Markdown conversion must be deterministic");
    if let Ok(conversion) = first {
        assert!(conversion.spec.is_object());
        let _ = support::stable_json(&conversion);
    }
});
