package conformance

const CoverageSchemaVersion = "ooxml-cli.conformance.coverage.v1"

// CoverageReport is a deterministic, machine-readable summary of the
// repair-focused conformance harness. It documents local evidence and remaining
// external-oracle gaps for agents before they rely on the harness.
type CoverageReport struct {
	SchemaVersion    string                `json:"schemaVersion"`
	Scope            string                `json:"scope"`
	Status           string                `json:"status"`
	HarnessStages    []CoverageStage       `json:"harnessStages"`
	RepairClasses    []RepairClassCoverage `json:"repairClasses"`
	FixtureSets      []FixtureSetCoverage  `json:"fixtureSets"`
	KnownLimitations []string              `json:"knownLimitations"`
}

// CoverageStage describes one layer of the conformance harness.
type CoverageStage struct {
	Name     string   `json:"name"`
	Status   string   `json:"status"`
	Evidence []string `json:"evidence,omitempty"`
}

// RepairClassCoverage links a repair-sensitive issue family to the diagnostic
// codes and tests/goldens that currently cover it.
type RepairClassCoverage struct {
	ID              string   `json:"id"`
	Status          string   `json:"status"`
	Families        []string `json:"families"`
	DiagnosticCodes []string `json:"diagnosticCodes,omitempty"`
	Evidence        []string `json:"evidence"`
}

// FixtureSetCoverage records the committed fixture/golden evidence used by the
// harness. It is provenance, not a live test result.
type FixtureSetCoverage struct {
	Name     string   `json:"name"`
	Status   string   `json:"status"`
	Families []string `json:"families,omitempty"`
	Evidence []string `json:"evidence,omitempty"`
}

// RepairCoverageReport returns the current conformance coverage contract.
func RepairCoverageReport() CoverageReport {
	return CoverageReport{
		SchemaVersion: CoverageSchemaVersion,
		Scope:         "pptx-xlsx-office-repair-plus-docx-targeted-invariants",
		Status:        "active",
		HarnessStages: []CoverageStage{
			{
				Name:   "package-open",
				Status: "implemented",
				Evidence: []string{
					"pkg/conformance.CheckPackage",
					"pkg/conformance.TestConformanceGoldenSummary",
				},
			},
			{
				Name:   "repo-validation",
				Status: "implemented",
				Evidence: []string{
					"pkg/validate.ValidatePackage",
					"pkg/conformance.TestConformanceGoldenSummary",
				},
			},
			{
				Name:   "repair-invariants",
				Status: "implemented",
				Evidence: []string{
					"pkg/conformance.CheckRepairInvariants",
					"pkg/conformance.TestRepairInvariants*",
				},
			},
			{
				Name:   "golden-summary",
				Status: "implemented",
				Evidence: []string{
					"testdata/golden/repair-conformance-summary.json",
					"testdata/golden/repair-conformance-office-open-summary.json",
					"testdata/golden/generated-repair-proof-bundle.json",
				},
			},
			{
				Name:   "office-open",
				Status: "local-engine-optional",
				Evidence: []string{
					"pkg/officecheck",
					"pkg/conformance.TestConformanceOfficeOpenGoldenSummary",
					"pkg/conformance.TestCheckPackageOfficeCheckFailureFailsReport",
					"internal/cli.TestConformanceCheckOfficeCheckJSON",
					"internal/cli.TestGeneratedRepairConformanceOfficeOpenIfAvailable",
				},
			},
		},
		RepairClasses: []RepairClassCoverage{
			{
				ID:       "content-types",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_CONTENT_TYPES_READ_ERROR",
					"OOXML_CONTENT_TYPES_PARSE_ERROR",
					"OOXML_CONTENT_TYPES_ROOT",
					"OOXML_CONTENT_TYPES_DEFAULT_REQUIRED",
					"OOXML_CONTENT_TYPES_DEFAULT_DUPLICATE",
					"OOXML_CONTENT_TYPES_OVERRIDE_REQUIRED",
					"OOXML_CONTENT_TYPES_OVERRIDE_DUPLICATE",
					"OOXML_CONTENT_TYPES_OVERRIDE_TARGET_MISSING",
					"OOXML_CONTENT_TYPES_PART_UNMAPPED",
					"OOXML_CONTENT_TYPE_MISMATCH",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchContentTypesRootProblem",
					"TestRepairInvariantsCatchContentTypesReadAndParseProblems",
					"TestRepairInvariantsCatchContentTypesPartProblems",
					"TestRepairInvariantsCatchContentTypesCoverageProblems",
					"TestRepairInvariantsCatchKnownContentTypeMismatch",
				},
			},
			{
				ID:       "relationships",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_RELS_READ_ERROR",
					"OOXML_RELS_PARSE_ERROR",
					"OOXML_RELS_ORPHANED",
					"OOXML_RELATIONSHIP_MISSING_ID",
					"OOXML_RELATIONSHIP_DUPLICATE_ID",
					"OOXML_RELATIONSHIP_MISSING_TYPE",
					"OOXML_RELATIONSHIP_MISSING_TARGET",
					"OOXML_RELATIONSHIP_TARGET_MODE",
					"OOXML_RELATIONSHIP_EXTERNAL_MODE_MISSING",
					"OOXML_RELATIONSHIP_TARGET_MISSING",
					"OOXML_RELATIONSHIP_TARGET_CONTENT_TYPE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchMalformedRelationshipPart",
					"TestRepairInvariantsCatchRelationshipPartReadError",
					"TestRepairInvariantsCatchRelationshipClosureProblems",
					"TestRepairInvariantsCatchRelationshipTargetContentTypeMismatch",
					"TestConformanceCommittedPPTXXLSXFixtureManifest",
				},
			},
			{
				ID:       "part-roots",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"XLSX_WORKBOOK_ROOT",
					"XLSX_WORKSHEET_ROOT",
					"XLSX_SHARED_STRINGS_ROOT",
					"XLSX_STYLES_ROOT",
					"XLSX_TABLE_ROOT",
					"XLSX_PIVOT_TABLE_ROOT",
					"XLSX_PIVOT_CACHE_ROOT",
					"XLSX_PIVOT_RECORDS_ROOT",
					"XLSX_CALC_CHAIN_ROOT",
					"PPTX_PRESENTATION_ROOT",
					"PPTX_SLIDE_ROOT",
					"PPTX_SLIDE_LAYOUT_ROOT",
					"PPTX_SLIDE_MASTER_ROOT",
					"XLSX_DRAWING_ROOT",
					"OOXML_CHART_ROOT",
					"OOXML_XML_PARSE_ERROR",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchKnownPartRootMismatches",
					"TestRepairInvariantsCatchKnownPartRootNamespaceMismatches",
					"TestRepairInvariantsCatchHighValuePartRootMismatches",
					"TestRepairInvariantsCatchSlideLayoutAndMasterProblems",
					"TestRepairInvariantsCatchXMLParseErrors",
				},
			},
			{
				ID:       "reference-lists",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"XLSX_WORKBOOK_SHEET_REFERENCE",
					"PPTX_PRESENTATION_REFERENCE",
					"XLSX_WORKSHEET_RELATIONSHIP_REFERENCE",
					"XLSX_WORKSHEET_HYPERLINK_REFERENCE",
					"XLSX_WORKSHEET_PIVOT_REFERENCE",
					"XLSX_WORKSHEET_TABLEPARTS_COUNT",
					"PPTX_SLIDE_LAYOUT_MASTER_REFERENCE",
					"PPTX_SLIDE_MASTER_LAYOUT_REFERENCE",
					"OOXML_CHART_RELATIONSHIP_REFERENCE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchWorkbookSheetReferenceProblems",
					"TestRepairInvariantsCatchWorksheetRelationshipReferenceProblems",
					"TestRepairInvariantsCatchWorksheetHyperlinkReferenceProblems",
					"TestRepairInvariantsAllowWorksheetHyperlinkReferences",
					"TestRepairInvariantsCatchWorksheetTablePartsCountMismatch",
					"TestRepairInvariantsCatchChartRelationshipReferenceProblems",
					"TestRepairInvariantsCatchPresentationReferenceProblems",
					"TestRepairInvariantsCatchSlideLayoutAndMasterProblems",
					"TestRepairInvariantsAllowSlideLayoutAndMasterReferences",
				},
			},
			{
				ID:       "pptx-animations",
				Status:   "covered",
				Families: []string{"pptx"},
				DiagnosticCodes: []string{
					"PPTX_ANIMATION_TARGET_REFERENCE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchSlideAnimationTargetProblems",
					"TestRepairInvariantsAllowSlideAnimationTargets",
				},
			},
			{
				ID:       "drawing-media-references",
				Status:   "covered",
				Families: []string{"docx", "pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_IMAGE_RELATIONSHIP_REFERENCE",
					"OOXML_IMAGE_PAYLOAD",
					"PPTX_MEDIA_RELATIONSHIP_REFERENCE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchDOCXImagePayloadProblems",
					"TestRepairInvariantsAllowDOCXImagePayloadReferences",
					"TestRepairInvariantsCatchDrawingMediaRelationshipProblems",
					"TestRepairInvariantsAllowDrawingMediaRelationshipReferences",
				},
			},
			{
				ID:       "chart-external-data",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_CHART_EXTERNAL_DATA_REFERENCE",
					"OOXML_CHART_EXTERNAL_DATA_EMBEDDED_WORKBOOK_OPEN",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchChartExternalDataRelationshipProblems",
					"TestRepairInvariantsCatchChartExternalDataCorruptEmbeddedWorkbookBytes",
					"TestRepairInvariantsAllowChartExternalDataRelationshipReferences",
				},
			},
			{
				ID:       "chart-axis-references",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_CHART_AXIS_REFERENCE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchChartAxisReferenceProblems",
					"TestRepairInvariantsAllowChartAxisReferences",
				},
			},
			{
				ID:       "chart-series-caches",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_CHART_SERIES_CACHE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchChartSeriesCacheProblems",
					"TestRepairInvariantsAllowChartSeriesCaches",
				},
			},
			{
				ID:       "xlsx-defined-names",
				Status:   "covered",
				Families: []string{"xlsx"},
				DiagnosticCodes: []string{
					"XLSX_DEFINED_NAME_REQUIRED",
					"XLSX_DEFINED_NAME_SCOPE",
					"XLSX_DEFINED_NAME_DUPLICATE",
					"XLSX_DEFINED_NAME_REFERENCE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchWorkbookDefinedNameProblems",
					"TestRepairInvariantsAllowWorkbookDefinedNames",
				},
			},
			{
				ID:       "xlsx-tables",
				Status:   "covered",
				Families: []string{"xlsx"},
				DiagnosticCodes: []string{
					"XLSX_TABLE_DEFINITION",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchTableDefinitionProblems",
					"TestRepairInvariantsAllowTableDefinitions",
				},
			},
			{
				ID:       "xlsx-pivots",
				Status:   "covered",
				Families: []string{"xlsx"},
				DiagnosticCodes: []string{
					"XLSX_WORKBOOK_PIVOT_CACHE_REFERENCE",
					"XLSX_PIVOT_TABLE_DEFINITION",
					"XLSX_PIVOT_CACHE_DEFINITION",
					"XLSX_PIVOT_CACHE_RECORDS_REFERENCE",
					"XLSX_PIVOT_RECORDS_DEFINITION",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchPivotDefinitionProblems",
					"TestRepairInvariantsAllowPivotDefinitions",
				},
			},
			{
				ID:       "schema-order",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"XLSX_WORKSHEET_CHILD_ORDER",
					"XLSX_TABLE_CHILD_ORDER",
					"XLSX_PIVOT_TABLE_CHILD_ORDER",
					"XLSX_PIVOT_CACHE_CHILD_ORDER",
					"PPTX_SLIDE_CHILD_ORDER",
					"PPTX_SLIDE_LAYOUT_CHILD_ORDER",
					"PPTX_SLIDE_MASTER_CHILD_ORDER",
					"XLSX_DRAWING_ANCHOR_ORDER",
					"XLSX_DRAWING_ANCHOR_REQUIRED",
					"OOXML_CHARTSPACE_CHILD_ORDER",
					"OOXML_CHART_CHILD_ORDER",
					"OOXML_PLOTAREA_CHILD_ORDER",
					"OOXML_BARCHART_CHILD_ORDER",
					"OOXML_LINECHART_CHILD_ORDER",
					"OOXML_AREACHART_CHILD_ORDER",
					"OOXML_PIECHART_CHILD_ORDER",
					"OOXML_SCATTERCHART_CHILD_ORDER",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchWorksheetChildOrder",
					"TestRepairInvariantsCatchTableDefinitionProblems",
					"TestRepairInvariantsAllowTableDefinitions",
					"TestRepairInvariantsCatchPivotDefinitionProblems",
					"TestRepairInvariantsAllowPivotDefinitions",
					"TestRepairInvariantsCatchSlideChildOrder",
					"TestRepairInvariantsCatchSlideLayoutAndMasterProblems",
					"TestRepairInvariantsAllowSlideLayoutAndMasterReferences",
					"TestRepairInvariantsCatchDrawingAnchorOrderAndRequiredShape",
					"TestRepairInvariantsCatchChartPartOrder",
					"TestRepairInvariantsCatchNestedChartPartOrder",
				},
			},
			{
				ID:       "xlsx-counts-styles",
				Status:   "covered",
				Families: []string{"xlsx"},
				DiagnosticCodes: []string{
					"XLSX_SHARED_STRINGS_COUNTS",
					"XLSX_STYLES_COUNT_MISMATCH",
					"XLSX_CELL_STYLE_REFERENCE",
					"XLSX_CELL_STYLE_INDEX_OUT_OF_RANGE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchSharedStringsCountProblems",
					"TestRepairInvariantsAllowSharedStringsOmittedCountsAndReuseCount",
					"TestRepairInvariantsCatchStylesCountProblems",
					"TestRepairInvariantsCatchWorksheetStyleReferenceProblems",
					"TestRepairInvariantsCatchWorksheetStyleReferenceWithoutStyles",
				},
			},
			{
				ID:       "xlsx-calc-chain",
				Status:   "covered",
				Families: []string{"xlsx"},
				DiagnosticCodes: []string{
					"XLSX_CALC_CHAIN_REFERENCE",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchCalcChainReferenceProblems",
					"TestRepairInvariantsAllowCalcChainReferences",
				},
			},
			{
				ID:       "zip-metadata",
				Status:   "covered",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_ZIP_TIMESTAMP_INVALID",
				},
				Evidence: []string{
					"TestRepairInvariantsCatchInvalidZipTimestamp",
				},
			},
			{
				ID:       "local-office-open",
				Status:   "optional-local-oracle",
				Families: []string{"pptx", "xlsx"},
				DiagnosticCodes: []string{
					"OOXML_OFFICE_CHECK_SKIPPED",
					"OOXML_OFFICE_CHECK_FAILED",
				},
				Evidence: []string{
					"TestConformanceOfficeOpenGoldenSummary",
					"TestCheckPackageOfficeCheckFailureFailsReport",
					"internal/cli.TestConformanceCheckOfficeCheckJSON",
					"internal/cli.TestGeneratedRepairConformanceOfficeOpenIfAvailable",
					"pkg/officecheck",
				},
			},
			{
				ID:       "real-microsoft-office",
				Status:   "external-oracle",
				Families: []string{"pptx", "xlsx"},
				Evidence: []string{
					"OFFICE_REPAIR_CONFORMANCE_GOAL.md",
				},
			},
		},
		FixtureSets: []FixtureSetCoverage{
			{
				Name:     "committed-pptx-xlsx-fixture-manifest",
				Status:   "implemented",
				Families: []string{"pptx", "xlsx"},
				Evidence: []string{
					"TestConformanceCommittedPPTXXLSXFixtureManifest",
					"testdata/pptx/*",
					"testdata/xlsx/*",
				},
			},
			{
				Name:     "repair-conformance-golden-summary",
				Status:   "implemented",
				Families: []string{"pptx", "xlsx"},
				Evidence: []string{
					"TestConformanceGoldenSummary",
					"testdata/golden/repair-conformance-summary.json",
				},
			},
			{
				Name:     "office-open-golden-summary",
				Status:   "implemented",
				Families: []string{"pptx", "xlsx"},
				Evidence: []string{
					"TestConformanceOfficeOpenGoldenSummary",
					"testdata/golden/repair-conformance-office-open-summary.json",
				},
			},
			{
				Name:     "generated-output-repair-conformance",
				Status:   "implemented",
				Families: []string{"pptx", "xlsx"},
				Evidence: []string{
					"internal/cli.TestGeneratedRepairConformanceGolden",
					"internal/cli.TestGeneratedRepairConformanceOfficeOpenIfAvailable",
					"testdata/golden/generated-repair-conformance-summary.json",
					"testdata/golden/generated-repair-proof-bundle.json",
				},
			},
			{
				Name:     "windows-office-edit-smoke-release-gate",
				Status:   "implemented",
				Families: []string{"docx", "pptx", "xlsx"},
				Evidence: []string{
					"Makefile",
					"tools/windows-office-edit-smoke.ps1",
					"docs/windows-office-oracle.md",
				},
			},
		},
		KnownLimitations: []string{
			"This is a repair-focused harness for practical generated DOCX/PPTX/XLSX files, not an exhaustive ISO/IEC 29500 conformance suite.",
			"Static repair and generated-output golden summaries remain PPTX/XLSX-only; DOCX coverage is targeted invariants plus Windows edit-smoke release gates.",
			"LibreOffice/soffice open checks are useful local evidence but are not proof that Microsoft Office will avoid a repair prompt.",
			"Real Microsoft Office repair prompts on Windows/macOS remain the final external oracle for user-facing compatibility.",
			"Macro code is not executed or compiled by this harness.",
		},
	}
}
