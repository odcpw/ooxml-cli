use serde_json::Value;

use crate::{CliResult, zip_entry_names, zip_entry_set};

mod chart_structure;
mod content_types;
mod deep_relationships;
mod embedded_workbook;
mod image_payloads;
mod package;
mod pptx_animations;
mod references;
mod relationships;
mod spec;
mod spreadsheet_semantics;
mod table_pivot;
mod types;
mod util;
mod xml_parts;

use chart_structure::check_chart_structure_invariants;
use content_types::{check_content_types_coverage, collect_parts, parse_content_types};
use deep_relationships::check_part_deep_relationship_invariants;
use image_payloads::check_part_image_payload_invariants;
use package::{check_zip_entry_metadata, read_zip_entry_metadata};
use pptx_animations::check_part_pptx_animation_invariants;
use references::check_reference_list_invariants;
use relationships::{check_package_relationship_closure, parse_relationship_part};
use spec::check_known_part_content_type;
use spreadsheet_semantics::{
    check_part_spreadsheet_semantic_invariants, collect_spreadsheet_semantic_context,
};
use table_pivot::check_table_pivot_invariants;
use util::{diag, is_rels_uri};
use xml_parts::check_part_xml_invariants;

pub(crate) fn check_repair_invariants(file: &str) -> CliResult<Vec<Value>> {
    let entries = zip_entry_names(file)?;
    let entry_set = zip_entry_set(&entries);
    let content_types = parse_content_types(file, &entry_set)?;
    let parts = collect_parts(&entries, &content_types);
    let zip_metadata = read_zip_entry_metadata(file)?;
    let spreadsheet_semantics = collect_spreadsheet_semantic_context(file, &entry_set, &parts);

    let mut diagnostics = Vec::new();
    diagnostics.extend(content_types.diagnostics.clone());
    if content_types.coverage_ok {
        diagnostics.extend(check_content_types_coverage(&entry_set, &content_types));
    }
    diagnostics.extend(check_package_relationship_closure(
        file, &entries, &entry_set, &parts,
    )?);
    diagnostics.extend(check_reference_list_invariants(file, &entry_set, &parts));

    for part in &parts {
        diagnostics.extend(check_known_part_content_type(&part.uri, &part.content_type));
        diagnostics.extend(check_zip_entry_metadata(&zip_metadata, part));
        if is_rels_uri(&part.uri) {
            match parse_relationship_part(file, &part.entry_name) {
                Ok(_) => {}
                Err(err) => diagnostics.push(diag(
                    "OOXML_RELS_PARSE_ERROR",
                    format!("failed to parse relationships part {}: {err}", part.uri),
                )),
            }
            continue;
        }
        diagnostics.extend(check_part_xml_invariants(file, part)?);
        diagnostics.extend(check_part_pptx_animation_invariants(file, part)?);
        diagnostics.extend(check_chart_structure_invariants(file, part));
        diagnostics.extend(check_table_pivot_invariants(file, part)?);
        diagnostics.extend(check_part_deep_relationship_invariants(
            file, part, &entry_set, &parts,
        )?);
        diagnostics.extend(check_part_image_payload_invariants(
            file, part, &entry_set, &parts,
        )?);
        diagnostics.extend(check_part_spreadsheet_semantic_invariants(
            file,
            part,
            &spreadsheet_semantics,
        ));
    }

    Ok(diagnostics)
}
