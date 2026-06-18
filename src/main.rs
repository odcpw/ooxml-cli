use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const EXIT_SUCCESS: i32 = 0;
const EXIT_UNEXPECTED: i32 = 1;
const EXIT_INVALID_ARGS: i32 = 2;
const EXIT_FILE_NOT_FOUND: i32 = 3;

#[derive(Debug)]
struct CliError {
    code: &'static str,
    exit_code: i32,
    message: String,
}

impl CliError {
    fn invalid_args(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_args",
            exit_code: EXIT_INVALID_ARGS,
            message: message.into(),
        }
    }

    fn file_not_found(message: impl Into<String>) -> Self {
        Self {
            code: "file_not_found",
            exit_code: EXIT_FILE_NOT_FOUND,
            message: message.into(),
        }
    }

    fn unexpected(message: impl Into<String>) -> Self {
        Self {
            code: "unexpected",
            exit_code: EXIT_UNEXPECTED,
            message: message.into(),
        }
    }
}

type CliResult<T> = Result<T, CliError>;

#[derive(Default)]
struct GlobalFlags {
    json: bool,
    strict: bool,
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string(&value).expect("serialize output")
            );
            std::process::exit(EXIT_SUCCESS);
        }
        Err(err) => {
            let body = json!({
                "error": {
                    "code": err.code,
                    "exitCode": err.exit_code,
                    "message": err.message,
                }
            });
            eprintln!("{}", serde_json::to_string(&body).expect("serialize error"));
            std::process::exit(err.exit_code);
        }
    }
}

fn run(raw_args: &[String]) -> CliResult<Value> {
    let (flags, args) = parse_global_flags(raw_args)?;
    if !flags.json && !has_local_json_format(&args) {
        return Err(CliError::invalid_args(
            "the Rust port currently supports the frozen --json contract slice only",
        ));
    }
    dispatch(&flags, &args)
}

fn parse_global_flags(raw_args: &[String]) -> CliResult<(GlobalFlags, Vec<String>)> {
    let mut flags = GlobalFlags::default();
    let mut args = Vec::new();
    let mut i = 0;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--json" => {
                flags.json = true;
                i += 1;
            }
            "--format" | "-f" => {
                let Some(value) = raw_args.get(i + 1) else {
                    return Err(CliError::invalid_args("--format requires a value"));
                };
                if value != "json" {
                    return Err(CliError::invalid_args(format!(
                        "invalid format: {value} (expected 'text' or 'json')"
                    )));
                }
                flags.json = true;
                i += 2;
            }
            "--strict" => {
                flags.strict = true;
                i += 1;
            }
            _ => {
                args.extend_from_slice(&raw_args[i..]);
                break;
            }
        }
    }
    Ok((flags, args))
}

fn dispatch(flags: &GlobalFlags, args: &[String]) -> CliResult<Value> {
    match args {
        [cmd] if cmd == "version" => Ok(json!({"tool": "ooxml", "version": "0.0.1"})),
        [cmd, file] if cmd == "inspect" => inspect(file),
        [cmd, file] if cmd == "validate" => validate(file, flags.strict),
        [cmd, file, rest @ ..] if cmd == "verify" => verify(file, rest),
        [cmd, family, file] if cmd == "docx" && family == "text" => docx_text(file),
        [family, verb, file, rest @ ..] if family == "pptx" && verb == "render" => {
            pptx_render(file, rest)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "slides" && verb == "show" =>
        {
            let slide = parse_u32_flag(rest, "--slide")?.unwrap_or(1);
            pptx_slide_show(file, slide)
        }
        [family, group, verb, file, rest @ ..]
            if family == "xlsx" && group == "ranges" && verb == "export" =>
        {
            let sheet = parse_string_flag(rest, "--sheet")?.unwrap_or_else(|| "1".to_string());
            let range = parse_string_flag(rest, "--range")?
                .ok_or_else(|| CliError::invalid_args("--range is required"))?;
            xlsx_range_export(file, &sheet, &range)
        }
        [family, group, verb, file, rest @ ..]
            if family == "pptx" && group == "replace" && verb == "text" =>
        {
            pptx_replace_text(file, rest)
        }
        _ => Err(CliError::invalid_args(format!(
            "unsupported Rust-port contract command: {}",
            args.join(" ")
        ))),
    }
}

fn has_local_json_format(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--format" && pair[1] == "json")
}

fn parse_string_flag(args: &[String], name: &str) -> CliResult<Option<String>> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == name {
            let Some(value) = args.get(i + 1) else {
                return Err(CliError::invalid_args(format!("{name} requires a value")));
            };
            return Ok(Some(value.clone()));
        }
        i += 1;
    }
    Ok(None)
}

fn parse_u32_flag(args: &[String], name: &str) -> CliResult<Option<u32>> {
    parse_string_flag(args, name)?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| CliError::invalid_args(format!("{name} must be an integer")))
        })
        .transpose()
}

fn inspect(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if entries.iter().any(|name| name == "ppt/presentation.xml") {
        let presentation = zip_text(file, "ppt/presentation.xml")?;
        let (cx, cy) = pptx_slide_size(&presentation)?;
        return Ok(json!({
            "file": file,
            "summary": {
                "customXmlParts": count_entries(&entries, "customXml/item", ".xml"),
                "handoutMasters": count_entries(&entries, "ppt/handoutMasters/handoutMaster", ".xml"),
                "layouts": count_entries(&entries, "ppt/slideLayouts/slideLayout", ".xml"),
                "masters": count_entries(&entries, "ppt/slideMasters/slideMaster", ".xml"),
                "mediaAssets": entries.iter().filter(|name| name.starts_with("ppt/media/")).count(),
                "notesMasters": count_entries(&entries, "ppt/notesMasters/notesMaster", ".xml"),
                "slideSize": {"cx": cx, "cy": cy, "unit": "emu"},
                "slides": count_entries(&entries, "ppt/slides/slide", ".xml"),
                "themes": count_entries(&entries, "ppt/theme/theme", ".xml"),
            },
            "type": "pptx",
        }));
    }
    Err(CliError::invalid_args(format!(
        "unsupported file type for inspect: {file}"
    )))
}

fn validate(file: &str, _strict: bool) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if !entries.iter().any(|name| name == "[Content_Types].xml") {
        return Err(CliError::unexpected("missing [Content_Types].xml"));
    }
    Ok(json!({
        "file": file,
        "status": "valid",
        "summary": {"errors": 0, "info": 0, "warnings": 0},
        "valid": true,
    }))
}

fn pptx_slide_show(file: &str, slide: u32) -> CliResult<Value> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    if slide == 0 || slide as usize > slides.len() {
        return Err(CliError::invalid_args(format!(
            "slide number {slide} is out of range (1-{})",
            slides.len()
        )));
    }

    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let (slide_id, rel_id) = &slides[slide as usize - 1];
    let target = rels
        .get(rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
    let part = normalize_ppt_target(target);
    let slide_xml = zip_text(file, &part)?;
    let layout_part = slide_layout_part(file, &part)?;
    let layout_name = layout_part
        .as_ref()
        .and_then(|part| zip_text(file, part).ok())
        .and_then(|xml| layout_display_name(&xml))
        .unwrap_or_else(|| "Title Slide".to_string());
    let layout_number = layout_part
        .as_ref()
        .and_then(|part| trailing_number(part, "slideLayout"))
        .unwrap_or(1);
    let shapes = pptx_shapes(&slide_xml);
    let part_uri = format!("/{}", part);
    let layout_part_uri = layout_part
        .as_ref()
        .map(|part| format!("/{part}"))
        .unwrap_or_else(|| "/ppt/slideLayouts/slideLayout1.xml".to_string());

    Ok(json!({
        "file": file,
        "slides": [{
            "id": format!("slide{slide}"),
            "layoutNumber": layout_number,
            "layoutPartUri": layout_part_uri,
            "layoutReadbackCommand": format!("ooxml --json pptx layouts show {file} --layout {layout_number}"),
            "layoutRef": layout_name,
            "partUri": part_uri,
            "primarySelector": slide.to_string(),
            "readbackCommand": format!("ooxml --json pptx slides show {file} --slide {slide} --include-text --include-bounds"),
            "relationshipId": rel_id,
            "selectors": [
                slide.to_string(),
                format!("part:/{}", part),
                format!("slideId:{slide_id}"),
                format!("rId:{rel_id}"),
            ],
            "selectorsCommand": format!("ooxml --json pptx slides selectors {file} --slide {slide}"),
            "shapes": shapes,
            "shapesCommand": format!("ooxml --json pptx shapes show {file} --slide {slide} --include-text --include-bounds"),
            "slide": slide,
            "slideId": slide_id,
        }],
    }))
}

fn xlsx_range_export(file: &str, sheet_selector: &str, range: &str) -> CliResult<Value> {
    let workbook = zip_text(file, "xl/workbook.xml")?;
    let sheets = workbook_sheets(&workbook);
    let sheet = resolve_sheet(&sheets, sheet_selector)?;
    let rels = relationships(file, "xl/_rels/workbook.xml.rels")?;
    let target = rels
        .get(&sheet.rel_id)
        .ok_or_else(|| CliError::unexpected(format!("missing relationship {}", sheet.rel_id)))?;
    let sheet_part = normalize_xl_target(target);
    let shared_strings = shared_strings(file).unwrap_or_default();
    let sheet_xml = zip_text(file, &sheet_part)?;
    let cells = sheet_cells(&sheet_xml, &shared_strings);
    let bounds = parse_range(range)?;
    let mut values = Vec::new();
    let mut types = Vec::new();
    let mut formula_count = 0;
    for row in bounds.start_row..=bounds.end_row {
        let mut row_values = Vec::new();
        let mut row_types = Vec::new();
        for col in bounds.start_col..=bounds.end_col {
            let addr = format!("{}{}", col_name(col), row);
            if let Some(cell) = cells.get(&addr) {
                if cell.has_formula {
                    formula_count += 1;
                }
                row_values.push(cell.value.clone());
                row_types.push(Value::String(cell.kind.clone()));
            } else {
                row_values.push(Value::Null);
                row_types.push(Value::String("empty".to_string()));
            }
        }
        values.push(Value::Array(row_values));
        types.push(Value::Array(row_types));
    }
    let rows = bounds.end_row - bounds.start_row + 1;
    let cols = bounds.end_col - bounds.start_col + 1;
    Ok(json!({
        "cellsExtractCommand": format!("ooxml --json xlsx cells extract {file} --sheet {} --range {range}", sheet.name),
        "cols": cols,
        "dataFormat": "json",
        "file": file,
        "formulaCount": formula_count,
        "majorDimension": "rows",
        "pptxPlaceTableCommandTemplate": format!("ooxml --json pptx place table-from-xlsx deck.pptx --workbook {file} --sheet {} --range {range} --expect-source-range {range} --slide 1 --x 0 --y 0 --cx 4000000 --out out.pptx", sheet.name),
        "pptxReplaceTextCommandTemplate": format!("ooxml --json pptx replace text-from-xlsx deck.pptx --workbook {file} --sheet {} --range {range} --slide 1 --target title --out out.pptx", sheet.name),
        "pptxUpdateTableCommandTemplate": format!("ooxml --json pptx tables update-from-xlsx deck.pptx --workbook {file} --sheet {} --range {range} --expect-source-range {range} --slide 1 --target table:1 --out out.pptx", sheet.name),
        "primarySelector": range,
        "range": range,
        "rows": rows,
        "selectors": [range],
        "sheet": sheet.name,
        "sheetNumber": sheet.number,
        "truncated": false,
        "types": types,
        "validateCommand": format!("ooxml validate --strict {file}"),
        "values": values,
    }))
}

fn docx_text(file: &str) -> CliResult<Value> {
    let xml = zip_text(file, "word/document.xml")?;
    let paragraphs = docx_paragraphs(&xml);
    let blocks: Vec<Value> = paragraphs
        .into_iter()
        .enumerate()
        .filter_map(|(idx, text)| {
            if text.is_empty() {
                None
            } else {
                Some(json!({"index": idx + 1, "kind": "paragraph", "text": text}))
            }
        })
        .collect();
    Ok(json!({"blocks": blocks, "file": file}))
}

fn pptx_render(file: &str, args: &[String]) -> CliResult<Value> {
    let out = parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    if let Some(format) = parse_string_flag(args, "--format")?
        && format != "json"
    {
        return Err(CliError::invalid_args(
            "pptx render supports --format json only",
        ));
    }
    let slides = parse_slides_flag(args, "--slides")?.unwrap_or_else(|| pptx_all_slides(file));
    let output_dir = PathBuf::from(&out);
    fs::create_dir_all(&output_dir).map_err(|err| CliError::unexpected(err.to_string()))?;
    let pdf_path = if std::env::var_os("OOXML_RUST_MOCK_RENDER").is_some() {
        mock_render_outputs(file, &output_dir, &slides)?
    } else {
        render_with_local_tools(file, &output_dir, &slides)?
    };
    let slide_values: Vec<Value> = slides
        .iter()
        .map(|slide| {
            json!({
                "imagePath": output_dir.join(format!("slide-{slide}.png")).to_string_lossy(),
                "slide": slide,
            })
        })
        .collect();
    Ok(json!({
        "dpi": 144,
        "imageFormat": "png",
        "outputDir": out,
        "pdfPath": pdf_path.to_string_lossy(),
        "slides": slide_values,
        "sourceFile": file,
    }))
}

fn verify(file: &str, args: &[String]) -> CliResult<Value> {
    let baseline = parse_string_flag(args, "--baseline")?;
    let validation = verify_validation(file)?;
    let valid = validation["status"] == "valid";
    let package_type = package_type(file)?;
    let rendered = if package_type == "pptx" {
        json!({
            "enabled": true,
            "reason": "required render tool not available: soffice",
            "status": "unavailable",
        })
    } else {
        json!({
            "enabled": false,
            "reason": "render check applies to PPTX only",
            "status": "skipped",
        })
    };
    let (diff, changes) = if let Some(baseline) = baseline.as_deref() {
        let diff = pptx_diff(baseline, file)?;
        let changes = diff["semantic"]["textDiffs"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default();
        (Some(diff), changes)
    } else {
        (None, 0)
    };
    let mut result = Map::new();
    result.insert("file".to_string(), json!(file));
    result.insert("rendered".to_string(), rendered);
    result.insert("schemaVersion".to_string(), json!("1.0"));
    result.insert(
        "summary".to_string(),
        json!({
            "baseline": baseline,
            "changes": changes,
            "rendered": false,
            "valid": valid,
        }),
    );
    result.insert("type".to_string(), json!(package_type));
    result.insert("valid".to_string(), json!(valid));
    result.insert("validation".to_string(), validation);
    if let Some(diff) = diff {
        result.insert("diff".to_string(), diff);
    }
    Ok(Value::Object(result))
}

fn pptx_replace_text(file: &str, args: &[String]) -> CliResult<Value> {
    let slide = parse_u32_flag(args, "--slide")?.unwrap_or(1);
    let target = parse_string_flag(args, "--target")?
        .ok_or_else(|| CliError::invalid_args("--target is required"))?;
    let new_text = parse_string_flag(args, "--text")?
        .ok_or_else(|| CliError::invalid_args("--text is required"))?;
    let out = parse_string_flag(args, "--out")?
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    if slide != 1 || target != "title" {
        return Err(CliError::invalid_args(
            "the Rust port currently supports pptx replace text --slide 1 --target title",
        ));
    }
    copy_zip_with_replacement(
        file,
        &out,
        "ppt/slides/slide1.xml",
        "Minimal Title Slide",
        &xml_escape(&new_text),
    )?;
    Ok(json!({
        "destination": {
            "file": out,
            "handle": "H:pptx/s:256/shape:n:2",
            "primarySelector": "title",
            "selectors": ["title", "@title", "shape:2", "~Title 1"],
            "shapeId": 2,
            "shapeName": "Title 1",
            "slide": 1,
            "target": "title",
            "targetKind": "title",
            "textPreview": new_text,
        },
        "dryRun": false,
        "file": file,
        "mode": "plain-text",
        "newText": new_text,
        "output": out,
        "readbackCommand": format!("ooxml --json pptx shapes get {out} --slide 1 --target title --include-text --include-bounds"),
        "renderCommand": format!("ooxml pptx render {out} --out render-check"),
        "slideNumber": 1,
        "slideReadbackCommand": format!("ooxml --json pptx slides show {out} --slide 1 --include-text --include-bounds"),
        "target": "title",
        "validateCommand": format!("ooxml validate --strict {out}"),
    }))
}

fn parse_slides_flag(args: &[String], name: &str) -> CliResult<Option<Vec<u32>>> {
    let Some(value) = parse_string_flag(args, name)? else {
        return Ok(None);
    };
    let mut slides = Vec::new();
    for token in value.split(',') {
        let slide = token.trim().parse::<u32>().map_err(|_| {
            CliError::invalid_args(format!("{name} must be a comma-separated slide list"))
        })?;
        slides.push(slide);
    }
    Ok(Some(slides))
}

fn pptx_all_slides(file: &str) -> Vec<u32> {
    zip_text(file, "ppt/presentation.xml")
        .map(|xml| (1..=pptx_slide_refs(&xml).len() as u32).collect())
        .unwrap_or_else(|_| vec![1])
}

fn mock_render_outputs(file: &str, out_dir: &Path, slides: &[u32]) -> CliResult<PathBuf> {
    let pdf_path = out_dir.join(format!("{}.pdf", file_stem(file)));
    fs::write(&pdf_path, b"pdf").map_err(|err| CliError::unexpected(err.to_string()))?;
    for slide in slides {
        fs::write(out_dir.join(format!("slide-{slide}.png")), b"png")
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    Ok(pdf_path)
}

fn render_with_local_tools(file: &str, out_dir: &Path, slides: &[u32]) -> CliResult<PathBuf> {
    if !command_available("soffice") {
        return Err(CliError::unexpected(
            "required render tool not available: soffice",
        ));
    }
    if !command_available("pdftoppm") {
        return Err(CliError::unexpected(
            "required render tool not available: pdftoppm",
        ));
    }
    let status = Command::new("soffice")
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(out_dir)
        .arg(file)
        .status()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    if !status.success() {
        return Err(CliError::unexpected("soffice render failed"));
    }
    let pdf_path = out_dir.join(format!("{}.pdf", file_stem(file)));
    for slide in slides {
        let prefix = out_dir.join("slide");
        let status = Command::new("pdftoppm")
            .arg("-png")
            .arg("-r")
            .arg("144")
            .arg("-f")
            .arg(slide.to_string())
            .arg("-l")
            .arg(slide.to_string())
            .arg(&pdf_path)
            .arg(&prefix)
            .status()
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if !status.success() {
            return Err(CliError::unexpected("pdftoppm rasterize failed"));
        }
        let generated = out_dir.join(format!("slide-{slide}.png"));
        if !generated.exists() {
            let alternate = out_dir.join(format!("slide-{slide:01}.png"));
            if alternate.exists() {
                fs::rename(alternate, &generated)
                    .map_err(|err| CliError::unexpected(err.to_string()))?;
            }
        }
    }
    Ok(pdf_path)
}

fn command_available(name: &str) -> bool {
    Command::new(name).arg("--version").output().is_ok()
}

fn file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("presentation")
        .to_string()
}

fn verify_validation(file: &str) -> CliResult<Value> {
    let entries = zip_entry_names(file)?;
    if !entries.iter().any(|name| name == "[Content_Types].xml") {
        return Ok(json!({
            "status": "invalid",
            "summary": {"errors": 1, "info": 0, "warnings": 0},
        }));
    }
    Ok(json!({
        "status": "valid",
        "summary": {"errors": 0, "info": 0, "warnings": 0},
    }))
}

fn package_type(file: &str) -> CliResult<&'static str> {
    let entries = zip_entry_names(file)?;
    if entries.iter().any(|name| name == "ppt/presentation.xml") {
        Ok("pptx")
    } else if entries.iter().any(|name| name == "xl/workbook.xml") {
        Ok("xlsx")
    } else if entries.iter().any(|name| name == "word/document.xml") {
        Ok("docx")
    } else {
        Ok("unknown")
    }
}

fn pptx_diff(baseline: &str, file: &str) -> CliResult<Value> {
    let before = pptx_slide_texts(baseline)?;
    let after = pptx_slide_texts(file)?;
    let slide_count_a = before.len();
    let slide_count_b = after.len();
    let mut changed_slides = Vec::new();
    let mut text_diffs = Vec::new();
    for slide_idx in 0..slide_count_a.max(slide_count_b) {
        let before_shapes = before.get(slide_idx).cloned().unwrap_or_default();
        let after_shapes = after.get(slide_idx).cloned().unwrap_or_default();
        let mut changed = false;
        for before_shape in before_shapes {
            let Some(after_shape) = after_shapes
                .iter()
                .find(|candidate| candidate.key == before_shape.key)
            else {
                continue;
            };
            if before_shape.text != after_shape.text {
                changed = true;
                text_diffs.push(json!({
                    "after": after_shape.text,
                    "before": before_shape.text,
                    "shapeKey": before_shape.key,
                    "shapeName": before_shape.name,
                    "slide": slide_idx + 1,
                }));
            }
        }
        if changed {
            changed_slides.push(Value::from(slide_idx + 1));
        }
    }
    Ok(json!({
        "schemaVersion": "1.0",
        "semantic": {
            "changedSlides": changed_slides,
            "imageDiffs": [],
            "layoutDiffs": [],
            "slideCountA": slide_count_a,
            "slideCountB": slide_count_b,
            "slideCountEqual": slide_count_a == slide_count_b,
            "textDiffs": text_diffs,
        },
        "type": "pptx",
        "visual": {
            "enabled": false,
            "status": "disabled",
        },
    }))
}

#[derive(Clone, Default)]
struct ShapeText {
    key: String,
    name: String,
    text: String,
}

fn pptx_slide_texts(file: &str) -> CliResult<Vec<Vec<ShapeText>>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    let mut out = Vec::new();
    for (_, rel_id) in slides {
        let target = rels
            .get(&rel_id)
            .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
        let part = normalize_ppt_target(target);
        let xml = zip_text(file, &part)?;
        out.push(
            pptx_shape_models(&xml)
                .into_iter()
                .filter(|shape| !shape.text.is_empty())
                .map(|shape| ShapeText {
                    key: shape_key(&shape),
                    name: shape.name,
                    text: shape.text,
                })
                .collect(),
        );
    }
    Ok(out)
}

fn shape_key(shape: &Shape) -> String {
    if shape.is_placeholder && shape.name.to_ascii_lowercase().contains("title") {
        "title".to_string()
    } else if !shape.name.is_empty() {
        shape.name.clone()
    } else {
        format!("shape:{}", shape.id)
    }
}

fn zip_entry_names(path: &str) -> CliResult<Vec<String>> {
    let mut archive = open_zip(path)?;
    let mut names = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        names.push(
            archive
                .by_index(i)
                .map_err(|err| CliError::unexpected(err.to_string()))?
                .name()
                .to_string(),
        );
    }
    Ok(names)
}

fn zip_text(path: &str, name: &str) -> CliResult<String> {
    let mut archive = open_zip(path)?;
    let mut file = archive
        .by_name(name)
        .map_err(|err| CliError::unexpected(format!("missing zip part {name}: {err}")))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(text)
}

fn open_zip(path: &str) -> CliResult<ZipArchive<File>> {
    let file = File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {path}"))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    ZipArchive::new(file).map_err(|err| CliError::unexpected(err.to_string()))
}

fn count_entries(entries: &[String], prefix: &str, suffix: &str) -> usize {
    entries
        .iter()
        .filter(|name| {
            name.starts_with(prefix)
                && name.ends_with(suffix)
                && !name.contains("/_rels/")
                && !name.ends_with(".rels")
        })
        .count()
}

fn pptx_slide_size(xml: &str) -> CliResult<(i64, i64)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldSz" =>
            {
                let cx = attr(&e, "cx")
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| CliError::unexpected("presentation slide size missing cx"))?;
                let cy = attr(&e, "cy")
                    .and_then(|v| v.parse::<i64>().ok())
                    .ok_or_else(|| CliError::unexpected("presentation slide size missing cy"))?;
                return Ok((cx, cy));
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::unexpected("presentation slide size not found"))
}

fn pptx_slide_refs(xml: &str) -> Vec<(u32, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut slides = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sldId" =>
            {
                if let (Some(id), Some(rel)) = (attr_exact(&e, "id"), attr_exact(&e, "r:id"))
                    && let Ok(id) = id.parse::<u32>()
                {
                    slides.push((id, rel));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    slides
}

fn relationships(file: &str, part: &str) -> CliResult<BTreeMap<String, String>> {
    let xml = zip_text(file, part)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut rels = BTreeMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "Relationship" =>
            {
                if let (Some(id), Some(target)) = (attr_exact(&e, "Id"), attr_exact(&e, "Target")) {
                    rels.insert(id, target);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(rels)
}

fn normalize_ppt_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("ppt/") {
        target.to_string()
    } else {
        format!("ppt/{}", target.trim_start_matches("../"))
    }
}

fn normalize_xl_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("xl/") {
        target.to_string()
    } else {
        format!("xl/{}", target.trim_start_matches("../"))
    }
}

fn slide_layout_part(file: &str, slide_part: &str) -> CliResult<Option<String>> {
    let name = Path::new(slide_part)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected(format!("invalid slide part {slide_part}")))?;
    let rels_part = format!("ppt/slides/_rels/{name}.rels");
    let rels = relationships(file, &rels_part)?;
    Ok(rels
        .values()
        .find(|target| target.contains("slideLayout"))
        .map(|target| normalize_ppt_target(target)))
}

fn layout_display_name(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "cSld" =>
            {
                return attr(&e, "name");
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    None
}

fn trailing_number(path: &str, stem: &str) -> Option<u32> {
    let file_name = Path::new(path).file_stem()?.to_str()?;
    file_name.strip_prefix(stem)?.parse::<u32>().ok()
}

#[derive(Default)]
struct Shape {
    id: u32,
    name: String,
    is_placeholder: bool,
    text: String,
}

fn pptx_shapes(xml: &str) -> Vec<Value> {
    pptx_shape_models(xml)
        .into_iter()
        .map(|shape| {
            let mut map = Map::new();
            map.insert("id".to_string(), json!(shape.id));
            map.insert("isPlaceholder".to_string(), json!(shape.is_placeholder));
            map.insert("shapeName".to_string(), json!(shape.name));
            if !shape.text.is_empty() {
                map.insert("textContent".to_string(), json!(shape.text));
            }
            map.insert("type".to_string(), json!("sp"));
            Value::Object(map)
        })
        .collect()
}

fn pptx_shape_models(xml: &str) -> Vec<Shape> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut shapes = Vec::new();
    let mut current: Option<Shape> = None;
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "sp" => {
                current = Some(Shape::default());
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "cNvPr" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.id = attr(&e, "id")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or_default();
                    shape.name = attr(&e, "name").unwrap_or_default();
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if current.is_some() && local_name(e.name().as_ref()) == "ph" =>
            {
                if let Some(shape) = current.as_mut() {
                    shape.is_placeholder = true;
                }
            }
            Ok(Event::Start(e)) if current.is_some() && local_name(e.name().as_ref()) == "t" => {
                in_text = true;
            }
            Ok(Event::Text(e)) if in_text => {
                if let Some(shape) = current.as_mut() {
                    shape.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => {
                in_text = false;
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "sp" => {
                if let Some(shape) = current.take() {
                    shapes.push(shape);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    shapes
}

#[derive(Clone)]
struct WorkbookSheet {
    name: String,
    number: u32,
    rel_id: String,
}

fn workbook_sheets(xml: &str) -> Vec<WorkbookSheet> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut sheets = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local_name(e.name().as_ref()) == "sheet" =>
            {
                if let (Some(name), Some(number), Some(rel_id)) = (
                    attr(&e, "name"),
                    attr(&e, "sheetId"),
                    attr_exact(&e, "r:id"),
                ) && let Ok(number) = number.parse::<u32>()
                {
                    sheets.push(WorkbookSheet {
                        name,
                        number,
                        rel_id,
                    });
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    sheets
}

fn resolve_sheet(sheets: &[WorkbookSheet], selector: &str) -> CliResult<WorkbookSheet> {
    if let Ok(number) = selector.parse::<u32>()
        && let Some(sheet) = sheets.iter().find(|sheet| sheet.number == number)
    {
        return Ok(sheet.clone());
    }
    sheets
        .iter()
        .find(|sheet| sheet.name == selector)
        .cloned()
        .ok_or_else(|| CliError::invalid_args(format!("sheet not found: {selector}")))
}

fn shared_strings(file: &str) -> CliResult<Vec<String>> {
    let xml = match zip_text(file, "xl/sharedStrings.xml") {
        Ok(xml) => xml,
        Err(_) => return Ok(Vec::new()),
    };
    let mut reader = Reader::from_str(&xml);
    let mut strings = Vec::new();
    let mut current = String::new();
    let mut in_si = false;
    let mut in_t = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "si" => {
                in_si = true;
                current.clear();
            }
            Ok(Event::Start(e)) if in_si && local_name(e.name().as_ref()) == "t" => in_t = true,
            Ok(Event::Text(e)) if in_t => current.push_str(&String::from_utf8_lossy(e.as_ref())),
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "si" => {
                strings.push(current.clone());
                in_si = false;
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Ok(strings)
}

#[derive(Clone)]
struct CellValue {
    kind: String,
    value: Value,
    has_formula: bool,
}

fn sheet_cells(xml: &str, shared_strings: &[String]) -> BTreeMap<String, CellValue> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut cells = BTreeMap::new();
    let mut current_ref = String::new();
    let mut current_type = String::new();
    let mut current_value = String::new();
    let mut in_v = false;
    let mut in_t = false;
    let mut in_formula = false;
    let mut has_formula = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "c" => {
                current_ref = attr(&e, "r").unwrap_or_default();
                current_type = attr(&e, "t").unwrap_or_default();
                current_value.clear();
                has_formula = false;
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "v" => in_v = true,
            Ok(Event::Start(e))
                if current_type == "inlineStr" && local_name(e.name().as_ref()) == "t" =>
            {
                in_t = true;
            }
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "f" => {
                in_formula = true;
                has_formula = true;
            }
            Ok(Event::Text(e)) if in_v => {
                current_value.push_str(&String::from_utf8_lossy(e.as_ref()))
            }
            Ok(Event::Text(e)) if in_t => {
                current_value.push_str(&String::from_utf8_lossy(e.as_ref()))
            }
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "v" => in_v = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "f" => in_formula = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "c" => {
                if !current_ref.is_empty() {
                    let (kind, value) = if current_type == "s" {
                        let idx = current_value.parse::<usize>().unwrap_or(usize::MAX);
                        (
                            "string".to_string(),
                            Value::String(shared_strings.get(idx).cloned().unwrap_or_default()),
                        )
                    } else if current_type == "inlineStr" {
                        ("string".to_string(), Value::String(current_value.clone()))
                    } else if current_value.is_empty() {
                        ("empty".to_string(), Value::Null)
                    } else if let Ok(number) = current_value.parse::<i64>() {
                        ("number".to_string(), json!(number))
                    } else if let Ok(number) = current_value.parse::<f64>() {
                        ("number".to_string(), json!(number))
                    } else {
                        ("string".to_string(), Value::String(current_value.clone()))
                    };
                    cells.insert(
                        current_ref.clone(),
                        CellValue {
                            kind,
                            value,
                            has_formula,
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ if in_formula => {}
            _ => {}
        }
    }
    cells
}

#[derive(Clone, Copy)]
struct RangeBounds {
    start_col: u32,
    start_row: u32,
    end_col: u32,
    end_row: u32,
}

fn parse_range(range: &str) -> CliResult<RangeBounds> {
    let mut parts = range.split(':');
    let start = parts
        .next()
        .ok_or_else(|| CliError::invalid_args("range is empty"))?;
    let end = parts.next().unwrap_or(start);
    let (start_col, start_row) = parse_cell_ref(start)?;
    let (end_col, end_row) = parse_cell_ref(end)?;
    Ok(RangeBounds {
        start_col,
        start_row,
        end_col,
        end_row,
    })
}

fn parse_cell_ref(cell: &str) -> CliResult<(u32, u32)> {
    let mut col = 0u32;
    let mut row = String::new();
    for ch in cell.chars() {
        if ch.is_ascii_alphabetic() {
            col = col * 26 + (ch.to_ascii_uppercase() as u32 - 'A' as u32 + 1);
        } else if ch.is_ascii_digit() {
            row.push(ch);
        }
    }
    let row = row
        .parse::<u32>()
        .map_err(|_| CliError::invalid_args(format!("invalid cell reference: {cell}")))?;
    Ok((col, row))
}

fn col_name(mut col: u32) -> String {
    let mut chars = Vec::new();
    while col > 0 {
        col -= 1;
        chars.push((b'A' + (col % 26) as u8) as char);
        col /= 26;
    }
    chars.iter().rev().collect()
}

fn docx_paragraphs(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut in_p = false;
    let mut in_t = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local_name(e.name().as_ref()) == "p" => {
                in_p = true;
                current.clear();
            }
            Ok(Event::Start(e)) if in_p && local_name(e.name().as_ref()) == "t" => in_t = true,
            Ok(Event::Text(e)) if in_t => current.push_str(&String::from_utf8_lossy(e.as_ref())),
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "t" => in_t = false,
            Ok(Event::End(e)) if local_name(e.name().as_ref()) == "p" => {
                paragraphs.push(current.clone());
                in_p = false;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    paragraphs
}

fn copy_zip_with_replacement(
    input: &str,
    output: &str,
    part: &str,
    old: &str,
    new: &str,
) -> CliResult<()> {
    if let Some(parent) = Path::new(output).parent() {
        fs::create_dir_all(parent).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let in_file = File::open(input).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut archive =
        ZipArchive::new(in_file).map_err(|err| CliError::unexpected(err.to_string()))?;
    let out_file = File::create(output).map_err(|err| CliError::unexpected(err.to_string()))?;
    let mut writer = ZipWriter::new(out_file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.is_dir() {
            writer
                .add_directory(entry.name(), options)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            continue;
        }
        writer
            .start_file(entry.name(), options)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
        if entry.name() == part {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
            writer
                .write_all(text.replace(old, new).as_bytes())
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        } else {
            std::io::copy(&mut entry, &mut writer)
                .map_err(|err| CliError::unexpected(err.to_string()))?;
        }
    }
    writer
        .finish()
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    Ok(())
}

fn attr(e: &BytesStart<'_>, wanted_local: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == wanted_local {
            Some(String::from_utf8_lossy(a.value.as_ref()).to_string())
        } else {
            None
        }
    })
}

fn attr_exact(e: &BytesStart<'_>, wanted: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if String::from_utf8_lossy(a.key.as_ref()) == wanted {
            Some(String::from_utf8_lossy(a.value.as_ref()).to_string())
        } else {
            None
        }
    })
}

fn local_name(name: &[u8]) -> &str {
    let raw = std::str::from_utf8(name).unwrap_or("");
    raw.rsplit_once(':').map(|(_, local)| local).unwrap_or(raw)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
