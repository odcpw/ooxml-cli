use crate::{
    CliResult, relationship_entries, relationships_part_for, resolve_relationship_target, zip_text,
};

use super::shape_model::{BoundsSource, Placeholder, Shape, pptx_shape_models};

const SLIDE_LAYOUT_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const SLIDE_MASTER_REL_TYPE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";

/// Resolve the effective geometry of every shape on a slide.
///
/// This is the shared inheritance seam for readback and layout analysis. A
/// placeholder uses its own transform first, then its layout placeholder
/// matched by `idx` and then by type, then its master placeholder by type.
/// Keeping the resolved source on the model lets future semantic surfaces such
/// as outline readback reuse the result without reimplementing inheritance.
pub(crate) fn pptx_resolved_shape_models(
    file: &str,
    slide_part: &str,
    slide_xml: &str,
) -> CliResult<Vec<Shape>> {
    let mut slide_shapes = pptx_shape_models(slide_xml);
    let slide_uri = package_uri(slide_part);
    let layout_uri = related_part(file, &slide_uri, SLIDE_LAYOUT_REL_TYPE);
    let layout_shapes = layout_uri
        .as_deref()
        .and_then(|uri| zip_text(file, uri.trim_start_matches('/')).ok())
        .map(|xml| pptx_shape_models(&xml))
        .unwrap_or_default();
    let master_shapes = layout_uri
        .as_deref()
        .and_then(|uri| related_part(file, uri, SLIDE_MASTER_REL_TYPE))
        .and_then(|uri| zip_text(file, uri.trim_start_matches('/')).ok())
        .map(|xml| pptx_shape_models(&xml))
        .unwrap_or_default();

    for shape in &mut slide_shapes {
        if shape.bounds.is_some() {
            shape.bounds_source = Some(BoundsSource::Slide);
        }
        let Some(slide_placeholder) = shape.placeholder.as_mut() else {
            continue;
        };
        if !slide_placeholder.literal_type.is_empty() {
            slide_placeholder.resolved_type = slide_placeholder.literal_type.clone();
            slide_placeholder.type_source = Some(BoundsSource::Slide);
        }

        let layout_match = match_layout_placeholder(slide_placeholder, &layout_shapes);
        if slide_placeholder.resolved_type.is_empty()
            && let Some(layout_shape) = layout_match
            && let Some(layout_placeholder) = layout_shape.placeholder.as_ref()
            && !layout_placeholder.literal_type.is_empty()
        {
            slide_placeholder.resolved_type = layout_placeholder.literal_type.clone();
            slide_placeholder.type_source = Some(BoundsSource::Layout);
        }
        if shape.bounds.is_none()
            && let Some(layout_shape) = layout_match
            && let Some(bounds) = layout_shape.bounds.as_ref()
        {
            shape.bounds = Some(bounds.clone());
            shape.bounds_source = Some(BoundsSource::Layout);
        }

        if shape.bounds.is_none()
            && let Some(master_shape) = match_master_placeholder(slide_placeholder, &master_shapes)
            && let Some(bounds) = master_shape.bounds.as_ref()
        {
            shape.bounds = Some(bounds.clone());
            shape.bounds_source = Some(BoundsSource::Master);
            if slide_placeholder.resolved_type.is_empty()
                && let Some(master_placeholder) = master_shape.placeholder.as_ref()
            {
                slide_placeholder.resolved_type = master_placeholder.literal_type.clone();
                slide_placeholder.type_source = Some(BoundsSource::Master);
            }
        }
    }
    Ok(slide_shapes)
}

fn package_uri(part: &str) -> String {
    format!("/{}", part.trim_start_matches('/'))
}

fn related_part(file: &str, source_uri: &str, rel_type: &str) -> Option<String> {
    relationship_entries(file, &relationships_part_for(source_uri))
        .ok()?
        .into_iter()
        .find(|rel| rel.rel_type == rel_type)
        .map(|rel| resolve_relationship_target(source_uri, &rel.target))
}

fn match_layout_placeholder<'a>(
    placeholder: &Placeholder,
    layout_shapes: &'a [Shape],
) -> Option<&'a Shape> {
    if let Some(index) = placeholder.index
        && let Some(shape) = layout_shapes.iter().find(|shape| {
            shape
                .placeholder
                .as_ref()
                .is_some_and(|candidate| candidate.index == Some(index))
        })
    {
        return Some(shape);
    }
    placeholder_type(placeholder).and_then(|wanted| {
        layout_shapes.iter().find(|shape| {
            shape
                .placeholder
                .as_ref()
                .and_then(placeholder_type)
                .is_some_and(|candidate| candidate == wanted)
        })
    })
}

fn match_master_placeholder<'a>(
    placeholder: &Placeholder,
    master_shapes: &'a [Shape],
) -> Option<&'a Shape> {
    let wanted = placeholder_type(placeholder)?;
    master_shapes.iter().find(|shape| {
        shape
            .placeholder
            .as_ref()
            .and_then(placeholder_type)
            .is_some_and(|candidate| candidate == wanted)
    })
}

fn placeholder_type(placeholder: &Placeholder) -> Option<&str> {
    if !placeholder.resolved_type.is_empty() {
        Some(placeholder.resolved_type.as_str())
    } else if !placeholder.literal_type.is_empty() {
        Some(placeholder.literal_type.as_str())
    } else {
        None
    }
}
