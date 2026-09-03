use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn run_json(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml");
    assert_eq!(
        output.status.code(),
        Some(0),
        "command failed: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse ooxml JSON")
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "ooxml-mutation-envelope-{label}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&path).expect("create test directory");
    path
}

#[derive(Debug)]
struct ContractCase {
    path: &'static str,
    destination_kind: &'static str,
    args: Vec<String>,
    removes_destination: bool,
}

#[derive(Debug)]
struct ContractRow {
    command: String,
    exit_zero: bool,
    one_json_object: bool,
    envelope_fields: bool,
    destination_kind: bool,
    schema_valid: bool,
    readback_exit_zero: bool,
    selector_resolved: bool,
    validate_exit_zero: bool,
    diagnostics: Vec<String>,
}

impl ContractRow {
    fn passed(&self) -> bool {
        self.exit_zero
            && self.one_json_object
            && self.envelope_fields
            && self.destination_kind
            && self.schema_valid
            && self.readback_exit_zero
            && self.selector_resolved
            && self.validate_exit_zero
    }
}

fn owned_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

fn run_owned(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn emitted_ooxml_args(command: &str) -> Result<Vec<String>, String> {
    let command = command
        .strip_prefix("ooxml ")
        .ok_or_else(|| format!("emitted command must start with ooxml: {command}"))?;
    shell_words(command)
}

fn shell_words(command: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut chars = command.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    let mut started = false;
    while let Some(ch) = chars.next() {
        if in_single {
            if ch == '\'' {
                in_single = false;
            } else {
                current.push(ch);
            }
            continue;
        }
        if in_double {
            match ch {
                '"' => in_double = false,
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                _ => current.push(ch),
            }
            continue;
        }
        match ch {
            '\'' => {
                in_single = true;
                started = true;
            }
            '"' => {
                in_double = true;
                started = true;
            }
            '\\' => {
                started = true;
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            ch if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            _ => {
                started = true;
                current.push(ch);
            }
        }
    }
    if in_single || in_double {
        return Err("unterminated quote".to_string());
    }
    if started {
        words.push(current);
    }
    Ok(words)
}

#[test]
fn emitted_command_parser_preserves_quoted_windows_backslashes() {
    let path = r"C:\Users\RUNNER~1\AppData\Local\Temp\docx-field-insert.docx";
    let args = emitted_ooxml_args(&format!("ooxml --json docx fields list '{path}'"))
        .expect("parse emitted Windows command");
    assert_eq!(args.last().map(String::as_str), Some(path));
}

fn output_json(output: &Output) -> Result<Value, String> {
    let stdout = String::from_utf8(output.stdout.clone()).map_err(|err| err.to_string())?;
    if stdout.lines().count() != 1 {
        return Err(format!("expected one JSON line, got {stdout:?}"));
    }
    serde_json::from_str(&stdout).map_err(|err| format!("{err}: {stdout:?}"))
}

fn json_contains_scalar(value: &Value, expected: &str) -> bool {
    match value {
        Value::Array(values) => values
            .iter()
            .any(|value| json_contains_scalar(value, expected)),
        Value::Object(object) => object
            .values()
            .any(|value| json_contains_scalar(value, expected)),
        Value::String(value) => value == expected || value.contains(expected),
        Value::Number(value) => value.to_string() == expected,
        _ => false,
    }
}

fn required_envelope_fields_present(envelope: &Value) -> bool {
    let Some(object) = envelope.as_object() else {
        return false;
    };
    let required = [
        "file",
        "family",
        "command",
        "destination",
        "changed",
        "readbackCommand",
        "validateCommand",
        "conformanceCommand",
        "checkCommand",
        "warnings",
        "aliasesApplied",
        "validated",
    ];
    required
        .iter()
        .all(|field| object.get(*field).is_some_and(|value| !value.is_null()))
        && [
            "partUri",
            "primarySelector",
            "selectors",
            "handle",
            "kind",
            "summary",
        ]
        .iter()
        .all(|field| {
            object["destination"]
                .get(*field)
                .is_some_and(|value| !value.is_null())
        })
}

fn validates_pinned_schema(envelope: &Value, schema: &Value) -> bool {
    let required = schema["required"].as_array().is_some_and(|fields| {
        fields.iter().all(|field| {
            field
                .as_str()
                .is_some_and(|field| envelope.get(field).is_some_and(|value| !value.is_null()))
        })
    });
    let destination_required = schema["$defs"]["destination"]["required"]
        .as_array()
        .is_some_and(|fields| {
            fields.iter().all(|field| {
                field.as_str().is_some_and(|field| {
                    envelope["destination"]
                        .get(field)
                        .is_some_and(|value| !value.is_null())
                })
            })
        });
    let strings_nonempty = [
        "file",
        "family",
        "command",
        "readbackCommand",
        "validateCommand",
        "conformanceCommand",
        "checkCommand",
    ]
    .iter()
    .all(|field| {
        envelope[*field]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    });
    let destination_strings_nonempty =
        ["partUri", "primarySelector", "handle", "kind"]
            .iter()
            .all(|field| {
                envelope["destination"][*field]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            });
    let selectors_valid =
        envelope["destination"]["selectors"]
            .as_array()
            .is_some_and(|selectors| {
                !selectors.is_empty()
                    && selectors
                        .iter()
                        .all(|selector| selector.as_str().is_some_and(|value| !value.is_empty()))
            });
    required
        && destination_required
        && strings_nonempty
        && destination_strings_nonempty
        && selectors_valid
        && envelope["changed"].is_array()
        && envelope["warnings"].is_array()
        && envelope["aliasesApplied"].is_array()
        && envelope["validated"].is_boolean()
        && envelope["destination"]["summary"].is_object()
}

fn run_contract_case(case: &ContractCase, schema: &Value) -> ContractRow {
    let output = run_owned(&case.args);
    let mut row = ContractRow {
        command: case.path.to_string(),
        exit_zero: output.status.success(),
        one_json_object: false,
        envelope_fields: false,
        destination_kind: false,
        schema_valid: false,
        readback_exit_zero: false,
        selector_resolved: false,
        validate_exit_zero: false,
        diagnostics: Vec::new(),
    };
    if !row.exit_zero {
        row.diagnostics.push(format!(
            "exit={:?} stdout={} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
        return row;
    }
    let response = match output_json(&output) {
        Ok(response) => {
            row.one_json_object = true;
            response
        }
        Err(err) => {
            row.diagnostics.push(err);
            return row;
        }
    };
    let envelope = &response["mutationEnvelope"];
    row.envelope_fields = required_envelope_fields_present(envelope);
    row.destination_kind = envelope["destination"]["kind"] == case.destination_kind;
    row.schema_valid = validates_pinned_schema(envelope, schema);

    let readback = envelope["readbackCommand"].as_str().and_then(|command| {
        emitted_ooxml_args(command)
            .map_err(|err| row.diagnostics.push(err))
            .ok()
            .map(|args| run_owned(&args))
    });
    if let Some(readback) = readback {
        row.readback_exit_zero = readback.status.success();
        if let Ok(readback_json) = output_json(&readback) {
            let primary = envelope["destination"]["primarySelector"]
                .as_str()
                .unwrap_or_default();
            let handle = envelope["destination"]["handle"]
                .as_str()
                .unwrap_or_default();
            let selector_tail = primary.rsplit(':').next().unwrap_or(primary);
            row.selector_resolved = case.removes_destination
                || primary == "package"
                || json_contains_scalar(&readback_json, primary)
                || json_contains_scalar(&readback_json, handle)
                || json_contains_scalar(&readback_json, selector_tail);
            if !row.selector_resolved {
                row.diagnostics.push(format!(
                    "selector {primary:?} / handle {handle:?} absent from readback {readback_json}"
                ));
            }
        } else {
            row.diagnostics.push(format!(
                "readback stdout={} stderr={}",
                String::from_utf8_lossy(&readback.stdout),
                String::from_utf8_lossy(&readback.stderr)
            ));
        }
    }

    let validation = envelope["validateCommand"].as_str().and_then(|command| {
        emitted_ooxml_args(command)
            .map_err(|err| row.diagnostics.push(err))
            .ok()
            .map(|args| run_owned(&args))
    });
    if let Some(validation) = validation {
        row.validate_exit_zero = validation.status.success();
        if !row.validate_exit_zero {
            row.diagnostics.push(format!(
                "validate stdout={} stderr={}",
                String::from_utf8_lossy(&validation.stdout),
                String::from_utf8_lossy(&validation.stderr)
            ));
        }
    }
    row
}

fn assert_contract_matrix(rows: &[ContractRow]) {
    if rows.iter().all(ContractRow::passed) {
        return;
    }
    let mut matrix = String::from(
        "command | exit | json | fields | kind | schema | readback | selector | validate\n",
    );
    for row in rows {
        matrix.push_str(&format!(
            "{} | {} | {} | {} | {} | {} | {} | {} | {}\n",
            row.command,
            row.exit_zero,
            row.one_json_object,
            row.envelope_fields,
            row.destination_kind,
            row.schema_valid,
            row.readback_exit_zero,
            row.selector_resolved,
            row.validate_exit_zero
        ));
        for diagnostic in &row.diagnostics {
            matrix.push_str(&format!("  {diagnostic}\n"));
        }
    }
    panic!("mutation envelope contract failures:\n{matrix}");
}

fn case(path: &'static str, destination_kind: &'static str, args: Vec<String>) -> ContractCase {
    ContractCase {
        path,
        destination_kind,
        args,
        removes_destination: false,
    }
}

fn removal_case(
    path: &'static str,
    destination_kind: &'static str,
    args: Vec<String>,
) -> ContractCase {
    ContractCase {
        path,
        destination_kind,
        args,
        removes_destination: true,
    }
}

fn with_out(mut args: Vec<String>, out: &Path) -> Vec<String> {
    args.push("--out".to_string());
    args.push(out.to_string_lossy().to_string());
    args
}

fn contract_case_with_out(
    dir: &Path,
    path: &'static str,
    destination_kind: &'static str,
    input: Option<&str>,
    tail: &[&str],
    removes_destination: bool,
) -> ContractCase {
    let mut args = vec!["--json".to_string()];
    args.extend(path.split_whitespace().skip(1).map(str::to_string));
    args.extend(input.map(str::to_string));
    args.extend(tail.iter().map(|arg| (*arg).to_string()));
    let extension = match path.split_whitespace().nth(1) {
        Some("docx") => "docx",
        Some("pptx") => "pptx",
        _ => "xlsx",
    };
    let leaf = path
        .strip_prefix("ooxml ")
        .unwrap_or(path)
        .replace(' ', "-");
    let args = with_out(args, &dir.join(format!("{leaf}.{extension}")));
    ContractCase {
        path,
        destination_kind,
        args,
        removes_destination,
    }
}

fn setup_with_out(dir: &Path, name: &str, extension: &str, args: Vec<String>) -> String {
    let output_path = dir.join(format!("setup-{name}.{extension}"));
    let args = with_out(args, &output_path);
    let output = run_owned(&args);
    assert!(
        output.status.success(),
        "setup {name} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output_path.to_string_lossy().to_string()
}

fn docx_contract_cases(dir: &Path, scaffold_source: &Path) -> Vec<ContractCase> {
    let out = |name: &str| dir.join(format!("docx-{name}.docx"));
    let scaffold_source = scaffold_source.to_string_lossy().to_string();
    vec![
        case(
            "ooxml docx scaffold",
            "package",
            with_out(
                owned_args(&["--json", "docx", "scaffold", "--text", "Envelope contract"]),
                &out("scaffold"),
            ),
        ),
        case(
            "ooxml docx blocks replace",
            "paragraph",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "blocks",
                    "replace",
                    "testdata/docx/styled-headings/document.docx",
                    "--block",
                    "1",
                    "--text",
                    "Envelope replacement",
                ]),
                &out("blocks-replace"),
            ),
        ),
        removal_case(
            "ooxml docx blocks delete",
            "paragraph",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "blocks",
                    "delete",
                    "testdata/docx/mixed-blocks/document.docx",
                    "--block",
                    "1",
                ]),
                &out("blocks-delete"),
            ),
        ),
        case(
            "ooxml docx blocks insert-after",
            "paragraph",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "blocks",
                    "insert-after",
                    "testdata/docx/mixed-blocks/document.docx",
                    "--block",
                    "1",
                    "--text",
                    "Envelope insertion",
                ]),
                &out("blocks-insert"),
            ),
        ),
        case(
            "ooxml docx breaks insert",
            "section",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "breaks",
                    "insert",
                    &scaffold_source,
                    "--page",
                ]),
                &out("break"),
            ),
        ),
        case(
            "ooxml docx sections set",
            "section",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "sections",
                    "set",
                    "testdata/docx/minimal/document.docx",
                    "--section",
                    "1",
                    "--orientation",
                    "landscape",
                    "--size",
                    "A4",
                    "--margins",
                    "1in,1in,1in,1in",
                ]),
                &out("section"),
            ),
        ),
        case(
            "ooxml docx paragraphs append",
            "paragraph",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "paragraphs",
                    "append",
                    "testdata/docx/styled-headings/document.docx",
                    "--text",
                    "Envelope append",
                ]),
                &out("paragraph-append"),
            ),
        ),
        case(
            "ooxml docx paragraphs insert",
            "paragraph",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "paragraphs",
                    "insert",
                    "testdata/docx/styled-headings/document.docx",
                    "--insert-after",
                    "0",
                    "--text",
                    "Envelope insert",
                ]),
                &out("paragraph-insert"),
            ),
        ),
        case(
            "ooxml docx paragraphs set",
            "paragraph",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "paragraphs",
                    "set",
                    "testdata/docx/styled-headings/document.docx",
                    "--index",
                    "1",
                    "--text",
                    "Envelope set",
                ]),
                &out("paragraph-set"),
            ),
        ),
        case(
            "ooxml docx paragraphs clear",
            "paragraph",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "paragraphs",
                    "clear",
                    "testdata/docx/styled-headings/document.docx",
                    "--index",
                    "1",
                ]),
                &out("paragraph-clear"),
            ),
        ),
        case(
            "ooxml docx styles apply",
            "styled-object",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "styles",
                    "apply",
                    "testdata/docx/apply-styles/document.docx",
                    "--index",
                    "1",
                    "--target",
                    "paragraph",
                    "--style",
                    "Heading2",
                ]),
                &out("style"),
            ),
        ),
        case(
            "ooxml docx comments add",
            "comment",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "comments",
                    "add",
                    "testdata/docx/minimal/document.docx",
                    "--anchor-block",
                    "1",
                    "--author",
                    "Contract",
                    "--text",
                    "Envelope comment",
                    "--date",
                    "2025-06-06T10:30:00Z",
                ]),
                &out("comment-add"),
            ),
        ),
        case(
            "ooxml docx comments edit",
            "comment",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "comments",
                    "edit",
                    "testdata/docx/with-comments/document.docx",
                    "--comment-id",
                    "0",
                    "--text",
                    "Envelope update",
                    "--author",
                    "Contract",
                    "--date",
                    "2030-01-02T03:04:05Z",
                ]),
                &out("comment-edit"),
            ),
        ),
        removal_case(
            "ooxml docx comments remove",
            "comment",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "comments",
                    "remove",
                    "testdata/docx/with-comments/document.docx",
                    "--comment-id",
                    "0",
                ]),
                &out("comment-remove"),
            ),
        ),
        case(
            "ooxml docx fields insert",
            "field",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "fields",
                    "insert",
                    "testdata/docx/minimal/document.docx",
                    "--location",
                    "body:1",
                    "--field-code",
                    "PAGE",
                    "--result",
                    "1",
                ]),
                &out("field-insert"),
            ),
        ),
        case(
            "ooxml docx fields set-result",
            "field",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "fields",
                    "set-result",
                    "testdata/docx/with-fields/document.docx",
                    "--selector",
                    "body:1:0",
                    "--result",
                    "42",
                ]),
                &out("field-result"),
            ),
        ),
        case(
            "ooxml docx headers set-text",
            "header",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "headers",
                    "set-text",
                    "testdata/docx/minimal/document.docx",
                    "--type",
                    "default",
                    "--index",
                    "1",
                    "--text",
                    "Envelope header",
                ]),
                &out("header"),
            ),
        ),
        case(
            "ooxml docx footers set-text",
            "footer",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "footers",
                    "set-text",
                    "testdata/docx/minimal/document.docx",
                    "--type",
                    "default",
                    "--index",
                    "1",
                    "--text",
                    "Envelope footer",
                ]),
                &out("footer"),
            ),
        ),
        case(
            "ooxml docx images replace",
            "image",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "images",
                    "replace",
                    "testdata/docx/with-image/document.docx",
                    "--image",
                    "1",
                    "--file",
                    "testdata/test_image.png",
                    "--width",
                    "1828800",
                    "--height",
                    "914400",
                ]),
                &out("image-replace"),
            ),
        ),
        case(
            "ooxml docx images insert",
            "image",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "images",
                    "insert",
                    "testdata/docx/styled-headings/document.docx",
                    "--after",
                    "1",
                    "--file",
                    "testdata/test_image.png",
                    "--width",
                    "914400",
                    "--height",
                    "914400",
                ]),
                &out("image-insert"),
            ),
        ),
        case(
            "ooxml docx replace",
            "text-match",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "replace",
                    "testdata/docx/split-runs/document.docx",
                    "--find",
                    "hello",
                    "--replace",
                    "hi",
                    "--expect-count",
                    "2",
                ]),
                &out("replace"),
            ),
        ),
        case(
            "ooxml docx tables create",
            "table",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "tables",
                    "create",
                    "testdata/docx/minimal/document.docx",
                    "--values",
                    r#"[["Region","Units"],["West",12]]"#,
                ]),
                &out("table-create"),
            ),
        ),
        case(
            "ooxml docx tables set-style",
            "table",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "tables",
                    "set-style",
                    "testdata/docx/apply-styles/document.docx",
                    "--table",
                    "1",
                    "--style",
                    "TableGrid",
                ]),
                &out("table-style"),
            ),
        ),
        case(
            "ooxml docx tables set-cell",
            "table",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "tables",
                    "set-cell",
                    "testdata/docx/table/document.docx",
                    "--table",
                    "1",
                    "--row",
                    "1",
                    "--col",
                    "2",
                    "--text",
                    "Envelope cell",
                ]),
                &out("table-cell"),
            ),
        ),
        case(
            "ooxml docx tables clear-cell",
            "table",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "tables",
                    "clear-cell",
                    "testdata/docx/table/document.docx",
                    "--table",
                    "1",
                    "--row",
                    "1",
                    "--col",
                    "2",
                ]),
                &out("table-clear"),
            ),
        ),
        case(
            "ooxml docx tables insert-row",
            "table",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "tables",
                    "insert-row",
                    "testdata/docx/table/document.docx",
                    "--table",
                    "1",
                    "--at",
                    "2",
                ]),
                &out("table-row-insert"),
            ),
        ),
        removal_case(
            "ooxml docx tables delete-row",
            "table",
            with_out(
                owned_args(&[
                    "--json",
                    "docx",
                    "tables",
                    "delete-row",
                    "testdata/docx/table/document.docx",
                    "--table",
                    "1",
                    "--row",
                    "1",
                ]),
                &out("table-row-delete"),
            ),
        ),
    ]
}

#[test]
fn docx_mutation_commands_satisfy_the_envelope_contract() {
    let dir = temp_dir("docx-contract-matrix");
    let scaffold_source = dir.join("docx-contract-source.docx");
    run_json(&[
        "--json",
        "docx",
        "scaffold",
        "--out",
        &scaffold_source.to_string_lossy(),
        "--text",
        "Contract source",
    ]);
    let schema_response = run_json(&["--json", "capabilities", "--schema", "mutation-envelope"]);
    let cases = docx_contract_cases(&dir, &scaffold_source);
    assert_eq!(cases.len(), 27, "reviewed DOCX mutation denominator");
    let mut paths = cases.iter().map(|case| case.path).collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(
        paths.len(),
        cases.len(),
        "DOCX command paths must be unique"
    );
    let rows = cases
        .iter()
        .map(|case| run_contract_case(case, &schema_response["document"]))
        .collect::<Vec<_>>();
    assert_contract_matrix(&rows);
    fs::remove_dir_all(dir).expect("remove DOCX contract directory");
}

fn xlsx_contract_cases(dir: &Path) -> Vec<ContractCase> {
    let minimal = "testdata/xlsx/minimal-workbook/workbook.xlsx";
    let chart = "testdata/xlsx/chart-workbook/workbook.xlsx";
    let table = "testdata/xlsx/outline-table/workbook.xlsx";

    let two_sheets = setup_with_out(
        dir,
        "xlsx-two-sheets",
        "xlsx",
        owned_args(&[
            "--json", "xlsx", "sheets", "add", minimal, "--name", "Sheet2",
        ]),
    );
    let three_sheets = setup_with_out(
        dir,
        "xlsx-three-sheets",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "sheets",
            "add",
            &two_sheets,
            "--name",
            "Sheet3",
        ]),
    );
    let comment_source = setup_with_out(
        dir,
        "xlsx-comment",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "comments",
            "add",
            minimal,
            "--sheet",
            "Sheet1",
            "--cell",
            "A1",
            "--author",
            "Contract",
            "--text",
            "Seed comment",
        ]),
    );
    let conditional_one = setup_with_out(
        dir,
        "xlsx-conditional-one",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "conditional-formats",
            "add",
            minimal,
            "--sheet",
            "1",
            "--range",
            "A1:A5",
            "--type",
            "expression",
            "--formula",
            "A1>0",
            "--priority",
            "1",
        ]),
    );
    let conditional_two = setup_with_out(
        dir,
        "xlsx-conditional-two",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "conditional-formats",
            "add",
            &conditional_one,
            "--sheet",
            "1",
            "--range",
            "B1:B5",
            "--type",
            "expression",
            "--formula",
            "B1>0",
            "--priority",
            "2",
        ]),
    );
    let validation_source = setup_with_out(
        dir,
        "xlsx-validation",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "data-validations",
            "create",
            minimal,
            "--sheet",
            "1",
            "--range",
            "A1:A10",
            "--type",
            "list",
            "--list-values",
            "Red,Green,Blue",
        ]),
    );
    let hyperlink_source = setup_with_out(
        dir,
        "xlsx-hyperlink",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "hyperlinks",
            "add",
            minimal,
            "--sheet",
            "Sheet1",
            "--cell",
            "A1",
            "--url",
            "https://example.com/original",
        ]),
    );
    let filter_source = setup_with_out(
        dir,
        "xlsx-filter",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "filters-sorts",
            "set-autofilter",
            minimal,
            "--sheet",
            "1",
            "--range",
            "A1:C3",
        ]),
    );
    let column_filter_source = setup_with_out(
        dir,
        "xlsx-column-filter",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "filters-sorts",
            "add-column-filter",
            &filter_source,
            "--sheet",
            "1",
            "--column",
            "0",
            "--values",
            "North,South",
        ]),
    );
    let sort_source = setup_with_out(
        dir,
        "xlsx-sort",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "filters-sorts",
            "set-sort",
            minimal,
            "--sheet",
            "1",
            "--ref",
            "A1:C3",
            "--column",
            "A",
        ]),
    );
    let name_source = setup_with_out(
        dir,
        "xlsx-name",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "names",
            "add",
            minimal,
            "--name",
            "SalesData",
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B2",
        ]),
    );
    let freeze_source = setup_with_out(
        dir,
        "xlsx-freeze",
        "xlsx",
        owned_args(&[
            "--json", "xlsx", "freeze", "set", minimal, "--sheet", "Sheet1", "--rows", "1",
        ]),
    );

    let mut cases = vec![
        contract_case_with_out(
            dir,
            "ooxml xlsx scaffold",
            "package",
            None,
            &["--sheet", "Data"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx sheets add",
            "sheet",
            Some(minimal),
            &["--name", "Added"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx sheets rename",
            "sheet",
            Some(chart),
            &["--sheet", "Data", "--name", "Facts"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx sheets move",
            "sheet",
            Some(&three_sheets),
            &["--sheet", "Sheet3", "--to", "1"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx sheets delete",
            "sheet",
            Some(&three_sheets),
            &["--sheet", "Sheet3"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx sheets set-tab-color",
            "sheet",
            Some(minimal),
            &["--sheet", "Sheet1", "--color", "#112233"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx sheets set-print",
            "sheet",
            Some(minimal),
            &[
                "--sheet",
                "Sheet1",
                "--landscape",
                "--fit-to-width",
                "1",
                "--repeat-header-rows",
                "1",
                "--gridlines",
                "off",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx colwidths set",
            "range",
            Some(minimal),
            &["--sheet", "Sheet1", "--range", "A:B", "--width", "12"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx colwidths autofit",
            "range",
            Some("testdata/xlsx/used-range/workbook.xlsx"),
            &[
                "--sheet", "Sparse", "--range", "A:C", "--min", "5", "--max", "30",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx rowheights set",
            "range",
            Some(minimal),
            &["--sheet", "Sheet1", "--range", "1:2", "--height", "20"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx rows insert",
            "range",
            Some(minimal),
            &["--sheet", "Sheet1", "--at", "2"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx rows delete",
            "range",
            Some(minimal),
            &["--sheet", "Sheet1", "--row", "2"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx cols insert",
            "range",
            Some(minimal),
            &["--sheet", "Sheet1", "--at", "B"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx cols delete",
            "range",
            Some(minimal),
            &["--sheet", "Sheet1", "--col", "B"],
            true,
        ),
    ];

    for (leaf, tail) in [
        (
            "create",
            vec![
                "--type",
                "bar",
                "--sheet",
                "Data",
                "--range",
                "A1:B4",
                "--title",
                "Contract Chart",
                "--anchor",
                "D1",
            ],
        ),
        (
            "update-source",
            vec![
                "--chart",
                "chart:1",
                "--series",
                "1",
                "--role",
                "values",
                "--source-sheet",
                "Data",
                "--source-range",
                "$B$2:$B$3",
            ],
        ),
        (
            "set-title",
            vec!["--chart", "chart:1", "--title", "Contract Revenue"],
        ),
        (
            "set-legend",
            vec!["--chart", "chart:1", "--position", "bottom"],
        ),
        (
            "set-chart-area-fill",
            vec!["--chart", "chart:1", "--fill-color", "FFEEDD"],
        ),
        (
            "set-plot-area-fill",
            vec!["--chart", "chart:1", "--fill-color", "CCEEFF"],
        ),
        (
            "set-series-style",
            vec![
                "--chart",
                "chart:1",
                "--series",
                "1",
                "--fill-color",
                "FF8800",
            ],
        ),
        ("convert-type", vec!["--chart", "chart:1", "--to", "line"]),
        (
            "copy-style",
            vec![
                "--chart",
                "chart:1",
                "--from",
                chart,
                "--from-chart",
                "chart:1",
            ],
        ),
        (
            "set-axis",
            vec![
                "--chart",
                "chart:1",
                "--axis",
                "value",
                "--title",
                "Contract Axis",
            ],
        ),
    ] {
        let path = match leaf {
            "create" => "ooxml xlsx charts create",
            "update-source" => "ooxml xlsx charts update-source",
            "set-title" => "ooxml xlsx charts set-title",
            "set-legend" => "ooxml xlsx charts set-legend",
            "set-chart-area-fill" => "ooxml xlsx charts set-chart-area-fill",
            "set-plot-area-fill" => "ooxml xlsx charts set-plot-area-fill",
            "set-series-style" => "ooxml xlsx charts set-series-style",
            "convert-type" => "ooxml xlsx charts convert-type",
            "copy-style" => "ooxml xlsx charts copy-style",
            "set-axis" => "ooxml xlsx charts set-axis",
            _ => unreachable!(),
        };
        cases.push(contract_case_with_out(
            dir,
            path,
            "chart",
            Some(chart),
            &tail,
            false,
        ));
    }

    cases.extend([
        contract_case_with_out(
            dir,
            "ooxml xlsx comments add",
            "comment",
            Some(minimal),
            &[
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--author",
                "Contract",
                "--text",
                "Envelope comment",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx comments update",
            "comment",
            Some(&comment_source),
            &[
                "--sheet",
                "Sheet1",
                "--comment-id",
                "0",
                "--text",
                "Updated envelope comment",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx comments remove",
            "comment",
            Some(&comment_source),
            &["--sheet", "Sheet1", "--comment-id", "0"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx conditional-formats add",
            "conditional-format",
            Some(minimal),
            &[
                "--sheet",
                "1",
                "--range",
                "A1:A5",
                "--type",
                "expression",
                "--formula",
                "A1>0",
                "--priority",
                "1",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx conditional-formats delete",
            "conditional-format",
            Some(&conditional_one),
            &["--sheet", "1", "--rule", "priority:1"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx conditional-formats reorder",
            "conditional-format",
            Some(&conditional_two),
            &["--sheet", "1", "--rule", "cfRule:2", "--priority", "1"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx data-validations create",
            "data-validation",
            Some(minimal),
            &[
                "--sheet",
                "1",
                "--range",
                "A1:A10",
                "--type",
                "list",
                "--list-values",
                "Red,Green,Blue",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx data-validations update",
            "data-validation",
            Some(&validation_source),
            &[
                "--sheet",
                "1",
                "--range",
                "A1:A10",
                "--list-values",
                "Red,Green,Blue,Amber",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx data-validations delete",
            "data-validation",
            Some(&validation_source),
            &["--sheet", "1", "--range", "A1:A10"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx hyperlinks add",
            "hyperlink",
            Some(minimal),
            &[
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.com/report",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx hyperlinks update",
            "hyperlink",
            Some(&hyperlink_source),
            &[
                "--sheet",
                "Sheet1",
                "--cell",
                "A1",
                "--url",
                "https://example.net/new",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx hyperlinks delete",
            "hyperlink",
            Some(&hyperlink_source),
            &["--sheet", "Sheet1", "--cell", "A1"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx filters-sorts set-autofilter",
            "range",
            Some(minimal),
            &["--sheet", "1", "--range", "A1:C3"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx filters-sorts clear-autofilter",
            "range",
            Some(&filter_source),
            &["--sheet", "1"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx filters-sorts add-column-filter",
            "range",
            Some(&filter_source),
            &["--sheet", "1", "--column", "0", "--values", "North,South"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx filters-sorts clear-column-filter",
            "range",
            Some(&column_filter_source),
            &["--sheet", "1", "--column", "0"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx filters-sorts set-sort",
            "range",
            Some(minimal),
            &["--sheet", "1", "--ref", "A1:C3", "--column", "A"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx filters-sorts clear-sort",
            "range",
            Some(&sort_source),
            &["--sheet", "1"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx names add",
            "name",
            Some(minimal),
            &[
                "--name",
                "ContractName",
                "--sheet",
                "Sheet1",
                "--range",
                "A1:B2",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx names update",
            "name",
            Some(&name_source),
            &["--name", "SalesData", "--ref", "SUM('Sheet1'!$B$1:$B$2)"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx names rename",
            "name",
            Some(&name_source),
            &["--name", "SalesData", "--new-name", "RevenueData"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx names delete",
            "name",
            Some(&name_source),
            &["--name", "SalesData"],
            true,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx tables create",
            "table",
            Some(chart),
            &[
                "--sheet",
                "Data",
                "--range",
                "A1:B4",
                "--table",
                "ContractTable",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx tables append-rows",
            "table",
            Some(table),
            &[
                "--table",
                "Sales",
                "--values",
                r#"[["North",30],["South",40]]"#,
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx tables append-records",
            "table",
            Some(table),
            &[
                "--table",
                "Sales",
                "--expect-range",
                "A1:B4",
                "--records",
                r#"[{"Region":"North","Revenue":30}]"#,
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx tables set-column-format",
            "table",
            Some(table),
            &[
                "--table", "Sales", "--column", "Revenue", "--preset", "currency",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx pivots create",
            "pivot",
            Some(table),
            &[
                "--table",
                "Sales",
                "--name",
                "ContractPivot",
                "--rows",
                "Region",
                "--values",
                "Revenue:sum",
                "--anchor",
                "D1",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx workbook metadata update",
            "package",
            Some(minimal),
            &["--title", "Envelope contract"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx ranges set",
            "range",
            Some(minimal),
            &[
                "--sheet",
                "Sheet1",
                "--range",
                "A1:B2",
                "--values",
                r#"[["Name",42],["Tail",true]]"#,
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx ranges set-format",
            "range",
            Some(minimal),
            &[
                "--sheet", "Sheet1", "--range", "A1:B2", "--preset", "currency",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx ranges set-style",
            "range",
            Some(minimal),
            &[
                "--sheet",
                "Sheet1",
                "--range",
                "A1:B2",
                "--font-bold",
                "--fill-color",
                "#DDEEFF",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx cells set",
            "cell",
            Some(minimal),
            &[
                "--sheet", "Sheet1", "--cell", "B2", "--value", "42", "--type", "number",
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx cells clear",
            "cell",
            Some(minimal),
            &["--sheet", "Sheet1", "--ref", "A1"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx cells set-batch",
            "cell",
            Some(minimal),
            &[
                "--sheet",
                "Sheet1",
                "--cells",
                r#"[{"ref":"B1","value":"64","type":"number"},{"ref":"A2","value":"batch"}]"#,
            ],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx freeze set",
            "sheet",
            Some(minimal),
            &["--sheet", "Sheet1", "--rows", "2", "--cols", "1"],
            false,
        ),
        contract_case_with_out(
            dir,
            "ooxml xlsx freeze clear",
            "sheet",
            Some(&freeze_source),
            &["--sheet", "Sheet1"],
            false,
        ),
    ]);
    cases
}

#[test]
fn xlsx_mutation_commands_satisfy_the_envelope_contract() {
    let dir = temp_dir("xlsx-contract-matrix");
    let schema_response = run_json(&["--json", "capabilities", "--schema", "mutation-envelope"]);
    let cases = xlsx_contract_cases(&dir);
    assert_eq!(cases.len(), 60, "reviewed XLSX mutation denominator");
    let mut paths = cases.iter().map(|case| case.path).collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(
        paths.len(),
        cases.len(),
        "XLSX command paths must be unique"
    );
    let rows = cases
        .iter()
        .map(|case| run_contract_case(case, &schema_response["document"]))
        .collect::<Vec<_>>();
    assert_contract_matrix(&rows);
    fs::remove_dir_all(dir).expect("remove XLSX contract directory");
}

fn pptx_contract_cases(dir: &Path) -> Vec<ContractCase> {
    let title = "testdata/pptx/title-content/presentation.pptx";
    let multi = "testdata/pptx/slide-assembly-multi/presentation.pptx";
    let chart = "testdata/pptx/chart-simple/presentation.pptx";
    let table = "testdata/pptx/table-slide/presentation.pptx";
    let animation = "testdata/pptx/animations-synthetic/presentation.pptx";
    let workbook = "testdata/xlsx/outline-table/workbook.xlsx";

    let compose_items = dir.join("compose-items.json");
    fs::write(
        &compose_items,
        r#"[{"kind":"text","text":"Envelope compose","fontSize":18}]"#,
    )
    .expect("write compose items");
    let table_data = dir.join("table-data.json");
    fs::write(&table_data, r#"[["Region","Amount"],["North",42]]"#).expect("write table data");
    let media = dir.join("contract.mp4");
    let replacement_media = dir.join("contract-replacement.mp4");
    fs::write(&media, b"opaque contract media").expect("write media");
    fs::write(&replacement_media, b"opaque replacement media").expect("write replacement media");

    let animation_json = run_json(&["--json", "pptx", "animations", "list", animation]);
    let effects = animation_json["slides"][0]["effects"]
        .as_array()
        .expect("animation effects");
    let effect_id = effects[0]["effectId"]
        .as_i64()
        .expect("effect id")
        .to_string();
    let animation_order = effects
        .iter()
        .rev()
        .map(|effect| {
            effect["clickStepId"]
                .as_i64()
                .expect("click step")
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(",");

    let comment_source = setup_with_out(
        dir,
        "pptx-comment",
        "pptx",
        owned_args(&[
            "--json",
            "pptx",
            "comments",
            "add",
            title,
            "--slide",
            "1",
            "--author",
            "Contract",
            "--text",
            "Envelope comment",
            "--date",
            "2026-09-03T12:00:00Z",
        ]),
    );
    let comment_json = run_json(&[
        "--json",
        "pptx",
        "comments",
        "list",
        &comment_source,
        "--slide",
        "1",
    ]);
    let comment_handle = comment_json["slides"][0]["comments"][0]["handle"]
        .as_str()
        .expect("comment handle")
        .to_string();
    let media_source = setup_with_out(
        dir,
        "pptx-media",
        "pptx",
        vec![
            "--json".into(),
            "pptx".into(),
            "media".into(),
            "add".into(),
            title.into(),
            "--slide".into(),
            "1".into(),
            "--file".into(),
            media.to_string_lossy().to_string(),
            "--name".into(),
            "ContractMedia".into(),
        ],
    );
    let media_json = run_json(&["--json", "pptx", "media", "list", &media_source]);
    let media_shape = media_json["slides"][0]["clips"][0]["shapeId"]
        .as_i64()
        .expect("media shape id")
        .to_string();

    let custom =
        |path: &'static str, kind: &'static str, args: Vec<String>, removes_destination: bool| {
            let leaf = path.strip_prefix("ooxml ").unwrap().replace(' ', "-");
            ContractCase {
                path,
                destination_kind: kind,
                args: with_out(args, &dir.join(format!("{leaf}.pptx"))),
                removes_destination,
            }
        };
    let c = |path, kind, input, tail: &[&str]| {
        contract_case_with_out(dir, path, kind, Some(input), tail, false)
    };
    let remove = |path, kind, input, tail: &[&str]| {
        contract_case_with_out(dir, path, kind, Some(input), tail, true)
    };

    let mut cases = vec![
        custom(
            "ooxml pptx slides compose",
            "slide",
            vec![
                "--json".into(),
                "pptx".into(),
                "slides".into(),
                "compose".into(),
                title.into(),
                "--slide".into(),
                "1".into(),
                "--items".into(),
                compose_items.to_string_lossy().to_string(),
            ],
            false,
        ),
        remove("ooxml pptx slides delete", "slide", multi, &["2"]),
        c("ooxml pptx slides move", "slide", multi, &["1", "3"]),
        c("ooxml pptx slides reorder", "slide", multi, &["3,1,2,4,5"]),
        c(
            "ooxml pptx slides import-slide",
            "slide",
            title,
            &[
                "--source",
                multi,
                "--slide",
                "1",
                "--layout-policy",
                "import",
                "--theme-policy",
                "import",
            ],
        ),
        custom(
            "ooxml pptx slides merge",
            "slide",
            owned_args(&[
                "--json",
                "pptx",
                "slides",
                "merge",
                title,
                multi,
                "--layout-policy",
                "import",
                "--theme-policy",
                "import",
            ]),
            false,
        ),
        c("ooxml pptx clone-slide", "slide", title, &["--slide", "1"]),
        c(
            "ooxml pptx new-slide-from-layout",
            "slide",
            title,
            &["--layout", "1", "--set-text", "title=Envelope"],
        ),
        custom(
            "ooxml pptx template compile",
            "template",
            owned_args(&[
                "--json",
                "pptx",
                "template",
                "compile",
                "testdata/pptx/template-branded/manifest.json",
                "testdata/pptx/template-branded/spec-simple.yaml",
                "--archetype",
                "testdata/pptx/template-branded/presentation.pptx",
            ]),
            false,
        ),
        c(
            "ooxml pptx scaffold",
            "package",
            "--title",
            &["Envelope scaffold"],
        ),
        c(
            "ooxml pptx add-textbox",
            "shape",
            title,
            &[
                "--slide",
                "1",
                "--text",
                "Envelope textbox",
                "--x",
                "100000",
                "--y",
                "100000",
                "--cx",
                "1000000",
                "--cy",
                "500000",
            ],
        ),
        c(
            "ooxml pptx text set",
            "shape",
            title,
            &[
                "--slide",
                "1",
                "--target",
                "title",
                "--text",
                "Envelope text",
            ],
        ),
        c(
            "ooxml pptx fields set",
            "field",
            title,
            &[
                "--footer",
                "Envelope footer",
                "--show-footer",
                "true",
                "--show-slide-number",
                "true",
            ],
        ),
        c(
            "ooxml pptx theme update",
            "style",
            title,
            &[
                "--color",
                "accent1=336699",
                "--major-font",
                "Aptos Display",
                "--minor-font",
                "Aptos",
            ],
        ),
        c(
            "ooxml pptx place image",
            "image",
            title,
            &[
                "--slide",
                "1",
                "--image",
                "testdata/test_image.png",
                "--x",
                "0",
                "--y",
                "0",
                "--cx",
                "1000000",
                "--cy",
                "1000000",
            ],
        ),
        custom(
            "ooxml pptx place table",
            "table",
            vec![
                "--json".into(),
                "pptx".into(),
                "place".into(),
                "table".into(),
                title.into(),
                "--slide".into(),
                "1".into(),
                "--data".into(),
                table_data.to_string_lossy().to_string(),
                "--format".into(),
                "json".into(),
                "--x".into(),
                "0".into(),
                "--y".into(),
                "0".into(),
                "--cx".into(),
                "3000000".into(),
                "--cy".into(),
                "1500000".into(),
            ],
            false,
        ),
        c(
            "ooxml pptx place table-from-xlsx",
            "table",
            title,
            &[
                "--slide",
                "1",
                "--workbook",
                workbook,
                "--sheet",
                "Data",
                "--range",
                "A1:C3",
                "--x",
                "0",
                "--y",
                "0",
                "--cx",
                "3000000",
                "--cy",
                "1500000",
            ],
        ),
        c(
            "ooxml pptx shapes set-bounds",
            "shape",
            title,
            &[
                "--slide",
                "1",
                "--target",
                "title",
                "--bounds",
                "100000,100000,3000000,1000000",
            ],
        ),
        remove(
            "ooxml pptx shapes delete",
            "shape",
            title,
            &["--slide", "1", "--target", "title"],
        ),
        c(
            "ooxml pptx animations add",
            "animation",
            title,
            &["--slide", "1", "--shape", "shape:2", "--effect", "fade"],
        ),
        remove(
            "ooxml pptx animations remove",
            "animation",
            animation,
            &["--slide", "1", "--effect-id", &effect_id],
        ),
        c(
            "ooxml pptx animations reorder",
            "animation",
            animation,
            &["--slide", "1", "--order", &animation_order],
        ),
        remove(
            "ooxml pptx animations prune-stale",
            "animation",
            animation,
            &["--slide", "4"],
        ),
        c(
            "ooxml pptx masters add-placeholder",
            "master",
            title,
            &[
                "--master",
                "1",
                "--type",
                "text",
                "--bounds",
                "100000,100000,1000000,500000",
            ],
        ),
        c(
            "ooxml pptx masters import",
            "master",
            title,
            &[
                "--source",
                multi,
                "--master",
                "1",
                "--theme-policy",
                "import",
            ],
        ),
        c(
            "ooxml pptx layouts clone",
            "layout",
            title,
            &["--layout", "1", "--name", "EnvelopeClone"],
        ),
        c(
            "ooxml pptx layouts import",
            "layout",
            title,
            &[
                "--source",
                multi,
                "--layout",
                "1",
                "--theme-policy",
                "import",
            ],
        ),
        c(
            "ooxml pptx layouts rename",
            "layout",
            title,
            &["--layout", "2", "--name", "EnvelopeRenamed"],
        ),
        c(
            "ooxml pptx layouts set-bounds",
            "layout",
            title,
            &[
                "--layout",
                "2",
                "--target",
                "shape:3",
                "--bounds",
                "111111,222222,333333,444444",
            ],
        ),
        remove(
            "ooxml pptx layouts delete-shape",
            "layout",
            title,
            &["--layout", "2", "--target", "shape:3"],
        ),
        c(
            "ooxml pptx layouts add-placeholder",
            "layout",
            title,
            &[
                "--layout",
                "7",
                "--type",
                "pic",
                "--idx",
                "0",
                "--bounds",
                "1000,2000,3000,4000",
            ],
        ),
        c(
            "ooxml pptx charts create",
            "chart",
            multi,
            &[
                "--slide",
                "1",
                "--type",
                "bar",
                "--title",
                "Envelope Chart",
                "--values-json",
                r#"[["","North","South"],["Q1",10,20],["Q2",15,25]]"#,
            ],
        ),
        c(
            "ooxml pptx charts update-data",
            "chart",
            chart,
            &[
                "--slide",
                "1",
                "--chart",
                "chart:1",
                "--series",
                "1",
                "--values-json",
                r#"["12","24","36"]"#,
                "--categories-json",
                r#"["East","West","Central"]"#,
            ],
        ),
        c(
            "ooxml pptx charts set-title",
            "chart",
            chart,
            &[
                "--slide",
                "1",
                "--chart",
                "chart:1",
                "--title",
                "Envelope Title",
            ],
        ),
        c(
            "ooxml pptx charts set-legend",
            "chart",
            chart,
            &["--slide", "1", "--chart", "chart:1", "--position", "bottom"],
        ),
        c(
            "ooxml pptx charts set-chart-area-fill",
            "chart",
            chart,
            &[
                "--slide",
                "1",
                "--chart",
                "chart:1",
                "--fill-color",
                "FFEEDD",
            ],
        ),
        c(
            "ooxml pptx charts set-plot-area-fill",
            "chart",
            chart,
            &[
                "--slide",
                "1",
                "--chart",
                "chart:1",
                "--fill-color",
                "DDEEFF",
            ],
        ),
        c(
            "ooxml pptx charts set-series-style",
            "chart",
            chart,
            &[
                "--slide",
                "1",
                "--chart",
                "chart:1",
                "--series",
                "1",
                "--fill-color",
                "FF8800",
            ],
        ),
        c(
            "ooxml pptx charts set-axis",
            "chart",
            chart,
            &[
                "--slide",
                "1",
                "--chart",
                "chart:1",
                "--axis",
                "value",
                "--title",
                "Envelope Axis",
            ],
        ),
        c(
            "ooxml pptx charts convert-type",
            "chart",
            chart,
            &["--slide", "1", "--chart", "chart:1", "--to", "line"],
        ),
        c(
            "ooxml pptx charts copy-style",
            "chart",
            chart,
            &[
                "--chart",
                "chart:2",
                "--from",
                chart,
                "--from-slide",
                "1",
                "--from-chart",
                "chart:1",
            ],
        ),
        c(
            "ooxml pptx tables set-cell",
            "table",
            table,
            &[
                "--slide",
                "2",
                "--target",
                "table:1",
                "--row",
                "2",
                "--col",
                "2",
                "--text",
                "Envelope cell",
            ],
        ),
        remove(
            "ooxml pptx tables delete-row",
            "table",
            table,
            &["--slide", "2", "--target", "table:1", "--row", "2"],
        ),
        c(
            "ooxml pptx tables insert-row",
            "table",
            table,
            &["--slide", "2", "--target", "table:1", "--at", "2"],
        ),
        remove(
            "ooxml pptx tables delete-col",
            "table",
            table,
            &["--slide", "2", "--target", "table:1", "--col", "2"],
        ),
        c(
            "ooxml pptx tables insert-col",
            "table",
            table,
            &[
                "--slide",
                "2",
                "--target",
                "table:1",
                "--at",
                "1",
                "--width-emu",
                "1234567",
            ],
        ),
        c(
            "ooxml pptx tables update-from-xlsx",
            "table",
            table,
            &[
                "--slide",
                "2",
                "--target",
                "table:1",
                "--workbook",
                workbook,
                "--sheet",
                "Data",
                "--range",
                "A1:C3",
            ],
        ),
        custom(
            "ooxml pptx media add",
            "media",
            vec![
                "--json".into(),
                "pptx".into(),
                "media".into(),
                "add".into(),
                title.into(),
                "--slide".into(),
                "1".into(),
                "--file".into(),
                media.to_string_lossy().to_string(),
                "--name".into(),
                "EnvelopeMedia".into(),
            ],
            false,
        ),
        custom(
            "ooxml pptx media replace",
            "media",
            vec![
                "--json".into(),
                "pptx".into(),
                "media".into(),
                "replace".into(),
                media_source,
                "--slide".into(),
                "1".into(),
                "--shape".into(),
                media_shape,
                "--file".into(),
                replacement_media.to_string_lossy().to_string(),
            ],
            false,
        ),
        c(
            "ooxml pptx notes set",
            "slide",
            title,
            &["--slide", "1", "--text", "Envelope notes"],
        ),
        remove(
            "ooxml pptx notes clear",
            "slide",
            "testdata/pptx/notes-slide/presentation.pptx",
            &["--slide", "1"],
        ),
        c(
            "ooxml pptx comments add",
            "comment",
            title,
            &[
                "--slide",
                "1",
                "--author",
                "Contract",
                "--text",
                "Envelope add",
                "--date",
                "2026-09-03T12:00:00Z",
            ],
        ),
        c(
            "ooxml pptx comments edit",
            "comment",
            &comment_source,
            &["--handle", &comment_handle, "--text", "Envelope edited"],
        ),
        remove(
            "ooxml pptx comments remove",
            "comment",
            &comment_source,
            &["--handle", &comment_handle],
        ),
        c(
            "ooxml pptx replace text",
            "shape",
            title,
            &[
                "--slide",
                "1",
                "--target",
                "title",
                "--text",
                "Envelope replacement",
            ],
        ),
        c(
            "ooxml pptx replace text-occurrences",
            "shape",
            title,
            &[
                "--match-text",
                "Title",
                "--new-text",
                "Envelope occurrence",
                "--expect-count",
                "1",
            ],
        ),
        c(
            "ooxml pptx replace text-from-xlsx",
            "shape",
            title,
            &[
                "--slide",
                "1",
                "--target",
                "title",
                "--workbook",
                workbook,
                "--sheet",
                "Data",
                "--range",
                "A1:B2",
            ],
        ),
        c(
            "ooxml pptx replace images",
            "image",
            "testdata/pptx/slide-assembly-notes-media/presentation.pptx",
            &[
                "--slide",
                "2",
                "--target",
                "shape:4",
                "--image",
                "testdata/test_image.png",
            ],
        ),
    ];

    let binding_workbook = setup_with_out(
        dir,
        "pptx-binding-workbook",
        "xlsx",
        owned_args(&["--json", "xlsx", "scaffold", "--sheet", "Sheet1"]),
    );
    let binding_workbook = setup_with_out(
        dir,
        "pptx-binding-data",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "ranges",
            "set",
            &binding_workbook,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:H2",
            "--values",
            r#"[["id","op","slide","target","sourceSheet","sourceRange","mode","text"],["title","replace-text",1,"title","Sheet1","J1","preserve-format",""]]"#,
        ]),
    );
    let binding_workbook = setup_with_out(
        dir,
        "pptx-binding-value",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "cells",
            "set",
            &binding_workbook,
            "--sheet",
            "Sheet1",
            "--cell",
            "J1",
            "--value",
            "Bound envelope",
        ]),
    );
    cases.insert(
        10,
        c(
            "ooxml pptx xlsx-bindings apply",
            "slide",
            title,
            &[
                "--workbook",
                &binding_workbook,
                "--sheet",
                "Sheet1",
                "--range",
                "A1:H2",
            ],
        ),
    );

    let map_workbook = setup_with_out(
        dir,
        "pptx-map-workbook",
        "xlsx",
        owned_args(&["--json", "xlsx", "scaffold", "--sheet", "Map"]),
    );
    let map_workbook = setup_with_out(
        dir,
        "pptx-map-data",
        "xlsx",
        owned_args(&[
            "--json",
            "xlsx",
            "ranges",
            "set",
            &map_workbook,
            "--sheet",
            "Map",
            "--range",
            "A1:C2",
            "--values",
            r#"[["slide","target","text"],[1,"title","Mapped envelope"]]"#,
        ]),
    );
    cases.insert(
        cases.len() - 1,
        c(
            "ooxml pptx replace text-map-from-xlsx",
            "shape",
            title,
            &[
                "--workbook",
                &map_workbook,
                "--sheet",
                "Map",
                "--range",
                "A1:C2",
            ],
        ),
    );
    cases
}

#[test]
fn pptx_mutation_commands_satisfy_the_envelope_contract() {
    let dir = temp_dir("pptx-contract-matrix");
    let schema_response = run_json(&["--json", "capabilities", "--schema", "mutation-envelope"]);
    let cases = pptx_contract_cases(&dir);
    assert_eq!(cases.len(), 60, "reviewed PPTX mutation denominator");
    let mut paths = cases.iter().map(|case| case.path).collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();
    assert_eq!(
        paths.len(),
        cases.len(),
        "PPTX command paths must be unique"
    );
    let rows = cases
        .iter()
        .map(|case| run_contract_case(case, &schema_response["document"]))
        .collect::<Vec<_>>();
    assert_contract_matrix(&rows);
    fs::remove_dir_all(dir).expect("remove PPTX contract directory");
}

#[test]
fn docx_scaffold_and_paragraph_append_emit_additive_envelopes() {
    let dir = temp_dir("docx");
    let scaffold = dir.join("scaffold.docx");
    let appended = dir.join("appended.docx");
    let scaffold_arg = scaffold.to_string_lossy();
    let appended_arg = appended.to_string_lossy();

    let created = run_json(&[
        "--json",
        "docx",
        "scaffold",
        "--out",
        &scaffold_arg,
        "--text",
        "Envelope seed",
    ]);
    assert_eq!(created["created"], true, "legacy key must remain");
    assert_eq!(created["validated"], true, "legacy key must remain");
    let envelope = &created["mutationEnvelope"];
    assert_eq!(envelope["file"], scaffold_arg.as_ref());
    assert_eq!(envelope["family"], "docx");
    assert_eq!(envelope["destination"]["kind"], "package");
    assert_eq!(envelope["destination"]["partUri"], "/");
    assert_eq!(
        envelope["checkCommand"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>(),
        ["ooxml", "--json", "check"]
    );
    assert!(envelope["renderCommand"].as_str().is_some());
    assert_eq!(envelope["warnings"], serde_json::json!([]));
    assert_eq!(envelope["aliasesApplied"], serde_json::json!([]));
    assert!(Path::new(&*scaffold_arg).is_file());

    let appended_result = run_json(&[
        "--json",
        "docx",
        "paragraphs",
        "append",
        &scaffold_arg,
        "--text",
        "Envelope destination",
        "--out",
        &appended_arg,
    ]);
    assert_eq!(appended_result["index"], 2, "legacy key must remain");
    let envelope = &appended_result["mutationEnvelope"];
    assert_eq!(envelope["file"], appended_arg.as_ref());
    assert_eq!(envelope["destination"]["partUri"], "/word/document.xml");
    assert_eq!(envelope["destination"]["primarySelector"], "block:2");
    assert_eq!(envelope["changed"][0]["selector"], "block:2");
    assert!(
        envelope["changed"][0]["afterHash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:") && hash.len() == 71)
    );
    assert!(Path::new(&*appended_arg).is_file());

    let validated = run_json(&["--json", "validate", "--strict", &appended_arg]);
    assert_eq!(validated["valid"], true);
    let readback = run_json(&["--json", "docx", "blocks", &appended_arg]);
    assert_eq!(readback["blocks"][1]["text"], "Envelope destination");

    fs::remove_dir_all(dir).expect("remove test directory");
}

#[test]
fn capabilities_serves_the_pinned_mutation_envelope_schema() {
    let response = run_json(&["--json", "capabilities", "--schema", "mutation-envelope"]);
    assert_eq!(response["schema"], "mutation-envelope");
    let schema = &response["document"];
    assert_eq!(
        schema["$id"],
        "https://ooxml-cli.dev/schemas/mutation-envelope.schema.json"
    );
    assert_eq!(schema["additionalProperties"], false);
    for required in [
        "file",
        "family",
        "command",
        "destination",
        "changed",
        "readbackCommand",
        "validateCommand",
        "conformanceCommand",
        "checkCommand",
        "warnings",
        "aliasesApplied",
        "validated",
    ] {
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == required),
            "missing required schema field {required}"
        );
    }
}

#[test]
fn xlsx_and_pptx_scaffolds_emit_family_specific_envelopes() {
    let dir = temp_dir("scaffolds");
    for (family, extension, title_flag) in
        [("xlsx", "xlsx", "--sheet"), ("pptx", "pptx", "--title")]
    {
        let output = dir.join(format!("created.{extension}"));
        let output_arg = output.to_string_lossy();
        let result = run_json(&[
            "--json",
            family,
            "scaffold",
            "--out",
            &output_arg,
            title_flag,
            "Envelope seed",
        ]);
        let envelope = &result["mutationEnvelope"];
        assert_eq!(envelope["file"], output_arg.as_ref(), "{family}");
        assert_eq!(envelope["family"], family, "{family}");
        assert_eq!(envelope["destination"]["kind"], "package", "{family}");
        assert_eq!(envelope["destination"]["partUri"], "/", "{family}");
        assert!(envelope["renderCommand"].is_string(), "{family}");
        assert_eq!(
            envelope.get("layoutCheckCommand").is_some(),
            family == "pptx",
            "{family}"
        );
        let validated = run_json(&["--json", "validate", "--strict", &output_arg]);
        assert_eq!(validated["valid"], true, "{family}");
        let outline = run_json(&["--json", "outline", &output_arg, "--depth", "3"]);
        assert_eq!(outline["file"], output_arg.as_ref(), "{family}");
    }
    fs::remove_dir_all(dir).expect("remove test directory");
}

#[test]
fn xlsx_cell_and_pptx_shape_envelopes_preserve_writer_destinations() {
    let dir = temp_dir("objects");

    let xlsx_source = dir.join("source.xlsx");
    let xlsx_output = dir.join("cell.xlsx");
    let xlsx_source_arg = xlsx_source.to_string_lossy();
    let xlsx_output_arg = xlsx_output.to_string_lossy();
    run_json(&[
        "--json",
        "xlsx",
        "scaffold",
        "--out",
        &xlsx_source_arg,
        "--sheet",
        "Sheet1",
    ]);
    let cell = run_json(&[
        "--json",
        "xlsx",
        "cells",
        "set",
        &xlsx_source_arg,
        "--sheet",
        "Sheet1",
        "--cell",
        "A1",
        "--value",
        "Envelope cell",
        "--out",
        &xlsx_output_arg,
    ]);
    let cell_envelope = &cell["mutationEnvelope"];
    assert_eq!(cell_envelope["destination"]["kind"], "cell");
    assert_eq!(cell_envelope["destination"]["primarySelector"], "cell:A1");
    assert_eq!(cell_envelope["destination"]["handle"], cell["handle"]);
    assert_eq!(
        cell_envelope["destination"]["partUri"],
        "/xl/worksheets/sheet1.xml"
    );
    assert!(
        cell_envelope["readbackCommand"]
            .as_str()
            .unwrap()
            .contains("xlsx cells extract")
    );

    let pptx_source = dir.join("source.pptx");
    let pptx_output = dir.join("shape.pptx");
    let pptx_source_arg = pptx_source.to_string_lossy();
    let pptx_output_arg = pptx_output.to_string_lossy();
    run_json(&[
        "--json",
        "pptx",
        "scaffold",
        "--out",
        &pptx_source_arg,
        "--title",
        "Envelope slide",
    ]);
    let shape = run_json(&[
        "--json",
        "pptx",
        "add-textbox",
        &pptx_source_arg,
        "--slide",
        "1",
        "--text",
        "Envelope shape",
        "--x",
        "100000",
        "--y",
        "100000",
        "--cx",
        "1000000",
        "--cy",
        "500000",
        "--out",
        &pptx_output_arg,
    ]);
    let shape_envelope = &shape["mutationEnvelope"];
    assert_eq!(shape_envelope["destination"]["kind"], "shape");
    assert_eq!(
        shape_envelope["destination"]["primarySelector"],
        shape["destination"]["primarySelector"]
    );
    assert_eq!(
        shape_envelope["destination"]["selectors"],
        serde_json::json!([
            shape["destination"]["primarySelector"].clone(),
            shape["destination"]["handle"].clone(),
            shape["destination"]["selectors"][1].clone()
        ])
    );
    assert!(shape_envelope["layoutCheckCommand"].is_string());
    let layout = run_json(&["--json", "pptx", "validate-layout", &pptx_output_arg]);
    assert!(layout["slideReports"].is_array());

    for file in [&xlsx_output_arg, &pptx_output_arg] {
        let validated = run_json(&["--json", "validate", "--strict", file]);
        assert_eq!(validated["valid"], true, "{}", file);
    }
    fs::remove_dir_all(dir).expect("remove test directory");
}
