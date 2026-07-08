mod animations;
mod charts;
mod comments;
mod fields;
mod import_merge;
mod layouts;
mod notes;
mod placement;
mod replace;
mod shapes;
mod slides;
mod tables;
mod text;
mod theme;

pub(crate) use animations::{
    pptx_animations_add, pptx_animations_prune_stale, pptx_animations_remove,
    pptx_animations_reorder,
};
pub(crate) use charts::{
    pptx_charts_convert_type, pptx_charts_copy_style, pptx_charts_create, pptx_charts_set_axis,
    pptx_charts_set_chart_area_fill, pptx_charts_set_legend, pptx_charts_set_plot_area_fill,
    pptx_charts_set_series_style, pptx_charts_set_title, pptx_charts_update_data,
};
pub(crate) use comments::{pptx_comments_add, pptx_comments_edit, pptx_comments_remove};
pub(crate) use fields::pptx_fields_set;
pub(crate) use import_merge::{
    pptx_layouts_import, pptx_masters_import, pptx_slides_import_slide, pptx_slides_merge,
};
pub(crate) use layouts::{
    pptx_layouts_add_placeholder, pptx_layouts_clone, pptx_layouts_delete_shape,
    pptx_layouts_rename, pptx_layouts_set_bounds, pptx_masters_add_placeholder,
};
pub(crate) use notes::{pptx_notes_clear, pptx_notes_set};
pub(crate) use placement::{
    pptx_add_textbox, pptx_place_image, pptx_place_table, pptx_place_table_from_xlsx,
};
pub(crate) use replace::{
    pptx_replace_images, pptx_replace_text, pptx_replace_text_from_xlsx,
    pptx_replace_text_in_place, pptx_replace_text_map_from_xlsx, pptx_replace_text_occurrences,
};
pub(crate) use shapes::{pptx_shapes_delete, pptx_shapes_set_bounds};
pub(crate) use slides::{
    pptx_clone_slide, pptx_new_slide_from_layout, pptx_slides_delete, pptx_slides_move,
    pptx_slides_reorder,
};
pub(crate) use tables::{
    pptx_tables_delete_col, pptx_tables_delete_row, pptx_tables_insert_col, pptx_tables_insert_row,
    pptx_tables_set_cell, pptx_tables_update_from_xlsx,
};
pub(crate) use text::pptx_text_set;
pub(crate) use theme::pptx_theme_update;
