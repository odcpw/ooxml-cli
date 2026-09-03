use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;
use std::str::FromStr;

const COMMON_SCHEMA: &str = include_str!("schemas/common-v1.json");
const PPTX_SCHEMA: &str = include_str!("schemas/pptx-build-v1.json");
const XLSX_SCHEMA: &str = include_str!("schemas/xlsx-build-v1.json");
const DOCX_SCHEMA: &str = include_str!("schemas/docx-build-v1.json");

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildFamily {
    Pptx,
    Xlsx,
    Docx,
}

impl BuildFamily {
    pub const ALL: [Self; 3] = [Self::Pptx, Self::Xlsx, Self::Docx];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pptx => "pptx",
            Self::Xlsx => "xlsx",
            Self::Docx => "docx",
        }
    }

    pub const fn schema_name(self) -> &'static str {
        match self {
            Self::Pptx => "pptx-build",
            Self::Xlsx => "xlsx-build",
            Self::Docx => "docx-build",
        }
    }

    const fn schema_source(self) -> &'static str {
        match self {
            Self::Pptx => PPTX_SCHEMA,
            Self::Xlsx => XLSX_SCHEMA,
            Self::Docx => DOCX_SCHEMA,
        }
    }
}

impl fmt::Display for BuildFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for BuildFamily {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pptx" | "pptx-build" => Ok(Self::Pptx),
            "xlsx" | "xlsx-build" => Ok(Self::Xlsx),
            "docx" | "docx-build" => Ok(Self::Docx),
            other => Err(format!(
                "unknown build schema {other:?}; expected pptx-build, xlsx-build, or docx-build"
            )),
        }
    }
}

pub fn schema_by_name(name: &str) -> Result<Value, String> {
    BuildFamily::from_str(name).map(schema_document)
}

pub fn schema_document(family: BuildFamily) -> Value {
    let mut document: Value = serde_json::from_str(family.schema_source())
        .expect("committed family schema is valid JSON");
    let common: Map<String, Value> =
        serde_json::from_str(COMMON_SCHEMA).expect("committed common schema is valid JSON");
    let definitions = document
        .get_mut("$defs")
        .and_then(Value::as_object_mut)
        .expect("committed family schema has $defs");
    for (name, definition) in common {
        assert!(
            definitions.insert(name.clone(), definition).is_none(),
            "family schema duplicates common definition {name}"
        );
    }
    document
}

pub fn schema_text(family: BuildFamily) -> String {
    let mut text = serde_json::to_string_pretty(&schema_document(family))
        .expect("build schema is JSON serializable");
    text.push('\n');
    text
}
