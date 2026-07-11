use serde_json::Value;

use crate::command_manifest::{CommandId, XlsxCommandId};
use crate::{
    CliError, CliResult, XlsxCellsSetOptions, xlsx_cells_set, xlsx_sheets_list, xlsx_sheets_show,
};

pub(crate) fn xlsx_sheets_read(id: CommandId, file: &str, sheet: Option<&str>) -> CliResult<Value> {
    match id {
        CommandId::Xlsx(XlsxCommandId::SheetsList) => xlsx_sheets_list(file),
        CommandId::Xlsx(XlsxCommandId::SheetsShow) => xlsx_sheets_show(file, sheet),
        _ => Err(CliError::unexpected(
            "typed XLSX sheets read adapter received an unsupported command ID",
        )),
    }
}

pub(crate) fn xlsx_cells_set_by_id(
    id: CommandId,
    file: &str,
    options: XlsxCellsSetOptions<'_>,
) -> CliResult<Value> {
    match id {
        CommandId::Xlsx(XlsxCommandId::CellsSet) => xlsx_cells_set(file, options),
        _ => Err(CliError::unexpected(
            "typed XLSX cells set adapter received an unsupported command ID",
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

    #[test]
    fn cells_set_adapter_rejects_other_ids_before_file_access() {
        let error = xlsx_cells_set_by_id(
            CommandId::Xlsx(XlsxCommandId::CellsClear),
            "unused.xlsx",
            XlsxCellsSetOptions {
                sheet: None,
                cell: None,
                ref_: None,
                value: None,
                formula: None,
                value_type: None,
                out: None,
                backup: None,
                dry_run: false,
                no_validate: false,
                in_place: false,
            },
        )
        .expect_err("non-cells-set ID must be rejected before file access");
        assert_eq!(
            error.message,
            "typed XLSX cells set adapter received an unsupported command ID"
        );
    }
}
