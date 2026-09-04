#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 256 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let first = ooxml_cli::fuzzing::brand(data);
    support::assert_teaching_error(&first);
    let second = ooxml_cli::fuzzing::brand(data);
    assert_eq!(first, second, "brand parsing must be deterministic");
    if let Ok(canonical) = first {
        assert!(canonical.is_object());
        let encoded = support::stable_json(&canonical);
        assert_eq!(ooxml_cli::fuzzing::brand(&encoded), Ok(canonical));
    }
});
