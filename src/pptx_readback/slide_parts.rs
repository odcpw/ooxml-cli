use super::{normalize_ppt_target, pptx_slide_refs};
use crate::{CliError, CliResult, relationships, zip_text};
#[derive(Clone)]
pub(super) struct PptxSlidePartRef {
    pub(super) number: u32,
    pub(super) slide_id: u32,
    pub(super) part: String,
}

pub(super) fn pptx_slide_part_refs(file: &str) -> CliResult<Vec<PptxSlidePartRef>> {
    let presentation = zip_text(file, "ppt/presentation.xml")?;
    let slides = pptx_slide_refs(&presentation);
    let rels = relationships(file, "ppt/_rels/presentation.xml.rels")?;
    slides
        .iter()
        .enumerate()
        .map(|(index, (slide_id, rel_id))| {
            let target = rels
                .get(rel_id)
                .ok_or_else(|| CliError::unexpected(format!("missing relationship {rel_id}")))?;
            Ok(PptxSlidePartRef {
                number: index as u32 + 1,
                slide_id: *slide_id,
                part: normalize_ppt_target(target),
            })
        })
        .collect()
}
