use serde_json::{Value, json};

use super::super::super::op::{ServeOp, push_serve_plan_bool_flag, push_serve_plan_string_flag};
use crate::command_manifest::DocxCommandId;
use crate::docx_images::{DocxImageInsertOptions, DocxImagePipelineArgs, docx_images_insert};
use crate::{
    CliError, CliResult, DocxParagraphMutationOptions, json_bool, json_i64, json_optional_string,
    json_string,
};

pub(super) fn serve_docx_images_op(
    working: &str,
    command_id: DocxCommandId,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    if command_id != DocxCommandId::ImagesInsert {
        return Err(CliError::invalid_args(format!(
            "unsupported serve op command: {command}"
        )));
    }
    let after = json_i64(args, "after")?.unwrap_or(0);
    if after < 0 {
        return Err(CliError::invalid_args("after must be >= 0"));
    }
    let image_file = json_string(args, "image")?;
    let width = json_i64(args, "width")?.unwrap_or(0);
    let height = json_i64(args, "height")?.unwrap_or(0);
    if width <= 0 || height <= 0 {
        return Err(CliError::invalid_args(
            "width and height are required and must be > 0 (EMU)",
        ));
    }
    let caption = json_optional_string(args, "caption");
    let align = json_optional_string(args, "align").unwrap_or_default();
    let fit = json_optional_string(args, "fit");
    let max_dpi = args
        .get("max-dpi")
        .or_else(|| args.get("maxDpi"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_u64().map(|value| value.to_string()))
        });
    let alt = json_optional_string(args, "alt").unwrap_or_default();
    let keep_original = json_bool(args, "keep-original")
        .or_else(|| json_bool(args, "keepOriginal"))
        .unwrap_or(false);
    let readback = docx_images_insert(
        working,
        DocxImageInsertOptions {
            after: after as usize,
            image_file: &image_file,
            expected_hash: "",
            width,
            height,
            caption: caption.as_deref(),
            align: &align,
            image: DocxImagePipelineArgs {
                fit: fit.as_deref(),
                max_dpi: max_dpi.as_deref(),
                keep_original,
                alt: &alt,
            },
            mutation: DocxParagraphMutationOptions {
                text: None,
                text_file: None,
                style: "",
                out: None,
                backup: None,
                dry_run: false,
                in_place: true,
                no_validate: true,
            },
        },
    )?;
    let mut plan_flags = vec![
        json!("--after"),
        json!(after.to_string()),
        json!("--file"),
        json!(image_file),
        json!("--width"),
        json!(width.to_string()),
        json!("--height"),
        json!(height.to_string()),
    ];
    push_serve_plan_string_flag(&mut plan_flags, "--caption", caption.as_deref());
    push_serve_plan_string_flag(
        &mut plan_flags,
        "--align",
        (!align.is_empty()).then_some(align.as_str()),
    );
    push_serve_plan_string_flag(&mut plan_flags, "--fit", fit.as_deref());
    push_serve_plan_string_flag(&mut plan_flags, "--max-dpi", max_dpi.as_deref());
    push_serve_plan_string_flag(
        &mut plan_flags,
        "--alt",
        (!alt.is_empty()).then_some(alt.as_str()),
    );
    push_serve_plan_bool_flag(
        &mut plan_flags,
        "--keep-original",
        keep_original.then_some(true),
    );
    Ok(ServeOp::GenericMutationOp {
        command: command.to_string(),
        plan_flags,
        readback_file: working.to_string(),
        readback,
    })
}
