#![no_main]

mod support;

use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let first = ooxml_cli::fuzzing::image(data);
    support::assert_teaching_error(&first);
    let second = ooxml_cli::fuzzing::image(data);
    assert_eq!(first, second, "image probing must be deterministic");
    if let Ok(probe) = first {
        assert!(probe["nativeWidth"].as_u64().is_some_and(|width| width > 0));
        assert!(
            probe["nativeHeight"]
                .as_u64()
                .is_some_and(|height| height > 0)
        );
        assert!(
            probe["exifOrientation"]
                .as_u64()
                .is_some_and(|orientation| (1..=8).contains(&orientation))
        );
        let _ = support::stable_json(&probe);
    }
});
