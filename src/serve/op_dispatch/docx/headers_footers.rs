use serde_json::{Value, json};

use super::super::super::op::{ServeOp, push_serve_plan_string_flag};
use crate::{
    CliError, CliResult, DocxHeaderFooterSetTextOptions, docx_headers_footers_set_text, json_i64,
    json_optional_string, normalize_docx_header_footer_show_type, resolve_required_docx_table_text,
};

pub(super) fn serve_docx_headers_footers_op(
    working: &str,
    command: &str,
    args: &Value,
) -> CliResult<ServeOp> {
    let op = match command {
        "docx headers set-text" | "docx footers set-text" => {
            let kind = if command.contains("footers") {
                "footer"
            } else {
                "header"
            };
            let id = json_optional_string(args, "id").unwrap_or_default();
            let ref_type =
                json_optional_string(args, "type").unwrap_or_else(|| "default".to_string());
            let ref_type = normalize_docx_header_footer_show_type(&ref_type)?;
            let section_value = json_i64(args, "section")?;
            let section = section_value.unwrap_or(0);
            let index_value = json_i64(args, "index")?;
            let index = index_value.unwrap_or(1);
            let selector = json_optional_string(args, "selector");
            let text = json_optional_string(args, "text");
            let text_file = json_optional_string(args, "text-file")
                .or_else(|| json_optional_string(args, "textFile"));
            let text_set = args.get("text").is_some();
            let text_file_set = args.get("text-file").is_some() || args.get("textFile").is_some();
            let text = resolve_required_docx_table_text(
                text.as_deref(),
                text_file.as_deref(),
                text_set,
                text_file_set,
            )?;
            let readback = docx_headers_footers_set_text(
                working,
                kind,
                DocxHeaderFooterSetTextOptions {
                    id: &id,
                    ref_type: &ref_type,
                    section,
                    index,
                    selector: selector.as_deref(),
                    selector_given: selector.is_some(),
                    index_given: index_value.is_some(),
                    text: &text,
                    out: None,
                    backup: None,
                    dry_run: false,
                    in_place: true,
                    no_validate: true,
                },
            )?;
            let mut plan_flags = Vec::new();
            push_serve_plan_string_flag(
                &mut plan_flags,
                "--id",
                (!id.is_empty()).then_some(id.as_str()),
            );
            if args.get("type").is_some() {
                push_serve_plan_string_flag(&mut plan_flags, "--type", Some(ref_type.as_str()));
            }
            if let Some(section) = section_value {
                plan_flags.push(json!("--section"));
                plan_flags.push(json!(section.to_string()));
            }
            if let Some(index) = index_value {
                plan_flags.push(json!("--index"));
                plan_flags.push(json!(index.to_string()));
            }
            push_serve_plan_string_flag(&mut plan_flags, "--selector", selector.as_deref());
            if text_set {
                push_serve_plan_string_flag(&mut plan_flags, "--text", Some(text.as_str()));
            }
            if text_file_set {
                push_serve_plan_string_flag(&mut plan_flags, "--text-file", text_file.as_deref());
            }
            ServeOp::DocxHeaderFooterSetText {
                command: command.to_string(),
                plan_flags,
                readback_file: working.to_string(),
                readback,
            }
        }
        _ => {
            return Err(CliError::invalid_args(format!(
                "unsupported serve op command: {command}"
            )));
        }
    };
    Ok(op)
}
