use ooxml_cli::fuzzing::InputError;
use serde::Serialize;

pub fn assert_teaching_error<T>(result: &Result<T, InputError>) {
    if let Err(error) = result {
        assert!(
            !error.code.trim().is_empty(),
            "fuzz error code must be useful"
        );
        assert!(
            !error.message.trim().is_empty(),
            "fuzz error message must be useful"
        );
    }
}

pub fn stable_json<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("fuzz result remains JSON serializable")
}
