package cli

import (
	"fmt"
	"strings"

	"github.com/spf13/cobra"
)

var robotDocsCmd = &cobra.Command{
	Use:     "robot-docs",
	Aliases: []string{"agent"},
	Short:   "Print in-tool guidance for automation agents",
	Long:    "Automation-focused guides for using ooxml without external documentation lookup.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var robotDocsGuideCmd = &cobra.Command{
	Use:   "guide",
	Short: "Print a compact agent guide",
	Long:  "Print a paste-ready guide for fast PPTX, XLSX, and VBA automation with ooxml.",
	Args:  cobra.NoArgs,
	RunE: func(cmd *cobra.Command, args []string) error {
		guide := buildRobotDocsGuide()
		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, guide)
		}
		return outputRobotDocsGuideText(cmd, guide)
	},
}

type robotDocsGuide struct {
	Tool       string             `json:"tool"`
	Version    string             `json:"version"`
	Principles []string           `json:"principles"`
	Sections   []robotDocsSection `json:"sections"`
	Warnings   []string           `json:"warnings"`
}

type robotDocsSection struct {
	Name     string   `json:"name"`
	Commands []string `json:"commands"`
}

func buildRobotDocsGuide() robotDocsGuide {
	return robotDocsGuide{
		Tool:    "ooxml",
		Version: Version,
		Principles: []string{
			"Use --json for agent-readable output. It is the shortcut for --format json.",
			"Inspect before mutating, mutate to --out, then validate with --strict.",
			"Prefer stable selectors such as slide numbers, sheet names, table names, and shape/table targets from prior JSON output.",
			"Reuse generated command fields such as readbackCommand, showCommand, cellsExtractCommand, rangesExportCommand, and validateCommand when present.",
			"Use --in-place only when the caller explicitly wants the input file modified.",
		},
		Sections: []robotDocsSection{
			{
				Name: "Discovery",
				Commands: []string{
					"ooxml capabilities --json",
					"ooxml capabilities --json --for shape",
					"ooxml robot-docs guide",
					"ooxml agent guide",
					"ooxml --json inspect <file>",
					"ooxml validate --strict <file>",
				},
			},
			{
				Name: "Preflight and release proof",
				Commands: []string{
					"ooxml --json doctor",
					"ooxml --json doctor health",
					"ooxml --json doctor capabilities",
					"ooxml doctor robot-docs",
					"make check-release-fast",
					"make check-release-slow",
					"make check-office-vba-schema",
					"make check-office-vba-com",
					`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -RequireOpenXmlSdk -RunConformance -SkipOffice`,
					`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-edit-smoke.ps1 -RepoRoot . -MutationParallelism 4 -OfficeOracleTimeoutSeconds 120 -RequireOpenXmlSdk -RunConformance`,
					`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -SkipOffice -EnableVbaObjectModelAccess`,
					`powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\windows-office-vba-smoke.ps1 -RepoRoot . -RequireOpenXmlSdk -EnableVbaObjectModelAccess -OfficeOracleTimeoutSeconds 120`,
				},
			},
			{
				Name: "PPTX read",
				Commands: []string{
					"ooxml --json pptx slides list deck.pptx",
					"ooxml --json pptx slides show deck.pptx --slide 1 --include-text --include-bounds",
					"ooxml --json pptx slides selectors deck.pptx --slide 1",
					"ooxml --json pptx shapes show deck.pptx --slide 1",
					"ooxml --json pptx charts list deck.pptx",
					"ooxml --json pptx charts show deck.pptx --slide 1 --chart chart:1",
					"ooxml --json pptx tables show deck.pptx --slide 1 --target table:1",
					"ooxml --json pptx layouts show deck.pptx --layout 1",
				},
			},
			{
				Name: "PPTX mutate",
				Commands: []string{
					"ooxml --json pptx clone-slide deck.pptx --slide 1 --out edited.pptx",
					"ooxml --json pptx slides show edited.pptx --slide 2 --include-text --include-bounds",
					"ooxml --json pptx replace text deck.pptx --slide 1 --target title --text NEW --out edited.pptx",
					"ooxml --json pptx replace text-occurrences deck.pptx --match-text \"Old Client\" --new-text \"New Client\" --expect-count 12 --dry-run",
					"ooxml --json pptx replace text-occurrences deck.pptx --match-text \"Old Client\" --new-text \"New Client\" --expect-count 12 --expect-plan-hash sha256:... --out edited.pptx",
					"ooxml --json pptx tables set-cell deck.pptx --slide 1 --target table:1 --row 1 --col 1 --text Value --out edited.pptx",
					"ooxml --json pptx tables update-from-xlsx deck.pptx --workbook workbook.xlsx --sheet Sheet1 --range A1:C5 --expect-source-range A1:C5 --slide 1 --target table:1 --out edited.pptx",
					"ooxml --json pptx charts update-data deck.pptx --slide 1 --chart chart:1 --series 1 --values-json '[\"150\",\"175\",\"210\"]' --categories-json '[\"North\",\"South\",\"West\"]' --expect-point-count 3 --expect-values-hash sha256:... --out edited.pptx",
					"ooxml --json xlsx tables show workbook.xlsx --table Sales",
					"ooxml --json pptx place table-from-xlsx deck.pptx --workbook workbook.xlsx --table Sales --expect-source-range A1:C5 --slide 1 --x 0 --y 0 --cx 4000000 --out edited.pptx",
					"ooxml --json pptx place image deck.pptx --slide 1 --image hero.png --x 0 --y 0 --cx 4000000 --cy 2250000 --fit-mode cover --out edited.pptx",
					"ooxml --json pptx replace images deck.pptx --slide 2 --target shape:2 --image hero.png --fit-mode contain --out edited.pptx",
					"ooxml --json pptx shapes set-bounds deck.pptx --slide 2 --target body --bounds 914400,914400,7315200,3657600 --out edited.pptx",
					"ooxml --json pptx xlsx-bindings plan deck.pptx --workbook workbook.xlsx --table DeckBindings",
					"ooxml --json pptx xlsx-bindings apply deck.pptx --workbook workbook.xlsx --table DeckBindings --out edited.pptx",
					"ooxml --json pptx xlsx-bindings apply deck.pptx --workbook workbook.xlsx --table DeckImageBindings --dry-run",
					"ooxml --json pptx xlsx-bindings apply deck.pptx --workbook workbook.xlsx --table DeckBoundsBindings --out edited.pptx",
					"ooxml pptx render edited.pptx --out render-check",
					"ooxml validate --strict edited.pptx",
				},
			},
			{
				Name: "XLSX read",
				Commands: []string{
					"ooxml --json xlsx sheets list workbook.xlsx",
					"ooxml --json xlsx sheets show workbook.xlsx --sheet Sheet1",
					"ooxml --json xlsx cells extract workbook.xlsx --sheet Sheet1 --range A1:C10",
					"ooxml --json xlsx ranges export workbook.xlsx --sheet Sheet1 --range A1:C10 --include-types",
					"ooxml --json xlsx ranges set-format workbook.xlsx --sheet Sheet1 --range B2:B20 --preset currency --out edited.xlsx",
					"ooxml --json xlsx tables list workbook.xlsx --sheet Sheet1",
					"ooxml --json xlsx tables show workbook.xlsx --table Sales",
					"ooxml --json xlsx charts list workbook.xlsx",
					"ooxml --json xlsx charts show workbook.xlsx --chart chart:1",
					"ooxml --json xlsx pivots list workbook.xlsx",
					"ooxml --json xlsx pivots show workbook.xlsx --pivot pivot:1",
					"ooxml --json xlsx names list workbook.xlsx",
					"ooxml --json xlsx names show workbook.xlsx --name SalesData",
				},
			},
			{
				Name: "XLSX mutate",
				Commands: []string{
					"ooxml --json xlsx cells set workbook.xlsx --sheet Sheet1 --cell B2 --value '42' --out edited.xlsx",
					"ooxml --json xlsx ranges set workbook.xlsx --sheet Sheet1 --anchor A1 --values '[[\"A\",\"B\"],[1,2]]' --out edited.xlsx",
					"ooxml --json xlsx ranges set-format workbook.xlsx --sheet Sheet1 --range B2:B20 --preset currency --out edited.xlsx",
					"ooxml --json xlsx sheets rename workbook.xlsx --sheet OldName --name NewName --out edited.xlsx",
					"ooxml --json xlsx names add workbook.xlsx --name SalesData --sheet Sheet1 --range A1:C10 --out edited.xlsx",
					"ooxml --json xlsx names update edited.xlsx --name SalesData --sheet Sheet1 --range A1:D10 --expect-ref \"'Sheet1'!\\$A\\$1:\\$C\\$10\" --out edited.xlsx",
					"ooxml --json xlsx names delete edited.xlsx --name SalesData --expect-ref \"'Sheet1'!\\$A\\$1:\\$D\\$10\" --out edited.xlsx",
					"ooxml --json xlsx charts update-source workbook.xlsx --chart chart:1 --series 1 --role values --source-sheet Sheet1 --source-range '$B$2:$B$20' --expect-source-range '$B$2:$B$10' --out edited.xlsx",
					"ooxml --json xlsx tables append-rows workbook.xlsx --table Sales --values '[[\"Q1\",100]]' --out edited.xlsx",
					"ooxml validate --strict edited.xlsx",
				},
			},
			{
				Name: "VBA project and module operations",
				Commands: []string{
					"ooxml --json vba inspect workbook.xlsm",
					"ooxml --json vba create workbook.xlsm --family xlsx --source macros/Module1.bas --source macros/Worker.cls --extract-bin vbaProject.bin --enable-vba-object-model-access --force",
					"ooxml --json vba create deck.pptm --family pptx --source macros/Module1.bas --force",
					"ooxml --json vba extract-bin workbook.xlsm --out vbaProject.bin",
					"ooxml --json vba list workbook.xlsm",
					"ooxml --json vba extract workbook.xlsm --out-dir macros",
					"ooxml --json vba replace-module workbook.xlsm --module Module1 --source macros/Module1.bas --expect-sha256 <sha256-from-list> --out edited.xlsm",
					"ooxml --json vba add-module source-only.xlsm --source macros/NewModule.bas --expect-module-count 2 --out added-source-only.xlsm",
					"ooxml --json vba remove-module source-only-edited.xlsm --module Module1 --expect-sha256 <sha256-from-list> --out removed-module.xlsm",
					"ooxml --json vba attach target.xlsx --bin office-authored-vbaProject.bin --out target-with-vba.xlsm",
					"ooxml --json vba inspect target-with-vba.xlsm",
					"ooxml validate --strict target-with-vba.xlsm",
					"ooxml --json vba office-check target-with-vba.xlsm",
					"ooxml --json vba remove target.xlsm --out target-no-vba.xlsx",
					"ooxml --json vba inspect target-no-vba.xlsx",
				},
			},
		},
		Warnings: []string{
			"ooxml vba create can create fresh Office-authored XLSM/PPTM files from .bas/.cls sources on Windows desktop Office; other VBA commands inspect/extract source modules, replace existing modules, and add/remove modules only for synthetic/source-only projects. Real Office-shaped module-set changes are refused, so use vba create or an Office-authored vbaProject.bin plus attach for those edits. vba office-check provides local LibreOffice/soffice open-check evidence, but it does not execute or compile macros and is not Microsoft Office proof.",
			"After PPTX edits, validate and render before treating the file as user-ready.",
			"`pptx replace text-occurrences` updates slide-visible shape/table text only; notes, masters, layouts, charts, comments, and split-run phrase matches need explicit follow-up commands.",
			"After XLSX edits, validate and inspect the touched sheet/table before handing off.",
		},
	}
}

func outputRobotDocsGuideText(cmd *cobra.Command, guide robotDocsGuide) error {
	var b strings.Builder
	fmt.Fprintf(&b, "ooxml agent guide\n")
	fmt.Fprintf(&b, "Version: %s\n\n", guide.Version)

	fmt.Fprintf(&b, "Principles:\n")
	for _, principle := range guide.Principles {
		fmt.Fprintf(&b, "  - %s\n", principle)
	}

	for _, section := range guide.Sections {
		fmt.Fprintf(&b, "\n%s:\n", section.Name)
		for _, command := range section.Commands {
			fmt.Fprintf(&b, "  %s\n", command)
		}
	}

	fmt.Fprintf(&b, "\nWarnings:\n")
	for _, warning := range guide.Warnings {
		fmt.Fprintf(&b, "  - %s\n", warning)
	}

	return writeGlobalOutput(cmd, []byte(b.String()))
}

func init() {
	robotDocsCmd.AddCommand(robotDocsGuideCmd)
	rootCmd.AddCommand(robotDocsCmd)
}
