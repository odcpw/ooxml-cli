package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	findpkg "github.com/ooxml-cli/ooxml-cli/pkg/find"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	findType       string
	findIgnoreCase bool
	findRegex      bool
	findMax        int
	findToOps      bool
	findReplace    string
	findApply      bool
)

var findCmd = &cobra.Command{
	Use:   "find <query> <file>",
	Short: "Search PPTX/XLSX/DOCX for text, formulas, and defined names; return hits with stable selectors and pre-filled mutation commands",
	Long: `Search an OOXML package for a query and report every hit.

find is READ-ONLY. For each hit it reports the package type, a location, stable
selectors, the matched value, surrounding context, and a PRE-FILLED mutation
command you can run to edit that hit. Generated commands use <NEW> and <OUT>
placeholders for the replacement value and output path, which find cannot know.

Searched targets by package type:
  pptx  slide shape text, table-cell text, and speaker notes
  xlsx  cell values, cell formulas, and workbook defined names
  docx  document body paragraph and table text

Flags --type text|formula|name|all narrow the search. text covers PPTX/DOCX
text and XLSX cell values; formula covers XLSX formulas; name covers XLSX
defined names. PPTX/DOCX yield no hits for --type formula or name.

Exit codes: 0 = success (including zero hits), 2 = invalid args, 3 = file not
found, 4 = unsupported package type.`,
	Args:          cobra.ExactArgs(2),
	SilenceUsage:  true,
	SilenceErrors: true,
	RunE: func(cmd *cobra.Command, args []string) error {
		query := args[0]
		filePath := args[1]

		if query == "" {
			return InvalidArgsError("query must not be empty")
		}
		matchType, err := findpkg.ParseMatchType(findType)
		if err != nil {
			return InvalidArgsError(err.Error())
		}
		if findMax < 0 {
			return InvalidArgsError("--max must be >= 0")
		}
		if err := validateFindComposeFlags(cmd); err != nil {
			return err
		}

		if _, statErr := os.Stat(filePath); statErr != nil {
			return FileNotFoundError(filePath)
		}

		opts := findpkg.Options{
			Query:      query,
			Type:       matchType,
			IgnoreCase: findIgnoreCase,
			Regex:      findRegex,
			Max:        findMax,
		}

		// Composition paths (read-only --to-ops, or --replace --apply). Find
		// stays read-only unless --apply is given.
		if findToOps {
			return runFindToOps(cmd, filePath, query, opts, findReplace)
		}
		if findApply {
			mutOpts, mErr := GetValidatedMutationOptions(cmd)
			if mErr != nil {
				return mErr
			}
			return runFindApply(cmd, filePath, query, opts, findReplace, mutOpts)
		}

		// Standard read-only find.
		pkg, err := opc.Open(filePath)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()

		typeKey, terr := findPackageTypeKey(pkg)
		if terr != nil {
			return terr
		}

		result, err := findpkg.Search(pkg, typeKey, opts)
		if err != nil {
			return mapFindSearchError(err, findRegex)
		}

		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, result)
		}
		return writeGlobalOutput(cmd, []byte(renderFindText(result)))
	},
}

// validateFindComposeFlags enforces the interaction rules for the composition
// flags (--to-ops, --replace, --apply). find is read-only by default; the only
// state-changing combination is --replace + --apply + an output target.
//
// Truth table:
//
//	(none)                  -> standard read-only find
//	--to-ops                -> emit ops.json (read-only); --replace optional
//	--replace --apply       -> compose and apply (requires --out/--in-place/--dry-run)
//	--apply without --replace        -> error
//	--replace without --to-ops/apply -> error
//	--to-ops --apply                 -> error
func validateFindComposeFlags(cmd *cobra.Command) error {
	replaceChanged := cmd.Flags().Changed("replace")

	if findToOps && findApply {
		return InvalidArgsError("--to-ops and --apply are mutually exclusive; use --to-ops to emit ops, or --apply to run them")
	}
	if findApply && !replaceChanged {
		return InvalidArgsError("--apply requires --replace <new>")
	}
	// With --apply the replacement is written into the file, so an empty value is
	// almost never intended and would otherwise leave the literal "<NEW>"
	// placeholder in the document. Empty --replace is only meaningful for --to-ops
	// (it emits the placeholder for the caller to fill in).
	if findApply && replaceChanged && findReplace == "" {
		return InvalidArgsError("--replace must be non-empty with --apply (use --to-ops to emit a placeholder)")
	}
	if replaceChanged && !findToOps && !findApply {
		return InvalidArgsError("--replace requires --to-ops (emit) or --apply (run); find is read-only otherwise")
	}
	return nil
}

// mapFindSearchError converts a findpkg.Search error into a CLIError. An invalid
// regex is a user (args) error; anything else is unexpected.
func mapFindSearchError(err error, regex bool) error {
	if regex && strings.Contains(err.Error(), "regular expression") {
		return InvalidArgsError(err.Error())
	}
	return NewCLIErrorf(ExitUnexpected, "search failed: %v", err)
}

func renderFindText(result *findpkg.Result) string {
	var b strings.Builder
	fmt.Fprintf(&b, "%s: %d hit(s) for %q (type=%s)\n",
		result.PackageType, result.TotalHits, result.Query, result.Type)
	if result.Truncated {
		fmt.Fprintf(&b, "(truncated to --max %d)\n", result.Max)
	}
	for _, hit := range result.Hits {
		fmt.Fprintf(&b, "  [%d] %s %s\n", hit.Index, hit.Kind, hit.Location)
		fmt.Fprintf(&b, "      matched: %q\n", hit.MatchedValue)
		if hit.Context != "" && hit.Context != hit.MatchedValue {
			fmt.Fprintf(&b, "      context: %s\n", hit.Context)
		}
		if hit.MutationCommand != "" {
			fmt.Fprintf(&b, "      mutate:  %s\n", hit.MutationCommand)
		} else if hit.MutationNote != "" {
			fmt.Fprintf(&b, "      note:    %s\n", hit.MutationNote)
		}
	}
	return strings.TrimRight(b.String(), "\n")
}

// ---------------------------------------------------------------------------
// capabilities + robot-docs
// ---------------------------------------------------------------------------

type findCapabilities struct {
	Tool            string            `json:"tool"`
	ContractVersion string            `json:"contractVersion"`
	ReadOnly        bool              `json:"readOnly"`
	PackageTypes    []string          `json:"packageTypes"`
	SearchTypes     []string          `json:"searchTypes"`
	HitKinds        []findCapHitKind  `json:"hitKinds"`
	Flags           []string          `json:"flags"`
	ExitCodes       []findCapExitCode `json:"exitCodes"`
	Notes           []string          `json:"notes"`
}

type findCapHitKind struct {
	Kind            string `json:"kind"`
	PackageType     string `json:"packageType"`
	Description     string `json:"description"`
	CommandTemplate string `json:"commandTemplate"`
}

type findCapExitCode struct {
	Code    int    `json:"code"`
	Meaning string `json:"meaning"`
}

var findCapabilitiesCmd = &cobra.Command{
	Use:   "capabilities",
	Short: "Print the machine-readable find contract",
	Args:  findReservedQueryArgs("capabilities"),
	RunE: func(cmd *cobra.Command, args []string) error {
		caps := findCapabilities{
			Tool:            "ooxml",
			ContractVersion: findpkg.ContractVersion,
			ReadOnly:        true,
			PackageTypes:    []string{"pptx", "xlsx", "docx"},
			SearchTypes:     []string{"all", "text", "formula", "name"},
			HitKinds: []findCapHitKind{
				{string(findpkg.KindPPTXText), "pptx", "slide shape or table-cell visible text",
					"ooxml --json pptx replace text-occurrences <file> --match-text <MATCHED> --new-text <NEW> --for-shape <H:pptx/s:SLD/shape:n:ID> --out <OUT>"},
				{string(findpkg.KindPPTXNotes), "pptx", "speaker-notes text (no semantic mutation command; edit notes part directly)", ""},
				{string(findpkg.KindXLSXValue), "xlsx", "worksheet cell value",
					"ooxml --json xlsx cells set <file> --sheet <S> --cell <A1> --value <NEW> --out <OUT>"},
				{string(findpkg.KindXLSXFormula), "xlsx", "worksheet cell formula",
					"ooxml --json xlsx cells set <file> --sheet <S> --cell <A1> --formula <NEW> --out <OUT>"},
				{string(findpkg.KindXLSXName), "xlsx", "workbook defined name (name or ref)",
					"ooxml --json xlsx names update <file> --name <NAME> --ref <NEW> --out <OUT>"},
				{string(findpkg.KindDOCXText), "docx", "document body paragraph or table text",
					"ooxml --json docx replace <file> --find <MATCHED> --replace <NEW> --out <OUT>"},
			},
			Flags: []string{"--json", "--type", "--ignore-case", "--regex", "--max", "--to-ops", "--replace", "--apply", "--out", "--in-place", "--backup", "--no-validate", "--dry-run"},
			ExitCodes: []findCapExitCode{
				{0, "success (including zero hits)"},
				{2, "invalid arguments"},
				{3, "file not found"},
				{4, "unsupported package type"},
			},
			Notes: []string{
				"find is read-only by default; it modifies files only with --replace --apply --out/--in-place.",
				"--to-ops emits an apply-compatible ops.json (a bare array of {command,args}) on stdout; save it and pass to `ooxml apply --ops <file>`. Read-only. --replace substitutes the replacement value; otherwise the <NEW> placeholder is left in place.",
				"--replace <new> --apply --out <file> runs the discovered mutations through the apply engine atomically (one final validation, per-op readback). --dry-run prints the plan and executes nothing.",
				"hits with no semantic mutation command (e.g. pptx speaker notes) are skipped from emitted/applied ops and reported on stderr.",
				"Generated mutation commands use <NEW> and <OUT> placeholders; fill them before running.",
				"matchedValue holds the exact literal substring that matched and is what mutation commands use; context may include surrounding text.",
				"PPTX/DOCX text matching is per visible text node/line; matches split across runs may not be editable by the generated command.",
				"--regex uses Go regexp syntax; --ignore-case applies to both literal and regex queries.",
				"--max caps returned hits in file order; result.truncated indicates more existed.",
				"A literal query of 'capabilities' or 'robot-docs' is shadowed by find subcommands; search it with `ooxml --json find -- capabilities <file>` or `ooxml --json find -- robot-docs <file>`.",
			},
		}
		return writeGlobalJSON(cmd, caps)
	},
}

var findRobotDocsCmd = &cobra.Command{
	Use:   "robot-docs",
	Short: "Print a paste-ready agent handbook for ooxml find",
	Args:  findReservedQueryArgs("robot-docs"),
	RunE: func(cmd *cobra.Command, args []string) error {
		return writeGlobalOutput(cmd, []byte(findRobotDocs()))
	},
}

func findReservedQueryArgs(query string) cobra.PositionalArgs {
	return func(cmd *cobra.Command, args []string) error {
		if len(args) == 0 {
			return nil
		}
		return InvalidArgsError(fmt.Sprintf("literal find query %q is shadowed by the `find %s` subcommand; search it with `ooxml --json find -- %s <file>`", query, query, query))
	}
}

func findRobotDocs() string {
	return strings.TrimSpace(`
ooxml find — agent handbook (semantic cross-object search)

PURPOSE
  Locate text, formulas, and defined names across a PPTX/XLSX/DOCX package and,
  for each hit, hand back a PRE-FILLED mutation command so you can edit it
  without re-parsing the file. find is READ-ONLY.

USAGE
  ooxml --json find <query> <file> [--type all|text|formula|name] [--ignore-case] [--regex] [--max N]
  ooxml --json find <query> <file> --to-ops [--replace <new>]   Emit apply-compatible ops.json (read-only).
  ooxml --json find <query> <file> --replace <new> --apply (--out <f>|--in-place|--dry-run)  Find+apply in one step.
  ooxml --json find capabilities      Machine-readable contract (hit kinds, templates, exit codes).
  ooxml --json find -- capabilities deck.pptx  Search the literal word "capabilities".
  ooxml find robot-docs               This handbook.

WHAT IS SEARCHED
  pptx  slide shape text, table-cell text, speaker notes
  xlsx  cell values, cell formulas, workbook defined names
  docx  document body paragraph and table text

FLAGS
  --type        all (default) | text | formula | name. text = PPTX/DOCX text +
                XLSX cell values; formula = XLSX formulas; name = XLSX names.
  --ignore-case case-insensitive matching (applies to literal and regex).
  --regex       treat <query> as a Go regexp pattern.
  --max N       cap returned hits (file order); result.truncated flags overflow.

HOW TO USE THE JSON
  .contractVersion   pinned contract id ("` + findpkg.ContractVersion + `").
  .packageType       pptx|xlsx|docx.
  .totalHits         number of hits returned (after --max).
  .truncated         true when more hits existed than --max allowed.
  .hits[]            each: {index, kind, location, partUri, primarySelector,
                     selectors[], matchedValue, context, mutationCommand,
                     mutationNote, metadata}.
  matchedValue is the exact literal substring that matched; it is what the
  mutationCommand's match argument uses. context may include surrounding text.

RUNNING A MUTATION FROM A HIT
  Generated commands contain <NEW> (replacement) and <OUT> (output path)
  placeholders, plus a literal <file> token. Substitute all three, then run.
  Example:
    cmd=$(ooxml --json find 'Old Corp' deck.pptx | jq -r '.hits[0].mutationCommand')
    cmd=${cmd/<file>/deck.pptx}; cmd=${cmd/<NEW>/New Corp}; cmd=${cmd/<OUT>/out.pptx}
    eval "$cmd"
  Speaker-notes hits (kind=pptx-notes) have an empty mutationCommand and a
  mutationNote; there is no semantic command for notes today.

COMPOSING FIND + APPLY (no shell-string parsing)
  --to-ops builds an apply-compatible ops.json directly from each hit's
  STRUCTURED fields and prints a bare JSON array of {command,args} to stdout:
    ooxml --json find 'Old Corp' deck.pptx --replace 'New Corp' --to-ops > ops.json
    ooxml --json apply deck.pptx --ops ops.json --out edited.pptx
  Or do both in one invocation (atomic, single final validation, readback):
    ooxml --json find 'Old Corp' deck.pptx --replace 'New Corp' --apply --out edited.pptx
    ooxml --json find 'Old Corp' deck.pptx --replace 'New Corp' --apply --dry-run
  Rules: --apply requires --replace and one of --out/--in-place/--dry-run;
  --to-ops and --apply are mutually exclusive; --replace alone is an error.
  Hits with no mutation command are skipped and reported on stderr.

EXIT CODES
  0 success (zero hits is still 0) · 2 invalid args · 3 file not found ·
  4 unsupported package type.

GOTCHA
  Bare 'capabilities'/'robot-docs' are subcommands, so they shadow those as a
  first argument. Any other query is treated as a search term.
`)
}

func init() {
	findCmd.Flags().StringVar(&findType, "type", "all", "search scope: all|text|formula|name")
	findCmd.Flags().BoolVar(&findIgnoreCase, "ignore-case", false, "match case-insensitively")
	findCmd.Flags().BoolVar(&findRegex, "regex", false, "treat query as a Go regexp pattern")
	findCmd.Flags().IntVar(&findMax, "max", 0, "cap returned hits (0 = unlimited)")
	findCmd.Flags().BoolVar(&findToOps, "to-ops", false, "emit an apply-compatible ops.json (array of {command,args}) to stdout; read-only")
	findCmd.Flags().StringVar(&findReplace, "replace", "", "replacement value substituted into generated ops (used with --to-ops or --apply)")
	findCmd.Flags().BoolVar(&findApply, "apply", false, "run the generated ops through the apply engine (requires --replace and --out/--in-place/--dry-run)")
	AddMutationFlags(findCmd)
	findCmd.AddCommand(findCapabilitiesCmd)
	findCmd.AddCommand(findRobotDocsCmd)
	GetRootCmd().AddCommand(findCmd)
}
