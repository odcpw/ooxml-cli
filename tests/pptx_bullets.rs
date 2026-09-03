use serde_json::Value;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

fn temp_dir(label: &str) -> PathBuf {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "ooxml-pptx-bullets-{label}-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml");
    assert!(
        output.status.success(),
        "args={args:?}\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON stdout")
}

fn validate(path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(["--json", "--strict", "validate", path.to_str().unwrap()])
        .output()
        .expect("run strict validator");
    assert!(
        output.status.success(),
        "strict validation failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );

    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let dotnet = PathBuf::from(home).join("dotnet/dotnet");
    let validator = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !dotnet.exists() || !validator.exists() {
        return;
    }
    let sdk = Command::new(dotnet)
        .arg(validator)
        .arg(path)
        .output()
        .expect("run Open XML SDK validator");
    assert!(
        sdk.status.success(),
        "SDK child-order validation failed for {}:\n{}\n{}",
        path.display(),
        String::from_utf8_lossy(&sdk.stdout),
        String::from_utf8_lossy(&sdk.stderr)
    );
}

fn zip_text(path: &Path, part: &str) -> String {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let mut entry = archive.by_name(part).unwrap();
    let mut text = String::new();
    entry.read_to_string(&mut text).unwrap();
    text
}

fn tx_body_for_shape(xml: &str, shape_id: u32) -> String {
    let marker = format!(r#"<p:cNvPr id="{shape_id}""#);
    let shape_start = xml.find(&marker).expect("shape marker");
    let body_start = xml[shape_start..]
        .find("<p:txBody>")
        .map(|offset| shape_start + offset)
        .expect("text body start");
    let body_end = xml[body_start..]
        .find("</p:txBody>")
        .map(|offset| body_start + offset + "</p:txBody>".len())
        .expect("text body end");
    xml[body_start..body_end].to_string()
}

#[test]
fn plaintext_paragraphs_are_shared_by_layout_text_and_text_set() {
    let dir = temp_dir("shared");
    let from_layout = dir.join("from-layout.pptx");
    let from_text_set = dir.join("from-text-set.pptx");
    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    let text = "- One\n- Two\n- Three\n\t- Nested A\n\t* Nested B";

    run(&[
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "Title and Content",
        "--set-text",
        &format!("body={text}"),
        "--out",
        from_layout.to_str().unwrap(),
    ]);
    run(&[
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--text",
        text,
        "--out",
        from_text_set.to_str().unwrap(),
    ]);
    let layout_xml = zip_text(&from_layout, "ppt/slides/slide5.xml");
    let set_xml = zip_text(&from_text_set, "ppt/slides/slide2.xml");
    assert_eq!(
        tx_body_for_shape(&layout_xml, 3),
        tx_body_for_shape(&set_xml, 3),
        "all plaintext routes must use one paragraph builder"
    );
    assert_eq!(
        tx_body_for_shape(&layout_xml, 3).matches("<a:p>").count(),
        5
    );
    assert!(!tx_body_for_shape(&layout_xml, 3).contains("One\n"));

    let readback = run(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        from_layout.to_str().unwrap(),
        "--slide",
        "5",
        "--target",
        "body",
        "--include-text",
    ]);
    let paragraphs = readback["shapes"][0]["paragraphs"].as_array().unwrap();
    assert_eq!(paragraphs.len(), 5);
    assert!(
        paragraphs
            .iter()
            .all(|paragraph| paragraph["bullet"] == true)
    );
    assert_eq!(paragraphs[0]["level"], 0);
    assert_eq!(paragraphs[3]["level"], 1);
    assert_eq!(paragraphs[4]["text"], "Nested B");

    let extracted = run(&[
        "--json",
        "pptx",
        "extract",
        "text",
        from_layout.to_str().unwrap(),
        "--slide",
        "5",
    ]);
    let body = extracted["slides"][0]["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|shape| shape["key"] == "body:1")
        .expect("extracted body placeholder");
    assert_eq!(body["text"]["paragraphs"][0]["bullet"], true);
    assert_eq!(body["text"]["paragraphs"][3]["level"], 1);

    validate(&from_layout);
    validate(&from_text_set);
}

#[test]
fn textbox_json_emits_explicit_bullets_and_rich_runs() {
    let dir = temp_dir("textbox-json");
    let paragraphs_file = dir.join("paragraphs.json");
    let output = dir.join("textbox.pptx");
    fs::write(
        &paragraphs_file,
        r#"[
          {"text":"Intro","bold":true,"size":24,"color":"112233","align":"center"},
          {"text":"","level":1,"bullet":true,"runs":[
            {"text":"Strong","bold":true},
            {"text":" and emphasis","italic":true}
          ]}
        ]"#,
    )
    .unwrap();

    run(&[
        "--json",
        "pptx",
        "add-textbox",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "1",
        "--paragraphs-file",
        paragraphs_file.to_str().unwrap(),
        "--x",
        "1in",
        "--y",
        "1in",
        "--cx",
        "5in",
        "--cy",
        "2in",
        "--out",
        output.to_str().unwrap(),
    ]);
    let xml = zip_text(&output, "ppt/slides/slide1.xml");
    let body = tx_body_for_shape(&xml, 4);
    assert!(body.contains("<a:buChar char=\"•\"/>"));
    assert!(body.contains("lvl=\"1\""));
    assert!(body.contains("b=\"1\""));
    assert!(body.contains("i=\"1\""));
    assert!(body.contains("val=\"112233\""));

    let readback = run(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        output.to_str().unwrap(),
        "--slide",
        "1",
        "--target",
        "shape:4",
        "--include-text",
    ]);
    assert_eq!(readback["shapes"][0]["paragraphs"][0]["text"], "Intro");
    assert_eq!(
        readback["shapes"][0]["paragraphs"][1]["text"],
        "Strong and emphasis"
    );
    assert_eq!(readback["shapes"][0]["paragraphs"][1]["level"], 1);
    assert_eq!(readback["shapes"][0]["paragraphs"][1]["bullet"], true);
    validate(&output);
}

#[test]
fn plaintext_textbox_markers_create_explicit_bullet_paragraphs() {
    let dir = temp_dir("textbox-text");
    let scaffold = dir.join("scaffold.pptx");
    let output = dir.join("textbox.pptx");
    run(&[
        "--json",
        "pptx",
        "scaffold",
        scaffold.to_str().unwrap(),
        "--title",
        "Paragraph builder scaffold",
    ]);
    run(&[
        "--json",
        "pptx",
        "add-textbox",
        scaffold.to_str().unwrap(),
        "--slide",
        "1",
        "--text",
        "- First\n\t* Nested\nPlain",
        "--x",
        "1in",
        "--y",
        "1in",
        "--cx",
        "5in",
        "--cy",
        "2in",
        "--out",
        output.to_str().unwrap(),
    ]);
    let xml = zip_text(&output, "ppt/slides/slide1.xml");
    let body = tx_body_for_shape(&xml, 4);
    assert_eq!(body.matches("<a:p>").count(), 3);
    assert_eq!(body.matches("<a:buChar char=\"•\"/>").count(), 2);
    assert_eq!(body.matches("<a:buNone/>").count(), 1);
    let readback = run(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        output.to_str().unwrap(),
        "--slide",
        "1",
        "--target",
        "shape:4",
        "--include-text",
    ]);
    assert_eq!(readback["shapes"][0]["paragraphs"][0]["bullet"], true);
    assert_eq!(readback["shapes"][0]["paragraphs"][1]["level"], 1);
    assert_eq!(readback["shapes"][0]["paragraphs"][2]["bullet"], false);
    validate(&output);
}

#[test]
fn paragraphs_file_is_shared_by_layout_text_and_text_set() {
    let dir = temp_dir("shared-json");
    let paragraphs_file = dir.join("paragraphs.json");
    let from_layout = dir.join("from-layout.pptx");
    let from_text_set = dir.join("from-text-set.pptx");
    let from_text_set_repeat = dir.join("from-text-set-repeat.pptx");
    fs::write(
        &paragraphs_file,
        r#"[
          {"text":"Summary","bold":true,"size":22,"color":"123456","align":"center"},
          {"text":"","level":1,"bullet":true,"runs":[
            {"text":"Detail ","italic":true},
            {"text":"emphasis","bold":true}
          ]}
        ]"#,
    )
    .unwrap();
    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    run(&[
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "Title and Content",
        "--paragraphs-file",
        &format!("body={}", paragraphs_file.display()),
        "--out",
        from_layout.to_str().unwrap(),
    ]);
    run(&[
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--paragraphs-file",
        paragraphs_file.to_str().unwrap(),
        "--out",
        from_text_set.to_str().unwrap(),
    ]);
    run(&[
        "--json",
        "pptx",
        "text",
        "set",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--paragraphs-file",
        paragraphs_file.to_str().unwrap(),
        "--out",
        from_text_set_repeat.to_str().unwrap(),
    ]);
    let layout_body = tx_body_for_shape(&zip_text(&from_layout, "ppt/slides/slide5.xml"), 3);
    let set_body = tx_body_for_shape(&zip_text(&from_text_set, "ppt/slides/slide2.xml"), 3);
    assert_eq!(layout_body, set_body);
    assert!(layout_body.contains("algn=\"ctr\""));
    assert!(layout_body.contains("val=\"123456\""));
    assert!(layout_body.contains("i=\"1\""));
    assert!(layout_body.contains("b=\"1\""));
    assert_eq!(
        fs::read(&from_text_set).unwrap(),
        fs::read(&from_text_set_repeat).unwrap(),
        "identical paragraph mutations must be byte-deterministic"
    );
    validate(&from_layout);
    validate(&from_text_set);
}

#[test]
fn text_set_append_preserves_existing_paragraphs() {
    let dir = temp_dir("append");
    let output = dir.join("append.pptx");
    let result = run(&[
        "--json",
        "pptx",
        "text",
        "set",
        "testdata/pptx/title-content/presentation.pptx",
        "--slide",
        "2",
        "--target",
        "body",
        "--text",
        "- Appended one\n\t- Appended nested",
        "--append",
        "--out",
        output.to_str().unwrap(),
    ]);
    assert_eq!(result["mode"], "paragraph-content");
    assert_eq!(result["paragraphCount"], 2);
    assert_eq!(result["append"], true);
    let paragraphs = result["destination"]["paragraphs"].as_array().unwrap();
    assert!(
        paragraphs.len() >= 3,
        "original content plus two appended paragraphs"
    );
    assert_eq!(paragraphs[paragraphs.len() - 2]["text"], "Appended one");
    assert_eq!(paragraphs[paragraphs.len() - 1]["level"], 1);
    validate(&output);
}

#[test]
fn committed_render_fixture_preserves_three_plus_two_bullet_hierarchy() {
    let fixture = Path::new("testdata/pptx/bullets/presentation.pptx");
    let readback = run(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        fixture.to_str().unwrap(),
        "--slide",
        "5",
        "--target",
        "body",
        "--include-text",
    ]);
    let paragraphs = readback["shapes"][0]["paragraphs"].as_array().unwrap();
    assert_eq!(paragraphs.len(), 5);
    assert!(
        paragraphs[..3]
            .iter()
            .all(|paragraph| paragraph["bullet"] == true && paragraph["level"] == 0)
    );
    assert!(
        paragraphs[3..]
            .iter()
            .all(|paragraph| paragraph["bullet"] == true && paragraph["level"] == 1)
    );
    validate(fixture);
}
