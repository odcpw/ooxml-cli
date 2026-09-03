use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Value, json};
use std::collections::BTreeSet;

use super::{finding::CheckFinding, fixed_path};
use crate::{
    CliError, CliResult, append_xml_text_event, command_arg, find_xlsx_workbook_part,
    is_xml_text_event, local_name, parse_range, workbook_sheets, xlsx_charts_list, xlsx_names_list,
    xlsx_pivots_list, xlsx_tables_list, zip_text,
};

const DOCS: &str = "docs/testing-strategy.md";

#[derive(Clone, Copy)]
struct FormulaFindingCodes {
    broken_reference: &'static str,
    missing_sheet: &'static str,
}

const FORMULA_CODES: FormulaFindingCodes = FormulaFindingCodes {
    broken_reference: "XLSX_FORMULA_BROKEN_REFERENCE",
    missing_sheet: "XLSX_FORMULA_MISSING_SHEET",
};
const DEFINED_NAME_CODES: FormulaFindingCodes = FormulaFindingCodes {
    broken_reference: "XLSX_DEFINED_NAME_BROKEN_REFERENCE",
    missing_sheet: "XLSX_DEFINED_NAME_REFERENCE_INVALID",
};
const CHART_SOURCE_CODES: FormulaFindingCodes = FormulaFindingCodes {
    broken_reference: "XLSX_CHART_SOURCE_BROKEN_REFERENCE",
    missing_sheet: "XLSX_CHART_SOURCE_INVALID",
};

pub(super) fn reference_findings(file: &str, entries: &[String]) -> CliResult<Vec<CheckFinding>> {
    let workbook_part = find_xlsx_workbook_part(file, entries)?;
    let workbook_xml = zip_text(file, &workbook_part)?;
    let sheets = workbook_sheets(&workbook_xml)?;
    let sheet_names = sheets
        .iter()
        .map(|sheet| sheet.name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut findings = Vec::new();

    scan_formula_parts(file, entries, &sheet_names, &mut findings)?;
    inspect_defined_names(file, &workbook_part, &sheet_names, &mut findings);
    inspect_tables(file, &sheet_names, &mut findings);
    inspect_charts(file, &sheet_names, &mut findings);
    inspect_pivots(file, &sheet_names, &mut findings);
    Ok(findings)
}

fn scan_formula_parts(
    file: &str,
    entries: &[String],
    sheet_names: &BTreeSet<String>,
    findings: &mut Vec<CheckFinding>,
) -> CliResult<()> {
    for part in entries
        .iter()
        .filter(|part| part.starts_with("xl/worksheets/") && part.ends_with(".xml"))
    {
        let xml = zip_text(file, part)?;
        for (index, formula) in formula_texts(&xml)?.into_iter().enumerate() {
            add_formula_findings(
                file,
                &format!("/{part}"),
                json!({"formula": index + 1}),
                &formula,
                sheet_names,
                FORMULA_CODES,
                findings,
            );
        }
    }
    Ok(())
}

fn formula_texts(xml: &str) -> CliResult<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut formulas = Vec::new();
    let mut in_formula = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == "f" => {
                formulas.push(String::new());
                in_formula = true;
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == "f" => {
                in_formula = false;
            }
            Ok(event) if in_formula && is_xml_text_event(&event) => {
                if let Some(formula) = formulas.last_mut() {
                    append_xml_text_event(formula, &event);
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(CliError::unexpected(error.to_string())),
            _ => {}
        }
    }
    Ok(formulas)
}

fn inspect_defined_names(
    file: &str,
    workbook_part: &str,
    sheet_names: &BTreeSet<String>,
    findings: &mut Vec<CheckFinding>,
) {
    match xlsx_names_list(file, None) {
        Ok(report) => {
            for name in array(&report, "names") {
                let reference = name["ref"].as_str().unwrap_or_default();
                let location = json!({"definedName": name["name"]});
                add_formula_findings(
                    file,
                    &format!("/{workbook_part}"),
                    location,
                    reference,
                    sheet_names,
                    DEFINED_NAME_CODES,
                    findings,
                );
            }
        }
        Err(error) => findings.push(component_failure(
            file,
            "XLSX_DEFINED_NAME_INTEGRITY",
            &format!("/{workbook_part}"),
            error,
            "ooxml --json xlsx names list",
        )),
    }
}

fn inspect_tables(file: &str, sheet_names: &BTreeSet<String>, findings: &mut Vec<CheckFinding>) {
    match xlsx_tables_list(file, None) {
        Ok(report) => {
            for table in array(&report, "tables") {
                let part = table["partUri"].clone();
                let range = table["range"].as_str().unwrap_or_default();
                let sheet = table["sheet"].as_str().unwrap_or_default();
                let invalid_range = range.is_empty() || parse_range(range).is_err();
                let missing_sheet =
                    !sheet.is_empty() && !sheet_names.contains(&sheet.to_ascii_lowercase());
                if invalid_range || missing_sheet {
                    findings.push(CheckFinding::new(
                        "error",
                        "XLSX_TABLE_REFERENCE_INVALID",
                        part,
                        json!({"table": table["name"], "sheet": sheet, "range": range}),
                        "table source must name an existing worksheet and a valid A1 range",
                        format!(
                            "ooxml --json xlsx tables show {} --table {}",
                            command_arg(file),
                            command_arg(table["primarySelector"].as_str().unwrap_or("1")),
                        ),
                        DOCS,
                    ));
                }
            }
        }
        Err(error) => findings.push(component_failure(
            file,
            "XLSX_TABLE_REFERENCE_INVALID",
            "/xl/tables",
            error,
            "ooxml --json xlsx tables list",
        )),
    }
}

fn inspect_charts(file: &str, sheet_names: &BTreeSet<String>, findings: &mut Vec<CheckFinding>) {
    match xlsx_charts_list(file, None) {
        Ok(report) => {
            for chart in array(&report, "charts") {
                for series in array(chart, "series") {
                    for role in [
                        "name",
                        "categories",
                        "values",
                        "xValues",
                        "yValues",
                        "bubbleSize",
                    ] {
                        let Some(source) = series.get(role).filter(|source| source.is_object())
                        else {
                            continue;
                        };
                        let formula = source["formula"].as_str().unwrap_or_default();
                        let location = json!({
                            "chart": chart["primarySelector"],
                            "series": series["number"],
                            "role": role,
                        });
                        add_formula_findings(
                            file,
                            chart["partUri"].as_str().unwrap_or("/xl/charts"),
                            location,
                            formula,
                            sheet_names,
                            CHART_SOURCE_CODES,
                            findings,
                        );
                    }
                }
            }
        }
        Err(error) => findings.push(component_failure(
            file,
            "XLSX_CHART_SOURCE_INVALID",
            "/xl/charts",
            error,
            "ooxml --json xlsx charts list",
        )),
    }
}

fn inspect_pivots(file: &str, sheet_names: &BTreeSet<String>, findings: &mut Vec<CheckFinding>) {
    match xlsx_pivots_list(file, None) {
        Ok(report) => {
            for pivot in array(&report, "pivots") {
                let source = &pivot["cache"]["source"];
                if !source.is_object() {
                    continue;
                }
                let sheet = source["sheet"].as_str().unwrap_or_default();
                let range = source["range"].as_str().unwrap_or_default();
                let invalid_sheet =
                    !sheet.is_empty() && !sheet_names.contains(&sheet.to_ascii_lowercase());
                let invalid_range = !range.is_empty() && parse_range(range).is_err();
                if invalid_sheet || invalid_range || (sheet.is_empty() && range.is_empty()) {
                    findings.push(CheckFinding::new(
                        "error",
                        "XLSX_PIVOT_SOURCE_INVALID",
                        pivot["partUri"].clone(),
                        json!({"pivot": pivot["primarySelector"], "source": source}),
                        "pivot cache source must resolve to a defined name or an existing worksheet range",
                        format!(
                            "ooxml --json xlsx pivots show {} --pivot {}",
                            command_arg(file),
                            command_arg(pivot["primarySelector"].as_str().unwrap_or("1")),
                        ),
                        DOCS,
                    ));
                }
            }
        }
        Err(error) => findings.push(component_failure(
            file,
            "XLSX_PIVOT_SOURCE_INVALID",
            "/xl/pivotTables",
            error,
            "ooxml --json xlsx pivots list",
        )),
    }
}

fn add_formula_findings(
    file: &str,
    part: &str,
    location: Value,
    formula: &str,
    sheet_names: &BTreeSet<String>,
    codes: FormulaFindingCodes,
    findings: &mut Vec<CheckFinding>,
) {
    if formula.contains("#REF!") {
        findings.push(CheckFinding::new(
            "error",
            codes.broken_reference,
            json!(part),
            location.clone(),
            format!("formula contains a broken #REF! reference: {formula}"),
            format!(
                "ooxml --json xlsx cells extract {} --include-formulas",
                command_arg(file)
            ),
            DOCS,
        ));
    }
    for sheet in referenced_sheets(formula) {
        if !sheet_names.contains(&sheet.to_ascii_lowercase()) {
            let fix_command = if codes.missing_sheet == CHART_SOURCE_CODES.missing_sheet {
                format!(
                    "ooxml --json xlsx sheets add {} --name {} --out {}",
                    command_arg(file),
                    command_arg(&sheet),
                    command_arg(&fixed_path(file, "chart-source-fixed")),
                )
            } else {
                format!("ooxml --json xlsx sheets list {}", command_arg(file))
            };
            findings.push(CheckFinding::new(
                "error",
                codes.missing_sheet,
                json!(part),
                location.clone(),
                format!("formula references missing worksheet {sheet:?}: {formula}"),
                fix_command,
                DOCS,
            ));
        }
    }
}

fn referenced_sheets(formula: &str) -> BTreeSet<String> {
    let mut sheets = BTreeSet::new();
    for (bang, _) in formula.match_indices('!') {
        let prefix = &formula[..bang];
        let token = if let Some(quoted) = prefix.strip_suffix('\'') {
            quoted
                .rfind('\'')
                .map(|start| &quoted[start + 1..])
                .unwrap_or_default()
        } else {
            let start = prefix
                .rfind(|character: char| {
                    matches!(
                        character,
                        '=' | '+' | '-' | '*' | '/' | '(' | ')' | ',' | ':' | ' '
                    )
                })
                .map_or(0, |index| index + 1);
            &prefix[start..]
        };
        let token = token.replace("''", "'");
        if !token.is_empty() && !token.contains(['[', ']']) {
            sheets.insert(token);
        }
    }
    sheets
}

fn component_failure(
    file: &str,
    code: &str,
    part: &str,
    error: CliError,
    inspect_command: &str,
) -> CheckFinding {
    CheckFinding::new(
        "error",
        code,
        json!(part),
        Value::Null,
        error.message,
        format!("{inspect_command} {}", command_arg(file)),
        DOCS,
    )
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_local_sheet_references_without_external_workbooks() {
        assert_eq!(
            referenced_sheets("SUM('Q1 Data'!$A$1,Sheet2!B3,[book.xlsx]Sheet3!C4)"),
            BTreeSet::from(["Q1 Data".to_string(), "Sheet2".to_string()])
        );
    }
}
