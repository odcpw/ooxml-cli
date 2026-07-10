use super::*;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn release_real_file_traces_cover_high_value_surfaces() {
    let temp_dir = trace_temp_dir("release-real-file-traces");
    let summary = serde_json::json!({
        "workflow": "release-real-file-traces",
        "scope": [
            "xlsx charts",
            "xlsx data-validations",
            "xlsx conditional-formats",
            "vba pure package/source",
            "pptx charts"
        ],
        "proofLevel": {
            "portableRust": [
                "real input fixtures",
                "real saved outputs",
                "strict validation",
                "conformance check",
                "readback commands",
                "semantic zip-part checks",
                "macro-preservation checks for XLSM mutation inputs"
            ],
            "officeCom": "not run by this portable Rust contract test"
        },
        "xlsxCharts": trace_xlsx_chart(&temp_dir),
        "xlsxDataValidations": trace_xlsx_data_validation(&temp_dir),
        "xlsxConditionalFormats": trace_xlsx_conditional_format(&temp_dir),
        "vba": trace_vba_matrix(&temp_dir),
        "pptxCharts": trace_pptx_chart(&temp_dir),
    });

    assert_release_trace_golden(&summary);
    let _ = std::fs::remove_dir_all(&temp_dir);
}

fn trace_xlsx_chart(temp_dir: &Path) -> Value {
    let output = temp_dir.join("xlsx-chart-style.xlsx");
    let output_str = path_string(&output);
    let mutation = run_json_ok(
        "xlsx chart set-series-style",
        &[
            "--json",
            "xlsx",
            "charts",
            "set-series-style",
            "testdata/xlsx/libreoffice-chart-workbook/workbook.xlsx",
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--fill-color",
            "FF8800",
            "--line-color",
            "114477",
            "--line-width-pt",
            "2",
            "--out",
            &output_str,
        ],
    );
    assert!(output.exists(), "XLSX chart output exists");
    assert_rust_emitted_ooxml_command_exits_zero(&mutation, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&mutation, "chartShowCommand");

    let validation = assert_strict_valid(&output_str);
    let conformance = assert_conformance_passed(&output_str);
    let show = run_json_ok(
        "xlsx chart readback",
        &[
            "--json",
            "xlsx",
            "charts",
            "show",
            &output_str,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
        ],
    );
    let chart = &show["charts"][0];
    let first_series = &chart["style"]["series"][0];
    assert_eq!(first_series["fillColor"], "FF8800");
    assert_eq!(first_series["lineColor"], "114477");
    assert_eq!(first_series["lineWidthPt"], 2);

    let chart_xml = read_zip_string(&output, "xl/charts/chart1.xml");
    for needle in ["FF8800", "114477", r#"w="25400""#] {
        assert!(
            chart_xml.contains(needle),
            "XLSX chart XML missing {needle:?}"
        );
    }
    let macro_preservation = trace_xlsm_chart_macro_preservation(temp_dir);

    serde_json::json!({
        "inputFixture": "testdata/xlsx/libreoffice-chart-workbook/workbook.xlsx",
        "producer": "LibreOffice headless re-export",
        "command": "xlsx charts set-series-style",
        "outputExtension": ".xlsx",
        "validationStatus": validation["status"],
        "conformanceStatus": conformance["status"],
        "followUps": follow_ups(&mutation, &["validateCommand", "chartShowCommand"]),
        "chart": {
            "primarySelector": chart["primarySelector"],
            "partUri": chart["partUri"],
            "title": chart["title"],
            "types": chart["types"],
            "seriesCount": chart["series"].as_array().expect("chart series").len(),
            "firstSeries": {
                "name": first_series["name"],
                "fillColor": first_series["fillColor"],
                "lineColor": first_series["lineColor"],
                "lineWidthPt": first_series["lineWidthPt"]
            }
        },
        "zipPartNeedles": ["FF8800", "114477", "w=\"25400\""],
        "macroPreservation": macro_preservation
    })
}

fn trace_xlsx_data_validation(temp_dir: &Path) -> Value {
    let created = temp_dir.join("data-validation-created.xlsx");
    let updated = temp_dir.join("data-validation-updated.xlsx");
    let created_str = path_string(&created);
    let updated_str = path_string(&updated);

    let create = run_json_ok(
        "data validation create",
        &[
            "--json",
            "xlsx",
            "data-validations",
            "create",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "A1:A10",
            "--type",
            "list",
            "--list-values",
            "Red,Green,Blue",
            "--show-input-message",
            "--input-title",
            "Pick",
            "--input-message",
            "Choose a color",
            "--out",
            &created_str,
        ],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&create, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&create, "dataValidationsShowCommand");
    assert_eq!(
        assert_strict_valid(&created_str)["valid"],
        Value::Bool(true)
    );

    let update = run_json_ok(
        "data validation update",
        &[
            "--json",
            "xlsx",
            "data-validations",
            "update",
            &created_str,
            "--sheet",
            "1",
            "--range",
            "A1:A10",
            "--list-values",
            "Red,Green,Blue,Amber",
            "--allow-blank",
            "--expect-type",
            "list",
            "--out",
            &updated_str,
        ],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&update, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&update, "dataValidationsShowCommand");

    let validation = assert_strict_valid(&updated_str);
    let conformance = assert_conformance_passed(&updated_str);
    let show = run_json_ok(
        "data validation readback",
        &[
            "--json",
            "xlsx",
            "data-validations",
            "show",
            &updated_str,
            "--sheet",
            "1",
            "--range",
            "A1:A10",
        ],
    );
    let rule = &show;
    assert_eq!(rule["type"], "list");
    assert_eq!(rule["formula1"], "\"Red,Green,Blue,Amber\"");
    assert_eq!(rule["allowBlank"], true);

    let sheet_xml = read_zip_string(&updated, "xl/worksheets/sheet1.xml");
    assert!(sheet_xml.contains(r#"type="list""#));
    assert!(sheet_xml.contains("Red,Green,Blue,Amber"));
    let macro_preservation = trace_xlsm_data_validation_macro_preservation(temp_dir);

    serde_json::json!({
        "inputFixture": "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "commands": ["xlsx data-validations create", "xlsx data-validations update", "xlsx data-validations show"],
        "outputExtension": ".xlsx",
        "validationStatus": validation["status"],
        "conformanceStatus": conformance["status"],
        "followUps": follow_ups(&update, &["validateCommand", "dataValidationsListCommand", "dataValidationsShowCommand"]),
        "rule": {
            "primarySelector": rule["primarySelector"],
            "sqref": rule["sqref"],
            "type": rule["type"],
            "formula1": rule["formula1"],
            "allowBlank": rule["allowBlank"],
            "showInputMessage": rule["showInputMessage"]
        },
        "zipPartNeedles": ["type=\"list\"", "Red,Green,Blue,Amber"],
        "macroPreservation": macro_preservation
    })
}

fn trace_xlsx_conditional_format(temp_dir: &Path) -> Value {
    let output = temp_dir.join("conditional-format.xlsx");
    let output_str = path_string(&output);
    let add = run_json_ok(
        "conditional format add",
        &[
            "--json",
            "xlsx",
            "conditional-formats",
            "add",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "--sheet",
            "1",
            "--range",
            "B1:B10",
            "--type",
            "cell-is",
            "--operator",
            "greaterThan",
            "--formula",
            "40",
            "--out",
            &output_str,
        ],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&add, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&add, "conditionalFormatsShowCommand");

    let validation = assert_strict_valid(&output_str);
    let conformance = assert_conformance_passed(&output_str);
    let show = run_json_ok(
        "conditional format readback",
        &[
            "--json",
            "xlsx",
            "conditional-formats",
            "show",
            &output_str,
            "--sheet",
            "1",
            "--rule",
            "cfRule:1",
        ],
    );
    assert_eq!(show["type"], "cellIs");
    assert_eq!(show["formula"], "40");
    assert_eq!(show["operator"], "greaterThan");

    let sheet_xml = read_zip_string(&output, "xl/worksheets/sheet1.xml");
    for needle in [
        r#"sqref="B1:B10""#,
        r#"type="cellIs""#,
        r#"operator="greaterThan""#,
        "<formula>40</formula>",
    ] {
        assert!(
            sheet_xml.contains(needle),
            "conditional-format XML missing {needle:?}"
        );
    }

    serde_json::json!({
        "inputFixture": "testdata/xlsx/minimal-workbook/workbook.xlsx",
        "command": "xlsx conditional-formats add",
        "outputExtension": ".xlsx",
        "validationStatus": validation["status"],
        "conformanceStatus": conformance["status"],
        "followUps": follow_ups(&add, &["validateCommand", "conditionalFormatsListCommand", "conditionalFormatsShowCommand"]),
        "rule": {
            "primarySelector": show["primarySelector"],
            "sqref": show["sqref"],
            "type": show["type"],
            "operator": show["operator"],
            "formula": show["formula"],
            "priority": show["priority"]
        },
        "zipPartNeedles": ["sqref=\"B1:B10\"", "type=\"cellIs\"", "operator=\"greaterThan\"", "<formula>40</formula>"]
    })
}

fn trace_vba_matrix(temp_dir: &Path) -> Value {
    let families = vec![
        trace_vba_family(
            temp_dir,
            "xlsx",
            "testdata/xlsx/minimal-workbook/workbook.xlsx",
            "trace.xlsm",
            &[
                "testdata/golden/vba-authoring/xlsx-class/AgentSmoke.bas",
                "testdata/golden/vba-authoring/xlsx-class/Worker.cls",
            ],
        ),
        trace_vba_family(
            temp_dir,
            "pptx",
            "testdata/pptx/minimal-title/presentation.pptx",
            "trace.pptm",
            &[
                "testdata/golden/vba-authoring/pptx-class/AgentSlide.bas",
                "testdata/golden/vba-authoring/pptx-class/Worker.cls",
            ],
        ),
        trace_vba_family(
            temp_dir,
            "docx",
            "testdata/docx/minimal/document.docx",
            "trace.docm",
            &[
                "testdata/golden/vba-authoring/docx-class/AgentDoc.bas",
                "testdata/golden/vba-authoring/docx-class/Worker.cls",
            ],
        ),
    ];

    serde_json::json!({
        "workflow": "vba create --pure -> validate -> conformance -> list -> extract",
        "families": families
    })
}

fn trace_vba_family(
    temp_dir: &Path,
    family: &str,
    input: &str,
    output_name: &str,
    sources: &[&str],
) -> Value {
    let family_dir = temp_dir.join(format!("vba-{family}"));
    std::fs::create_dir_all(&family_dir).expect("vba family temp dir");
    let output = family_dir.join(output_name);
    let output_str = path_string(&output);

    let mut args = vec![
        "--json".to_string(),
        "vba".to_string(),
        "create".to_string(),
        input.to_string(),
        "--pure".to_string(),
        "--family".to_string(),
        family.to_string(),
    ];
    for source in sources {
        args.push("--source".to_string());
        args.push((*source).to_string());
    }
    args.push("--out".to_string());
    args.push(output_str.clone());
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let create = run_json_ok(&format!("vba create {family}"), &refs);
    assert_rust_emitted_ooxml_command_exits_zero(&create, "validateCommand");

    let validation = assert_strict_valid(&output_str);
    let conformance = assert_conformance_passed(&output_str);
    let list = run_json_ok(
        &format!("vba list {family}"),
        &["--json", "vba", "list", &output_str],
    );
    let extract_dir = family_dir.join("extracted");
    let extract_dir_str = path_string(&extract_dir);
    let extract = run_json_ok(
        &format!("vba extract {family}"),
        &[
            "--json",
            "vba",
            "extract",
            &output_str,
            "--out-dir",
            &extract_dir_str,
        ],
    );
    let inspect = run_json_ok(
        &format!("vba inspect {family}"),
        &["--json", "vba", "inspect", &output_str],
    );

    let modules = list["project"]["modules"]
        .as_array()
        .expect("VBA modules")
        .iter()
        .map(|module| {
            let expected_path = extract_dir.join(format!(
                "{}{}",
                module["name"].as_str().expect("module name"),
                module["extension"].as_str().expect("module extension")
            ));
            assert!(
                expected_path.exists(),
                "extracted VBA module missing: {}",
                expected_path.display()
            );
            serde_json::json!({
                "name": module["name"],
                "kind": module["kind"],
                "extension": module["extension"],
                "primarySelector": module["primarySelector"],
                "lineCount": module["lineCount"]
            })
        })
        .collect::<Vec<_>>();

    assert_eq!(inspect["vba"]["family"], family);
    assert_eq!(inspect["vba"]["hasVbaProject"], true);
    assert_eq!(inspect["vba"]["macroEnabled"], true);

    serde_json::json!({
        "family": family,
        "inputFixture": input,
        "outputExtension": Path::new(output_name).extension().and_then(|ext| ext.to_str()).map(|ext| format!(".{ext}")).unwrap_or_default(),
        "sources": sources,
        "backend": create["backend"],
        "createMode": create["createMode"],
        "validationStatus": validation["status"],
        "conformanceStatus": conformance["status"],
        "vbaPartUri": inspect["vba"]["vbaProject"]["partUri"],
        "mainPartUri": inspect["vba"]["mainPartUri"],
        "mainContentType": inspect["vba"]["mainContentType"],
        "projectSha256": inspect["vba"]["vbaProject"]["sha256"],
        "moduleCount": list["project"]["moduleCount"],
        "modules": modules,
        "followUps": follow_ups(&create, &["validateCommand", "conformanceCommand", "inspectCommand", "packageReadbackCommand", "extractBinCommand", "officeCheckCommand"]),
        "extractModuleCount": extract["modules"].as_array().expect("extracted modules").len()
    })
}

fn trace_pptx_chart(temp_dir: &Path) -> Value {
    let output = temp_dir.join("pptx-chart-style.pptx");
    let output_str = path_string(&output);
    let mutation = run_json_ok(
        "pptx chart set-series-style",
        &[
            "--json",
            "pptx",
            "charts",
            "set-series-style",
            "testdata/pptx/libreoffice-chart-simple/presentation.pptx",
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--fill-color",
            "FF8800",
            "--line-color",
            "114477",
            "--line-width-pt",
            "2",
            "--out",
            &output_str,
        ],
    );
    assert!(output.exists(), "PPTX chart output exists");
    assert_rust_emitted_ooxml_command_exits_zero(&mutation, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&mutation, "chartShowCommand");

    let validation = assert_strict_valid(&output_str);
    let conformance = assert_conformance_passed(&output_str);
    let show = run_json_ok(
        "pptx chart readback",
        &[
            "--json",
            "pptx",
            "charts",
            "show",
            &output_str,
            "--slide",
            "1",
            "--chart",
            "chart:1",
        ],
    );
    let chart = &show["charts"][0];
    let first_series = &chart["style"]["series"][0];
    assert_eq!(first_series["fillColor"], "FF8800");
    assert_eq!(first_series["lineColor"], "114477");
    assert_eq!(first_series["lineWidthPt"], 2);

    let render_dir = temp_dir.join("pptx-render-smoke");
    let render_dir_str = path_string(&render_dir);
    let render = run_json_ok_with_env(
        "pptx chart mock render smoke",
        &[
            "--json",
            "pptx",
            "render",
            &output_str,
            "--slides",
            "1",
            "--format",
            "json",
            "--out",
            &render_dir_str,
        ],
        &[("OOXML_RUST_MOCK_RENDER", "1")],
    );
    assert!(
        render_dir.join("slide-1.png").exists(),
        "mock render slide PNG"
    );

    let chart_xml = read_zip_string(&output, "ppt/charts/chart1.xml");
    for needle in ["FF8800", "114477", r#"w="25400""#] {
        assert!(
            chart_xml.contains(needle),
            "PPTX chart XML missing {needle:?}"
        );
    }

    serde_json::json!({
        "inputFixture": "testdata/pptx/libreoffice-chart-simple/presentation.pptx",
        "producer": "LibreOffice headless re-export",
        "command": "pptx charts set-series-style",
        "outputExtension": ".pptx",
        "validationStatus": validation["status"],
        "conformanceStatus": conformance["status"],
        "followUps": follow_ups(&mutation, &["validateCommand", "chartShowCommand", "renderCommand"]),
        "chart": {
            "primarySelector": chart["primarySelector"],
            "partUri": chart["partUri"],
            "slide": chart["slide"],
            "shapeName": chart["shapeName"],
            "title": chart["title"],
            "types": chart["types"],
            "seriesCount": chart["series"].as_array().expect("chart series").len(),
            "firstSeries": {
                "name": first_series["name"],
                "fillColor": first_series["fillColor"],
                "lineColor": first_series["lineColor"],
                "lineWidthPt": first_series["lineWidthPt"]
            }
        },
        "renderSmoke": {
            "mode": "OOXML_RUST_MOCK_RENDER",
            "slides": render["slides"].as_array().expect("render slides").len(),
            "imageFormat": render["imageFormat"]
        },
        "zipPartNeedles": ["FF8800", "114477", "w=\"25400\""]
    })
}

fn trace_xlsm_chart_macro_preservation(temp_dir: &Path) -> Value {
    let input = create_macro_enabled_xlsx(
        temp_dir,
        "xlsx-chart-macro-input",
        "testdata/xlsx/libreoffice-chart-workbook/workbook.xlsx",
    );
    let input_str = path_string(&input);
    let output = temp_dir.join("xlsx-chart-macro-preserved.xlsm");
    let output_str = path_string(&output);
    let mutation = run_json_ok(
        "xlsx chart preserves macros",
        &[
            "--json",
            "xlsx",
            "charts",
            "set-series-style",
            &input_str,
            "--sheet",
            "Data",
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--fill-color",
            "55AA33",
            "--out",
            &output_str,
        ],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&mutation, "validateCommand");
    let validation = assert_strict_valid(&output_str);
    let conformance = assert_conformance_passed(&output_str);
    let macro_state = assert_xlsm_macro_state(&output_str);
    let chart_xml = read_zip_string(&output, "xl/charts/chart1.xml");
    assert!(chart_xml.contains("55AA33"), "XLSM chart mutation style");
    serde_json::json!({
        "command": "xlsx charts set-series-style",
        "outputExtension": ".xlsm",
        "validationStatus": validation["status"],
        "conformanceStatus": conformance["status"],
        "macroEnabled": macro_state["macroEnabled"],
        "hasVbaProject": macro_state["hasVbaProject"],
        "vbaPartUri": macro_state["vbaPartUri"],
        "mainContentType": macro_state["mainContentType"],
        "chartStyleNeedle": "55AA33"
    })
}

fn trace_xlsm_data_validation_macro_preservation(temp_dir: &Path) -> Value {
    let input = create_macro_enabled_xlsx(
        temp_dir,
        "data-validation-macro-input",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
    );
    let input_str = path_string(&input);
    let output = temp_dir.join("data-validation-macro-preserved.xlsm");
    let output_str = path_string(&output);
    let mutation = run_json_ok(
        "data validation preserves macros",
        &[
            "--json",
            "xlsx",
            "data-validations",
            "create",
            &input_str,
            "--sheet",
            "1",
            "--range",
            "C1:C5",
            "--type",
            "whole",
            "--operator",
            "greaterThan",
            "--formula1",
            "0",
            "--out",
            &output_str,
        ],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&mutation, "validateCommand");
    assert_rust_emitted_ooxml_command_succeeds(&mutation, "dataValidationsShowCommand");
    let validation = assert_strict_valid(&output_str);
    let conformance = assert_conformance_passed(&output_str);
    let macro_state = assert_xlsm_macro_state(&output_str);
    let show = run_json_ok(
        "data validation macro readback",
        &[
            "--json",
            "xlsx",
            "data-validations",
            "show",
            &output_str,
            "--sheet",
            "1",
            "--range",
            "C1:C5",
        ],
    );
    assert_eq!(show["type"], "whole");
    assert_eq!(show["formula1"], "0");
    serde_json::json!({
        "command": "xlsx data-validations create",
        "outputExtension": ".xlsm",
        "validationStatus": validation["status"],
        "conformanceStatus": conformance["status"],
        "macroEnabled": macro_state["macroEnabled"],
        "hasVbaProject": macro_state["hasVbaProject"],
        "vbaPartUri": macro_state["vbaPartUri"],
        "mainContentType": macro_state["mainContentType"],
        "rule": {
            "sqref": show["sqref"],
            "type": show["type"],
            "formula1": show["formula1"]
        }
    })
}

fn create_macro_enabled_xlsx(temp_dir: &Path, label: &str, input: &str) -> PathBuf {
    let output = temp_dir.join(format!("{label}.xlsm"));
    let output_str = path_string(&output);
    let create = run_json_ok(
        &format!("create macro-enabled xlsx {label}"),
        &[
            "--json",
            "vba",
            "create",
            input,
            "--pure",
            "--family",
            "xlsx",
            "--source",
            "testdata/golden/vba-authoring/xlsx-class/AgentSmoke.bas",
            "--source",
            "testdata/golden/vba-authoring/xlsx-class/Worker.cls",
            "--out",
            &output_str,
        ],
    );
    assert_rust_emitted_ooxml_command_exits_zero(&create, "validateCommand");
    assert_eq!(assert_strict_valid(&output_str)["valid"], Value::Bool(true));
    assert_eq!(
        assert_conformance_passed(&output_str)["status"],
        Value::String("passed".to_string())
    );
    assert_xlsm_macro_state(&output_str);
    output
}

fn assert_xlsm_macro_state(file: &str) -> Value {
    let inspect = run_json_ok(
        "vba inspect macro state",
        &["--json", "vba", "inspect", file],
    );
    assert_eq!(inspect["vba"]["family"], "xlsx");
    assert_eq!(inspect["vba"]["macroEnabled"], true);
    assert_eq!(inspect["vba"]["hasVbaProject"], true);
    assert_eq!(
        inspect["vba"]["vbaProject"]["partUri"],
        "/xl/vbaProject.bin"
    );
    serde_json::json!({
        "macroEnabled": inspect["vba"]["macroEnabled"],
        "hasVbaProject": inspect["vba"]["hasVbaProject"],
        "vbaPartUri": inspect["vba"]["vbaProject"]["partUri"],
        "mainContentType": inspect["vba"]["mainContentType"]
    })
}

fn assert_strict_valid(file: &str) -> Value {
    let validation = run_json_ok(
        &format!("strict validate {file}"),
        &["--json", "validate", "--strict", file],
    );
    assert_eq!(validation["valid"], true, "strict validate {file}");
    assert_eq!(validation["summary"]["errors"], 0, "strict errors {file}");
    validation
}

fn assert_conformance_passed(file: &str) -> Value {
    let conformance = run_json_ok(
        &format!("conformance check {file}"),
        &["--json", "conformance", "check", file],
    );
    assert_eq!(conformance["status"], "passed", "conformance status {file}");
    assert_eq!(
        conformance["summary"]["failed"], 0,
        "conformance failures {file}"
    );
    conformance
}

fn follow_ups(result: &Value, fields: &[&str]) -> Value {
    let items = fields
        .iter()
        .map(|field| {
            serde_json::json!({
                "field": field,
                "present": result[*field].as_str().is_some_and(|command| command.starts_with("ooxml "))
            })
        })
        .collect::<Vec<_>>();
    Value::Array(items)
}

fn run_json_ok(label: &str, args: &[&str]) -> Value {
    let (code, stdout, stderr) = run_ooxml(args);
    assert_eq!(code, 0, "{label} exit for {args:?}");
    assert_eq!(stderr, None, "{label} stderr for {args:?}");
    stdout.unwrap_or_else(|| panic!("{label} stdout for {args:?}"))
}

fn run_json_ok_with_env(label: &str, args: &[&str], envs: &[(&str, &str)]) -> Value {
    let (code, stdout, stderr) = run_ooxml_with_env(args, envs);
    assert_eq!(code, 0, "{label} exit for {args:?}");
    assert_eq!(stderr, None, "{label} stderr for {args:?}");
    stdout.unwrap_or_else(|| panic!("{label} stdout for {args:?}"))
}

fn assert_release_trace_golden(summary: &Value) {
    let golden_path = Path::new("testdata/golden/release-real-file-traces.json");
    let actual = format!(
        "{}\n",
        serde_json::to_string_pretty(summary).expect("release trace summary JSON")
    );
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        std::fs::write(golden_path, actual).expect("write release trace golden");
        return;
    }

    let expected = std::fs::read_to_string(golden_path).unwrap_or_else(|err| {
        panic!(
            "missing release trace golden {}: {err}. Run UPDATE_GOLDENS=1 cargo test --test rust_contract_smoke release_real_file_traces_cover_high_value_surfaces",
            golden_path.display()
        )
    });
    let expected: Value = serde_json::from_str(&expected).unwrap_or_else(|err| {
        panic!(
            "invalid release trace golden JSON {}: {err}",
            golden_path.display()
        )
    });
    if expected != *summary {
        let actual_path = golden_path.with_extension("actual");
        std::fs::write(&actual_path, actual).expect("write release trace actual");
        panic!(
            "release trace golden drift: compare {} {}",
            golden_path.display(),
            actual_path.display()
        );
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn trace_temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-{name}-{}-{suffix}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir).expect("trace temp dir");
    temp_dir
}
