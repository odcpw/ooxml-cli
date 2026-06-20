use crate::{CliError, CliResult, builtin_num_format_code};

#[derive(Clone)]
pub(super) struct XlsxNumberFormatSpec {
    pub(super) preset: String,
    pub(super) format_code: String,
    pub(super) number_format_id: u32,
    pub(super) builtin: bool,
}

pub(super) fn resolve_xlsx_number_format(
    preset: Option<&str>,
    format_code: Option<&str>,
    decimals: i64,
    currency_symbol: Option<&str>,
) -> CliResult<XlsxNumberFormatSpec> {
    let preset = preset.unwrap_or_default().trim().to_ascii_lowercase();
    let format_code = format_code.unwrap_or_default().trim();
    if preset.is_empty() == format_code.is_empty() {
        return Err(CliError::invalid_args(
            "specify exactly one of preset or format code",
        ));
    }
    if !(0..=10).contains(&decimals) {
        return Err(CliError::invalid_args("decimals must be between 0 and 10"));
    }
    if !format_code.is_empty() {
        return Ok(XlsxNumberFormatSpec {
            preset: "custom".to_string(),
            format_code: format_code.to_string(),
            number_format_id: 0,
            builtin: false,
        });
    }
    match preset.as_str() {
        "general" => builtin_xlsx_number_format_spec("general", 0),
        "integer" => builtin_xlsx_number_format_spec("integer", 3),
        "number" => {
            let code = fixed_decimal_format("#,##0", decimals);
            match decimals {
                0 => builtin_xlsx_number_format_spec("number", 3),
                2 => builtin_xlsx_number_format_spec("number", 4),
                _ => custom_xlsx_number_format_spec("number", &code),
            }
        }
        "percent" => {
            let code = format!("{}%", fixed_decimal_format("0", decimals));
            match decimals {
                0 => builtin_xlsx_number_format_spec("percent", 9),
                2 => builtin_xlsx_number_format_spec("percent", 10),
                _ => custom_xlsx_number_format_spec("percent", &code),
            }
        }
        "currency" => {
            let symbol = currency_symbol.unwrap_or("$");
            let code = format!(
                "{}{}",
                xlsx_format_literal(symbol),
                fixed_decimal_format("#,##0", decimals)
            );
            custom_xlsx_number_format_spec("currency", &code)
        }
        "date" => custom_xlsx_number_format_spec("date", "yyyy-mm-dd"),
        "datetime" => custom_xlsx_number_format_spec("datetime", "yyyy-mm-dd h:mm"),
        "text" => builtin_xlsx_number_format_spec("text", 49),
        _ => Err(CliError::invalid_args(format!(
            "invalid preset {:?} (must be integer, number, currency, percent, date, datetime, text, or general)",
            preset
        ))),
    }
}

fn builtin_xlsx_number_format_spec(
    preset: &str,
    number_format_id: u32,
) -> CliResult<XlsxNumberFormatSpec> {
    let code = builtin_num_format_code(number_format_id).ok_or_else(|| {
        CliError::unexpected(format!(
            "unknown built-in number format id {number_format_id}"
        ))
    })?;
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id,
        builtin: true,
    })
}

fn custom_xlsx_number_format_spec(preset: &str, code: &str) -> CliResult<XlsxNumberFormatSpec> {
    Ok(XlsxNumberFormatSpec {
        preset: preset.to_string(),
        format_code: code.to_string(),
        number_format_id: 0,
        builtin: false,
    })
}

fn fixed_decimal_format(base: &str, decimals: i64) -> String {
    if decimals == 0 {
        base.to_string()
    } else {
        format!("{base}.{}", "0".repeat(decimals as usize))
    }
}

fn xlsx_format_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
