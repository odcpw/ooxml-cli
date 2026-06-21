// Package capabilities holds the hand-authored, pure metadata that enriches
// `ooxml capabilities` for agents: runnable per-command examples, common-error
// notes, and an object-kind taxonomy used to build a reverse lookup
// (which commands target a given object kind).
//
// Everything here is deterministic data plus pure functions over it, so it is
// table-testable without a cobra tree. The CLI layer (internal/cli) consumes
// this metadata: it derives cobra's Command.Example field from Examples (only
// where a command has no hand-authored example), surfaces Examples/CommonErrors
// in the JSON contract, and builds the object-kind index from TargetObjectKinds.
package capabilities

import "sort"

// MetadataSchemaVersion is the stable contract version of the enrichment data
// exposed through `ooxml capabilities --json`. Bump when the emitted agent
// contract shape changes: examples/common errors, object-kind index, handles, or
// command/flag metadata.
const MetadataSchemaVersion = "ooxml-cli.agent-capabilities.v4"

// Example is one runnable invocation of a command, with a short description and
// an optional note about what it prints. Command is a full `ooxml ...` string.
type Example struct {
	Command        string `json:"command"`
	Description    string `json:"description"`
	ExpectedOutput string `json:"expectedOutput,omitempty"`
}

// CommonError is a hand-authored note pairing a recognizable failure pattern
// (substring of a message or exit-code name) with an actionable solution.
type CommonError struct {
	Pattern  string `json:"pattern"`
	Solution string `json:"solution"`
}

// CommandMetadata is the enrichment for a single command, keyed elsewhere by the
// command path (e.g. "ooxml pptx shapes show").
type CommandMetadata struct {
	Examples          []Example
	CommonErrors      []CommonError
	TargetObjectKinds []string
}

// ObjectKinds is the object-kind taxonomy this CLI reasons about. It is the
// closed vocabulary used by --for and by TargetObjectKinds. Keep it sorted.
var ObjectKinds = []string{
	"cell",
	"chart",
	"comment",
	"conditional-format",
	"data-validation",
	"footer",
	"header",
	"hyperlink",
	"image",
	"layout",
	"master",
	"module",
	"name",
	"package",
	"paragraph",
	"pivot",
	"placeholder",
	"range",
	"shape",
	"sheet",
	"slide",
	"style",
	"table",
}

// IsObjectKind reports whether kind is part of the taxonomy.
func IsObjectKind(kind string) bool {
	for _, k := range ObjectKinds {
		if k == kind {
			return true
		}
	}
	return false
}

// commandMetadata is the hand-authored source of truth. Keys are full command
// paths exactly as cobra reports them (`ooxml <group> <sub>`). Only the
// highest-use commands are covered; absence means "no enrichment", which the CLI
// renders as empty arrays (never null).
var commandMetadata = map[string]CommandMetadata{
	"ooxml find": {
		Examples: []Example{
			{
				Command:        "ooxml --json find \"Acme Corp\" deck.pptx",
				Description:    "Search a package semantically and return stable selectors plus pre-filled mutation commands.",
				ExpectedOutput: "JSON hits with object kind, selector/handle when available, and mutation command templates.",
			},
			{
				Command:        "ooxml --json find \"Old Name\" deck.pptx --replace \"New Name\" --to-ops > ops.json",
				Description:    "Compose a transactional apply plan from safe search hits.",
				ExpectedOutput: "An apply ops.json document suitable for 'ooxml apply'.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "find supports PPTX/PPTM, XLSX/XLSM, and DOCX packages; inspect the file first if detection is unclear."},
			{Pattern: "no scoped mutation", Solution: "Use the reported selector/handle manually, or narrow the query so find can emit a safe scoped op."},
		},
		TargetObjectKinds: []string{"package", "shape", "cell", "paragraph"},
	},
	"ooxml apply": {
		Examples: []Example{
			{
				Command:        "ooxml --json apply workbook.xlsx --ops ops.json --out edited.xlsx",
				Description:    "Run an ordered mutation batch all-or-nothing with one final validation.",
				ExpectedOutput: "JSON result with per-op readbacks, output path, and validate command.",
			},
			{
				Command:        "ooxml --json apply deck.pptx --ops ops.json --dry-run",
				Description:    "Inspect the deterministic subprocess argv plan without writing a file.",
				ExpectedOutput: "JSON plan with dryRun=true and resolved argv entries.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid ops JSON", Solution: "Use an array of {\"command\": \"xlsx cells set\", \"args\": {...}} objects; generate it with 'find --to-ops' when possible."},
			{Pattern: "owned by the apply/serve/MCP session", Solution: "Remove --out/--in-place/--dry-run/--backup/no-validate from op args and set them only on the outer apply command."},
			{Pattern: "HANDLE_STALE", Solution: "Refresh selectors/handles with inspect/find, then regenerate the ops file so the batch does not retarget stale objects."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml inspect": {
		Examples: []Example{
			{
				Command:        "ooxml --json inspect deck.pptx",
				Description:    "Identify the package type and high-level structure before editing.",
				ExpectedOutput: "JSON describing the package type, parts, and counts.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "file_not_found", Solution: "Check the path; inspect needs an existing .pptx/.xlsx/.docx file."},
			{Pattern: "unsupported_type", Solution: "Confirm the file is an OOXML package; inspect does not read legacy .ppt/.xls/.doc."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml validate": {
		Examples: []Example{
			{
				Command:        "ooxml validate --strict edited.pptx",
				Description:    "Validate a package after mutating it, before handing it to a user.",
				ExpectedOutput: "Validation report; exit 5 (validation_failed) when blocking issues exist.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "validation_failed", Solution: "Read the reported issues; re-run without --strict to see warnings vs. blocking errors."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml conformance check": {
		Examples: []Example{
			{
				Command:        "ooxml --json conformance check edited.xlsx",
				Description:    "Run repo validation plus Office-repair-sensitive XML/package invariants.",
				ExpectedOutput: "JSON conformance report with package-open, repo-validation, repair-invariants, and rollup status.",
			},
			{
				Command:        "ooxml --json conformance check edited.pptx --office-check --office-check-out-dir office-open-proof",
				Description:    "Add optional local LibreOffice/soffice open evidence and retain the converted proof artifact when the local engine is usable.",
				ExpectedOutput: "JSON conformance report including an office-open check, retained output path, or a skipped missing-engine result.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "validation_failed", Solution: "Fix the reported repo-validation or repair-invariants diagnostics before handing the file to a user."},
			{Pattern: "OOXML_OFFICE_CHECK_FAILED", Solution: "If a known-good baseline also fails, run 'ooxml doctor'; otherwise treat this as a consumer-open regression."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml conformance coverage": {
		Examples: []Example{
			{
				Command:        "ooxml --json conformance coverage",
				Description:    "Show which Office-repair conformance classes are covered by local checks, fixtures, and goldens.",
				ExpectedOutput: "JSON coverage/provenance report listing harness stages, repair classes, fixture sets, and known limitations.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "external-oracle", Solution: "Run real Microsoft Office open checks on Windows/macOS when this report marks a class as external-oracle."},
			{Pattern: "local-engine-optional", Solution: "Run 'ooxml doctor' and install LibreOffice/soffice if local open-check evidence is needed."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml verify": {
		Examples: []Example{
			{
				Command:        "ooxml --json verify edited.pptx --baseline original.pptx",
				Description:    "Validate, optionally render, and semantic-diff an edited package against a baseline.",
				ExpectedOutput: "JSON verification envelope with validation, render, and diff status.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "validation_failed", Solution: "Run the emitted validate command and fix structural diagnostics before comparing visual output."},
			{Pattern: "render_failed", Solution: "Run 'ooxml doctor' to check LibreOffice/render dependencies, or verify without render when visual proof is unavailable."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml doctor": {
		Examples: []Example{
			{
				Command:        "ooxml --json doctor",
				Description:    "Check stale binary, render engine, fonts, temp dirs, workdir, Go/.NET, and Office proof readiness.",
				ExpectedOutput: "JSON report with findings, severity, and remediation commands.",
			},
			{
				Command:        "ooxml doctor health",
				Description:    "Return a compact health summary for agent preflight checks.",
				ExpectedOutput: "Human-readable health status; exit 1 when findings are present.",
			},
			{
				Command:        "ooxml --json doctor capabilities",
				Description:    "Discover the proof ladder and release gates for strict validation, repair conformance, Open XML SDK schema validation, and Microsoft Office COM open proof.",
				ExpectedOutput: "JSON contract with checks, proofLevels, releaseGates, exit codes, and remediation notes.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "stale_binary", Solution: "Run the emitted go install command so the on-PATH ooxml matches the checkout."},
			{Pattern: "missing_render_engine", Solution: "Install LibreOffice/soffice or skip render-dependent visual checks."},
			{Pattern: "missing_openxml_sdk", Solution: "Install a .NET SDK so check-release-fast/check-release-slow can run Open XML SDK schema validation."},
			{Pattern: "missing_microsoft_office_com", Solution: "Use check-release-fast for schema/conformance proof, or install desktop Office before running check-release-slow."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml serve": {
		Examples: []Example{
			{
				Command:        "ooxml serve",
				Description:    "Start the newline-delimited JSON-RPC session engine and feature-detect methods/capabilities.",
				ExpectedOutput: "One JSON-RPC response containing methods, package types, and the capabilities contract.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "must specify exactly one of out, in-place, or dry-run", Solution: "Set the write mode on the open request, not on individual op args."},
			{Pattern: "inspect command is not allowed", Solution: "Use 'op' for mutations and reserve 'inspect' for read-only commands without artifact-writing flags."},
			{Pattern: "owned by the apply/serve/MCP session", Solution: "Remove session-owned mutation flags from op args and set them on open/commit/session options instead."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml mcp": {
		Examples: []Example{
			{
				Command:        "ooxml mcp",
				Description:    "Expose the OOXML session engine as MCP tools/resources for agents that support MCP.",
				ExpectedOutput: "MCP server over stdio with capabilities and command resources.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unknown tool", Solution: "Call tools/list or read resource://capabilities to discover the supported generic session tools."},
			{Pattern: "invalid_args", Solution: "Read resource://command/{path} for the exact command flags and dashless JSON arg names."},
			{Pattern: "next_actions", Solution: "Follow the structured next_actions returned in MCP error data before retrying."},
		},
		TargetObjectKinds: []string{"package"},
	},
	"ooxml pptx slides list": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx slides list deck.pptx",
				Description:    "Enumerate slides with their indexes before targeting one.",
				ExpectedOutput: "JSON array of slides with slide numbers and titles.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "slides commands require a .pptx/.pptm package."},
		},
		TargetObjectKinds: []string{"slide"},
	},
	"ooxml pptx slides show": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx slides show deck.pptx --slide 1 --include-text --include-bounds",
				Description:    "Inspect one slide with text content and geometry.",
				ExpectedOutput: "JSON slide record including shapes, text, and bounds.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Use 'ooxml pptx slides list <file>' to find valid slide numbers."},
		},
		TargetObjectKinds: []string{"slide", "shape"},
	},
	"ooxml pptx shapes show": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx shapes show deck.pptx --slide 1",
				Description:    "List shapes on a slide with their primary selectors.",
				ExpectedOutput: "JSON array of shapes with primarySelector and bounds.",
			},
			{
				Command:        "ooxml --json pptx shapes show deck.pptx --slide 1 --include-text --include-bounds",
				Description:    "Include text content and geometry for each shape.",
				ExpectedOutput: "Extended shape records with text and bounds.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml pptx slides list <file>' to find valid slide numbers."},
		},
		TargetObjectKinds: []string{"shape", "placeholder"},
	},
	"ooxml pptx replace text": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx replace text deck.pptx --slide 1 --target title --text NEW --out edited.pptx",
				Description:    "Replace the text of a targeted shape on a slide.",
				ExpectedOutput: "JSON summary of the replacement; writes edited.pptx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Use 'ooxml pptx shapes show <file> --slide N' to discover valid targets/selectors."},
		},
		TargetObjectKinds: []string{"shape", "placeholder", "paragraph"},
	},
	"ooxml pptx layouts list": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx layouts list deck.pptx",
				Description:    "List slide layouts with pasteable layout selectors and placeholder summaries.",
				ExpectedOutput: "JSON layout records with primarySelector, selectors, part URI, master id, and placeholders.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "layout commands require a .pptx/.pptm package."},
		},
		TargetObjectKinds: []string{"layout", "placeholder"},
	},
	"ooxml pptx layouts show": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx layouts show deck.pptx --layout 1",
				Description:    "Inspect one layout by selector before editing placeholders or geometry.",
				ExpectedOutput: "JSON layout detail with placeholder records and theme/default-style context.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "layout not found", Solution: "Run 'ooxml --json pptx layouts list <file>' and use a listed primarySelector or selector."},
		},
		TargetObjectKinds: []string{"layout", "placeholder"},
	},
	"ooxml pptx layouts rename": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx layouts rename deck.pptx --layout 1 --name \"Title Grid\" --out edited.pptx",
				Description:    "Rename a layout using a selector from layouts list.",
				ExpectedOutput: "JSON mutation result with layout readback commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "layout not found", Solution: "Refresh layout selectors with 'ooxml --json pptx layouts list <file>'."},
		},
		TargetObjectKinds: []string{"layout"},
	},
	"ooxml pptx layouts set-bounds": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx layouts set-bounds deck.pptx --layout 1 --target title --bounds 0,0,4000000,600000 --out edited.pptx",
				Description:    "Move or resize one layout placeholder/shape with explicit EMU bounds.",
				ExpectedOutput: "JSON mutation result with old/new bounds and layout readback commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target not found", Solution: "Run 'ooxml --json pptx layouts show <file> --layout <selector>' to discover placeholder targets."},
		},
		TargetObjectKinds: []string{"layout", "placeholder", "shape"},
	},
	"ooxml pptx layouts add-placeholder": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx layouts add-placeholder deck.pptx --layout 1 --type text --bounds 0,1000000,4000000,2000000 --out edited.pptx",
				Description:    "Add a practical placeholder to a layout.",
				ExpectedOutput: "JSON mutation result with new placeholder readback.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "layout not found", Solution: "Run 'ooxml --json pptx layouts list <file>' before authoring layout placeholders."},
		},
		TargetObjectKinds: []string{"layout", "placeholder"},
	},
	"ooxml pptx masters list": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx masters list deck.pptx",
				Description:    "List slide masters with pasteable master selectors and layout counts.",
				ExpectedOutput: "JSON master records with primarySelector, selectors, master URI, theme URI, and layout count.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "master commands require a .pptx/.pptm package."},
		},
		TargetObjectKinds: []string{"master", "layout"},
	},
	"ooxml pptx masters show": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx masters show deck.pptx --master 1",
				Description:    "Inspect one slide master before editing defaults or placeholders.",
				ExpectedOutput: "JSON master detail with layouts, placeholders, theme, and style summaries.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "master not found", Solution: "Run 'ooxml --json pptx masters list <file>' and use a listed primarySelector."},
		},
		TargetObjectKinds: []string{"master", "layout", "placeholder"},
	},
	"ooxml pptx masters add-placeholder": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx masters add-placeholder deck.pptx --master 1 --type text --bounds 0,1000000,4000000,2000000 --out edited.pptx",
				Description:    "Add a practical placeholder to a master.",
				ExpectedOutput: "JSON mutation result with master/layout readback commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "master not found", Solution: "Run 'ooxml --json pptx masters list <file>' before authoring master placeholders."},
		},
		TargetObjectKinds: []string{"master", "placeholder"},
	},
	"ooxml pptx charts show": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts show deck.pptx --slide 1 --chart chart:1",
				Description:    "Inspect a chart's series and categories before updating it.",
				ExpectedOutput: "JSON chart record with series, categories, and point counts.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml pptx charts list <file>' to find valid chart selectors."},
		},
		TargetObjectKinds: []string{"chart"},
	},
	"ooxml pptx charts update-data": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts update-data deck.pptx --slide 1 --chart chart:1 --series 1 --values-json '[\"150\",\"175\"]' --expect-point-count 2 --out edited.pptx",
				Description:    "Replace a chart series' values with a guard on the point count.",
				ExpectedOutput: "JSON summary of the update; writes edited.pptx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml pptx charts list <file>' then 'charts show' to confirm series and counts."},
			{Pattern: "diff_threshold", Solution: "Re-run 'charts show' to read current values; align --expect-point-count/--expect-values-hash."},
		},
		TargetObjectKinds: []string{"chart"},
	},
	"ooxml pptx charts create": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts create deck.pptx --slide 1 --type column --values-json '[[\"\",\"Q1\",\"Q2\"],[\"Revenue\",120,150]]' --title \"Revenue\" --out edited.pptx",
				Description:    "Author a new slide chart from inline matrix data.",
				ExpectedOutput: "JSON create result with chart selector, shape id, chart show/list commands, validate command, and render command.",
			},
			{
				Command:        "ooxml --json pptx charts create deck.pptx --slide 1 --type line --source-file data.xlsx --source-sheet Sheet1 --source-range A1:C5 --expect-source-range A1:C5 --embed-workbook --out edited.pptx",
				Description:    "Create a slide chart from a workbook range with a stale-source guard and embedded workbook.",
				ExpectedOutput: "JSON create result with source workbook metadata and follow-up chart readback commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--type is required", Solution: "Choose a common chart type: bar, column, line, area, pie, or scatter."},
			{Pattern: "source range mismatch", Solution: "Re-read the workbook range and update --expect-source-range before creating the chart."},
			{Pattern: "slide", Solution: "Run 'ooxml --json pptx slides list <file>' and retry with a valid --slide number."},
		},
		TargetObjectKinds: []string{"chart", "slide", "range"},
	},
	"ooxml pptx charts set-title": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts set-title deck.pptx --slide 1 --chart chart:1 --title \"Q4 Revenue\" --font-family Aptos --font-bold=true --out edited.pptx",
				Description:    "Set a PPTX chart title and basic title font styling.",
				ExpectedOutput: "JSON chart style readback with the updated title and follow-up validate/render commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json pptx charts list <file>' to refresh chart selectors."},
			{Pattern: "expect-title", Solution: "Re-read the current title with 'pptx charts show' and retry with the current --expect-title guard."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx charts set-legend": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts set-legend deck.pptx --slide 1 --chart chart:1 --position bottom --overlay=false --out edited.pptx",
				Description:    "Set or create a PPTX chart legend using common business positions.",
				ExpectedOutput: "JSON chart style readback with legend position and overlay state.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "set-legend requires", Solution: "Pass --position right|left|top|bottom|none and/or --overlay=true|false."},
			{Pattern: "expect-position", Solution: "Refresh the current legend with 'pptx charts show' before retrying a stale guarded edit."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx charts set-axis": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts set-axis deck.pptx --slide 1 --chart chart:1 --axis value --title \"Revenue\" --number-format '$#,##0' --major-gridlines=true --out edited.pptx",
				Description:    "Set practical value-axis title, number format, and gridline styling.",
				ExpectedOutput: "JSON chart style readback with updated axis metadata.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "List charts first; use the chart selector and axis kind shown by 'pptx charts show'."},
			{Pattern: "expect-axis", Solution: "Refresh axis readback before retrying stale guarded axis edits."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx charts convert-type": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts convert-type deck.pptx --slide 1 --chart chart:1 --to line --expect-type column --out edited.pptx",
				Description:    "Convert an existing slide chart among common chart families with a current-type guard.",
				ExpectedOutput: "JSON chart style/readback result with previousType, newType, validate command, and render command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--to is required", Solution: "Choose bar, column, line, area, pie, or scatter."},
			{Pattern: "expect-type", Solution: "Refresh the chart with 'pptx charts show' and retry with the current type."},
			{Pattern: "pie", Solution: "Pie conversion requires a compatible single-series shape; use line/column/bar for multi-series charts."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx charts set-plot-area-fill": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts set-plot-area-fill deck.pptx --slide 1 --chart chart:1 --fill-color '#F3F6FA' --out edited.pptx",
				Description:    "Set or clear the plot-area background fill of a slide chart.",
				ExpectedOutput: "JSON chart style readback showing previousFill and newFill.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--fill-color is required", Solution: "Pass a #RRGGBB color or 'none' to clear the fill."},
			{Pattern: "expect-fill", Solution: "Refresh chart style readback and retry with the current fill guard (#RRGGBB, scheme:name, or none)."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx charts set-chart-area-fill": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts set-chart-area-fill deck.pptx --slide 1 --chart chart:1 --fill-color '#FFFFFF' --out edited.pptx",
				Description:    "Set or clear the chart-area/background fill of a slide chart.",
				ExpectedOutput: "JSON chart style readback showing previousFill and newFill.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--fill-color is required", Solution: "Pass a #RRGGBB color or 'none' to clear the fill."},
			{Pattern: "expect-fill", Solution: "Refresh chart style readback and retry with the current fill guard (#RRGGBB, scheme:name, or none)."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx charts copy-style": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts copy-style target.pptx --slide 1 --chart chart:1 --from template.pptx --from-slide 1 --from-chart chart:1 --expect-series-count 3 --out branded.pptx",
				Description:    "Copy practical chart style from a template chart onto a target slide chart without copying chart data.",
				ExpectedOutput: "JSON chart style readback with applied style facets and validate/render commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--from", Solution: "Provide a template PPTX/POTX that contains the chart style to copy."},
			{Pattern: "expect-series-count", Solution: "Refresh the target chart series count before applying template style."},
			{Pattern: "target_not_found", Solution: "Run 'pptx charts list' on both target and template files to refresh selectors."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx charts set-series-style": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx charts set-series-style deck.pptx --slide 1 --chart chart:1 --series 1 --fill-color '#1F77B4' --line-width-pt 2 --out edited.pptx",
				Description:    "Style a PPTX chart series with common corporate color and line controls.",
				ExpectedOutput: "JSON chart style readback for the changed series.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json pptx charts list <file>' and confirm the chart/series number with 'charts show'."},
			{Pattern: "expect-series-count", Solution: "Refresh the series count before retrying a stale guarded series style edit."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml pptx animations add": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx animations add deck.pptx --slide 1 --shape shape:4 --effect appear --out edited.pptx",
				Description:    "Add a simple appear animation to a slide shape.",
				ExpectedOutput: "JSON animation readback with generated list/validate/render follow-up commands.",
			},
			{
				Command:        "ooxml --json pptx animations add deck.pptx --slide 1 --shape shape:5 --effect fade --by-paragraph --expect-paragraph-count 4 --out edited.pptx",
				Description:    "Add a paragraph-by-paragraph animation with a stale paragraph-count guard.",
				ExpectedOutput: "JSON animation readback showing the paragraph build.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json pptx shapes show <file> --slide N --include-text' and retry with a listed shape selector or handle."},
			{Pattern: "expect-paragraph-count", Solution: "Refresh the shape text/paragraph count before applying a by-paragraph animation."},
		},
		TargetObjectKinds: []string{"slide", "shape"},
	},
	"ooxml pptx comments list": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx comments list deck.pptx --slide 1",
				Description:    "List comments anchored to a slide.",
				ExpectedOutput: "JSON comment records with author, text, and ids for edit/remove.",
			},
		},
		TargetObjectKinds: []string{"comment", "slide"},
	},
	"ooxml pptx comments add": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx comments add deck.pptx --slide 1 --author \"Analyst\" --text \"Check this number\" --out edited.pptx",
				Description:    "Add a legacy slide comment.",
				ExpectedOutput: "JSON mutation result plus comments-list, validate, and render follow-up commands.",
			},
		},
		TargetObjectKinds: []string{"comment", "slide"},
	},
	"ooxml pptx comments edit": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx comments edit deck.pptx --slide 1 --comment-id 0 --text \"Updated note\" --out edited.pptx",
				Description:    "Edit a slide comment's text, author, or date.",
				ExpectedOutput: "JSON mutation result plus comments-list, validate, and render follow-up commands.",
			},
		},
		TargetObjectKinds: []string{"comment", "slide"},
	},
	"ooxml pptx comments remove": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx comments remove deck.pptx --slide 1 --comment-id 0 --out edited.pptx",
				Description:    "Remove a slide comment by id.",
				ExpectedOutput: "JSON mutation result plus comments-list, validate, and render follow-up commands.",
			},
		},
		TargetObjectKinds: []string{"comment", "slide"},
	},
	"ooxml pptx extract images": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx extract images deck.pptx --slide 1 --out extracted-images",
				Description:    "Extract slide images and write a manifest.",
				ExpectedOutput: "JSON extraction manifest and image files under the output directory.",
			},
		},
		TargetObjectKinds: []string{"image", "slide"},
	},
	"ooxml pptx place image": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx place image deck.pptx --slide 1 --image logo.png --x 914400 --y 914400 --cx 1828800 --cy 914400 --out edited.pptx",
				Description:    "Place an image at explicit EMU coordinates on a slide.",
				ExpectedOutput: "JSON placement result with the created shape id and validate/render follow-up commands.",
			},
		},
		TargetObjectKinds: []string{"image", "slide", "shape"},
	},
	"ooxml pptx replace images": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx replace images deck.pptx --slide 1 --target shape:4 --image replacement.png --out edited.pptx",
				Description:    "Replace an existing picture shape with a new image.",
				ExpectedOutput: "JSON replacement result with image relationship metadata and validate/render follow-up commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json pptx shapes show <file> --slide N --include-bounds' and retry with a listed picture shape selector or handle."},
		},
		TargetObjectKinds: []string{"image", "shape", "slide"},
	},
	"ooxml pptx clone-slide": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx clone-slide deck.pptx --slide 1 --out edited.pptx",
				Description:    "Duplicate a slide, appending the copy to the deck.",
				ExpectedOutput: "JSON summary with the new slide number; writes edited.pptx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Use 'ooxml pptx slides list <file>' to find a valid source slide number."},
		},
		TargetObjectKinds: []string{"slide"},
	},
	"ooxml xlsx sheets list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx sheets list workbook.xlsx",
				Description:    "Enumerate worksheets before reading or editing cells.",
				ExpectedOutput: "JSON array of sheet names and indexes.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "sheets commands require a .xlsx/.xlsm workbook."},
		},
		TargetObjectKinds: []string{"sheet"},
	},
	"ooxml xlsx sheets show": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx sheets show workbook.xlsx --sheet Sheet1",
				Description:    "Inspect a worksheet's used range and dimensions.",
				ExpectedOutput: "JSON sheet record with used range and metadata.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml xlsx sheets list <file>' to find valid sheet names."},
		},
		TargetObjectKinds: []string{"sheet"},
	},
	"ooxml xlsx cells set": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx cells set workbook.xlsx --sheet Sheet1 --cell B2 --value '42' --out edited.xlsx",
				Description:    "Write a value into a single cell.",
				ExpectedOutput: "JSON summary of the write; writes edited.xlsx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Confirm the sheet with 'ooxml xlsx sheets list <file>'; A1-style cell refs are required."},
		},
		TargetObjectKinds: []string{"cell", "sheet"},
	},
	"ooxml xlsx cells clear": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx cells clear workbook.xlsx --sheet Sheet1 --range B2:B10 --out edited.xlsx",
				Description:    "Clear cell contents while preserving existing cell formatting.",
				ExpectedOutput: "JSON summary with cleared refs and range readback.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "HANDLE_STALE", Solution: "Refresh the cell handle with 'xlsx cells extract' after row/column edits before clearing."},
		},
		TargetObjectKinds: []string{"cell", "range", "sheet"},
	},
	"ooxml xlsx ranges export": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:C5 --include-types",
				Description:    "Export a cell range with value types for downstream binding.",
				ExpectedOutput: "JSON grid of cell values (and types with --include-types).",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Verify the sheet name and that the A1:C5 range is within the used range."},
		},
		TargetObjectKinds: []string{"range", "cell", "sheet"},
	},
	"ooxml xlsx ranges set-format": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx ranges set-format workbook.xlsx --sheet Sheet1 --range B2:B20 --preset currency --out edited.xlsx",
				Description:    "Apply a number-format preset across a range.",
				ExpectedOutput: "JSON summary of the formatting change; writes edited.xlsx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid_args", Solution: "List valid presets in the command help; or pass an explicit format code instead of --preset."},
		},
		TargetObjectKinds: []string{"range", "style", "sheet"},
	},
	"ooxml xlsx ranges set-style": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx ranges set-style workbook.xlsx --sheet Sheet1 --range A1:D1 --font-bold=true --fill-color '#D9EAF7' --alignment-wrap-text --out edited.xlsx",
				Description:    "Apply common visual styling to a worksheet range.",
				ExpectedOutput: "JSON mutation result with styled range readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json xlsx sheets list <file>' and confirm the sheet/range before styling."},
			{Pattern: "invalid_args", Solution: "Use hex colors such as '#D9EAF7' and ordinary A1 ranges such as A1:D1."},
		},
		TargetObjectKinds: []string{"range", "style", "sheet"},
	},
	"ooxml xlsx names add": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx names add workbook.xlsx --name SalesData --sheet Sheet1 --range A1:C5 --out edited.xlsx",
				Description:    "Define a workbook-scoped named range.",
				ExpectedOutput: "JSON summary of the new name; writes edited.xlsx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid_args", Solution: "Names must be unique; run 'ooxml xlsx names list <file>' to check for collisions."},
		},
		TargetObjectKinds: []string{"name", "range", "sheet"},
	},
	"ooxml xlsx names list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx names list workbook.xlsx",
				Description:    "Enumerate defined names and their references.",
				ExpectedOutput: "JSON array of names with scope and refersTo references.",
			},
		},
		TargetObjectKinds: []string{"name"},
	},
	"ooxml xlsx names show": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx names show workbook.xlsx --name name:SalesData",
				Description:    "Show one defined name by published selector or legacy name.",
				ExpectedOutput: "JSON defined-name record with scope, ref, selectors, and handles.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "defined name", Solution: "Run 'ooxml --json xlsx names list <file>' and retry with a listed primarySelector such as name:SalesData."},
		},
		TargetObjectKinds: []string{"name", "range", "sheet"},
	},
	"ooxml xlsx names update": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx names update workbook.xlsx --name name:SalesData --sheet Sheet1 --range A1:D20 --expect-ref \"'Sheet1'!$A$1:$C$5\" --out edited.xlsx",
				Description:    "Update a defined name's reference with a stale-ref guard.",
				ExpectedOutput: "JSON mutation result with updated defined-name readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "defined name", Solution: "Refresh selectors and current refs with 'ooxml --json xlsx names list <file>' before retrying."},
			{Pattern: "defined name ref mismatch", Solution: "Refresh the current ref and retry with the current --expect-ref, or omit the guard only when intentionally overwriting."},
		},
		TargetObjectKinds: []string{"name", "range", "sheet"},
	},
	"ooxml xlsx names rename": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx names rename workbook.xlsx --name name:SalesData --new-name SalesDataFY26 --expect-ref \"'Sheet1'!$A$1:$C$5\" --out edited.xlsx",
				Description:    "Rename a defined name while preserving scope and reference.",
				ExpectedOutput: "JSON mutation result with renamed defined-name readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "defined name", Solution: "Run 'ooxml --json xlsx names list <file>' to discover valid selectors and existing names."},
			{Pattern: "defined name ref mismatch", Solution: "Refresh the current ref before retrying a guarded rename."},
		},
		TargetObjectKinds: []string{"name", "range", "sheet"},
	},
	"ooxml xlsx names delete": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx names delete workbook.xlsx --name name:SalesData --expect-ref \"'Sheet1'!$A$1:$C$5\" --out edited.xlsx",
				Description:    "Delete a defined name with a stale-ref guard.",
				ExpectedOutput: "JSON mutation result with deleted-name summary and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "defined name", Solution: "Run 'ooxml --json xlsx names list <file>' to discover valid selectors."},
			{Pattern: "defined name ref mismatch", Solution: "Refresh the current ref before retrying a guarded delete."},
		},
		TargetObjectKinds: []string{"name", "range", "sheet"},
	},
	"ooxml xlsx data-validations list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx data-validations list workbook.xlsx --sheet Sheet1",
				Description:    "List worksheet data-validation rules and their target ranges.",
				ExpectedOutput: "JSON data-validation records with sqref/range and validation settings.",
			},
		},
		TargetObjectKinds: []string{"data-validation", "range", "sheet"},
	},
	"ooxml xlsx data-validations show": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx data-validations show workbook.xlsx --sheet Sheet1 --range A2:A20",
				Description:    "Show the validation rule that targets a specific range.",
				ExpectedOutput: "JSON data-validation record for the requested sqref.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "no data validation", Solution: "Run 'ooxml --json xlsx data-validations list <file> --sheet <sheet>' to discover available ranges."},
		},
		TargetObjectKinds: []string{"data-validation", "range", "sheet"},
	},
	"ooxml xlsx data-validations create": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx data-validations create workbook.xlsx --sheet Sheet1 --range A2:A20 --type list --list-values \"Open,Closed,Blocked\" --out edited.xlsx",
				Description:    "Create a practical list validation on a worksheet range.",
				ExpectedOutput: "JSON mutation result with validation readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid_args", Solution: "Use ordinary A1 ranges and one of list|whole|decimal|date|text-length for --type."},
		},
		TargetObjectKinds: []string{"data-validation", "range", "sheet"},
	},
	"ooxml xlsx data-validations update": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx data-validations update workbook.xlsx --sheet Sheet1 --range A2:A20 --type list --list-values \"Open,Closed\" --expect-type list --out edited.xlsx",
				Description:    "Update an existing validation rule with a type guard.",
				ExpectedOutput: "JSON mutation result with updated validation readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "no data validation", Solution: "Run 'ooxml --json xlsx data-validations list <file> --sheet <sheet>' and retry with a listed range."},
			{Pattern: "expect-type", Solution: "Refresh the current rule before retrying a guarded update."},
		},
		TargetObjectKinds: []string{"data-validation", "range", "sheet"},
	},
	"ooxml xlsx data-validations delete": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx data-validations delete workbook.xlsx --sheet Sheet1 --range A2:A20 --expect-type list --out edited.xlsx",
				Description:    "Delete a validation rule by its target range with a type guard.",
				ExpectedOutput: "JSON mutation result with deleted validation summary and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "no data validation", Solution: "Run 'ooxml --json xlsx data-validations list <file> --sheet <sheet>' and retry with a listed range."},
		},
		TargetObjectKinds: []string{"data-validation", "range", "sheet"},
	},
	"ooxml xlsx conditional-formats list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx conditional-formats list workbook.xlsx --sheet Sheet1",
				Description:    "List worksheet conditional-formatting blocks and cfRule selectors.",
				ExpectedOutput: "JSON conditional-formatting records with sqref, rule selectors, type, priority, formulas, dxfId, and stopIfTrue.",
			},
		},
		TargetObjectKinds: []string{"conditional-format", "range", "sheet"},
	},
	"ooxml xlsx conditional-formats show": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx conditional-formats show workbook.xlsx --sheet Sheet1 --rule cfRule:1",
				Description:    "Show one conditional-formatting rule by a selector returned from list.",
				ExpectedOutput: "JSON conditional-formatting rule with sqref, type, priority, formulas, dxfId, and stopIfTrue.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "conditional format rule", Solution: "Run 'ooxml --json xlsx conditional-formats list <file> --sheet <sheet>' and retry with a listed cfRule selector."},
		},
		TargetObjectKinds: []string{"conditional-format", "range", "sheet"},
	},
	"ooxml xlsx conditional-formats add": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx conditional-formats add workbook.xlsx --sheet Sheet1 --range A2:A20 --type expression --formula A2>100 --dxf-id 0 --out edited.xlsx",
				Description:    "Add an expression conditional-formatting rule that references an existing differential style.",
				ExpectedOutput: "JSON mutation result with rule readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid_args", Solution: "Use ordinary A1 sqref ranges, --type expression, and an existing --dxf-id if styling is needed."},
		},
		TargetObjectKinds: []string{"conditional-format", "range", "sheet", "style"},
	},
	"ooxml xlsx conditional-formats delete": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx conditional-formats delete workbook.xlsx --sheet Sheet1 --rule cfRule:1 --out edited.xlsx",
				Description:    "Delete one conditional-formatting rule by selector.",
				ExpectedOutput: "JSON mutation result with deleted rule summary and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "conditional format rule", Solution: "Run 'ooxml --json xlsx conditional-formats list <file> --sheet <sheet>' and retry with a listed cfRule selector."},
		},
		TargetObjectKinds: []string{"conditional-format", "range", "sheet"},
	},
	"ooxml xlsx hyperlinks list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx hyperlinks list workbook.xlsx --sheet Sheet1",
				Description:    "List worksheet hyperlinks on a sheet.",
				ExpectedOutput: "JSON hyperlink records with cell/range ref, URL/location, display, tooltip, and relationship id.",
			},
		},
		TargetObjectKinds: []string{"hyperlink", "cell", "range", "sheet"},
	},
	"ooxml xlsx hyperlinks show": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx hyperlinks show workbook.xlsx --sheet Sheet1 --cell B2",
				Description:    "Show the hyperlink attached to one cell or range.",
				ExpectedOutput: "JSON hyperlink record for the requested ref.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "no hyperlink", Solution: "Run 'ooxml --json xlsx hyperlinks list <file> --sheet <sheet>' and retry with a listed ref."},
		},
		TargetObjectKinds: []string{"hyperlink", "cell", "range", "sheet"},
	},
	"ooxml xlsx hyperlinks add": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx hyperlinks add workbook.xlsx --sheet Sheet1 --cell B2 --url https://example.com --display \"Details\" --out edited.xlsx",
				Description:    "Add an external hyperlink to a cell.",
				ExpectedOutput: "JSON mutation result with hyperlink readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid_args", Solution: "Specify exactly one useful target with --url or --location, and use A1 refs for --cell."},
		},
		TargetObjectKinds: []string{"hyperlink", "cell", "range", "sheet"},
	},
	"ooxml xlsx hyperlinks update": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx hyperlinks update workbook.xlsx --sheet Sheet1 --cell B2 --url https://example.com/new --expect-url https://example.com --out edited.xlsx",
				Description:    "Update an existing hyperlink with a stale-target URL guard.",
				ExpectedOutput: "JSON mutation result with updated hyperlink readback and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "no hyperlink", Solution: "Run 'ooxml --json xlsx hyperlinks list <file> --sheet <sheet>' and retry with a listed ref."},
			{Pattern: "expect-url", Solution: "Refresh the current hyperlink target before retrying a guarded update."},
		},
		TargetObjectKinds: []string{"hyperlink", "cell", "range", "sheet"},
	},
	"ooxml xlsx hyperlinks delete": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx hyperlinks delete workbook.xlsx --sheet Sheet1 --cell B2 --expect-url https://example.com --out edited.xlsx",
				Description:    "Delete an existing hyperlink with a stale-target URL guard.",
				ExpectedOutput: "JSON mutation result with deleted hyperlink summary and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "no hyperlink", Solution: "Run 'ooxml --json xlsx hyperlinks list <file> --sheet <sheet>' and retry with a listed ref."},
		},
		TargetObjectKinds: []string{"hyperlink", "cell", "range", "sheet"},
	},
	"ooxml xlsx comments list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx comments list workbook.xlsx --sheet Sheet1",
				Description:    "List cell comments/notes on a worksheet.",
				ExpectedOutput: "JSON comment records with anchors, author, text, and handles.",
			},
		},
		TargetObjectKinds: []string{"comment", "cell", "sheet"},
	},
	"ooxml xlsx comments add": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx comments add workbook.xlsx --sheet Sheet1 --cell C3 --author \"Analyst\" --text \"Check this\" --out edited.xlsx",
				Description:    "Add a visible legacy cell comment/note.",
				ExpectedOutput: "JSON mutation result with list-command readback.",
			},
		},
		TargetObjectKinds: []string{"comment", "cell", "sheet"},
	},
	"ooxml xlsx comments update": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx comments update workbook.xlsx --handle H:xlsx/ws:1/comment:a:C3 --text \"Updated note\" --out edited.xlsx",
				Description:    "Update an existing worksheet comment/note by stable handle from comments list.",
				ExpectedOutput: "JSON mutation result with list-command readback.",
			},
		},
		TargetObjectKinds: []string{"comment", "cell", "sheet"},
	},
	"ooxml xlsx comments remove": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx comments remove workbook.xlsx --sheet Sheet1 --comment-id 0 --out edited.xlsx",
				Description:    "Remove a worksheet comment/note by id or stable comment handle.",
				ExpectedOutput: "JSON mutation result with list-command readback.",
			},
		},
		TargetObjectKinds: []string{"comment", "cell", "sheet"},
	},
	"ooxml xlsx charts list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts list workbook.xlsx",
				Description:    "Enumerate charts and their selectors before editing.",
				ExpectedOutput: "JSON array of charts with selectors and source sheets.",
			},
		},
		TargetObjectKinds: []string{"chart"},
	},
	"ooxml xlsx charts create": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts create workbook.xlsx --type column --sheet Sheet1 --range A1:C5 --expect-source-range A1:C5 --title \"Revenue\" --anchor E2 --out edited.xlsx",
				Description:    "Author a worksheet chart from a guarded source range.",
				ExpectedOutput: "JSON create result with chart part, drawing part, anchor, source range, and validate/list commands.",
			},
			{
				Command:        "ooxml --json xlsx charts create workbook.xlsx --type line --table Sales --title \"Sales Trend\" --out edited.xlsx",
				Description:    "Create a chart from an existing Excel table.",
				ExpectedOutput: "JSON create result with resolved source range and chart readback commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--type is required", Solution: "Choose a common chart type: bar, column, line, area, pie, or scatter."},
			{Pattern: "source range mismatch", Solution: "Refresh the source table/range and retry with the current --expect-source-range."},
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json xlsx sheets list <file>' and 'xlsx tables list' to verify source sheet/table names."},
		},
		TargetObjectKinds: []string{"chart", "range", "table", "sheet"},
	},
	"ooxml xlsx charts update-source": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts update-source workbook.xlsx --chart chart:1 --series 1 --role values --source-sheet Sheet1 --source-range '$B$2:$B$20' --out edited.xlsx",
				Description:    "Repoint a chart series at a new source range.",
				ExpectedOutput: "JSON summary of the rebinding; writes edited.xlsx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml xlsx charts list <file>' then 'charts show' to confirm chart/series selectors."},
		},
		TargetObjectKinds: []string{"chart", "range", "sheet"},
	},
	"ooxml xlsx charts set-title": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts set-title workbook.xlsx --chart chart:1 --title \"Q4 Revenue\" --font-family Aptos --font-bold=true --out edited.xlsx",
				Description:    "Set an XLSX chart title and basic title font styling.",
				ExpectedOutput: "JSON chart style readback with the updated title.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json xlsx charts list <file>' to refresh chart selectors."},
			{Pattern: "expect-title", Solution: "Re-read the current title with 'xlsx charts show' and retry with the current --expect-title guard."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx charts set-legend": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts set-legend workbook.xlsx --sheet Sheet1 --chart chart:1 --position right --overlay=false --out edited.xlsx",
				Description:    "Set or create an XLSX chart legend using common business positions.",
				ExpectedOutput: "JSON chart style readback with legend position and overlay state.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "set-legend requires", Solution: "Pass --position right|left|top|bottom|none and/or --overlay=true|false."},
			{Pattern: "expect-position", Solution: "Refresh the current legend with 'xlsx charts show' before retrying a stale guarded edit."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx charts set-axis": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts set-axis workbook.xlsx --sheet Sheet1 --chart chart:1 --axis value --title \"Revenue\" --number-format '$#,##0' --major-gridlines=true --out edited.xlsx",
				Description:    "Set practical worksheet-chart axis title, number format, scale, gridlines, and tick-label font controls.",
				ExpectedOutput: "JSON chart style readback with updated axis metadata.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "List charts first; use the sheet/chart selector shown by 'xlsx charts list' or 'xlsx charts show'."},
			{Pattern: "expect-axis", Solution: "Refresh axis readback before retrying stale guarded axis edits."},
			{Pattern: "ambiguous", Solution: "Scatter charts have two value axes; inspect the chart before choosing an axis edit path."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx charts convert-type": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts convert-type workbook.xlsx --sheet Sheet1 --chart chart:1 --to line --expect-type column --out edited.xlsx",
				Description:    "Convert an existing worksheet chart among common chart families with a current-type guard.",
				ExpectedOutput: "JSON chart style/readback result with previousType, newType, and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--to is required", Solution: "Choose bar, column, line, area, pie, or scatter."},
			{Pattern: "expect-type", Solution: "Refresh the chart with 'xlsx charts show' and retry with the current type."},
			{Pattern: "pie", Solution: "Pie conversion requires a compatible single-series shape; use line/column/bar for multi-series charts."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx charts set-plot-area-fill": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts set-plot-area-fill workbook.xlsx --sheet Sheet1 --chart chart:1 --fill-color '#F3F6FA' --out edited.xlsx",
				Description:    "Set or clear the plot-area background fill of a worksheet chart.",
				ExpectedOutput: "JSON chart style readback showing previousFill and newFill.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--fill-color is required", Solution: "Pass a #RRGGBB color or 'none' to clear the fill."},
			{Pattern: "expect-fill", Solution: "Refresh chart style readback and retry with the current fill guard (#RRGGBB, scheme:name, or none)."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx charts set-chart-area-fill": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts set-chart-area-fill workbook.xlsx --sheet Sheet1 --chart chart:1 --fill-color '#FFFFFF' --out edited.xlsx",
				Description:    "Set or clear the chart-area/background fill of a worksheet chart.",
				ExpectedOutput: "JSON chart style readback showing previousFill and newFill.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--fill-color is required", Solution: "Pass a #RRGGBB color or 'none' to clear the fill."},
			{Pattern: "expect-fill", Solution: "Refresh chart style readback and retry with the current fill guard (#RRGGBB, scheme:name, or none)."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx charts copy-style": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts copy-style target.xlsx --sheet Sheet1 --chart chart:1 --from template.xlsx --from-chart chart:1 --expect-series-count 3 --out branded.xlsx",
				Description:    "Copy practical chart style from a template worksheet chart without copying chart data.",
				ExpectedOutput: "JSON chart style readback with applied style facets and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "--from", Solution: "Provide a template XLSX/XLTX that contains the chart style to copy."},
			{Pattern: "expect-series-count", Solution: "Refresh the target chart series count before applying template style."},
			{Pattern: "target_not_found", Solution: "Run 'xlsx charts list' on both target and template files to refresh selectors."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx charts set-series-style": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx charts set-series-style workbook.xlsx --chart chart:1 --series 1 --fill-color '#1F77B4' --line-width-pt 2 --out edited.xlsx",
				Description:    "Style an XLSX chart series using common fill, line, and marker controls.",
				ExpectedOutput: "JSON chart style readback for the changed series.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json xlsx charts list <file>' and confirm the chart/series number with 'charts show'."},
			{Pattern: "expect-series-count", Solution: "Refresh the series count before retrying a stale guarded series style edit."},
		},
		TargetObjectKinds: []string{"chart", "style"},
	},
	"ooxml xlsx pivots list": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx pivots list workbook.xlsx",
				Description:    "Enumerate pivot tables and their selectors.",
				ExpectedOutput: "JSON array of pivot tables with selectors and source ranges.",
			},
		},
		TargetObjectKinds: []string{"pivot"},
	},
	"ooxml xlsx pivots create": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx pivots create workbook.xlsx --table Sales --name SalesPivot --target-sheet Pivot --rows Region --values Amount:sum --out edited.xlsx",
				Description:    "Create a practical pivot report from an existing table.",
				ExpectedOutput: "JSON pivot definition readback with validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json xlsx tables list <file>' or ranges export to confirm the source before creating the pivot."},
			{Pattern: "invalid_args", Solution: "Use ordinary field names from the source table and common aggregations such as sum/count/average."},
		},
		TargetObjectKinds: []string{"pivot", "table", "range", "sheet"},
	},
	"ooxml xlsx tables show": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx tables show workbook.xlsx --table Sales",
				Description:    "Inspect a worksheet table's columns and range.",
				ExpectedOutput: "JSON table record with columns and the table range.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "List tables with 'ooxml xlsx tables list <file>' to find valid names."},
		},
		TargetObjectKinds: []string{"table", "range", "sheet"},
	},
	"ooxml xlsx tables append-records": {
		Examples: []Example{
			{
				Command:        "ooxml --json xlsx tables append-records workbook.xlsx --table Sales --records '[{\"Region\":\"North\",\"Amount\":42}]' --out edited.xlsx",
				Description:    "Append typed records to a worksheet table by column name.",
				ExpectedOutput: "JSON table readback with appended row count and validate command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json xlsx tables list <file>' and use a listed table name or selector."},
			{Pattern: "invalid_args", Solution: "Ensure every record key matches an existing table column; inspect the table with 'xlsx tables show'."},
		},
		TargetObjectKinds: []string{"table", "range", "sheet"},
	},
	"ooxml pptx tables update-from-xlsx": {
		Examples: []Example{
			{
				Command:        "ooxml --json pptx tables update-from-xlsx deck.pptx --workbook workbook.xlsx --sheet Sheet1 --range A1:C5 --expect-source-range A1:C5 --slide 1 --target table:1 --out edited.pptx",
				Description:    "Refresh a slide table from a workbook range with a source guard.",
				ExpectedOutput: "JSON summary of the table update; writes edited.pptx.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Use 'ooxml pptx shapes show <file> --slide N' to find the table target."},
			{Pattern: "diff_threshold", Solution: "Re-check the source range; align --expect-source-range with the workbook."},
		},
		TargetObjectKinds: []string{"table", "shape", "range"},
	},
	"ooxml vba list": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba list source.xlsm",
				Description:    "List VBA modules with their names and SHA-256 digests.",
				ExpectedOutput: "JSON object with package state, parsed source project metadata, modules, compatibility warnings, and follow-up commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Confirm the package contains a VBA project with 'ooxml vba inspect <file>'."},
			{Pattern: "invalid_args", Solution: "If the project is not parseable MS-CFB/MS-OVBA, use 'ooxml --json vba extract-bin <file> --out vbaProject.bin' for opaque binary handling."},
			{Pattern: "VBA_HOST_", Solution: "Treat host-family warnings as Office compatibility risks; use an Office-native vbaProject.bin seed for the target package family."},
		},
		TargetObjectKinds: []string{"module"},
	},
	"ooxml vba inspect": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba inspect source.xlsm",
				Description:    "Inspect whether a package contains a VBA project and summarize module/readback capability.",
				ExpectedOutput: "JSON object with package VBA status, source project summary when parseable, warnings, and follow-up commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "vba inspect supports macro-capable OOXML packages and reports missing VBA projects explicitly."},
			{Pattern: "VBA_HOST_", Solution: "Use host-family warnings as compatibility risk; prefer Office-native XLSM/PPTM seeds."},
		},
		TargetObjectKinds: []string{"package", "module"},
	},
	"ooxml vba inspect-bin": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba inspect-bin vbaProject.bin --family pptx",
				Description:    "Inspect a standalone VBA seed before attaching it to a PowerPoint macro package.",
				ExpectedOutput: "JSON object with source modules, seed SHA-256, compatibility warnings, and an attach command template.",
			},
			{
				Command:        "ooxml --json vba inspect-bin vbaProject.bin --family xlsx",
				Description:    "Check whether a standalone VBA seed has obvious Excel host-family risks before attachment.",
				ExpectedOutput: "JSON object with module metadata, host-family warnings, and an attach command template.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "failed to read VBA binary", Solution: "Check the path passed to inspect-bin; it expects a standalone vbaProject.bin file."},
			{Pattern: "--family must be pptx or xlsx", Solution: "Use '--family pptx' for PPTM seeds or '--family xlsx' for XLSM seeds."},
			{Pattern: "Compound File Binary", Solution: "inspect-bin expects a real MS-CFB vbaProject.bin; use 'ooxml --json vba extract-bin <file> --out vbaProject.bin' to obtain one from an Office file."},
		},
		TargetObjectKinds: []string{"module"},
	},
	"ooxml vba create": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba create workbook.xlsm --family xlsx --source macros/Module1.bas --source macros/Worker.cls --extract-bin vbaProject.bin --enable-vba-object-model-access --force",
				Description:    "Create a fresh Office-authored macro-enabled workbook from .bas/.cls sources and optionally extract its vbaProject.bin seed.",
				ExpectedOutput: "JSON object with imported modules, proof level, output paths, and inspect/list/validate/office-check follow-up commands.",
			},
			{
				Command:        "ooxml --json vba create deck.pptm --family pptx --source macros/Module1.bas --force",
				Description:    "Create a fresh Office-authored macro-enabled PowerPoint deck from VBA source.",
				ExpectedOutput: "JSON object with the created PPTM path, imported module readback, and validation commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "requires Windows desktop Microsoft Office", Solution: "Run on a Windows machine with desktop Office installed, or create/obtain an Office-authored vbaProject.bin and use 'ooxml vba attach'."},
			{Pattern: "windows-office-vba-create.ps1 not found", Solution: "Run from the ooxml-cli checkout or pass --office-create-script .\\tools\\windows-office-vba-create.ps1."},
			{Pattern: "VBA source file not found", Solution: "Write the .bas/.cls source file first, then pass it with --source; repeat --source for multiple modules."},
			{Pattern: "Trust access to the VBA project object model", Solution: "Pass --enable-vba-object-model-access or enable Trust access manually in Office Trust Center."},
		},
		TargetObjectKinds: []string{"package", "module"},
	},
	"ooxml docx styles apply": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx styles apply report.docx --index 1 --target paragraph --style Heading1 --out styled.docx",
				Description:    "Apply a paragraph or run style to a stable DOCX target.",
				ExpectedOutput: "JSON style application readback with changed block selectors.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "HANDLE_STALE", Solution: "Refresh paragraph/style handles with 'docx paragraphs show' or 'docx styles list' before retrying."},
			{Pattern: "target_not_found", Solution: "List styles and paragraphs before applying a style to ensure both targets exist."},
		},
		TargetObjectKinds: []string{"style", "paragraph"},
	},
	"ooxml docx paragraphs set": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx paragraphs set report.docx --index 1 --text \"Updated summary\" --out edited.docx",
				Description:    "Replace one scoped DOCX paragraph.",
				ExpectedOutput: "JSON paragraph readback for the changed block.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "HANDLE_STALE", Solution: "Refresh paragraph handles; do not fall back to a broad document replace when a scoped edit was intended."},
			{Pattern: "target_not_found", Solution: "Use 'ooxml --json docx text <file>' or paragraph listing commands to locate the target."},
		},
		TargetObjectKinds: []string{"paragraph"},
	},
	"ooxml docx tables set-cell": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx tables set-cell report.docx --table 1 --row 2 --col 3 --text \"Approved\" --out edited.docx",
				Description:    "Set a DOCX table cell in ordinary business-document tables.",
				ExpectedOutput: "JSON table readback showing the changed cell.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json docx tables show <file>' to confirm table, row, and column indexes."},
			{Pattern: "invalid_args", Solution: "Use 1-based table/row/column indexes from the table readback."},
		},
		TargetObjectKinds: []string{"table"},
	},
	"ooxml docx headers list": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx headers list report.docx",
				Description:    "List section headers with pasteable selectors.",
				ExpectedOutput: "JSON section header/footer refs with primarySelector and selector aliases.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "file_not_found", Solution: "Pass a readable .docx path."},
			{Pattern: "unsupported_type", Solution: "Use a DOCX/DOCM package for docx headers."},
		},
		TargetObjectKinds: []string{"header", "footer"},
	},
	"ooxml docx headers show": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx headers show report.docx --selector header:1:default",
				Description:    "Show one header by selector from headers list.",
				ExpectedOutput: "JSON header readback with scoped paragraph selectors.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json docx headers list <file>' and retry with a listed selector."},
			{Pattern: "invalid_args", Solution: "Use exactly one targeting mode: --selector, --id, or --section/--type."},
		},
		TargetObjectKinds: []string{"header", "paragraph"},
	},
	"ooxml docx headers set-text": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx headers set-text report.docx --selector header:1:default/p:1 --text \"Confidential\" --out edited.docx",
				Description:    "Set one header paragraph by selector.",
				ExpectedOutput: "JSON mutation result with output path, selector readback, validate command, and show command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Use 'docx headers list' for header selectors and 'docx headers show' for paragraph selectors."},
			{Pattern: "invalid_args", Solution: "Use --selector or --id/--section/--type, and exactly one of --text or --text-file."},
		},
		TargetObjectKinds: []string{"header", "paragraph"},
	},
	"ooxml docx footers list": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx footers list report.docx",
				Description:    "List section footers with pasteable selectors.",
				ExpectedOutput: "JSON section header/footer refs with primarySelector and selector aliases.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "file_not_found", Solution: "Pass a readable .docx path."},
			{Pattern: "unsupported_type", Solution: "Use a DOCX/DOCM package for docx footers."},
		},
		TargetObjectKinds: []string{"footer", "header"},
	},
	"ooxml docx footers show": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx footers show report.docx --selector footer:1:default",
				Description:    "Show one footer by selector from footers list.",
				ExpectedOutput: "JSON footer readback with scoped paragraph selectors.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Run 'ooxml --json docx footers list <file>' and retry with a listed selector."},
			{Pattern: "invalid_args", Solution: "Use exactly one targeting mode: --selector, --id, or --section/--type."},
		},
		TargetObjectKinds: []string{"footer", "paragraph"},
	},
	"ooxml docx footers set-text": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx footers set-text report.docx --selector footer:1:default/p:1 --text \"Page Footer\" --out edited.docx",
				Description:    "Set one footer paragraph by selector.",
				ExpectedOutput: "JSON mutation result with output path, selector readback, validate command, and show command.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "target_not_found", Solution: "Use 'docx footers list' for footer selectors and 'docx footers show' for paragraph selectors."},
			{Pattern: "invalid_args", Solution: "Use --selector or --id/--section/--type, and exactly one of --text or --text-file."},
		},
		TargetObjectKinds: []string{"footer", "paragraph"},
	},
	"ooxml docx comments list": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx comments list report.docx",
				Description:    "List document comments with stable handles.",
				ExpectedOutput: "JSON comment records with ids, authors, text, and handles.",
			},
		},
		TargetObjectKinds: []string{"comment"},
	},
	"ooxml docx comments add": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx comments add report.docx --anchor-block 2 --author \"Analyst\" --text \"Review this\" --out edited.docx",
				Description:    "Add a document comment anchored to a block.",
				ExpectedOutput: "JSON mutation result with comments-list readback.",
			},
		},
		TargetObjectKinds: []string{"comment", "paragraph"},
	},
	"ooxml docx comments edit": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx comments edit report.docx --comment-id 0 --text \"Updated note\" --out edited.docx",
				Description:    "Edit a document comment by id or stable comment handle.",
				ExpectedOutput: "JSON mutation result with updated comment handle.",
			},
		},
		TargetObjectKinds: []string{"comment"},
	},
	"ooxml docx comments remove": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx comments remove report.docx --comment-id 0 --out edited.docx",
				Description:    "Remove a document comment and its range markers.",
				ExpectedOutput: "JSON mutation result identifying the removed comment.",
			},
		},
		TargetObjectKinds: []string{"comment"},
	},
	"ooxml docx images list": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx images list report.docx",
				Description:    "List inline images with relationship ids, media parts, and block anchors.",
				ExpectedOutput: "JSON image records with indexes, relationship ids, media URIs, and dimensions.",
			},
		},
		TargetObjectKinds: []string{"image", "paragraph"},
	},
	"ooxml docx images replace": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx images replace report.docx --image 1 --file replacement.png --out edited.docx",
				Description:    "Replace an existing inline DOCX image.",
				ExpectedOutput: "JSON replacement result with updated media part details.",
			},
		},
		TargetObjectKinds: []string{"image"},
	},
	"ooxml docx images insert": {
		Examples: []Example{
			{
				Command:        "ooxml --json docx images insert report.docx --after 0 --file image.png --width 914400 --height 914400 --out edited.docx",
				Description:    "Insert a new inline image after a body block.",
				ExpectedOutput: "JSON insertion result with image index, relationship id, media URI, and dimensions.",
			},
		},
		TargetObjectKinds: []string{"image", "paragraph"},
	},
	"ooxml vba office-check": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba office-check edited.xlsm",
				Description:    "Validate a macro-enabled workbook, then check whether the best local Office-compatible engine opens it.",
				ExpectedOutput: "JSON object with package validation status, local engine open-check status, and explicit limitations.",
			},
			{
				Command:        "ooxml --json vba office-check edited.pptm --out-dir office-check-output",
				Description:    "Keep the LibreOffice conversion artifact so an agent can inspect the open-check output.",
				ExpectedOutput: "JSON object with open-check proof and outputPath/outputBytes when conversion succeeds.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "missing_engine", Solution: "Run 'ooxml doctor' and install LibreOffice/soffice, or treat office-check as unavailable on this machine."},
			{Pattern: "package_validation_failed", Solution: "Run the emitted validateCommand and fix validation diagnostics before relying on an open check."},
			{Pattern: "engine_failed", Solution: "Open the emitted conversion output/log context if available; this is stronger evidence than package validation that the local engine rejected the file."},
		},
		TargetObjectKinds: []string{"module"},
	},
	"ooxml vba replace-module": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba replace-module source.xlsm --module module:Module1 --source macros/Module1.bas --expect-sha256 0000000000000000000000000000000000000000000000000000000000000000 --out edited.xlsm",
				Description:    "Replace one VBA source module with a stale-source guard.",
				ExpectedOutput: "JSON mutation result with source hashes, project readback, and validate/office-check follow-up commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "VBA module not found", Solution: "Run 'ooxml --json vba list <file>' to refresh valid module selectors; use a listed primarySelector such as module:Module1."},
			{Pattern: "experimental VBA source rewrite refused", Solution: "Run 'ooxml --json vba list <file>' first. For Office-shaped projects, exact no-op replace is byte-preserving; source-changing rewrites require --allow-experimental-vba-source-rewrite and user acceptance of Office-load risk."},
			{Pattern: "source hash mismatch", Solution: "Refresh the module hash with 'ooxml --json vba list <file>' and retry with the current --expect-sha256."},
		},
		TargetObjectKinds: []string{"module"},
	},
	"ooxml vba add-module": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba add-module source.xlsm --source macros/NewModule.bas --expect-module-count 2 --out edited.xlsm",
				Description:    "Add one .bas or .cls source module with a module-count guard.",
				ExpectedOutput: "JSON mutation result with added module readback and validate/office-check follow-up commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "experimental VBA source rewrite refused", Solution: "Office-shaped projects require --allow-experimental-vba-source-rewrite for source-changing edits; only use it after backup and user acceptance of Office-load risk."},
			{Pattern: "module count mismatch", Solution: "Refresh module count with 'ooxml --json vba list <file>' and retry with the current --expect-module-count."},
		},
		TargetObjectKinds: []string{"module"},
	},
	"ooxml vba remove-module": {
		Examples: []Example{
			{
				Command:        "ooxml --json vba remove-module source.xlsm --module module:Module1 --expect-sha256 0000000000000000000000000000000000000000000000000000000000000000 --out edited.xlsm",
				Description:    "Remove one VBA source module with a stale-source guard.",
				ExpectedOutput: "JSON mutation result with removed module summary and validate/office-check follow-up commands.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "VBA module not found", Solution: "Run 'ooxml --json vba list <file>' to refresh valid module selectors; use a listed primarySelector such as module:Module1."},
			{Pattern: "experimental VBA source rewrite refused", Solution: "Office-shaped projects require --allow-experimental-vba-source-rewrite for source-changing edits; only use it after backup and user acceptance of Office-load risk."},
			{Pattern: "source hash mismatch", Solution: "Refresh the module hash with 'ooxml --json vba list <file>' and retry with the current --expect-sha256."},
		},
		TargetObjectKinds: []string{"module"},
	},
	"ooxml template tokens": {
		Examples: []Example{
			{
				Command:        "ooxml --json template tokens brand-deck.pptx",
				Description:    "Extract design tokens (theme, default text styles, table/chart styles) from a PPTX/POTX template.",
				ExpectedOutput: "TemplateTokens JSON with schemaVersion, theme, and family-specific token lists.",
			},
			{
				Command:        "ooxml --json template tokens report-template.xlsx",
				Description:    "Extract theme, named cell styles, and chart styles from an XLSX/XLTX template.",
				ExpectedOutput: "TemplateTokens JSON with the xlsx token block.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "template tokens reads PPTX/POTX and XLSX/XLTX only; pass --for pptx|xlsx to override detection."},
			{Pattern: "file_not_found", Solution: "Check the path; tokens needs an existing template file."},
		},
		TargetObjectKinds: []string{"package", "style", "chart", "table"},
	},
	"ooxml template apply": {
		Examples: []Example{
			{
				Command:        "ooxml --json template apply deck.pptx --from brand.potx --out branded.pptx",
				Description:    "Apply changed theme colors and major/minor fonts from a source template onto every referenced target theme part.",
				ExpectedOutput: "TemplateApplyResult JSON listing effective color/font changes and already-up-to-date skips.",
			},
			{
				Command:        "ooxml --json template apply deck.pptx --from brand.potx --target-text-styles --out styled.pptx",
				Description:    "Apply representative PPTX level-1 master default text styles by role from a template onto every target slide master.",
				ExpectedOutput: "TemplateApplyResult JSON listing effective master text-style changes and skipped no-ops.",
			},
			{
				Command:        "ooxml --json template apply deck.pptx --tokens tokens.json --target-charts --dry-run",
				Description:    "Preview applying chart series styling from a tokens profile without writing output.",
				ExpectedOutput: "TemplateApplyResult JSON with dryRun=true and the chart parts that would change.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid_args", Solution: "Provide exactly one token source (--from, --tokens, or --profile) and one output mode (--out, --in-place, or --dry-run)."},
			{Pattern: "file_not_found", Solution: "Check the target and source paths; both the document and the template/profile must exist."},
		},
		TargetObjectKinds: []string{"package", "style", "chart"},
	},
	"ooxml template profile save": {
		Examples: []Example{
			{
				Command:        "ooxml template profile save brand.potx --out brand.json --name \"Acme Brand\"",
				Description:    "Extract a reusable design profile (theme colors + major/minor fonts) from a PPTX/POTX template.",
				ExpectedOutput: "A versioned design-profile JSON written to --out (theme colors, fonts, placeholder defaults).",
			},
			{
				Command:        "ooxml --json template profile save report.xltx --out report-brand.json",
				Description:    "Save a design profile from an XLSX/XLTX template for later reuse.",
				ExpectedOutput: "DesignProfile JSON with schemaVersion, metadata, and the theme design block.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "unsupported_type", Solution: "profile save reads PPTX/POTX and XLSX/XLTX only; pass --for pptx|xlsx to override detection."},
			{Pattern: "file_not_found", Solution: "Check the template path; profile save needs an existing template file."},
		},
		TargetObjectKinds: []string{"package", "style"},
	},
	"ooxml template profile inspect": {
		Examples: []Example{
			{
				Command:        "ooxml --json template profile inspect brand.json",
				Description:    "Validate a saved design profile and echo back its parsed contents.",
				ExpectedOutput: "DesignProfile JSON, or an invalid_args error if the file is not a valid profile.",
			},
		},
		CommonErrors: []CommonError{
			{Pattern: "invalid_args", Solution: "The file must be a design profile (format \"ooxml-design-profile\") with a schemaVersion and valid hex colors."},
			{Pattern: "file_not_found", Solution: "Check the profile path."},
		},
		TargetObjectKinds: []string{"package", "style"},
	},
}

// MetadataFor returns the enrichment for a command path, or false if none.
func MetadataFor(path string) (CommandMetadata, bool) {
	m, ok := commandMetadata[path]
	return m, ok
}

// CommandPaths returns every command path that has authored metadata, sorted.
func CommandPaths() []string {
	paths := make([]string, 0, len(commandMetadata))
	for p := range commandMetadata {
		paths = append(paths, p)
	}
	sort.Strings(paths)
	return paths
}

// BuildObjectKindIndex returns a deterministic reverse index from object kind to
// the sorted, de-duplicated command paths whose TargetObjectKinds include it.
// Only kinds in the taxonomy are emitted; unknown kinds in metadata are ignored.
func BuildObjectKindIndex() map[string][]string {
	index := make(map[string][]string)
	for path, meta := range commandMetadata {
		for _, kind := range meta.TargetObjectKinds {
			if !IsObjectKind(kind) {
				continue
			}
			index[kind] = append(index[kind], path)
		}
	}
	for kind := range index {
		index[kind] = sortedUnique(index[kind])
	}
	return index
}

// CommandsForKind returns the sorted command paths targeting the given kind, or
// an empty (non-nil) slice when the kind is unknown or unused. Never errors, so
// the CLI can return exit 0 for an unrecognized kind.
func CommandsForKind(kind string) []string {
	index := BuildObjectKindIndex()
	if paths, ok := index[kind]; ok {
		return paths
	}
	return []string{}
}

func sortedUnique(in []string) []string {
	seen := make(map[string]struct{}, len(in))
	out := make([]string, 0, len(in))
	for _, s := range in {
		if _, ok := seen[s]; ok {
			continue
		}
		seen[s] = struct{}{}
		out = append(out, s)
	}
	sort.Strings(out)
	return out
}
