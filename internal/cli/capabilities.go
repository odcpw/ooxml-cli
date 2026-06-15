package cli

import (
	"fmt"
	"sort"
	"strings"
	"unicode"

	"github.com/ooxml-cli/ooxml-cli/pkg/capabilities"
	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

// Bumped to v2 when per-command examples, commonErrors, targetObjectKinds, and
// the objectKindsIndex were added; to v3 when the additive `handles` section
// was added; to v4 when capability flags gained dashless `argName` for MCP/apply
// JSON args. The string lives in the capabilities pkg so the contract version
// and the enrichment shape are versioned together.
const capabilitiesContractVersion = capabilities.MetadataSchemaVersion

// capabilitiesForKind holds the value of the --for flag (reverse lookup).
var capabilitiesForKind string

var capabilitiesCmd = &cobra.Command{
	Use:   "capabilities",
	Short: "Print the machine-readable CLI contract",
	Long: `Print the ooxml command contract for agents and automation.

Use --json for a stable JSON document with global flags, commands, exit codes,
package types, and common PPTX/XLSX/VBA workflows.`,
	Args: cobra.NoArgs,
	RunE: func(cmd *cobra.Command, args []string) error {
		doc := buildCapabilitiesDocument()
		// Reverse lookup: --for <object-kind-or-family> narrows the document to
		// commands targeting that object kind, or commands in a top-level family
		// such as pptx/xlsx/docx/vba. Unknown filters yield an empty result and
		// exit 0 (graceful, mirrors "no matches").
		if filter := capabilitiesForKind; filter != "" {
			var resolution capabilitiesFilterResolution
			doc, resolution = filterCapabilities(doc, filter)
			if GetGlobalConfig(cmd).Format == "json" {
				return writeGlobalJSON(cmd, doc)
			}
			return outputCapabilitiesForFilterText(cmd, resolution, doc)
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, doc)
		}
		return outputCapabilitiesText(cmd, doc)
	},
}

type capabilitiesDocument struct {
	Tool            string               `json:"tool"`
	Version         string               `json:"version"`
	ContractVersion string               `json:"contractVersion"`
	PackageTypes    []string             `json:"packageTypes"`
	OutputModes     []string             `json:"outputModes"`
	GlobalFlags     []capabilityFlag     `json:"globalFlags"`
	Commands        []capabilityCommand  `json:"commands"`
	ObjectKinds     []string             `json:"objectKinds"`
	ObjectKindIndex map[string][]string  `json:"objectKindsIndex"`
	ExitCodes       []capabilityExitCode `json:"exitCodes"`
	Workflows       []capabilityWorkflow `json:"workflows"`
	Handles         capabilityHandles    `json:"handles"`
	Conventions     []string             `json:"conventions"`
	Notes           []string             `json:"notes"`
}

// capabilityHandles describes which commands accept handles and which read
// surfaces issue them. Handles are paste-safe object addresses that survive
// structural edits when backed by a native id; limitations such as XLSX cell A1
// stability are documented in Notes.
type capabilityHandles struct {
	// Field is the JSON field name handles are surfaced under in find/inspect.
	Field string `json:"field"`
	// Prefix is the unambiguous handle envelope prefix; any non-prefixed value
	// takes the legacy selector path unchanged.
	Prefix string `json:"prefix"`
	// Accepted is a compatibility summary flag: at least one command accepts
	// handles. Use AcceptedBy for exact command/flag support.
	Accepted bool `json:"accepted"`
	// AcceptedBy lists command/flag pairs that are documented to accept handles.
	AcceptedBy []capabilityHandleAcceptance `json:"acceptedBy,omitempty"`
	// Issued reports that find and inspect surface handles (read-only) in the
	// Field, omitted when no stable handle exists or the id is ambiguous.
	Issued bool `json:"issued"`
	// EmittedByFindToOps reports that find --to-ops / find apply emit handle-based
	// target args by default where a stable handle exists, so apply batches survive
	// structural shifts caused by earlier ops.
	EmittedByFindToOps bool `json:"emittedByFindToOps"`
	// Grammar lists per-format example handle shapes.
	Grammar []string `json:"grammar"`
	// Errors lists the typed handle error codes a mutate may return.
	Errors []string `json:"errors"`
	// Notes are short clarifications about handle behavior and limits.
	Notes []string `json:"notes"`
}

type capabilityHandleAcceptance struct {
	Format      string   `json:"format"`
	Command     string   `json:"command"`
	Flags       []string `json:"flags"`
	HandleKinds []string `json:"handleKinds"`
	Notes       string   `json:"notes,omitempty"`
}

type capabilityFlag struct {
	Name        string `json:"name"`
	ArgName     string `json:"argName,omitempty"`
	Shorthand   string `json:"shorthand,omitempty"`
	Type        string `json:"type"`
	Default     string `json:"default,omitempty"`
	Description string `json:"description"`
}

type capabilityCommand struct {
	Path              string                  `json:"path"`
	Use               string                  `json:"use"`
	Short             string                  `json:"short,omitempty"`
	Examples          []capabilityExample     `json:"examples,omitempty"`
	CommonErrors      []capabilityCommonError `json:"commonErrors,omitempty"`
	TargetObjectKinds []string                `json:"targetObjectKinds,omitempty"`
	LocalFlags        []capabilityFlag        `json:"localFlags,omitempty"`
	Subcommands       []string                `json:"subcommands,omitempty"`
	// OpCompatible reports whether this command can be driven as an apply/serve/MCP
	// op (a leaf mutation command whose only positional argument is the package
	// file). When false, OpIneligibleReason states why, so an agent can predict the
	// op-validation rejection from the manifest instead of discovering it at runtime.
	OpCompatible       bool   `json:"opCompatible"`
	OpIneligibleReason string `json:"opIneligibleReason,omitempty"`
}

type capabilityExitCode struct {
	Code        int    `json:"code"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

type capabilityWorkflow struct {
	Name     string   `json:"name"`
	Commands []string `json:"commands"`
}

func buildCapabilitiesDocument() capabilitiesDocument {
	return capabilitiesDocument{
		Tool:            "ooxml",
		Version:         Version,
		ContractVersion: capabilitiesContractVersion,
		PackageTypes: []string{
			"pptx",
			"pptm",
			"xlsx",
			"xlsm",
			"docx",
			"vbaProject.bin macro project payload",
		},
		OutputModes: []string{
			"text",
			"json via --json or --format json",
		},
		GlobalFlags:     collectGlobalFlags(rootCmd),
		Commands:        collectCommandCapabilities(rootCmd),
		ObjectKinds:     capabilities.ObjectKinds,
		ObjectKindIndex: capabilities.BuildObjectKindIndex(),
		ExitCodes: []capabilityExitCode{
			{Code: ExitSuccess, Name: "success", Description: "command completed successfully"},
			{Code: ExitUnexpected, Name: "unexpected", Description: "unexpected tool or package processing error"},
			{Code: ExitInvalidArgs, Name: "invalid_args", Description: "invalid command line arguments or incompatible options"},
			{Code: ExitFileNotFound, Name: "file_not_found", Description: "input file or referenced payload was not found"},
			{Code: ExitUnsupportedType, Name: "unsupported_type", Description: "file type is not supported by the requested command"},
			{Code: ExitValidationFailed, Name: "validation_failed", Description: "OOXML validation found blocking issues"},
			{Code: ExitTargetNotFound, Name: "target_not_found", Description: "requested slide, sheet, table, shape, or macro part was not found"},
			{Code: ExitRenderFailed, Name: "render_failed", Description: "rendering failed"},
			{Code: ExitDiffThreshold, Name: "diff_threshold", Description: "visual or package diff exceeded the configured threshold"},
			{Code: ExitPartialSuccess, Name: "partial_success", Description: "command produced usable output with non-fatal issues"},
		},
		Workflows: []capabilityWorkflow{
			{
				Name: "pptx inspect then edit",
				Commands: []string{
					"ooxml --json inspect deck.pptx",
					"ooxml --json pptx slides list deck.pptx",
					"ooxml --json pptx slides selectors deck.pptx --slide 1",
					"ooxml --json pptx shapes show deck.pptx --slide 1",
					"ooxml --json pptx charts list deck.pptx",
					"ooxml --json pptx charts show deck.pptx --slide 1 --chart chart:1",
					"ooxml --json pptx clone-slide deck.pptx --slide 1 --out edited.pptx",
					"ooxml --json pptx slides show edited.pptx --slide 2 --include-text --include-bounds",
					"ooxml --json pptx replace text deck.pptx --slide 1 --target title --text NEW --out edited.pptx",
					"ooxml --json pptx replace text-occurrences deck.pptx --match-text \"Old Client\" --new-text \"New Client\" --expect-count 12 --dry-run",
					"ooxml --json pptx replace text-occurrences deck.pptx --match-text \"Old Client\" --new-text \"New Client\" --expect-count 12 --expect-plan-hash sha256:... --out edited.pptx",
					"ooxml --json pptx charts update-data deck.pptx --slide 1 --chart chart:1 --series 1 --values-json '[\"150\",\"175\",\"210\"]' --categories-json '[\"North\",\"South\",\"West\"]' --expect-point-count 3 --expect-values-hash sha256:... --out edited.pptx",
					"ooxml validate --strict edited.pptx",
					"ooxml pptx render edited.pptx --out render-check",
				},
			},
			{
				Name: "pptx animations author",
				Commands: []string{
					"ooxml --json pptx animations list deck.pptx",
					"ooxml --json pptx animations add deck.pptx --slide 1 --shape shape:4 --effect appear --out edited.pptx",
					"ooxml --json pptx animations add deck.pptx --slide 1 --shape '~Title 1' --effect wipe --direction up --out edited.pptx",
					"ooxml --json pptx animations add deck.pptx --slide 1 --shape shape:5 --effect fade --by-paragraph --expect-paragraph-count 4 --out edited.pptx",
					"ooxml --json pptx animations reorder deck.pptx --slide 1 --order 7,3,5 --out edited.pptx",
					"ooxml --json pptx animations remove deck.pptx --slide 1 --effect-id 5 --out edited.pptx",
					"ooxml --json pptx animations prune-stale deck.pptx --dry-run",
					"ooxml validate --strict edited.pptx",
				},
			},
			{
				Name: "xlsx inspect then edit",
				Commands: []string{
					"ooxml --json inspect workbook.xlsx",
					"ooxml --json xlsx sheets list workbook.xlsx",
					"ooxml --json xlsx sheets show workbook.xlsx --sheet Sheet1",
					"ooxml --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:C5 --include-types",
					"ooxml --json xlsx cells set workbook.xlsx --sheet Sheet1 --cell B2 --value '42' --out edited.xlsx",
					"ooxml --json xlsx ranges set-format workbook.xlsx --sheet Sheet1 --range B2:B20 --preset currency --out edited.xlsx",
					"ooxml --json xlsx names list workbook.xlsx",
					"ooxml --json xlsx names add workbook.xlsx --name SalesData --sheet Sheet1 --range A1:C5 --out edited.xlsx",
					"ooxml --json xlsx names update edited.xlsx --name SalesData --sheet Sheet1 --range A1:D5 --expect-ref \"'Sheet1'!\\$A\\$1:\\$C\\$5\" --out edited.xlsx",
					"ooxml --json xlsx charts list workbook.xlsx",
					"ooxml --json xlsx charts show workbook.xlsx --chart chart:1",
					"ooxml --json xlsx charts update-source workbook.xlsx --chart chart:1 --series 1 --role values --source-sheet Sheet1 --source-range '$B$2:$B$20' --expect-source-range '$B$2:$B$10' --out edited.xlsx",
					"ooxml --json xlsx pivots list workbook.xlsx",
					"ooxml --json xlsx pivots show workbook.xlsx --pivot pivot:1",
					"ooxml validate --strict edited.xlsx",
				},
			},
			{
				Name: "pptx from xlsx bindings",
				Commands: []string{
					"ooxml --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:C5 --include-types",
					"ooxml --json xlsx tables show workbook.xlsx --table Sales",
					"ooxml --json pptx tables update-from-xlsx deck.pptx --workbook workbook.xlsx --sheet Sheet1 --range A1:C5 --expect-source-range A1:C5 --slide 1 --target table:1 --out edited.pptx",
					"ooxml --json pptx place table-from-xlsx deck.pptx --workbook workbook.xlsx --table Sales --expect-source-range A1:C5 --slide 1 --x 0 --y 0 --cx 4000000 --out edited.pptx",
					"ooxml --json pptx xlsx-bindings plan deck.pptx --workbook workbook.xlsx --table DeckBindings",
					"ooxml --json pptx xlsx-bindings apply deck.pptx --workbook workbook.xlsx --table DeckBindings --dry-run",
					"ooxml --json pptx xlsx-bindings apply deck.pptx --workbook workbook.xlsx --table DeckBindings --out edited.pptx",
					"ooxml --json pptx xlsx-bindings plan deck.pptx --workbook workbook.xlsx --table DeckImageBindings",
					"ooxml --json pptx xlsx-bindings apply deck.pptx --workbook workbook.xlsx --table DeckImageBindings --dry-run",
					"ooxml --json pptx xlsx-bindings apply deck.pptx --workbook workbook.xlsx --table DeckBoundsBindings --out edited.pptx",
					"ooxml validate --strict edited.pptx",
				},
			},
			{
				Name: "vba project and module handling",
				Commands: []string{
					"ooxml --json vba inspect source.xlsm",
					"ooxml --json vba create workbook.xlsm --family xlsx --source macros/Module1.bas --source macros/Worker.cls --extract-bin vbaProject.bin --enable-vba-object-model-access --force",
					"ooxml --json vba create deck.pptm --family pptx --source macros/Module1.bas --force",
					"ooxml --json vba extract-bin source.xlsm --out vbaProject.bin",
					"ooxml --json vba inspect-bin vbaProject.bin --family xlsx",
					"ooxml --json vba inspect-bin vbaProject.bin --family pptx",
					"ooxml --json vba list source.xlsm",
					"ooxml --json vba extract source.xlsm --out-dir macros",
					"ooxml --json vba replace-module source.xlsm --module Module1 --source macros/Module1.bas --expect-sha256 0000000000000000000000000000000000000000000000000000000000000000 --out edited.xlsm",
					"ooxml --json vba add-module source-only.xlsm --source macros/NewModule.bas --expect-module-count 2 --out added-source-only.xlsm",
					"ooxml --json vba remove-module source-only-edited.xlsm --module Module1 --expect-sha256 0000000000000000000000000000000000000000000000000000000000000000 --out removed-module.xlsm",
					"ooxml --json vba attach target.xlsx --bin office-authored-vbaProject.bin --out target-with-vba.xlsm",
					"ooxml --json vba inspect target-with-vba.xlsm",
					"ooxml validate --strict target-with-vba.xlsm",
					"ooxml --json vba office-check target-with-vba.xlsm",
					"ooxml --json xlsx sheets list target-with-vba.xlsm",
					"ooxml --json vba remove target-with-vba.xlsm --out target-no-vba.xlsx",
					"ooxml --json vba inspect target-no-vba.xlsx",
				},
			},
		},
		Handles: capabilityHandles{
			Field:              "handle",
			Prefix:             "H:",
			Accepted:           true,
			Issued:             true,
			EmittedByFindToOps: true,
			Grammar: []string{
				"H:pptx/s:<sldId>                     (PPTX slide scope)",
				"H:pptx/s:<sldId>/shape:n:<cNvPrId>   (PPTX shape)",
				"H:pptx/s:<sldId>/comment:idx:<id>:authorId:<authorId> (PPTX comment)",
				"H:xlsx/ws:<sheetId>                  (XLSX worksheet)",
				"H:xlsx/ws:<sheetId>/cell:a:<A1>      (XLSX cell; does NOT survive row/col insert, fails HANDLE_STALE)",
				"H:xlsx/wb/name:n:<name>              (XLSX workbook-scoped defined name)",
				"H:docx/pt:doc/comment:n:<id>         (DOCX comment)",
				"H:docx/pt:styles/style:n:<styleId>   (DOCX style)",
				"H:docx/pt:doc/para:m:<w14:paraId>    (DOCX paragraph; lazily injected on first mutate)",
			},
			AcceptedBy: []capabilityHandleAcceptance{
				{
					Format:      "pptx",
					Command:     "ooxml pptx replace text",
					Flags:       []string{"--target"},
					HandleKinds: []string{"pptx.shape"},
					Notes:       "A shape handle supplies the slide scope; --slide/--for-slides are optional for handle targets.",
				},
				{
					Format:      "pptx",
					Command:     "ooxml pptx replace images",
					Flags:       []string{"--target"},
					HandleKinds: []string{"pptx.shape"},
					Notes:       "A shape handle supplies the slide scope; --slide and --for-slides are rejected for handle targets.",
				},
				{
					Format:      "pptx",
					Command:     "ooxml pptx animations add",
					Flags:       []string{"--shape"},
					HandleKinds: []string{"pptx.shape"},
					Notes:       "A shape handle supplies the slide scope; --slide is ignored for handle targets.",
				},
				{
					Format:      "pptx",
					Command:     "ooxml pptx comments edit",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"pptx.comment"},
					Notes:       "A comment handle supplies slide scope, comment idx, and authorId; --slide, --comment-id, and --author-id are not needed.",
				},
				{
					Format:      "pptx",
					Command:     "ooxml pptx comments remove",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"pptx.comment"},
					Notes:       "A comment handle supplies slide scope, comment idx, and authorId; --slide, --comment-id, and --author-id are not needed.",
				},
				{
					Format:      "pptx",
					Command:     "ooxml pptx replace text-occurrences",
					Flags:       []string{"--for-slides"},
					HandleKinds: []string{"pptx.slide"},
					Notes:       "Restricts replacement to the slide resolved by durable sldId.",
				},
				{
					Format:      "pptx",
					Command:     "ooxml pptx replace text-occurrences",
					Flags:       []string{"--for-shape"},
					HandleKinds: []string{"pptx.shape"},
					Notes:       "Restricts replacement to one shape resolved by durable sldId plus cNvPr id.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx cells set",
					Flags:       []string{"--cell"},
					HandleKinds: []string{"xlsx.cell"},
					Notes:       "A cell handle supplies both sheet scope and A1 coordinate; --sheet is ignored.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx cells clear",
					Flags:       []string{"--range"},
					HandleKinds: []string{"xlsx.cell"},
					Notes:       "A cell handle can be passed through --range/--ref to supply both sheet scope and A1 coordinate; --sheet is ignored.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx names show",
					Flags:       []string{"--name"},
					HandleKinds: []string{"xlsx.definedName"},
					Notes:       "Workbook-scoped defined-name handles resolve in place of a bare name.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx names update",
					Flags:       []string{"--name"},
					HandleKinds: []string{"xlsx.definedName"},
					Notes:       "Workbook-scoped defined-name handles resolve in place of a bare name.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx names rename",
					Flags:       []string{"--name"},
					HandleKinds: []string{"xlsx.definedName"},
					Notes:       "Workbook-scoped defined-name handles resolve in place of a bare name.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx names delete",
					Flags:       []string{"--name"},
					HandleKinds: []string{"xlsx.definedName"},
					Notes:       "Workbook-scoped defined-name handles resolve in place of a bare name.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx comments remove",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"xlsx.comment"},
					Notes:       "A comment handle supplies sheet scope and anchor cell; --sheet and --comment-id are not needed.",
				},
				{
					Format:      "xlsx",
					Command:     "ooxml xlsx comments update",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"xlsx.comment"},
					Notes:       "A comment handle supplies sheet scope and anchor cell; --sheet and --comment-id are not needed.",
				},
				{
					Format:      "docx",
					Command:     "ooxml docx paragraphs set",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"docx.paragraph"},
					Notes:       "Targets a paragraph by w14:paraId marker; marker-less paragraphs are upgraded on first mutation.",
				},
				{
					Format:      "docx",
					Command:     "ooxml docx paragraphs clear",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"docx.paragraph"},
					Notes:       "Targets a paragraph by w14:paraId marker; marker-less paragraphs are upgraded on first mutation.",
				},
				{
					Format:      "docx",
					Command:     "ooxml docx comments edit",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"docx.comment"},
				},
				{
					Format:      "docx",
					Command:     "ooxml docx comments remove",
					Flags:       []string{"--handle"},
					HandleKinds: []string{"docx.comment"},
				},
				{
					Format:      "docx",
					Command:     "ooxml docx styles apply",
					Flags:       []string{"--handle", "--style"},
					HandleKinds: []string{"docx.paragraph", "docx.style"},
					Notes:       "--handle targets a paragraph/run block; --style accepts a style handle in place of a bare styleId.",
				},
			},
			Errors: []string{
				"HANDLE_MALFORMED",
				"HANDLE_FORMAT_MISMATCH",
				"HANDLE_STALE",
				"HANDLE_SCOPE_STALE",
				"HANDLE_AMBIGUOUS",
			},
			Notes: []string{
				"Handles are accepted only by the command/flag pairs listed in handles.acceptedBy; non-listed selectors may still require their legacy positional/name selector form.",
				"find and inspect surface the handle in the 'handle' field across pptx/xlsx/docx; it is omitted when no native id exists or the id is non-unique (ambiguous).",
				"find --to-ops and find apply emit handle-based target args by default where a stable handle exists. Native-id handles (slide/shape, sheet, defined-name, paragraph paraId) survive structural shifts (slide/sheet reorder, inserts) caused by earlier ops. Address-positional handles (XLSX cell/comment, A1-tagged) survive sheet reorder/rename but NOT a row/column insert/delete, so they are reported as position-dependent on stderr along with any op that has no stable handle.",
				"An XLSX cell/comment handle is address-positional: it survives sheet reorder/rename, and a row/column INSERT that shifts its A1 address fails cleanly with HANDLE_STALE. A row/column DELETE can shift a populated cell onto the stale address and write the wrong cell silently; to prevent this, apply rejects a batch that shifts rows/columns before an address-positional handle op. Run such edits as separate apply invocations (re-resolving the handle against the post-edit file) or target the cell positionally with --sheet/--cell.",
				"In serve/MCP sessions ops apply incrementally: after a row/column insert/delete in a session, RE-RESOLVE any address-positional cell/comment handle (inspect/find the cell again) before targeting it. The session engine rejects later address-positional handle ops until the target is re-expressed, preventing a stale pre-shift handle from silently mis-targeting on delete.",
			},
		},
		Conventions: []string{
			"stdout is data; diagnostics and errors go to stderr",
			"prefer --out for mutations; use --in-place only when you also want optional --backup",
			"--json is a global shortcut for --format json and wins when both are present",
			"--pretty affects JSON formatting but not field names or ordering",
			"validation should be run after package mutations before handing files to users",
			"apply/serve/MCP ops require a leaf mutation command whose only positional argument is the package file; supply every other value through args. Filter to commands whose opCompatible is true (opIneligibleReason explains the rest); multi-positional commands like 'pptx slides merge/move/reorder/delete' are not op-driveable.",
		},
		Notes: []string{
			"ooxml vba create can create fresh Office-authored XLSM/PPTM files from .bas/.cls sources on Windows desktop Office; other VBA commands move whole vbaProject.bin payloads, read parseable source modules, replace existing modules with guards, and add/remove modules only for synthetic/source-only projects. Real Office-shaped module-set changes are refused because version-dependent _VBA_PROJECT metadata must be regenerated. vba office-check provides local LibreOffice/soffice open-check evidence, but does not execute or compile macros and is not Microsoft Office proof; the Windows office-vba-smoke gate is the Microsoft Office COM proof for package attach/remove and existing-module replacement.",
			"DOCX commands exist but PPTX and XLSX are the primary automation surfaces.",
		},
	}
}

func collectGlobalFlags(cmd *cobra.Command) []capabilityFlag {
	var flags []capabilityFlag
	cmd.PersistentFlags().VisitAll(func(flag *pflag.Flag) {
		if flag.Hidden {
			return
		}
		flags = append(flags, capabilityFlagFromPFlag(flag))
	})
	sort.Slice(flags, func(i, j int) bool {
		return flags[i].Name < flags[j].Name
	})
	return flags
}

func collectCommandCapabilities(root *cobra.Command) []capabilityCommand {
	var commands []capabilityCommand
	var walk func(*cobra.Command)
	walk = func(parent *cobra.Command) {
		children := parent.Commands()
		sort.Slice(children, func(i, j int) bool {
			return children[i].CommandPath() < children[j].CommandPath()
		})
		for _, child := range children {
			if child.Hidden {
				continue
			}
			entry := capabilityCommand{
				Path:              child.CommandPath(),
				Use:               child.Use,
				Short:             child.Short,
				Examples:          examplesForPath(child.CommandPath()),
				CommonErrors:      commonErrorsForPath(child.CommandPath()),
				TargetObjectKinds: targetObjectKindsForPath(child.CommandPath()),
				LocalFlags:        collectLocalFlags(child),
			}
			if reason := operationCommandIncompatibility(child); reason == "" {
				entry.OpCompatible = true
			} else {
				entry.OpIneligibleReason = reason
			}
			for _, grandchild := range child.Commands() {
				if !grandchild.Hidden {
					entry.Subcommands = append(entry.Subcommands, grandchild.Name())
				}
			}
			sort.Strings(entry.Subcommands)
			commands = append(commands, entry)
			walk(child)
		}
	}
	walk(root)
	sort.Slice(commands, func(i, j int) bool {
		return commands[i].Path < commands[j].Path
	})
	return commands
}

func collectLocalFlags(cmd *cobra.Command) []capabilityFlag {
	var flags []capabilityFlag
	cmd.LocalFlags().VisitAll(func(flag *pflag.Flag) {
		flags = append(flags, capabilityFlagFromPFlag(flag))
	})
	sort.Slice(flags, func(i, j int) bool {
		return flags[i].Name < flags[j].Name
	})
	return flags
}

func capabilityFlagFromPFlag(flag *pflag.Flag) capabilityFlag {
	return capabilityFlag{
		Name:        "--" + flag.Name,
		ArgName:     capabilityArgNameFromFlagName(flag.Name),
		Shorthand:   flag.Shorthand,
		Type:        flag.Value.Type(),
		Default:     flag.DefValue,
		Description: flag.Usage,
	}
}

func capabilityArgNameFromFlagName(name string) string {
	parts := strings.FieldsFunc(name, func(r rune) bool {
		return r == '-' || r == '_' || unicode.IsSpace(r)
	})
	if len(parts) == 0 {
		return ""
	}
	out := strings.ToLower(parts[0])
	for _, part := range parts[1:] {
		if part == "" {
			continue
		}
		runes := []rune(strings.ToLower(part))
		runes[0] = unicode.ToUpper(runes[0])
		out += string(runes)
	}
	return out
}

func outputCapabilitiesText(cmd *cobra.Command, doc capabilitiesDocument) error {
	var b strings.Builder
	fmt.Fprintf(&b, "ooxml capabilities\n")
	fmt.Fprintf(&b, "Version: %s\n", doc.Version)
	fmt.Fprintf(&b, "Contract: %s\n", doc.ContractVersion)
	fmt.Fprintf(&b, "Output: text or JSON via --json / --format json\n")
	fmt.Fprintf(&b, "Package types: %s\n\n", strings.Join(doc.PackageTypes, ", "))

	fmt.Fprintf(&b, "Common agent commands:\n")
	for _, workflow := range doc.Workflows {
		fmt.Fprintf(&b, "\n%s:\n", workflow.Name)
		for _, command := range workflow.Commands {
			fmt.Fprintf(&b, "  %s\n", command)
		}
	}

	fmt.Fprintf(&b, "\nExit codes:\n")
	for _, exitCode := range doc.ExitCodes {
		fmt.Fprintf(&b, "  %d  %s  %s\n", exitCode.Code, exitCode.Name, exitCode.Description)
	}

	fmt.Fprintf(&b, "\nObject kinds (use --for <kind> to list targeting commands):\n  %s\n", strings.Join(doc.ObjectKinds, ", "))
	fmt.Fprintf(&b, "\nCommand families (use --for <family> to list family commands):\n  pptx, xlsx, docx, vba\n")
	fmt.Fprintf(&b, "\nRun `ooxml capabilities --json` for the full command and flag inventory.\n")
	return writeGlobalOutput(cmd, []byte(b.String()))
}

type capabilitiesFilterResolution struct {
	Query     string
	Mode      string
	Canonical string
	Prefix    string
	Known     bool
}

func filterCapabilities(doc capabilitiesDocument, rawFilter string) (capabilitiesDocument, capabilitiesFilterResolution) {
	query := normalizeCapabilitiesFilter(rawFilter)
	resolution := capabilitiesFilterResolution{
		Query:     query,
		Mode:      "unknown",
		Canonical: query,
	}
	if capabilities.IsObjectKind(query) {
		resolution.Mode = "object-kind"
		resolution.Known = true
		return filterCapabilitiesByObjectKind(doc, query), resolution
	}
	if family, prefix, ok := commandFamilyForCapabilitiesFilter(query); ok {
		resolution.Mode = "family"
		resolution.Canonical = family
		resolution.Prefix = prefix
		resolution.Known = true
		return filterCapabilitiesByCommandFamily(doc, family, prefix), resolution
	}
	doc.Commands = []capabilityCommand{}
	doc.ObjectKindIndex = map[string][]string{query: {}}
	doc.Notes = append([]string{fmt.Sprintf("No capabilities filter matched %q. Use an object kind such as shape/chart/module or a command family such as pptx/xlsx/docx/vba.", query)}, doc.Notes...)
	return doc, resolution
}

func normalizeCapabilitiesFilter(raw string) string {
	return strings.ToLower(strings.TrimSpace(raw))
}

func commandFamilyForCapabilitiesFilter(filter string) (family string, prefix string, ok bool) {
	switch filter {
	case "ppt", "pptx", "pptm", "powerpoint", "presentation", "presentations", "deck":
		return "pptx", "ooxml pptx", true
	case "excel", "spreadsheet", "spreadsheets", "workbook", "xls", "xlsx", "xlsm":
		return "xlsx", "ooxml xlsx", true
	case "doc", "docx", "docm", "word", "document", "documents":
		return "docx", "ooxml docx", true
	case "macro", "macros", "vba", "vbaproject", "vbaproject.bin":
		return "vba", "ooxml vba", true
	default:
		return "", "", false
	}
}

func filterCapabilitiesByObjectKind(doc capabilitiesDocument, kind string) capabilitiesDocument {
	matches := make([]capabilityCommand, 0)
	for _, c := range doc.Commands {
		for _, k := range c.TargetObjectKinds {
			if k == kind {
				matches = append(matches, c)
				break
			}
		}
	}
	doc.Commands = matches
	doc.ObjectKindIndex = map[string][]string{kind: capabilityCommandPaths(matches)}
	return doc
}

func filterCapabilitiesByCommandFamily(doc capabilitiesDocument, family, prefix string) capabilitiesDocument {
	matches := make([]capabilityCommand, 0)
	for _, c := range doc.Commands {
		if c.Path == prefix || strings.HasPrefix(c.Path, prefix+" ") {
			matches = append(matches, c)
		}
	}
	doc.Commands = matches
	doc.ObjectKindIndex = objectKindIndexForCapabilityCommands(matches)
	doc.Notes = append([]string{fmt.Sprintf("Filtered by command family %q (path prefix %q).", family, prefix)}, doc.Notes...)
	return doc
}

func capabilityCommandPaths(commands []capabilityCommand) []string {
	paths := make([]string, 0, len(commands))
	for _, c := range commands {
		paths = append(paths, c.Path)
	}
	sort.Strings(paths)
	return paths
}

func objectKindIndexForCapabilityCommands(commands []capabilityCommand) map[string][]string {
	index := make(map[string][]string)
	for _, c := range commands {
		for _, kind := range c.TargetObjectKinds {
			if !capabilities.IsObjectKind(kind) {
				continue
			}
			index[kind] = append(index[kind], c.Path)
		}
	}
	for kind, paths := range index {
		index[kind] = sortedUniqueStrings(paths)
	}
	return index
}

func sortedUniqueStrings(in []string) []string {
	if len(in) == 0 {
		return []string{}
	}
	sort.Strings(in)
	out := make([]string, 0, len(in))
	var last string
	for i, s := range in {
		if i > 0 && s == last {
			continue
		}
		out = append(out, s)
		last = s
	}
	return out
}

func outputCapabilitiesForFilterText(cmd *cobra.Command, resolution capabilitiesFilterResolution, doc capabilitiesDocument) error {
	var b strings.Builder
	switch resolution.Mode {
	case "object-kind":
		fmt.Fprintf(&b, "Commands targeting object kind %q:\n", resolution.Canonical)
	case "family":
		fmt.Fprintf(&b, "Commands in command family %q:\n", resolution.Canonical)
	default:
		fmt.Fprintf(&b, "Commands matching %q:\n", resolution.Query)
	}
	if len(doc.Commands) == 0 {
		fmt.Fprintf(&b, "  (none)\n")
		if !resolution.Known {
			fmt.Fprintf(&b, "\n%q is not a known object kind or command family.\n", resolution.Query)
			fmt.Fprintf(&b, "Known object kinds:\n  %s\n", strings.Join(capabilities.ObjectKinds, ", "))
			fmt.Fprintf(&b, "Known command families:\n  pptx, xlsx, docx, vba\n")
		}
	}
	for _, c := range doc.Commands {
		fmt.Fprintf(&b, "  %s\n", c.Path)
	}
	return writeGlobalOutput(cmd, []byte(b.String()))
}

func init() {
	capabilitiesCmd.Flags().StringVar(
		&capabilitiesForKind,
		"for",
		"",
		"list only commands targeting this object kind or command family (e.g. --for shape, --for vba); unknown filters return empty",
	)
	rootCmd.AddCommand(capabilitiesCmd)
}
