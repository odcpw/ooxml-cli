//! Shared declarative build-spec schemas, validation, and op-plan compilation.
//!
//! Family build commands own the mapping from rich family nodes to mutations.
//! This module owns the stable input vocabulary and the exact batch operation
//! model consumed by `apply`, serve, and MCP.

mod compiler;
mod loader;
mod pptx;
mod schema;
mod types;

pub use compiler::{
    BuildCompileError, BuildCompiler, BuildOperation, CompiledBuildPlan, PlanNode,
    compile_minimal_spec, operation_reference,
};
pub use loader::{
    BuildSpec, BuildSpecDiagnostic, BuildSpecError, load_spec_bytes, load_spec_file, load_spec_str,
};
pub(crate) use pptx::pptx_build;
pub use pptx::{CompiledPptxBuild, PptxBuildAsset, compile_pptx_spec, is_generated_asset_path};
pub use schema::{BuildFamily, schema_by_name, schema_document, schema_text};
pub use types::{
    Bounds, BrandRef, BuildLength, ChartData, ChartSeries, ImageRef, Paragraph, TableData, TextRun,
    XlsxRangeRef,
};
