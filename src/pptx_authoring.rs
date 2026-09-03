mod package;

use quick_xml::Reader;
use quick_xml::events::Event;
use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

use self::package::{
    PRESENTATION_PART, SLIDE_LAYOUT_PART, SLIDE_MASTER_PART, SLIDE_PART, SlideSize,
    TABLE_STYLES_PART, THEME_PART, ThemeChoice, layout_names, write_package,
};
use crate::pptx_mutation::{pptx_masters_import, pptx_new_slide_from_layout, pptx_slides_delete};
use crate::{
    CliError, CliResult, attr, command_arg, local_name, package_mutation_temp_path, package_type,
    zip_text,
};

pub(crate) struct PptxScaffoldOptions<'a> {
    pub(crate) title: Option<&'a str>,
    pub(crate) subtitle: Option<&'a str>,
    pub(crate) theme: Option<&'a str>,
    pub(crate) theme_seed: Option<&'a str>,
    pub(crate) brand: Option<&'a str>,
    pub(crate) template: Option<&'a str>,
    pub(crate) size: Option<&'a str>,
    pub(crate) force: bool,
    pub(crate) no_validate: bool,
}

struct ScaffoldPackageInfo {
    layout_names: Vec<String>,
    slide_size: SlideSize,
    slide_part: String,
    slide_id: String,
    slide_layout_part: String,
    slide_master_part: String,
    theme_part: String,
    theme_name: Option<String>,
    theme_seed: Option<String>,
    template: Option<String>,
}

struct TemplateStages<'a> {
    base: &'a str,
    imported: &'a str,
    with_slide: &'a str,
}

pub(crate) fn pptx_scaffold(output: &str, options: PptxScaffoldOptions<'_>) -> CliResult<Value> {
    validate_output(output, options.force)?;
    if options.brand.is_some() && (options.theme.is_some() || options.theme_seed.is_some()) {
        return Err(CliError::invalid_args(
            "--brand cannot be combined with --theme or --theme-seed",
        ));
    }
    if options.template.is_some()
        && (options.theme.is_some() || options.theme_seed.is_some() || options.size.is_some())
    {
        return Err(CliError::invalid_args(
            "--template cannot be combined with --theme, --theme-seed, or --size; the template master, theme, and slide size are inherited",
        ));
    }

    let title = options.title.unwrap_or("Title Slide");
    let subtitle = options.subtitle.unwrap_or("");
    let temp_path = package_mutation_temp_path(output, "pptx-scaffold");
    let package = if let Some(template) = options.template {
        write_template_package(template, &temp_path, title, subtitle)?
    } else {
        let size = SlideSize::parse(options.size)?;
        let theme = ThemeChoice::resolve(options.theme, options.theme_seed)?;
        write_package(&temp_path, title, subtitle, &size, &theme)?;
        ScaffoldPackageInfo {
            layout_names: layout_names()?,
            slide_size: size,
            slide_part: SLIDE_PART.to_string(),
            slide_id: "256".to_string(),
            slide_layout_part: SLIDE_LAYOUT_PART.to_string(),
            slide_master_part: SLIDE_MASTER_PART.to_string(),
            theme_part: THEME_PART.to_string(),
            theme_name: Some(theme.name),
            theme_seed: theme.seed,
            template: None,
        }
    };

    if let Some(brand) = options.brand {
        crate::brand::apply_to_staged_package(&temp_path, brand)?;
    }

    if !options.no_validate {
        crate::validate_owned_mutation_output(&temp_path)?;
    }
    crate::finish_mutation_output(output, &temp_path, Some(output), false, None, false)?;

    Ok(pptx_scaffold_result(
        output,
        title,
        subtitle,
        !options.no_validate,
        &package,
    ))
}

fn validate_output(output: &str, force: bool) -> CliResult<()> {
    if output.trim().is_empty() {
        return Err(CliError::invalid_args("output path is required"));
    }
    let output_path = Path::new(output);
    if output_path.is_dir() {
        return Err(CliError::invalid_args("output path is a directory"));
    }
    if output_path.exists() && !force {
        return Err(CliError::invalid_args(
            "output file already exists; pass --force to replace it",
        ));
    }
    Ok(())
}

fn write_template_package(
    template: &str,
    output: &str,
    title: &str,
    subtitle: &str,
) -> CliResult<ScaffoldPackageInfo> {
    if !Path::new(template).is_file() {
        return Err(CliError::invalid_args(format!(
            "PPTX template does not exist or is not a file: {template}"
        )));
    }
    if package_type(template)? != "pptx" {
        return Err(CliError::invalid_args(format!(
            "--template must name a PPTX package: {template}"
        )));
    }
    let size = template_slide_size(template)?;
    let theme = ThemeChoice::resolve(Some("neutral"), None)?;
    let base = package_mutation_temp_path(output, "pptx-template-base");
    let imported = package_mutation_temp_path(output, "pptx-template-master");
    let with_slide = package_mutation_temp_path(output, "pptx-template-slide");
    let result = assemble_template_package(
        template,
        output,
        title,
        subtitle,
        size,
        &theme,
        TemplateStages {
            base: &base,
            imported: &imported,
            with_slide: &with_slide,
        },
    );
    for path in [&base, &imported, &with_slide] {
        let _ = fs::remove_file(path);
    }
    result
}

fn assemble_template_package(
    template: &str,
    output: &str,
    title: &str,
    subtitle: &str,
    size: SlideSize,
    theme: &ThemeChoice,
    stages: TemplateStages<'_>,
) -> CliResult<ScaffoldPackageInfo> {
    write_package(stages.base, "", "", &size, theme)?;
    let import_args = vec![
        "--source".to_string(),
        template.to_string(),
        "--master".to_string(),
        "1".to_string(),
        "--theme-policy".to_string(),
        "import".to_string(),
        "--out".to_string(),
        stages.imported.to_string(),
        "--no-validate".to_string(),
    ];
    let import = pptx_masters_import(stages.base, &import_args)?;
    let target_master = import
        .get("targetMaster")
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;
    let target_master_id = format!("master-{target_master}");
    let all_layouts = crate::pptx_readback::pptx_presentation_layouts(stages.imported)?;
    let (layout_index, layout) = select_template_title_layout(&all_layouts, &target_master_id)?;
    let has_subtitle = layout
        .placeholders
        .iter()
        .any(|placeholder| placeholder.get("role").and_then(Value::as_str) == Some("subtitle"));
    if !subtitle.is_empty() && !has_subtitle {
        return Err(CliError::invalid_args(
            "the imported template master has no title layout with a subtitle placeholder",
        ));
    }
    let mut slide_args = vec![
        "--layout".to_string(),
        (layout_index + 1).to_string(),
        "--insert-after".to_string(),
        "1".to_string(),
        "--set-text".to_string(),
        format!("title={title}"),
    ];
    if has_subtitle {
        slide_args.push("--set-text".to_string());
        slide_args.push(format!("subtitle={subtitle}"));
    }
    slide_args.extend([
        "--out".to_string(),
        stages.with_slide.to_string(),
        "--no-validate".to_string(),
    ]);
    pptx_new_slide_from_layout(stages.imported, &slide_args)?;
    pptx_slides_delete(
        stages.with_slide,
        1,
        &[
            "--out".to_string(),
            output.to_string(),
            "--no-validate".to_string(),
        ],
    )?;

    let final_layouts = crate::pptx_readback::pptx_presentation_layouts(output)?;
    let active_layout = final_layouts
        .iter()
        .find(|candidate| candidate.part_uri == layout.part_uri)
        .unwrap_or(layout);
    Ok(ScaffoldPackageInfo {
        layout_names: final_layouts
            .iter()
            .map(|layout| layout.name.clone())
            .collect(),
        slide_size: size,
        slide_part: "ppt/slides/slide2.xml".to_string(),
        slide_id: "257".to_string(),
        slide_layout_part: active_layout.part_uri.trim_start_matches('/').to_string(),
        slide_master_part: import
            .get("targetMasterUri")
            .and_then(Value::as_str)
            .unwrap_or("/ppt/slideMasters/slideMaster1.xml")
            .trim_start_matches('/')
            .to_string(),
        theme_part: active_layout.theme_uri.trim_start_matches('/').to_string(),
        theme_name: None,
        theme_seed: None,
        template: Some(template.to_string()),
    })
}

fn select_template_title_layout<'a>(
    layouts: &'a [crate::pptx_readback::PptxLayoutInfo],
    master_id: &str,
) -> CliResult<(usize, &'a crate::pptx_readback::PptxLayoutInfo)> {
    let candidates = layouts
        .iter()
        .enumerate()
        .filter(|(_, layout)| layout.master_id == master_id)
        .collect::<Vec<_>>();
    candidates
        .iter()
        .copied()
        .find(|(_, layout)| layout.name.eq_ignore_ascii_case("Title Slide"))
        .or_else(|| {
            candidates.iter().copied().find(|(_, layout)| {
                layout.placeholders.iter().any(|placeholder| {
                    placeholder.get("role").and_then(Value::as_str) == Some("title")
                })
            })
        })
        .ok_or_else(|| {
            CliError::invalid_args("the imported template master has no title-capable layout")
        })
}

fn template_slide_size(template: &str) -> CliResult<SlideSize> {
    let presentation = zip_text(template, PRESENTATION_PART)?;
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) | Ok(Event::Empty(element))
                if local_name(element.name().as_ref()) == "sldSz" =>
            {
                let width = attr(&element, "cx")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(0);
                let height = attr(&element, "cy")
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(0);
                let preset = attr(&element, "type").unwrap_or_else(|| "screen16x9".to_string());
                return SlideSize::imported(width, height, preset);
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(CliError::unexpected(err.to_string())),
            _ => {}
        }
    }
    Err(CliError::invalid_args(
        "PPTX template has no presentation slide size",
    ))
}

fn pptx_scaffold_result(
    output: &str,
    title: &str,
    subtitle: &str,
    validated: bool,
    package: &ScaffoldPackageInfo,
) -> Value {
    let mut result = Map::new();
    result.insert("output".to_string(), json!(output));
    result.insert("created".to_string(), json!(true));
    result.insert("family".to_string(), json!("pptx"));
    result.insert("presentationPart".to_string(), json!(PRESENTATION_PART));
    result.insert("slidePart".to_string(), json!(package.slide_part));
    result.insert(
        "slideLayoutPart".to_string(),
        json!(package.slide_layout_part),
    );
    result.insert(
        "slideMasterPart".to_string(),
        json!(package.slide_master_part),
    );
    result.insert("themePart".to_string(), json!(package.theme_part));
    result.insert("tableStylesPart".to_string(), json!(TABLE_STYLES_PART));
    result.insert("slide".to_string(), json!(1));
    result.insert("slideId".to_string(), json!(package.slide_id));
    result.insert("initialSlideCount".to_string(), json!(1));
    result.insert("initialTitle".to_string(), json!(title));
    result.insert("initialSubtitle".to_string(), json!(subtitle));
    result.insert("title".to_string(), json!(title));
    result.insert("subtitle".to_string(), json!(subtitle));
    result.insert("layoutCount".to_string(), json!(package.layout_names.len()));
    result.insert("layouts".to_string(), json!(package.layout_names));
    result.insert(
        "size".to_string(),
        json!({
            "name": package.slide_size.name,
            "widthEmu": package.slide_size.width,
            "heightEmu": package.slide_size.height,
            "widthInches": package.slide_size.width as f64 / 914_400.0,
            "heightInches": package.slide_size.height as f64 / 914_400.0,
        }),
    );
    if let Some(theme) = package.theme_name.as_deref() {
        result.insert("theme".to_string(), json!(theme));
    }
    if let Some(seed) = package.theme_seed.as_deref() {
        result.insert("themeSeed".to_string(), json!(seed));
    }
    if let Some(template) = package.template.as_deref() {
        result.insert("template".to_string(), json!(template));
    }
    result.insert("validated".to_string(), json!(validated));
    result.insert(
        "validateCommand".to_string(),
        json!(format!("ooxml validate --strict {}", command_arg(output))),
    );
    result.insert(
        "conformanceCommand".to_string(),
        json!(format!(
            "ooxml --json conformance check {}",
            command_arg(output)
        )),
    );
    result.insert(
        "readbackCommand".to_string(),
        json!(format!(
            "ooxml --json pptx slides list {}",
            command_arg(output)
        )),
    );
    result.insert(
        "layoutsCommand".to_string(),
        json!(format!(
            "ooxml --json pptx layouts list {}",
            command_arg(output)
        )),
    );
    result.insert(
        "shapesCommand".to_string(),
        json!(format!(
            "ooxml --json pptx shapes show {} --slide 1 --include-text --include-bounds",
            command_arg(output)
        )),
    );
    Value::Object(result)
}
