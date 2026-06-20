pub(crate) const EXIT_SUCCESS: i32 = 0;
pub(crate) const EXIT_UNEXPECTED: i32 = 1;
pub(crate) const EXIT_INVALID_ARGS: i32 = 2;
pub(crate) const EXIT_FILE_NOT_FOUND: i32 = 3;
pub(crate) const EXIT_UNSUPPORTED_TYPE: i32 = 4;
pub(crate) const EXIT_VALIDATION_FAILED: i32 = 5;
pub(crate) const EXIT_TARGET_NOT_FOUND: i32 = 6;
pub(crate) const EXIT_RENDER_FAILED: i32 = 7;
pub(crate) const EXIT_PARTIAL_SUCCESS: i32 = 9;

#[derive(Debug)]
pub(crate) struct CliError {
    pub(crate) code: &'static str,
    pub(crate) exit_code: i32,
    pub(crate) message: String,
}

impl CliError {
    pub(crate) fn invalid_args(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_args",
            exit_code: EXIT_INVALID_ARGS,
            message: message.into(),
        }
    }

    pub(crate) fn file_not_found(message: impl Into<String>) -> Self {
        Self {
            code: "file_not_found",
            exit_code: EXIT_FILE_NOT_FOUND,
            message: message.into(),
        }
    }

    pub(crate) fn unexpected(message: impl Into<String>) -> Self {
        Self {
            code: "unexpected",
            exit_code: EXIT_UNEXPECTED,
            message: message.into(),
        }
    }

    pub(crate) fn unsupported_type(message: impl Into<String>) -> Self {
        Self {
            code: "unsupported_type",
            exit_code: EXIT_UNSUPPORTED_TYPE,
            message: message.into(),
        }
    }

    pub(crate) fn validation_failed(message: impl Into<String>) -> Self {
        Self {
            code: "validation_failed",
            exit_code: EXIT_VALIDATION_FAILED,
            message: message.into(),
        }
    }

    pub(crate) fn target_not_found(message: impl Into<String>) -> Self {
        Self {
            code: "target_not_found",
            exit_code: EXIT_TARGET_NOT_FOUND,
            message: message.into(),
        }
    }
}

pub(crate) type CliResult<T> = Result<T, CliError>;

#[derive(Default)]
pub(crate) struct GlobalFlags {
    pub(crate) json: bool,
    pub(crate) strict: bool,
}
