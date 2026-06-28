use serde_json::Value;

use crate::cli_args::{
    has_flag, output_path_arg, parse_string_flag, parse_string_flags, reject_unknown_flags,
};
use crate::cli_core::{CliError, CliResult};
use crate::{XlsxFormsEntryOptions, xlsx_forms_entry};

pub(super) fn dispatch_xlsx_forms(args: &[String]) -> CliResult<Value> {
    match args {
        [family, group, verb, rest @ ..]
            if family == "xlsx" && group == "forms" && verb == "entry" =>
        {
            let value_flags = ["--out", "--field", "--sheet", "--data-sheet", "--button"];
            let bool_flags = ["--force", "--no-validate"];
            reject_unknown_flags(rest, &value_flags, &bool_flags)?;
            let out = output_path_arg(rest, &value_flags, &bool_flags, "xlsx forms entry")?;
            let fields = parse_string_flags(rest, "--field")?;
            let sheet = parse_string_flag(rest, "--sheet")?;
            let data_sheet = parse_string_flag(rest, "--data-sheet")?;
            let button = parse_string_flag(rest, "--button")?;
            xlsx_forms_entry(XlsxFormsEntryOptions {
                out: &out,
                fields,
                form_sheet: sheet.as_deref(),
                data_sheet: data_sheet.as_deref(),
                button_caption: button.as_deref(),
                force: has_flag(rest, "--force"),
                no_validate: has_flag(rest, "--no-validate"),
            })
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported xlsx forms command: {}",
            args.join(" ")
        ))),
    }
}
