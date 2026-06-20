use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone)]
pub(super) struct PartInfo {
    pub(super) uri: String,
    pub(super) entry_name: String,
    pub(super) content_type: String,
}

#[derive(Clone, Default)]
pub(super) struct ContentTypesInfo {
    pub(super) defaults: BTreeSet<String>,
    pub(super) overrides: BTreeSet<String>,
    pub(super) default_types: BTreeMap<String, String>,
    pub(super) override_types: BTreeMap<String, String>,
    pub(super) diagnostics: Vec<Value>,
    pub(super) coverage_ok: bool,
}

#[derive(Clone, Default)]
pub(super) struct RelationshipRecord {
    pub(super) id: String,
    pub(super) rel_type: String,
    pub(super) target: String,
    pub(super) target_mode: String,
}

#[derive(Clone, Default)]
pub(super) struct XmlElementInfo {
    pub(super) local_name: String,
    pub(super) namespace: String,
    pub(super) attrs: BTreeMap<String, String>,
}

#[derive(Clone, Default)]
pub(super) struct XmlPartInfo {
    pub(super) root: Option<XmlElementInfo>,
    pub(super) children: Vec<XmlElementInfo>,
    pub(super) direct_child_counts: BTreeMap<(String, String), usize>,
}
