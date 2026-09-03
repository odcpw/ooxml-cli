use crate::command_manifest::{
    CommandId, CoreCommandId, DocxCommandId, PptxCommandId, VbaCommandId, XlsxCommandId,
};

struct ServeMutationSpec {
    id: CommandId,
    canonical: &'static str,
    aliases: &'static [&'static str],
}

const CF_ADD_ALIASES: &[&str] = &[
    "xlsx conditional-formatting add",
    "xlsx conditional-format add",
    "xlsx cf add",
];
const CF_DELETE_ALIASES: &[&str] = &[
    "xlsx conditional-formats remove",
    "xlsx conditional-formatting delete",
    "xlsx conditional-formatting remove",
    "xlsx conditional-format delete",
    "xlsx conditional-format remove",
    "xlsx cf delete",
    "xlsx cf remove",
];
const CF_REORDER_ALIASES: &[&str] = &[
    "xlsx conditional-formatting reorder",
    "xlsx conditional-format reorder",
    "xlsx cf reorder",
];

const SERVE_MUTATION_SPECS: &[ServeMutationSpec] = &[
    spec(
        CommandId::Core(CoreCommandId::RepairNormalize),
        "repair normalize",
        &[],
    ),
    spec(
        CommandId::Core(CoreCommandId::TemplateApply),
        "template apply",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::SheetsAdd),
        "xlsx sheets add",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::SheetsRename),
        "xlsx sheets rename",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::SheetsMove),
        "xlsx sheets move",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::SheetsDelete),
        "xlsx sheets delete",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::DataValidationsCreate),
        "xlsx data-validations create",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::DataValidationsUpdate),
        "xlsx data-validations update",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::DataValidationsDelete),
        "xlsx data-validations delete",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::HyperlinksAdd),
        "xlsx hyperlinks add",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::HyperlinksUpdate),
        "xlsx hyperlinks update",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::HyperlinksDelete),
        "xlsx hyperlinks delete",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::NamesAdd),
        "xlsx names add",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::NamesUpdate),
        "xlsx names update",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::NamesRename),
        "xlsx names rename",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::NamesDelete),
        "xlsx names delete",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::TablesCreate),
        "xlsx tables create",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::FreezeSet),
        "xlsx freeze set",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::FreezeClear),
        "xlsx freeze clear",
        &[],
    ),
    spec(CommandId::Docx(DocxCommandId::Replace), "docx replace", &[]),
    spec(
        CommandId::Docx(DocxCommandId::TablesCreate),
        "docx tables create",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::ImagesInsert),
        "docx images insert",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::TablesSetStyle),
        "docx tables set-style",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::BreaksInsert),
        "docx breaks insert",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::SectionsSet),
        "docx sections set",
        &[],
    ),
    spec(CommandId::Vba(VbaCommandId::Create), "vba create", &[]),
    spec(CommandId::Vba(VbaCommandId::Rebuild), "vba rebuild", &[]),
    spec(CommandId::Vba(VbaCommandId::Attach), "vba attach", &[]),
    spec(CommandId::Vba(VbaCommandId::Remove), "vba remove", &[]),
    spec(
        CommandId::Xlsx(XlsxCommandId::CellsSet),
        "xlsx cells set",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::CommentsAdd),
        "xlsx comments add",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::CommentsUpdate),
        "xlsx comments update",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::CommentsRemove),
        "xlsx comments remove",
        &["xlsx comments delete"],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::ConditionalFormatsAdd),
        "xlsx conditional-formats add",
        CF_ADD_ALIASES,
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::ConditionalFormatsDelete),
        "xlsx conditional-formats delete",
        CF_DELETE_ALIASES,
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::ConditionalFormatsReorder),
        "xlsx conditional-formats reorder",
        CF_REORDER_ALIASES,
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::RangesSet),
        "xlsx ranges set",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::RangesSetFormat),
        "xlsx ranges set-format",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::ChartsSetSeriesStyle),
        "xlsx charts set-series-style",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::ColwidthsSet),
        "xlsx colwidths set",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::RowheightsSet),
        "xlsx rowheights set",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::TablesAppendRows),
        "xlsx tables append-rows",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::TablesAppendRecords),
        "xlsx tables append-records",
        &[],
    ),
    spec(
        CommandId::Xlsx(XlsxCommandId::WorkbookMetadataUpdate),
        "xlsx workbook metadata update",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::HeadersSetText),
        "docx headers set-text",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::FootersSetText),
        "docx footers set-text",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::FieldsInsert),
        "docx fields insert",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::FieldsSetResult),
        "docx fields set-result",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::ParagraphsAppend),
        "docx paragraphs append",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::ParagraphsInsert),
        "docx paragraphs insert",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::ParagraphsSet),
        "docx paragraphs set",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::ParagraphsClear),
        "docx paragraphs clear",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::StylesApply),
        "docx styles apply",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::BlocksReplace),
        "docx blocks replace",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::BlocksDelete),
        "docx blocks delete",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::BlocksInsertAfter),
        "docx blocks insert-after",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::CommentsAdd),
        "docx comments add",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::CommentsEdit),
        "docx comments edit",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::CommentsRemove),
        "docx comments remove",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::TablesSetCell),
        "docx tables set-cell",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::TablesClearCell),
        "docx tables clear-cell",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::TablesInsertRow),
        "docx tables insert-row",
        &[],
    ),
    spec(
        CommandId::Docx(DocxCommandId::TablesDeleteRow),
        "docx tables delete-row",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::ReplaceText),
        "pptx replace text",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::TablesSetCell),
        "pptx tables set-cell",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::TablesDeleteRow),
        "pptx tables delete-row",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::TablesInsertRow),
        "pptx tables insert-row",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::TablesDeleteCol),
        "pptx tables delete-col",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::TablesInsertCol),
        "pptx tables insert-col",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::TablesUpdateFromXlsx),
        "pptx tables update-from-xlsx",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::NotesSet),
        "pptx notes set",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::NotesClear),
        "pptx notes clear",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::ShapesDelete),
        "pptx shapes delete",
        &[],
    ),
    spec(
        CommandId::Pptx(PptxCommandId::ReplaceTextOccurrences),
        "pptx replace text-occurrences",
        &[],
    ),
];

const fn spec(
    id: CommandId,
    canonical: &'static str,
    aliases: &'static [&'static str],
) -> ServeMutationSpec {
    ServeMutationSpec {
        id,
        canonical,
        aliases,
    }
}

pub(super) fn resolve_serve_mutation_command(command: &str) -> Option<CommandId> {
    let words = command
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let canonical = crate::agent_aliases::canonicalize_command_alias_path(&words);
    let canonical_words = canonical.iter().map(String::as_str).collect::<Vec<_>>();
    if let Some(id) = crate::command_manifest::command_id_for_canonical_path(&canonical_words)
        && crate::command_manifest::manifest_serve_mutation_ids().contains(&id)
    {
        return Some(id);
    }

    SERVE_MUTATION_SPECS
        .iter()
        .find(|spec| spec.canonical == command || spec.aliases.contains(&command))
        .map(|spec| spec.id)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::command_manifest::manifest_serve_mutation_ids;

    #[test]
    fn mutation_namespace_is_exact_unique_and_manifest_backed() {
        let manifest_id_list = manifest_serve_mutation_ids();
        let manifest_ids = manifest_id_list.iter().copied().collect::<BTreeSet<_>>();
        let manifest_vba_count = manifest_id_list
            .iter()
            .filter(|id| matches!(id, CommandId::Vba(_)))
            .count();
        let manifest_package_count = manifest_id_list.len() - manifest_vba_count;
        let ids = SERVE_MUTATION_SPECS
            .iter()
            .map(|spec| spec.id)
            .collect::<BTreeSet<_>>();
        let canonicals = SERVE_MUTATION_SPECS
            .iter()
            .map(|spec| spec.canonical)
            .collect::<BTreeSet<_>>();
        let aliases = SERVE_MUTATION_SPECS
            .iter()
            .flat_map(|spec| spec.aliases.iter().copied())
            .collect::<BTreeSet<_>>();
        let labels = SERVE_MUTATION_SPECS
            .iter()
            .flat_map(|spec| std::iter::once(spec.canonical).chain(spec.aliases.iter().copied()))
            .collect::<BTreeSet<_>>();

        assert_eq!(SERVE_MUTATION_SPECS.len(), 73);
        assert_eq!(
            manifest_ids.len(),
            manifest_id_list.len(),
            "manifest-derived Serve mutation IDs must be unique"
        );
        assert_eq!(
            (manifest_package_count, manifest_vba_count),
            (150, 4),
            "reviewed denominator: 150 nestable package mutations + 4 VBA mutations = 154"
        );
        assert_eq!(ids.len(), 73);
        assert_eq!(canonicals.len(), 73);
        assert!(ids.is_subset(&manifest_ids));
        assert_eq!(aliases.len(), 14);
        assert_eq!(labels.len(), 87);
        assert_eq!(
            aliases,
            BTreeSet::from([
                "xlsx comments delete",
                "xlsx conditional-formatting add",
                "xlsx conditional-format add",
                "xlsx cf add",
                "xlsx conditional-formats remove",
                "xlsx conditional-formatting delete",
                "xlsx conditional-formatting remove",
                "xlsx conditional-format delete",
                "xlsx conditional-format remove",
                "xlsx cf delete",
                "xlsx cf remove",
                "xlsx conditional-formatting reorder",
                "xlsx conditional-format reorder",
                "xlsx cf reorder",
            ])
        );
        for spec in SERVE_MUTATION_SPECS {
            assert_eq!(
                resolve_serve_mutation_command(spec.canonical),
                Some(spec.id)
            );
            for alias in spec.aliases {
                assert_eq!(resolve_serve_mutation_command(alias), Some(spec.id));
            }
        }
        for capability in crate::command_manifest::capability_commands()
            .into_iter()
            .filter(|capability| capability["opCompatible"] == true)
        {
            let command = capability["path"]
                .as_str()
                .expect("capability command path")
                .strip_prefix("ooxml ")
                .expect("canonical command prefix");
            assert!(
                resolve_serve_mutation_command(command).is_some(),
                "opCompatible command is absent from the serve namespace: {command}"
            );
        }
        for (family, expected) in [
            ("core", 2),
            ("xlsx", 32),
            ("docx", 24),
            ("pptx", 11),
            ("vba", 4),
        ] {
            assert_eq!(
                SERVE_MUTATION_SPECS
                    .iter()
                    .filter(|spec| {
                        matches!(
                            (family, spec.id),
                            ("core", CommandId::Core(_))
                                | ("xlsx", CommandId::Xlsx(_))
                                | ("docx", CommandId::Docx(_))
                                | ("pptx", CommandId::Pptx(_))
                                | ("vba", CommandId::Vba(_))
                        )
                    })
                    .count(),
                expected
            );
        }
        assert_eq!(resolve_serve_mutation_command("xlsx not-real"), None);
    }
}
