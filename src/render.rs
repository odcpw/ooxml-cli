use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    CliError, CliResult, XlsxSheetsDeleteOptions, find_xlsx_workbook_part, package_type,
    pptx_all_slides, resolve_sheet, workbook_sheets, xlsx_sheets_delete, zip_entry_names, zip_text,
};

pub(crate) const LIBREOFFICE_REMEDIATION: &str =
    "Install LibreOffice and ensure soffice is on PATH.";
const PDFTOPPM_REMEDIATION: &str = "Install Poppler utilities and ensure pdftoppm is on PATH.";
const DEFAULT_DPI: u32 = 144;
const MAX_DPI: u32 = 1200;

#[derive(Clone, Debug)]
pub(crate) struct RenderedImage {
    pub(crate) number: u32,
    pub(crate) path: PathBuf,
}

#[derive(Clone, Debug)]
pub(crate) struct RenderedSet {
    pub(crate) family: &'static str,
    pub(crate) images: Vec<RenderedImage>,
}

#[derive(Default)]
struct RenderOptions {
    out: Option<String>,
    dpi: Option<u32>,
    pages: Option<Vec<u32>>,
    slides: Option<Vec<u32>>,
    sheet: Option<String>,
}

struct RenderRequest<'a> {
    file: &'a str,
    family: &'static str,
    output_dir: PathBuf,
    dpi: u32,
    selected_pages: Option<Vec<u32>>,
    selected_sheet: Option<SelectedSheet>,
}

#[derive(Clone)]
struct SelectedSheet {
    name: String,
    sheet_id: u32,
    position: u32,
    state: String,
}

struct CompletedRender {
    pdf_path: PathBuf,
    images: Vec<RenderedImage>,
    engine: &'static str,
    engine_path: Option<String>,
}

pub(crate) fn render_command(file: &str, args: &[String]) -> CliResult<Value> {
    render_command_for_family(file, args, None)
}

pub(crate) fn render_command_for_family(
    file: &str,
    args: &[String],
    required_family: Option<&str>,
) -> CliResult<Value> {
    let options = parse_render_options(args)?;
    let out = options
        .out
        .as_deref()
        .ok_or_else(|| CliError::invalid_args("--out is required"))?;
    let family = checked_family(file, required_family)?;
    let request = build_request(file, family, out, &options)?;
    fs::create_dir_all(&request.output_dir).map_err(|err| CliError::unexpected(err.to_string()))?;

    if env::var_os("OOXML_RUST_MOCK_RENDER").is_some() {
        let completed = render_mock(&request)?;
        return Ok(completed_manifest(&request, completed));
    }

    let Some(soffice) = find_command(&["soffice", "libreoffice"]) else {
        return Ok(skipped_manifest(
            &request,
            &["soffice"],
            LIBREOFFICE_REMEDIATION,
        ));
    };
    let Some(pdftoppm) = find_command(&["pdftoppm"]) else {
        return Ok(skipped_manifest(
            &request,
            &["pdftoppm"],
            PDFTOPPM_REMEDIATION,
        ));
    };

    let completed = render_with_local_tools(&request, &soffice, &pdftoppm)?;
    Ok(completed_manifest(&request, completed))
}

pub(crate) fn render_for_diff(file: &str, out_dir: &Path) -> CliResult<RenderedSet> {
    let out = out_dir.to_string_lossy().to_string();
    let args = vec![
        "--out".to_string(),
        out,
        "--format".to_string(),
        "json".to_string(),
    ];
    let value = render_command(file, &args)?;
    if value["status"] == "skipped" {
        let missing = value["missingTools"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_str)
            .unwrap_or("soffice");
        return Err(CliError::unexpected(format!(
            "required render tool not available: {missing}"
        )));
    }
    let family = package_type(file)?;
    let item_key = item_key(family);
    let items = value[item_key]
        .as_array()
        .ok_or_else(|| CliError::unexpected(format!("render manifest missing {item_key}")))?;
    let mut images = Vec::with_capacity(items.len());
    for item in items {
        let number = item[number_key(family)]
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| CliError::unexpected("render manifest item missing page number"))?;
        let path = item["imagePath"]
            .as_str()
            .ok_or_else(|| CliError::unexpected("render manifest item missing imagePath"))?;
        images.push(RenderedImage {
            number,
            path: PathBuf::from(path),
        });
    }
    Ok(RenderedSet { family, images })
}

fn checked_family(file: &str, required_family: Option<&str>) -> CliResult<&'static str> {
    let family = package_type(file)?;
    if !matches!(family, "pptx" | "xlsx" | "docx") {
        return Err(CliError::unsupported_type(format!(
            "render supports pptx, xlsx, and docx packages (detected: {family})"
        )));
    }
    if let Some(required) = required_family
        && family != required
    {
        return Err(CliError::unsupported_type(format!(
            "{required} render requires a {required} package (detected: {family})"
        )));
    }
    Ok(family)
}

fn build_request<'a>(
    file: &'a str,
    family: &'static str,
    out: &str,
    options: &RenderOptions,
) -> CliResult<RenderRequest<'a>> {
    if options.pages.is_some() && options.slides.is_some() {
        return Err(CliError::invalid_args(
            "specify only one of --pages or --slides",
        ));
    }
    if options.slides.is_some() && family != "pptx" {
        return Err(CliError::invalid_args(
            "--slides is supported only for pptx; use --pages",
        ));
    }
    if options.sheet.is_some() && family != "xlsx" {
        return Err(CliError::invalid_args("--sheet is supported only for xlsx"));
    }
    let selected_sheet = options
        .sheet
        .as_deref()
        .map(|selector| select_xlsx_sheet(file, selector))
        .transpose()?;
    Ok(RenderRequest {
        file,
        family,
        output_dir: PathBuf::from(out),
        dpi: options.dpi.unwrap_or(DEFAULT_DPI),
        selected_pages: options.pages.clone().or_else(|| options.slides.clone()),
        selected_sheet,
    })
}

fn parse_render_options(args: &[String]) -> CliResult<RenderOptions> {
    let mut options = RenderOptions::default();
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--out" | "--dpi" | "--pages" | "--slides" | "--sheet" | "--format" | "-f" => {
                let Some(value) = args.get(index + 1) else {
                    return Err(CliError::invalid_args(format!("{arg} requires a value")));
                };
                apply_render_value(&mut options, arg, value)?;
                index += 2;
            }
            "--json" | "--thumbnails" => index += 1,
            _ if arg.starts_with("--out=") => {
                options.out = Some(arg["--out=".len()..].to_string());
                index += 1;
            }
            _ if arg.starts_with("--dpi=") => {
                options.dpi = Some(parse_dpi(&arg["--dpi=".len()..])?);
                index += 1;
            }
            _ if arg.starts_with("--pages=") => {
                options.pages = Some(parse_page_selection("--pages", &arg["--pages=".len()..])?);
                index += 1;
            }
            _ if arg.starts_with("--slides=") => {
                options.slides = Some(parse_page_selection("--slides", &arg["--slides=".len()..])?);
                index += 1;
            }
            _ if arg.starts_with("--sheet=") => {
                options.sheet = Some(arg["--sheet=".len()..].to_string());
                index += 1;
            }
            _ if arg.starts_with("--format=") => {
                validate_format(&arg["--format=".len()..])?;
                index += 1;
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::invalid_args(format!("unknown flag: {arg}")));
            }
            _ => {
                return Err(CliError::invalid_args(
                    "render accepts exactly one file argument",
                ));
            }
        }
    }
    Ok(options)
}

fn apply_render_value(options: &mut RenderOptions, flag: &str, value: &str) -> CliResult<()> {
    match flag {
        "--out" => options.out = Some(value.to_string()),
        "--dpi" => options.dpi = Some(parse_dpi(value)?),
        "--pages" => options.pages = Some(parse_page_selection(flag, value)?),
        "--slides" => options.slides = Some(parse_page_selection(flag, value)?),
        "--sheet" => options.sheet = Some(value.to_string()),
        "--format" | "-f" => validate_format(value)?,
        _ => unreachable!("render parser passed an unknown value flag"),
    }
    Ok(())
}

fn validate_format(value: &str) -> CliResult<()> {
    if value == "json" {
        Ok(())
    } else {
        Err(CliError::invalid_args("render supports --format json only"))
    }
}

fn parse_dpi(value: &str) -> CliResult<u32> {
    let dpi = value.parse::<u32>().map_err(|_| {
        CliError::invalid_args(format!("--dpi must be an integer from 1 to {MAX_DPI}"))
    })?;
    if dpi == 0 || dpi > MAX_DPI {
        return Err(CliError::invalid_args(format!(
            "--dpi must be an integer from 1 to {MAX_DPI}"
        )));
    }
    Ok(dpi)
}

fn parse_page_selection(flag: &str, value: &str) -> CliResult<Vec<u32>> {
    let mut pages = Vec::new();
    for token in value.split(',') {
        let token = token.trim();
        if token.is_empty() {
            return Err(page_selection_error(flag));
        }
        if let Some((start, end)) = token.split_once('-') {
            let start = parse_positive_page(flag, start)?;
            let end = parse_positive_page(flag, end)?;
            if start > end {
                return Err(page_selection_error(flag));
            }
            pages.extend(start..=end);
        } else {
            pages.push(parse_positive_page(flag, token)?);
        }
    }
    pages.sort_unstable();
    pages.dedup();
    if pages.is_empty() {
        return Err(page_selection_error(flag));
    }
    Ok(pages)
}

fn parse_positive_page(flag: &str, value: &str) -> CliResult<u32> {
    value
        .parse::<u32>()
        .ok()
        .filter(|page| *page > 0)
        .ok_or_else(|| page_selection_error(flag))
}

fn page_selection_error(flag: &str) -> CliError {
    CliError::invalid_args(format!(
        "{flag} must be a comma-separated list of positive pages or ranges (for example 1-3,5)"
    ))
}

fn select_xlsx_sheet(file: &str, selector: &str) -> CliResult<SelectedSheet> {
    let entries = zip_entry_names(file)?;
    let workbook_part = find_xlsx_workbook_part(file, &entries)?;
    let workbook = zip_text(file, &workbook_part)?;
    let sheets = workbook_sheets(&workbook)?;
    let selected = resolve_sheet(&sheets, selector)?;
    Ok(SelectedSheet {
        name: selected.name,
        sheet_id: selected.sheet_id,
        position: selected.position,
        state: selected.state,
    })
}

fn render_mock(request: &RenderRequest<'_>) -> CliResult<CompletedRender> {
    let pdf_path = request
        .output_dir
        .join(format!("{}.pdf", file_stem(request.file)));
    let source = fs::read(request.file).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            CliError::file_not_found(format!("file not found: {}", request.file))
        } else {
            CliError::unexpected(err.to_string())
        }
    })?;
    let source_hash = Sha256::digest(&source);
    fs::write(&pdf_path, format!("mock-pdf-v1:{source_hash:x}"))
        .map_err(|err| CliError::unexpected(err.to_string()))?;
    let pages = request.selected_pages.clone().unwrap_or_else(|| {
        if request.family == "pptx" {
            pptx_all_slides(request.file)
        } else {
            vec![1]
        }
    });
    let mut images = Vec::with_capacity(pages.len());
    for number in pages {
        let path = request.output_dir.join(image_name(request.family, number));
        fs::write(
            &path,
            format!(
                "mock-image-v1:{}:{number}:{}:{source_hash:x}",
                request.family, request.dpi
            ),
        )
        .map_err(|err| CliError::unexpected(err.to_string()))?;
        images.push(RenderedImage { number, path });
    }
    Ok(CompletedRender {
        pdf_path,
        images,
        engine: "mock",
        engine_path: None,
    })
}

fn render_with_local_tools(
    request: &RenderRequest<'_>,
    soffice: &str,
    pdftoppm: &str,
) -> CliResult<CompletedRender> {
    let prepared = prepare_input(request)?;
    let profile = LibreOfficeProfile::new()?;
    let converted_pdf = request
        .output_dir
        .join(format!("{}.pdf", file_stem_path(&prepared.path)));
    if converted_pdf.exists() {
        fs::remove_file(&converted_pdf).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let output = Command::new(soffice)
        .arg(format!("-env:UserInstallation={}", profile.url))
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(&request.output_dir)
        .arg(&prepared.path)
        .output()
        .map_err(|err| CliError::unexpected(format!("{soffice} failed: {err}")))?;
    if !output.status.success() || !converted_pdf.exists() {
        return Err(command_failure(soffice, "render", &output));
    }

    let pdf_path = request
        .output_dir
        .join(format!("{}.pdf", file_stem(request.file)));
    if converted_pdf != pdf_path {
        if pdf_path.exists() {
            fs::remove_file(&pdf_path).map_err(|err| CliError::unexpected(err.to_string()))?;
        }
        fs::rename(&converted_pdf, &pdf_path)
            .map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    let images = rasterize_pdf(
        &pdf_path,
        &request.output_dir,
        request.family,
        request.dpi,
        request.selected_pages.as_deref(),
        pdftoppm,
    )?;
    Ok(CompletedRender {
        pdf_path,
        images,
        engine: "libreoffice",
        engine_path: Some(soffice.to_string()),
    })
}

struct LibreOfficeProfile {
    path: PathBuf,
    url: String,
}

impl LibreOfficeProfile {
    fn new() -> CliResult<Self> {
        let path = env::temp_dir().join(format!(
            "ooxml-soffice-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&path).map_err(|err| CliError::unexpected(err.to_string()))?;
        let url_path = path.to_string_lossy().replace('\\', "/");
        let url = if url_path.starts_with('/') {
            format!("file://{url_path}")
        } else {
            format!("file:///{url_path}")
        };
        Ok(Self { path, url })
    }
}

impl Drop for LibreOfficeProfile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct PreparedInput {
    path: PathBuf,
    temporary_paths: Vec<PathBuf>,
}

impl Drop for PreparedInput {
    fn drop(&mut self) {
        for path in &self.temporary_paths {
            let _ = fs::remove_file(path);
        }
    }
}

fn prepare_input(request: &RenderRequest<'_>) -> CliResult<PreparedInput> {
    let Some(selected) = request.selected_sheet.as_ref() else {
        return Ok(PreparedInput {
            path: PathBuf::from(request.file),
            temporary_paths: Vec::new(),
        });
    };
    let entries = zip_entry_names(request.file)?;
    let workbook_part = find_xlsx_workbook_part(request.file, &entries)?;
    let workbook = zip_text(request.file, &workbook_part)?;
    let sheets = workbook_sheets(&workbook)?;
    if selected.state != "visible" {
        return Err(CliError::invalid_args(format!(
            "cannot render hidden worksheet: {}",
            selected.name
        )));
    }
    if sheets.len() == 1 && sheets[0].name == selected.name {
        return Ok(PreparedInput {
            path: PathBuf::from(request.file),
            temporary_paths: Vec::new(),
        });
    }
    let mut prepared = PreparedInput {
        path: PathBuf::from(request.file),
        temporary_paths: Vec::with_capacity(sheets.len().saturating_sub(1)),
    };
    for (index, sheet) in sheets
        .iter()
        .filter(|sheet| sheet.name != selected.name)
        .enumerate()
    {
        let next_path = request.output_dir.join(format!(
            ".ooxml-render-sheet-{}-{}-{index}.xlsx",
            std::process::id(),
            unique_suffix()
        ));
        let current = prepared.path.to_string_lossy().to_string();
        let next = next_path
            .to_str()
            .ok_or_else(|| CliError::unexpected("temporary render path is not valid UTF-8"))?;
        xlsx_sheets_delete(
            &current,
            XlsxSheetsDeleteOptions {
                sheet: Some(&sheet.name),
                out: Some(next),
                backup: None,
                dry_run: false,
                no_validate: false,
                in_place: false,
            },
        )?;
        prepared.temporary_paths.push(next_path.clone());
        prepared.path = next_path;
    }
    Ok(prepared)
}

fn rasterize_pdf(
    pdf_path: &Path,
    out_dir: &Path,
    family: &str,
    dpi: u32,
    selected_pages: Option<&[u32]>,
    pdftoppm: &str,
) -> CliResult<Vec<RenderedImage>> {
    let prefix = out_dir.join(format!(
        ".ooxml-raster-{}-{}",
        std::process::id(),
        unique_suffix()
    ));
    if let Some(pages) = selected_pages {
        let mut images = Vec::with_capacity(pages.len());
        for page in pages {
            let page_prefix = PathBuf::from(format!("{}-{page}", prefix.to_string_lossy()));
            let output = Command::new(pdftoppm)
                .args(["-png", "-singlefile", "-r"])
                .arg(dpi.to_string())
                .arg("-f")
                .arg(page.to_string())
                .arg("-l")
                .arg(page.to_string())
                .arg(pdf_path)
                .arg(&page_prefix)
                .output()
                .map_err(|err| CliError::unexpected(format!("{pdftoppm} failed: {err}")))?;
            if !output.status.success() {
                return Err(command_failure(pdftoppm, "rasterize", &output));
            }
            let Some(generated) = find_single_raster(&page_prefix, *page) else {
                return Err(CliError::unexpected(format!(
                    "{pdftoppm} rasterize failed: page {page} did not produce a PNG"
                )));
            };
            let final_path = out_dir.join(image_name(family, *page));
            move_rendered_image(&generated, &final_path)?;
            images.push(RenderedImage {
                number: *page,
                path: final_path,
            });
        }
        return Ok(images);
    }

    let output = Command::new(pdftoppm)
        .args(["-png", "-r"])
        .arg(dpi.to_string())
        .arg(pdf_path)
        .arg(&prefix)
        .output()
        .map_err(|err| CliError::unexpected(format!("{pdftoppm} failed: {err}")))?;
    if !output.status.success() {
        return Err(command_failure(pdftoppm, "rasterize", &output));
    }
    let mut generated = collect_rasters(&prefix)?;
    if generated.is_empty() {
        return Err(CliError::unexpected(format!(
            "{pdftoppm} rasterize failed: no PNG pages were produced"
        )));
    }
    generated.sort_by_key(|(page, _)| *page);
    let mut images = Vec::with_capacity(generated.len());
    for (page, generated_path) in generated {
        let final_path = out_dir.join(image_name(family, page));
        move_rendered_image(&generated_path, &final_path)?;
        images.push(RenderedImage {
            number: page,
            path: final_path,
        });
    }
    Ok(images)
}

fn find_single_raster(prefix: &Path, page: u32) -> Option<PathBuf> {
    let exact = PathBuf::from(format!("{}.png", prefix.to_string_lossy()));
    if exact.exists() {
        return Some(exact);
    }
    let numbered = PathBuf::from(format!("{}-{page}.png", prefix.to_string_lossy()));
    if numbered.exists() {
        return Some(numbered);
    }
    collect_rasters(prefix)
        .ok()
        .and_then(|mut items| items.drain(..).next().map(|(_, path)| path))
}

fn collect_rasters(prefix: &Path) -> CliResult<Vec<(u32, PathBuf)>> {
    let directory = prefix.parent().unwrap_or_else(|| Path::new("."));
    let prefix_name = prefix
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CliError::unexpected("raster prefix is not valid UTF-8"))?;
    let mut images = Vec::new();
    for entry in fs::read_dir(directory).map_err(|err| CliError::unexpected(err.to_string()))? {
        let entry = entry.map_err(|err| CliError::unexpected(err.to_string()))?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some(suffix) = name
            .strip_prefix(prefix_name)
            .and_then(|rest| rest.strip_prefix('-'))
            .and_then(|rest| rest.strip_suffix(".png"))
        else {
            continue;
        };
        if let Ok(page) = suffix.parse::<u32>() {
            images.push((page, entry.path()));
        }
    }
    Ok(images)
}

fn move_rendered_image(source: &Path, destination: &Path) -> CliResult<()> {
    if destination.exists() {
        fs::remove_file(destination).map_err(|err| CliError::unexpected(err.to_string()))?;
    }
    fs::rename(source, destination).map_err(|err| CliError::unexpected(err.to_string()))
}

fn completed_manifest(request: &RenderRequest<'_>, completed: CompletedRender) -> Value {
    let mut result = common_manifest(request, "ok");
    result.insert(
        "pdfPath".to_string(),
        json!(completed.pdf_path.to_string_lossy().to_string()),
    );
    result.insert("engine".to_string(), json!(completed.engine));
    if let Some(path) = completed.engine_path {
        result.insert("enginePath".to_string(), json!(path));
    }
    result.insert(
        item_key(request.family).to_string(),
        Value::Array(
            completed
                .images
                .into_iter()
                .map(|image| {
                    let mut item = Map::new();
                    item.insert(number_key(request.family).to_string(), json!(image.number));
                    item.insert(
                        "imagePath".to_string(),
                        json!(image.path.to_string_lossy().to_string()),
                    );
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    Value::Object(result)
}

fn skipped_manifest(request: &RenderRequest<'_>, missing: &[&str], remediation: &str) -> Value {
    let mut result = common_manifest(request, "skipped");
    result.insert("pdfPath".to_string(), Value::Null);
    result.insert("engine".to_string(), json!("libreoffice"));
    result.insert(item_key(request.family).to_string(), json!([]));
    result.insert("missingTools".to_string(), json!(missing));
    result.insert("remediation".to_string(), json!(remediation));
    result.insert(
        "doctorCommand".to_string(),
        json!("ooxml --json doctor --only render-engine,fonts"),
    );
    Value::Object(result)
}

fn common_manifest(request: &RenderRequest<'_>, status: &str) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert("schemaVersion".to_string(), json!("1.0"));
    result.insert("type".to_string(), json!(request.family));
    result.insert("status".to_string(), json!(status));
    result.insert("sourceFile".to_string(), json!(request.file));
    result.insert(
        "outputDir".to_string(),
        json!(request.output_dir.to_string_lossy().to_string()),
    );
    result.insert("dpi".to_string(), json!(request.dpi));
    result.insert("imageFormat".to_string(), json!("png"));
    result.insert(
        "doctorChecks".to_string(),
        json!(["render-engine", "fonts"]),
    );
    result.insert(
        "limitations".to_string(),
        json!(limitations(request.family)),
    );
    if let Some(sheet) = request.selected_sheet.as_ref() {
        result.insert(
            "sheet".to_string(),
            json!({
                "name": sheet.name,
                "position": sheet.position,
                "sheetId": sheet.sheet_id,
            }),
        );
    }
    result
}

fn limitations(family: &str) -> Vec<&'static str> {
    let mut limitations = vec![
        "LibreOffice rendering can substitute unavailable fonts and may differ from Microsoft Office layout.",
    ];
    match family {
        "pptx" => limitations.push(
            "Static pages do not represent animations, transitions, audio, or video playback.",
        ),
        "xlsx" => limitations.push(
            "Pagination follows LibreOffice Calc print areas, scaling, and page-break behavior.",
        ),
        "docx" => limitations
            .push("Pagination can differ from Microsoft Word when fonts or layout engines differ."),
        _ => {}
    }
    limitations
}

fn find_command(candidates: &[&str]) -> Option<String> {
    for candidate in candidates {
        if candidate.contains(std::path::MAIN_SEPARATOR) {
            if Path::new(candidate).is_file() {
                return Some((*candidate).to_string());
            }
            continue;
        }
        let Some(path) = env::var_os("PATH") else {
            continue;
        };
        for directory in env::split_paths(&path) {
            let executable = directory.join(candidate);
            if executable.is_file() {
                return Some(executable.to_string_lossy().to_string());
            }
            #[cfg(windows)]
            {
                let executable = directory.join(format!("{candidate}.exe"));
                if executable.is_file() {
                    return Some(executable.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

fn command_failure(program: &str, stage: &str, output: &std::process::Output) -> CliError {
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if detail.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        detail
    };
    if detail.is_empty() {
        CliError::unexpected(format!("{program} {stage} failed: {}", output.status))
    } else {
        CliError::unexpected(format!("{program} {stage} failed: {detail}"))
    }
}

fn item_key(family: &str) -> &'static str {
    if family == "pptx" { "slides" } else { "pages" }
}

fn number_key(family: &str) -> &'static str {
    if family == "pptx" { "slide" } else { "page" }
}

fn image_name(family: &str, number: u32) -> String {
    format!("{}-{number}.png", number_key(family))
}

fn file_stem(path: &str) -> String {
    file_stem_path(Path::new(path))
}

fn file_stem_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("document")
        .to_string()
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_selection_accepts_ranges_and_deduplicates() {
        assert_eq!(
            parse_page_selection("--pages", "3,1-2,2").expect("selection"),
            vec![1, 2, 3]
        );
    }
}
