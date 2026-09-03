use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckFinding {
    pub(crate) severity: String,
    pub(crate) code: String,
    pub(crate) part: Value,
    pub(crate) location: Value,
    pub(crate) message: String,
    pub(crate) fix_command: String,
    pub(crate) docs: String,
}

impl CheckFinding {
    pub(crate) fn new(
        severity: impl Into<String>,
        code: impl Into<String>,
        part: Value,
        location: Value,
        message: impl Into<String>,
        fix_command: impl Into<String>,
        docs: impl Into<String>,
    ) -> Self {
        Self {
            severity: severity.into(),
            code: code.into(),
            part,
            location,
            message: message.into(),
            fix_command: fix_command.into(),
            docs: docs.into(),
        }
    }

    pub(crate) fn sort_key(&self) -> (u8, &str, String, String, &str) {
        (
            severity_rank(&self.severity),
            &self.code,
            serde_json::to_string(&self.part).unwrap_or_default(),
            serde_json::to_string(&self.location).unwrap_or_default(),
            &self.message,
        )
    }

    pub(crate) fn dedup_key(&self) -> String {
        serde_json::to_string(self).expect("serialize check finding")
    }
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "error" => 0,
        "warning" => 1,
        "info" => 2,
        _ => 3,
    }
}
