#[test]
fn serve_op_supports_xlsx_charts_set_series_style() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-serve-xlsx-chart-style-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let input = temp_dir.join("input.xlsx");
    let output = temp_dir.join("serve-chart-style-out.xlsx");
    fs::copy("testdata/xlsx/chart-workbook/workbook.xlsx", &input).expect("stage xlsx");
    let input_str = input.to_str().expect("input path").to_string();
    let output_str = output.to_str().expect("output path").to_string();

    let mut child = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut stdin = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let mut reader = BufReader::new(stdout);

    let open_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            1,
            "open",
            serde_json::json!({"file": input_str, "out": output_str}),
        ),
    );
    let session = open_response["result"]["sessionId"]
        .as_str()
        .expect("session id")
        .to_string();

    let op_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(
            2,
            "op",
            serde_json::json!({
                "session": session,
                "command": "xlsx charts set-series-style",
                "args": {
                    "sheet": "Data",
                    "chart": "chart:1",
                    "series": 1,
                    "fillColor": "FF8800",
                    "line-color": "114477",
                    "lineWidthPt": 2,
                    "expectSeriesCount": 1
                },
            }),
        ),
    );
    assert!(
        op_response.get("error").is_none(),
        "charts set-series-style op failed: {op_response:?}"
    );
    let readback = &op_response["result"]["readback"];
    assert_eq!(
        readback["action"],
        Value::String("xlsx.chart.set-series-style".to_string())
    );
    assert_eq!(readback["series"], Value::from(1));
    assert_eq!(
        readback["chart"]["style"]["series"][0]["fillColor"],
        Value::String("FF8800".to_string())
    );
    assert_eq!(
        readback["chart"]["style"]["series"][0]["lineColor"],
        Value::String("114477".to_string())
    );
    assert_eq!(
        readback["chart"]["style"]["series"][0]["lineWidthPt"],
        Value::from(2)
    );

    let plan_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(3, "plan", serde_json::json!({"session": session})),
    );
    let argv = plan_response["result"]["plan"][0]["argv"]
        .as_array()
        .expect("plan argv");
    assert_eq!(argv[0], Value::String("xlsx".to_string()));
    assert_eq!(argv[1], Value::String("charts".to_string()));
    assert_eq!(argv[2], Value::String("set-series-style".to_string()));
    assert!(
        argv.windows(2)
            .any(|pair| pair[0] == serde_json::json!("--line-width-pt")
                && pair[1] == serde_json::json!("2")),
        "plan includes line-width flag: {argv:?}"
    );
    assert!(
        argv.contains(&Value::String("--no-validate".to_string())),
        "plan keeps serve mutation no-validate: {argv:?}"
    );

    let commit_response = serve_roundtrip(
        &mut stdin,
        &mut reader,
        &rpc_request(4, "commit", serde_json::json!({"session": session})),
    );
    assert!(
        commit_response.get("error").is_none(),
        "chart style commit failed: {commit_response:?}"
    );
    assert!(output.exists(), "serve commit output missing");
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["file"],
        Value::String(output_str.clone())
    );
    assert_eq!(
        commit_response["result"]["applied"][0]["readback"]["output"],
        Value::String(output_str.clone())
    );

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "--strict", "validate", &output_str]);
    assert_eq!(validate_code, 0, "chart style serve output validate exit");
    assert_eq!(
        validate_stderr, None,
        "chart style serve output validate stderr"
    );

    let (show_code, show_stdout, show_stderr) = run_ooxml(&[
        "--json",
        "xlsx",
        "charts",
        "show",
        &output_str,
        "--sheet",
        "Data",
        "--chart",
        "chart:1",
    ]);
    assert_eq!(show_code, 0, "chart style output show exit");
    assert_eq!(show_stderr, None, "chart style output show stderr");
    let show = show_stdout.expect("chart style output show");
    let style = &show["charts"][0]["style"]["series"][0];
    assert_eq!(style["fillColor"], Value::String("FF8800".to_string()));
    assert_eq!(style["lineColor"], Value::String("114477".to_string()));
    assert_eq!(style["lineWidthPt"], Value::from(2));

    let chart_xml = read_zip_string(Path::new(&output_str), "xl/charts/chart1.xml");
    assert!(chart_xml.contains("FF8800"), "chart XML has fill color");
    assert!(chart_xml.contains("114477"), "chart XML has line color");
    assert!(
        chart_xml.contains(r#"w="25400""#),
        "chart XML has line width"
    );

    drop(stdin);
    let status = child.wait().expect("serve exit");
    assert!(status.success());
    let _ = fs::remove_dir_all(&temp_dir);
}
