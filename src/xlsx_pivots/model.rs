use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

use crate::{RangeBounds, add_selector};

#[derive(Clone, Default)]
pub(super) struct XlsxPivotFieldRef {
    pub(super) index: i32,
    pub(super) name: String,
    pub(super) axis: String,
    pub(super) subtotal: String,
    pub(super) caption: String,
}

impl XlsxPivotFieldRef {
    fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("index".to_string(), json!(self.index));
        if !self.name.is_empty() {
            object.insert("name".to_string(), json!(self.name));
        }
        if !self.axis.is_empty() {
            object.insert("axis".to_string(), json!(self.axis));
        }
        if !self.subtotal.is_empty() {
            object.insert("subtotal".to_string(), json!(self.subtotal));
        }
        if !self.caption.is_empty() {
            object.insert("caption".to_string(), json!(self.caption));
        }
        Value::Object(object)
    }
}

#[derive(Clone, Default)]
pub(super) struct XlsxPivotSourceRef {
    pub(super) source_type: String,
    pub(super) sheet: String,
    pub(super) range: String,
    pub(super) name: String,
}

impl XlsxPivotSourceRef {
    fn to_json(&self) -> Value {
        let mut object = Map::new();
        if !self.source_type.is_empty() {
            object.insert("type".to_string(), json!(self.source_type));
        }
        if !self.sheet.is_empty() {
            object.insert("sheet".to_string(), json!(self.sheet));
        }
        if !self.range.is_empty() {
            object.insert("range".to_string(), json!(self.range));
        }
        if !self.name.is_empty() {
            object.insert("name".to_string(), json!(self.name));
        }
        Value::Object(object)
    }
}

#[derive(Clone, Default)]
pub(super) struct XlsxPivotCacheField {
    pub(super) index: i32,
    pub(super) name: String,
}

impl XlsxPivotCacheField {
    fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("index".to_string(), json!(self.index));
        if !self.name.is_empty() {
            object.insert("name".to_string(), json!(self.name));
        }
        Value::Object(object)
    }
}

#[derive(Clone, Default)]
pub(super) struct XlsxPivotCacheRef {
    pub(super) cache_id: i32,
    pub(super) part_uri: String,
    pub(super) relationship_id: String,
    pub(super) records_part_uri: String,
    pub(super) record_count: i32,
    pub(super) created_version: String,
    pub(super) refreshed_version: String,
    pub(super) refresh_on_load: bool,
    pub(super) save_data: Option<bool>,
    pub(super) source: XlsxPivotSourceRef,
    pub(super) fields: Vec<XlsxPivotCacheField>,
}

impl XlsxPivotCacheRef {
    pub(super) fn field_name(&self, index: i32) -> String {
        self.fields
            .iter()
            .find(|field| field.index == index)
            .map(|field| field.name.clone())
            .unwrap_or_default()
    }

    fn to_json(&self) -> Value {
        let mut object = Map::new();
        if self.cache_id > 0 {
            object.insert("cacheId".to_string(), json!(self.cache_id));
        }
        if !self.part_uri.is_empty() {
            object.insert("partUri".to_string(), json!(self.part_uri));
        }
        if !self.relationship_id.is_empty() {
            object.insert("relationshipId".to_string(), json!(self.relationship_id));
        }
        if !self.records_part_uri.is_empty() {
            object.insert("recordsPartUri".to_string(), json!(self.records_part_uri));
        }
        if self.record_count > 0 {
            object.insert("recordCount".to_string(), json!(self.record_count));
        }
        if !self.created_version.is_empty() {
            object.insert("createdVersion".to_string(), json!(self.created_version));
        }
        if !self.refreshed_version.is_empty() {
            object.insert(
                "refreshedVersion".to_string(),
                json!(self.refreshed_version),
            );
        }
        if self.refresh_on_load {
            object.insert("refreshOnLoad".to_string(), json!(true));
        }
        if let Some(save_data) = self.save_data {
            object.insert("saveData".to_string(), json!(save_data));
        }
        object.insert("source".to_string(), self.source.to_json());
        if !self.fields.is_empty() {
            object.insert(
                "fields".to_string(),
                Value::Array(
                    self.fields
                        .iter()
                        .map(XlsxPivotCacheField::to_json)
                        .collect(),
                ),
            );
        }
        Value::Object(object)
    }
}

#[derive(Clone, Default)]
pub(super) struct XlsxPivotRef {
    pub(super) number: u32,
    pub(super) sheet: String,
    pub(super) sheet_number: u32,
    pub(super) sheet_part_uri: String,
    pub(super) relationship_id: String,
    pub(super) part_uri: String,
    pub(super) name: String,
    pub(super) cache_id: i32,
    pub(super) location: String,
    pub(super) rows: u32,
    pub(super) cols: u32,
    pub(super) primary_selector: String,
    pub(super) selectors: Vec<String>,
    pub(super) cache: Option<XlsxPivotCacheRef>,
    pub(super) row_fields: Vec<XlsxPivotFieldRef>,
    pub(super) column_fields: Vec<XlsxPivotFieldRef>,
    pub(super) data_fields: Vec<XlsxPivotFieldRef>,
    pub(super) filter_fields: Vec<XlsxPivotFieldRef>,
    pub(super) fields: Vec<XlsxPivotFieldRef>,
}

impl XlsxPivotRef {
    pub(super) fn apply_selectors(&mut self) {
        self.primary_selector = if self.number > 0 {
            format!("pivot:{}", self.number)
        } else if !self.name.trim().is_empty() {
            format!("pivot:{}", self.name)
        } else {
            String::new()
        };
        let mut selectors = Vec::new();
        add_selector(&mut selectors, self.primary_selector.clone());
        if self.number > 0 {
            add_selector(&mut selectors, format!("pivot:{}", self.number));
            add_selector(&mut selectors, format!("#{}", self.number));
        }
        if !self.name.trim().is_empty() {
            add_selector(&mut selectors, format!("pivot:{}", self.name));
            add_selector(&mut selectors, format!("name:{}", self.name));
            add_selector(&mut selectors, format!("~{}", self.name));
            add_selector(&mut selectors, self.name.clone());
        }
        if self.cache_id > 0 {
            add_selector(&mut selectors, format!("cacheId:{}", self.cache_id));
        }
        if !self.relationship_id.trim().is_empty() {
            add_selector(&mut selectors, format!("rId:{}", self.relationship_id));
            add_selector(&mut selectors, format!("rid:{}", self.relationship_id));
        }
        if !self.part_uri.trim().is_empty() {
            add_selector(&mut selectors, format!("part:{}", self.part_uri));
        }
        self.selectors = selectors;
    }

    pub(super) fn to_json_object(&self) -> Map<String, Value> {
        let mut object = Map::new();
        object.insert("number".to_string(), json!(self.number));
        object.insert("sheet".to_string(), json!(self.sheet));
        object.insert("sheetNumber".to_string(), json!(self.sheet_number));
        if !self.sheet_part_uri.is_empty() {
            object.insert("sheetPartUri".to_string(), json!(self.sheet_part_uri));
        }
        if !self.relationship_id.is_empty() {
            object.insert("relationshipId".to_string(), json!(self.relationship_id));
        }
        if !self.part_uri.is_empty() {
            object.insert("partUri".to_string(), json!(self.part_uri));
        }
        if !self.name.is_empty() {
            object.insert("name".to_string(), json!(self.name));
        }
        if self.cache_id > 0 {
            object.insert("cacheId".to_string(), json!(self.cache_id));
        }
        if !self.location.is_empty() {
            object.insert("location".to_string(), json!(self.location));
        }
        if self.rows > 0 {
            object.insert("rows".to_string(), json!(self.rows));
        }
        if self.cols > 0 {
            object.insert("cols".to_string(), json!(self.cols));
        }
        if !self.primary_selector.is_empty() {
            object.insert("primarySelector".to_string(), json!(self.primary_selector));
        }
        if !self.selectors.is_empty() {
            object.insert("selectors".to_string(), json!(self.selectors));
        }
        if let Some(cache) = &self.cache {
            object.insert("cache".to_string(), cache.to_json());
        }
        insert_field_array(&mut object, "rowFields", &self.row_fields);
        insert_field_array(&mut object, "columnFields", &self.column_fields);
        insert_field_array(&mut object, "dataFields", &self.data_fields);
        insert_field_array(&mut object, "filterFields", &self.filter_fields);
        insert_field_array(&mut object, "fields", &self.fields);
        object
    }
}

fn insert_field_array(object: &mut Map<String, Value>, key: &str, fields: &[XlsxPivotFieldRef]) {
    if !fields.is_empty() {
        object.insert(
            key.to_string(),
            Value::Array(fields.iter().map(XlsxPivotFieldRef::to_json).collect()),
        );
    }
}

pub(crate) struct XlsxPivotsCreateOptions<'a> {
    pub(crate) sheet: Option<&'a str>,
    pub(crate) range: Option<&'a str>,
    pub(crate) table: Option<&'a str>,
    pub(crate) target_sheet: Option<&'a str>,
    pub(crate) anchor: Option<&'a str>,
    pub(crate) name: Option<&'a str>,
    pub(crate) rows: Option<&'a str>,
    pub(crate) cols: Option<&'a str>,
    pub(crate) filters: Option<&'a str>,
    pub(crate) values: Option<&'a str>,
    pub(crate) expect_source_range: Option<&'a str>,
    pub(crate) max_cells: i64,
    pub(crate) out: Option<&'a str>,
    pub(crate) backup: Option<&'a str>,
    pub(crate) dry_run: bool,
    pub(crate) no_validate: bool,
    pub(crate) in_place: bool,
}

#[derive(Clone)]
pub(super) struct PivotValueSpec {
    pub(super) name: String,
    pub(super) aggregation: String,
}

#[derive(Clone, Default)]
pub(super) struct PivotCell {
    pub(super) value: String,
    pub(super) null: bool,
}

pub(super) struct PivotSource {
    pub(super) sheet: String,
    pub(super) range: String,
    pub(super) bounds: RangeBounds,
    pub(super) cells: Vec<Vec<PivotCell>>,
}

pub(super) struct PivotFieldModel {
    pub(super) name: String,
    pub(super) numeric: bool,
    pub(super) has_items: bool,
    pub(super) items: Vec<String>,
    pub(super) item_is_num: Vec<bool>,
    pub(super) item_index: BTreeMap<String, usize>,
    pub(super) min_value: f64,
    pub(super) max_value: f64,
}

pub(super) struct PivotDataField {
    pub(super) field_index: usize,
    pub(super) subtotal: String,
    pub(super) caption: String,
}

pub(super) struct PivotCreateArtifacts {
    pub(super) name: String,
    pub(super) source_sheet: String,
    pub(super) source_range: String,
    pub(super) target_sheet: String,
    pub(super) location: String,
    pub(super) cache_id: i32,
    pub(super) cache_definition_uri: String,
    pub(super) cache_records_uri: String,
    pub(super) pivot_table_uri: String,
    pub(super) row_fields: Vec<String>,
    pub(super) col_fields: Vec<String>,
    pub(super) page_fields: Vec<String>,
    pub(super) value_fields: Vec<String>,
    pub(super) warnings: Vec<String>,
    pub(super) overrides: BTreeMap<String, String>,
}
