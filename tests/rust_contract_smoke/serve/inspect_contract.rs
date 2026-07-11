use std::collections::BTreeMap;

struct ServeInspectContractCase {
    family: &'static str,
    command: &'static str,
    aliases: &'static [&'static str],
    fixture: &'static str,
    args: Value,
    direct_argv: &'static [&'static str],
    wire_promised: bool,
}

fn serve_inspect_contract_cases() -> Vec<ServeInspectContractCase> {
    vec![
        inspect_case("xlsx", "xlsx ranges export", &[], "xlsx-minimal", serde_json::json!({"sheet": "1", "range": "A1"}), &["xlsx", "ranges", "export", "{file}", "--sheet", "1", "--range", "A1"], true),
        inspect_case("xlsx", "xlsx cells extract", &[], "xlsx-minimal", serde_json::json!({}), &["xlsx", "cells", "extract", "{file}"], true),
        inspect_case("xlsx", "xlsx comments list", &[], "xlsx-minimal", serde_json::json!({}), &["xlsx", "comments", "list", "{file}"], true),
        inspect_case(
            "xlsx",
            "xlsx conditional-formats list",
            &[
                "xlsx conditional-formatting list",
                "xlsx conditional-format list",
                "xlsx cf list",
            ],
            "xlsx-cf",
            serde_json::json!({"sheet": "1"}),
            &["xlsx", "conditional-formats", "list", "{file}", "--sheet", "1"],
            true,
        ),
        inspect_case(
            "xlsx",
            "xlsx conditional-formats show",
            &[
                "xlsx conditional-formatting show",
                "xlsx conditional-format show",
                "xlsx cf show",
            ],
            "xlsx-cf",
            serde_json::json!({"sheet": "1", "rule": "block:1/rule:1"}),
            &["xlsx", "conditional-formats", "show", "{file}", "--sheet", "1", "--rule", "block:1/rule:1"],
            true,
        ),
        inspect_case("xlsx", "xlsx sheets list", &[], "xlsx-minimal", serde_json::json!({}), &["xlsx", "sheets", "list", "{file}"], true),
        inspect_case("xlsx", "xlsx sheets show", &[], "xlsx-minimal", serde_json::json!({"sheet": "1"}), &["xlsx", "sheets", "show", "{file}", "--sheet", "1"], true),
        inspect_case("xlsx", "xlsx freeze show", &[], "xlsx-hyperlinks", serde_json::json!({"sheet": "1"}), &["xlsx", "freeze", "show", "{file}", "--sheet", "1"], true),
        inspect_case("xlsx", "xlsx hyperlinks list", &[], "xlsx-hyperlinks", serde_json::json!({"sheet": "1"}), &["xlsx", "hyperlinks", "list", "{file}", "--sheet", "1"], true),
        inspect_case("xlsx", "xlsx hyperlinks show", &[], "xlsx-hyperlinks", serde_json::json!({"sheet": "1", "cell": "A1"}), &["xlsx", "hyperlinks", "show", "{file}", "--sheet", "1", "--cell", "A1"], true),
        inspect_case("xlsx", "xlsx filters-sorts show", &[], "xlsx-minimal", serde_json::json!({"sheet": "1"}), &["xlsx", "filters-sorts", "show", "{file}", "--sheet", "1"], true),
        inspect_case("xlsx", "xlsx names list", &[], "xlsx-names", serde_json::json!({}), &["xlsx", "names", "list", "{file}"], true),
        inspect_case("xlsx", "xlsx names show", &[], "xlsx-names", serde_json::json!({"name": "GlobalName"}), &["xlsx", "names", "show", "{file}", "--name", "GlobalName"], false),
        inspect_case("xlsx", "xlsx tables list", &[], "xlsx-tables", serde_json::json!({"sheet": "Data"}), &["xlsx", "tables", "list", "{file}", "--sheet", "Data"], true),
        inspect_case("xlsx", "xlsx tables show", &[], "xlsx-tables", serde_json::json!({"sheet": "Data", "table": "Sales"}), &["xlsx", "tables", "show", "{file}", "--sheet", "Data", "--table", "Sales"], true),
        inspect_case("xlsx", "xlsx tables export", &[], "xlsx-tables", serde_json::json!({"sheet": "Data", "table": "Sales"}), &["xlsx", "tables", "export", "{file}", "--sheet", "Data", "--table", "Sales"], false),
        inspect_case("xlsx", "xlsx workbook metadata inspect", &[], "xlsx-minimal", serde_json::json!({}), &["xlsx", "workbook", "metadata", "inspect", "{file}"], false),
        inspect_case("docx", "docx text", &[], "docx-minimal", serde_json::json!({}), &["docx", "text", "{file}"], false),
        inspect_case("docx", "docx fields list", &[], "docx-minimal", serde_json::json!({}), &["docx", "fields", "list", "{file}"], false),
        inspect_case("docx", "docx headers list", &[], "docx-headers", serde_json::json!({}), &["docx", "headers", "list", "{file}"], false),
        inspect_case("docx", "docx footers list", &[], "docx-headers", serde_json::json!({}), &["docx", "footers", "list", "{file}"], false),
        inspect_case("docx", "docx headers show", &[], "docx-headers", serde_json::json!({"selector": "header:1:default"}), &["docx", "headers", "show", "{file}", "--selector", "header:1:default"], false),
        inspect_case("docx", "docx footers show", &[], "docx-headers", serde_json::json!({"id": "rId11"}), &["docx", "footers", "show", "{file}", "--id", "rId11"], false),
        inspect_case("docx", "docx images list", &[], "docx-minimal", serde_json::json!({}), &["docx", "images", "list", "{file}"], false),
        inspect_case("docx", "docx comments list", &[], "docx-minimal", serde_json::json!({}), &["docx", "comments", "list", "{file}"], false),
        inspect_case("docx", "docx blocks", &[], "docx-mixed-blocks", serde_json::json!({"block": 0}), &["docx", "blocks", "{file}", "--block", "0"], false),
        inspect_case("docx", "docx styles list", &[], "docx-styles", serde_json::json!({"type": "paragraph"}), &["docx", "styles", "list", "{file}", "--type", "paragraph"], false),
        inspect_case("docx", "docx styles show", &[], "docx-styles", serde_json::json!({"style": "Heading2"}), &["docx", "styles", "show", "{file}", "--style", "Heading2"], false),
        inspect_case("docx", "docx tables show", &[], "docx-tables", serde_json::json!({"table": 1}), &["docx", "tables", "show", "{file}", "--table", "1"], true),
        inspect_case("pptx", "pptx slides list", &[], "pptx-title-content", serde_json::json!({}), &["pptx", "slides", "list", "{file}"], true),
        inspect_case("pptx", "pptx slides selectors", &[], "pptx-title-content", serde_json::json!({"slide": 1}), &["pptx", "slides", "selectors", "{file}", "--slide", "1"], false),
        inspect_case("pptx", "pptx slides show", &[], "pptx-title-content", serde_json::json!({"slide": 1}), &["pptx", "slides", "show", "{file}", "--slide", "1"], true),
        inspect_case("pptx", "pptx extract text", &[], "pptx-title-content", serde_json::json!({"slide": 1}), &["pptx", "extract", "text", "{file}", "--slide", "1"], true),
        inspect_case("pptx", "pptx extract notes", &[], "pptx-notes", serde_json::json!({"slide": 1}), &["pptx", "extract", "notes", "{file}", "--slide", "1"], true),
        inspect_case("pptx", "pptx notes show", &[], "pptx-notes", serde_json::json!({"slide": 1}), &["pptx", "notes", "show", "{file}", "--slide", "1"], true),
        inspect_case("pptx", "pptx comments list", &[], "pptx-title-content", serde_json::json!({}), &["pptx", "comments", "list", "{file}"], false),
        inspect_case("pptx", "pptx masters list", &[], "pptx-title-content", serde_json::json!({}), &["pptx", "masters", "list", "{file}"], true),
        inspect_case("pptx", "pptx masters show", &[], "pptx-title-content", serde_json::json!({"master": 1}), &["pptx", "masters", "show", "{file}", "--master", "1"], false),
        inspect_case("pptx", "pptx layouts list", &[], "pptx-title-content", serde_json::json!({}), &["pptx", "layouts", "list", "{file}"], true),
        inspect_case("pptx", "pptx layouts show", &[], "pptx-title-content", serde_json::json!({"layout": "1"}), &["pptx", "layouts", "show", "{file}", "--layout", "1"], false),
        inspect_case("pptx", "pptx tables show", &[], "pptx-title-content", serde_json::json!({"slide": 1}), &["pptx", "tables", "show", "{file}", "--slide", "1"], true),
        inspect_case("pptx", "pptx shapes show", &[], "pptx-title-content", serde_json::json!({"slide": 1, "includeText": true, "includeBounds": true}), &["pptx", "shapes", "show", "{file}", "--slide", "1", "--include-text", "--include-bounds"], false),
    ]
}

fn inspect_case(
    family: &'static str,
    command: &'static str,
    aliases: &'static [&'static str],
    fixture: &'static str,
    args: Value,
    direct_argv: &'static [&'static str],
    wire_promised: bool,
) -> ServeInspectContractCase {
    ServeInspectContractCase {
        family,
        command,
        aliases,
        fixture,
        args,
        direct_argv,
        wire_promised,
    }
}

#[test]
fn serve_and_mcp_cover_the_full_canonical_inspect_contract() {
    let cases = serve_inspect_contract_cases();
    assert_inspect_contract_inventory(&cases);

    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-inspect-contract-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("inspect contract temp dir");
    let fixtures = prepare_inspect_contract_fixtures(&temp_dir);
    assert_eq!(fixtures.len(), 12);

    let mut serve_child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn Serve inspect contract");
    let mut serve_stdin = serve_child.stdin.take().expect("Serve stdin");
    let mut serve_reader = BufReader::new(serve_child.stdout.take().expect("Serve stdout"));

    let mut mcp_child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn MCP inspect contract");
    let mut mcp_stdin = mcp_child.stdin.take().expect("MCP stdin");
    let mut mcp_reader = BufReader::new(mcp_child.stdout.take().expect("MCP stdout"));

    let mut request_id = 1_i64;
    let mut serve_sessions = BTreeMap::new();
    let mut mcp_sessions = BTreeMap::new();
    let originals = fixtures
        .iter()
        .map(|(name, path)| (*name, fs::read(path).expect("read inspect contract fixture")))
        .collect::<BTreeMap<_, _>>();

    for (fixture, path) in &fixtures {
        let serve_open = serve_roundtrip(
            &mut serve_stdin,
            &mut serve_reader,
            &rpc_request(request_id, "open", serde_json::json!({"file": path})),
        );
        request_id += 1;
        assert!(
            serve_open.get("error").is_none(),
            "Serve open failed for {fixture}: {serve_open:?}"
        );
        serve_sessions.insert(
            *fixture,
            serve_open["result"]["sessionId"]
                .as_str()
                .expect("Serve session id")
                .to_string(),
        );

        let mcp_open = serve_roundtrip(
            &mut mcp_stdin,
            &mut mcp_reader,
            &rpc_request(
                request_id,
                "tools/call",
                serde_json::json!({"name": "open", "arguments": {"file": path}}),
            ),
        );
        request_id += 1;
        assert!(
            mcp_open.get("error").is_none(),
            "MCP open failed for {fixture}: {mcp_open:?}"
        );
        mcp_sessions.insert(
            *fixture,
            mcp_open["result"]["structuredContent"]["sessionId"]
                .as_str()
                .expect("MCP session id")
                .to_string(),
        );
    }

    let mut serve_working = BTreeMap::new();
    let mut mcp_working = BTreeMap::new();
    let mut serve_canonical = BTreeMap::new();
    let mut mcp_canonical = BTreeMap::new();
    for case in &cases {
        let serve_result = run_serve_inspect_contract_call(
            &mut serve_stdin,
            &mut serve_reader,
            request_id,
            &serve_sessions[case.fixture],
            case.command,
            case.args.clone(),
        );
        request_id += 1;
        remember_working_path(case.fixture, &serve_result, &mut serve_working);
        let direct_serve = run_direct_inspect_contract_call(
            case,
            &serve_working[case.fixture],
        );
        assert_eq!(
            serve_result, direct_serve,
            "Serve result must equal direct CLI JSON for {}",
            case.command
        );
        serve_canonical.insert(case.command, serve_result);

        let mcp_result = run_mcp_inspect_contract_call(
            &mut mcp_stdin,
            &mut mcp_reader,
            request_id,
            &mcp_sessions[case.fixture],
            case.command,
            case.args.clone(),
        );
        request_id += 1;
        remember_working_path(case.fixture, &mcp_result, &mut mcp_working);
        let direct_mcp = run_direct_inspect_contract_call(case, &mcp_working[case.fixture]);
        assert_eq!(
            mcp_result, direct_mcp,
            "MCP structuredContent must equal direct CLI JSON for {}",
            case.command
        );
        mcp_canonical.insert(case.command, mcp_result);
    }

    for case in &cases {
        for alias in case.aliases {
            let serve_alias = run_serve_inspect_contract_call(
                &mut serve_stdin,
                &mut serve_reader,
                request_id,
                &serve_sessions[case.fixture],
                alias,
                case.args.clone(),
            );
            request_id += 1;
            assert_eq!(
                serve_alias, serve_canonical[case.command],
                "Serve alias {alias} must equal {}",
                case.command
            );

            let mcp_alias = run_mcp_inspect_contract_call(
                &mut mcp_stdin,
                &mut mcp_reader,
                request_id,
                &mcp_sessions[case.fixture],
                alias,
                case.args.clone(),
            );
            request_id += 1;
            assert_eq!(
                mcp_alias, mcp_canonical[case.command],
                "MCP alias {alias} must equal {}",
                case.command
            );
        }
    }

    for (fixture, command) in [
        ("xlsx-minimal", "xlsx not-real"),
        ("docx-minimal", "docx not-real"),
        ("pptx-title-content", "pptx not-real"),
    ] {
        let expected = format!("unsupported serve inspect command: {command}");
        let serve_response = serve_roundtrip(
            &mut serve_stdin,
            &mut serve_reader,
            &rpc_request(
                request_id,
                "inspect",
                serde_json::json!({
                    "session": serve_sessions[fixture],
                    "command": command,
                    "args": {},
                }),
            ),
        );
        request_id += 1;
        assert_eq!(serve_response["error"]["message"], expected);
        assert_eq!(serve_response["error"]["data"]["type"], "invalid_args");

        let mcp_response = serve_roundtrip(
            &mut mcp_stdin,
            &mut mcp_reader,
            &rpc_request(
                request_id,
                "tools/call",
                serde_json::json!({
                    "name": "inspect",
                    "arguments": {
                        "session": mcp_sessions[fixture],
                        "command": command,
                        "args": {},
                    },
                }),
            ),
        );
        request_id += 1;
        assert_eq!(mcp_response["error"]["message"], expected);
        assert_eq!(mcp_response["error"]["data"]["type"], "invalid_args");
    }

    assert_eq!(serve_working.len(), fixtures.len());
    assert_eq!(mcp_working.len(), fixtures.len());
    for fixture in fixtures.keys() {
        assert_empty_inspect_plan(
            &mut serve_stdin,
            &mut serve_reader,
            request_id,
            &serve_sessions[fixture],
            false,
        );
        request_id += 1;
        assert_empty_inspect_plan(
            &mut mcp_stdin,
            &mut mcp_reader,
            request_id,
            &mcp_sessions[fixture],
            true,
        );
        request_id += 1;
        assert_eq!(
            fs::read(&serve_working[fixture]).expect("read Serve working package"),
            originals[fixture],
            "Serve inspect changed {fixture}"
        );
        assert_eq!(
            fs::read(&mcp_working[fixture]).expect("read MCP working package"),
            originals[fixture],
            "MCP inspect changed {fixture}"
        );
        assert_eq!(
            fs::read(&fixtures[fixture]).expect("read staged input package"),
            originals[fixture],
            "inspect changed staged fixture {fixture}"
        );
    }

    drop(serve_stdin);
    drop(mcp_stdin);
    assert!(serve_child.wait().expect("Serve exit").success());
    assert!(mcp_child.wait().expect("MCP exit").success());
    let _ = fs::remove_dir_all(&temp_dir);
}

fn assert_inspect_contract_inventory(cases: &[ServeInspectContractCase]) {
    assert_eq!(cases.len(), 42);
    let commands = cases.iter().map(|case| case.command).collect::<BTreeSet<_>>();
    assert_eq!(commands.len(), cases.len());
    for case in cases {
        assert_eq!(
            case.direct_argv.iter().filter(|arg| **arg == "{file}").count(),
            1,
            "direct argv must contain one explicit file slot for {}",
            case.command
        );
        assert_eq!(case.direct_argv.first().copied(), Some(case.family));
    }
    for (family, expected) in [("xlsx", 17), ("docx", 12), ("pptx", 13)] {
        assert_eq!(
            cases.iter().filter(|case| case.family == family).count(),
            expected,
            "canonical {family} inspect commands"
        );
    }
    let alias_set = cases
        .iter()
        .flat_map(|case| case.aliases.iter().copied())
        .collect::<BTreeSet<_>>();
    assert_eq!(cases.iter().map(|case| case.aliases.len()).sum::<usize>(), 6);
    assert_eq!(alias_set.len(), 6);
    assert_eq!(commands.len() + alias_set.len(), 48);
    assert_eq!(
        cases
            .iter()
            .filter(|case| !case.aliases.is_empty())
            .map(|case| case.command)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "xlsx conditional-formats list",
            "xlsx conditional-formats show",
        ])
    );
    assert_eq!(
        alias_set,
        BTreeSet::from([
            "xlsx conditional-formatting list",
            "xlsx conditional-format list",
            "xlsx cf list",
            "xlsx conditional-formatting show",
            "xlsx conditional-format show",
            "xlsx cf show",
        ])
    );

    let promised = cases
        .iter()
        .filter(|case| case.wire_promised)
        .map(|case| case.command)
        .collect::<BTreeSet<_>>();
    assert_eq!(promised.len(), 23);
    for (family, expected) in [("xlsx", 14), ("docx", 1), ("pptx", 8)] {
        assert_eq!(
            cases
                .iter()
                .filter(|case| case.family == family && case.wire_promised)
                .count(),
            expected,
            "wire-promised {family} inspect commands"
        );
    }
    let capabilities: Value = serde_json::from_str(include_str!(
        "../../../testdata/golden/command-manifest-contract/capabilities.json"
    ))
    .expect("frozen capabilities JSON");
    let capability_promised = capabilities["commands"]
        .as_array()
        .expect("capability commands")
        .iter()
        .filter(|command| {
            command["opIneligibleReason"]
                .as_str()
                .is_some_and(|reason| reason.contains("call via inspect in serve/MCP"))
        })
        .map(|command| {
            command["path"]
                .as_str()
                .expect("capability path")
                .strip_prefix("ooxml ")
                .expect("ooxml path prefix")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(promised, capability_promised);
}

fn run_serve_inspect_contract_call(
    stdin: &mut impl Write,
    reader: &mut impl BufRead,
    id: i64,
    session: &str,
    command: &str,
    args: Value,
) -> Value {
    let response = serve_roundtrip(
        stdin,
        reader,
        &rpc_request(
            id,
            "inspect",
            serde_json::json!({"session": session, "command": command, "args": args}),
        ),
    );
    assert_inspect_contract_success(command, &response);
    response["result"].clone()
}

fn run_mcp_inspect_contract_call(
    stdin: &mut impl Write,
    reader: &mut impl BufRead,
    id: i64,
    session: &str,
    command: &str,
    args: Value,
) -> Value {
    let response = serve_roundtrip(
        stdin,
        reader,
        &rpc_request(
            id,
            "tools/call",
            serde_json::json!({
                "name": "inspect",
                "arguments": {"session": session, "command": command, "args": args},
            }),
        ),
    );
    assert_inspect_contract_success(command, &response);
    response["result"]["structuredContent"].clone()
}

fn run_direct_inspect_contract_call(case: &ServeInspectContractCase, working: &str) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ooxml"));
    command.arg("--json");
    for arg in case.direct_argv {
        command.arg(if *arg == "{file}" { working } else { arg });
    }
    let output = command.output().expect("run direct inspect contract command");
    assert!(
        output.status.success(),
        "direct CLI failed for {} with argv {:?}: {}",
        case.command,
        case.direct_argv,
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed = serde_json::from_slice(&output.stdout);
    assert!(
        parsed.is_ok(),
        "direct CLI emitted invalid JSON for {}: {:?}: {}",
        case.command,
        parsed.as_ref().err(),
        String::from_utf8_lossy(&output.stdout)
    );
    parsed.expect("direct inspect JSON checked above")
}

fn assert_inspect_contract_success(command: &str, response: &Value) {
    if let Some(error) = response.get("error") {
        let message = error["message"].as_str().unwrap_or_default();
        assert!(
            !message.starts_with("unsupported serve inspect command:"),
            "generic unsupported inspect route for {command}: {response:?}"
        );
    }
    assert!(
        response.get("error").is_none(),
        "inspect contract call failed for {command}: {response:?}"
    );
}

fn remember_working_path(
    fixture: &'static str,
    result: &Value,
    paths: &mut BTreeMap<&'static str, String>,
) {
    if let Some(path) = result.get("file").and_then(Value::as_str)
        && let Some(existing) = paths.insert(fixture, path.to_string())
    {
        assert_eq!(existing, path, "working path changed for {fixture}");
    }
}

fn assert_empty_inspect_plan(
    stdin: &mut impl Write,
    reader: &mut impl BufRead,
    id: i64,
    session: &str,
    mcp: bool,
) {
    let request = if mcp {
        rpc_request(
            id,
            "tools/call",
            serde_json::json!({"name": "plan", "arguments": {"session": session}}),
        )
    } else {
        rpc_request(id, "plan", serde_json::json!({"session": session}))
    };
    let response = serve_roundtrip(stdin, reader, &request);
    assert!(response.get("error").is_none(), "plan failed: {response:?}");
    let result = if mcp {
        &response["result"]["structuredContent"]
    } else {
        &response["result"]
    };
    assert_eq!(result["opsCount"], 0);
    assert_eq!(result["plan"], serde_json::json!([]));
}

fn prepare_inspect_contract_fixtures(temp_dir: &Path) -> BTreeMap<&'static str, String> {
    let mut fixtures = BTreeMap::new();
    stage_inspect_fixture(
        temp_dir,
        "xlsx-minimal",
        "xlsx",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &mut fixtures,
    );
    let cf = temp_dir.join("xlsx-cf.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &cf,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:A5"/>
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
  <conditionalFormatting sqref="A1:A5"><cfRule type="expression" priority="1"><formula>A1&gt;0</formula></cfRule></conditionalFormatting>
</worksheet>"#,
    );
    fixtures.insert("xlsx-cf", cf.to_string_lossy().to_string());
    let hyperlinks = temp_dir.join("xlsx-hyperlinks.xlsx");
    write_freeze_hyperlink_inspect_xlsx(&hyperlinks);
    fixtures.insert(
        "xlsx-hyperlinks",
        hyperlinks.to_string_lossy().to_string(),
    );
    let names = temp_dir.join("xlsx-names.xlsx");
    write_defined_names_xlsx(&names);
    fixtures.insert("xlsx-names", names.to_string_lossy().to_string());
    let tables = temp_dir.join("xlsx-tables.xlsx");
    write_table_xlsx(&tables);
    fixtures.insert("xlsx-tables", tables.to_string_lossy().to_string());

    for (name, source) in [
        ("docx-minimal", "testdata/docx/minimal/document.docx"),
        ("docx-headers", "testdata/docx/headers/document.docx"),
        (
            "docx-mixed-blocks",
            "testdata/docx/mixed-blocks/document.docx",
        ),
        (
            "docx-styles",
            "testdata/docx/styles-catalog/document.docx",
        ),
        ("docx-tables", "testdata/docx/table/document.docx"),
    ] {
        stage_inspect_fixture(temp_dir, name, "docx", source, &mut fixtures);
    }
    stage_inspect_fixture(
        temp_dir,
        "pptx-title-content",
        "pptx",
        "testdata/pptx/title-content/presentation.pptx",
        &mut fixtures,
    );
    stage_inspect_fixture(
        temp_dir,
        "pptx-notes",
        "pptx",
        "testdata/pptx/notes-slide/presentation.pptx",
        &mut fixtures,
    );
    fixtures
}

fn stage_inspect_fixture(
    temp_dir: &Path,
    name: &'static str,
    extension: &str,
    source: &str,
    fixtures: &mut BTreeMap<&'static str, String>,
) {
    let destination = temp_dir.join(format!("{name}.{extension}"));
    fs::copy(source, &destination).expect("stage inspect contract fixture");
    fixtures.insert(name, destination.to_string_lossy().to_string());
}
