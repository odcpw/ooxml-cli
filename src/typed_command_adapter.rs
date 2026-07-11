use serde_json::Value;

use crate::command_manifest::{CommandId, XlsxCommandId};
use crate::{CliError, CliResult, xlsx_sheets_list, xlsx_sheets_show};

pub(crate) fn xlsx_sheets_read(id: CommandId, file: &str, sheet: Option<&str>) -> CliResult<Value> {
    match id {
        CommandId::Xlsx(XlsxCommandId::SheetsList) => xlsx_sheets_list(file),
        CommandId::Xlsx(XlsxCommandId::SheetsShow) => xlsx_sheets_show(file, sheet),
        _ => Err(CliError::unexpected(
            "typed XLSX sheets read adapter received an unsupported command ID",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_rejects_every_id_outside_the_guarded_pair() {
        let error = xlsx_sheets_read(
            CommandId::Xlsx(XlsxCommandId::SheetsAdd),
            "unused.xlsx",
            None,
        )
        .expect_err("non-read ID must be rejected before file access");
        assert_eq!(
            error.message,
            "typed XLSX sheets read adapter received an unsupported command ID"
        );
    }
}
