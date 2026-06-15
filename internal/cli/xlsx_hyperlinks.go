package cli

import (
	"fmt"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

var xlsxHyperlinksCmd = &cobra.Command{
	Use:     "hyperlinks",
	Aliases: []string{"hyperlink", "links"},
	Short:   "Inspect and mutate worksheet hyperlinks",
	Long:    "Commands for listing, showing, adding, updating, and deleting worksheet hyperlinks for cells and ranges.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

type XLSXHyperlinkJSON struct {
	Ref             string   `json:"ref"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	URL             string   `json:"url,omitempty"`
	Location        string   `json:"location,omitempty"`
	Display         string   `json:"display,omitempty"`
	Tooltip         string   `json:"tooltip,omitempty"`
	RelID           string   `json:"relId,omitempty"`
	Broken          bool     `json:"broken,omitempty"`
}

func hyperlinkToJSON(h mutate.Hyperlink) XLSXHyperlinkJSON {
	return XLSXHyperlinkJSON{
		Ref:             h.Ref,
		PrimarySelector: h.Ref,
		Selectors:       xlsxCellSelectors(h.Ref),
		URL:             h.URL,
		Location:        h.Location,
		Display:         h.Display,
		Tooltip:         h.Tooltip,
		RelID:           h.RelID,
		Broken:          h.Broken,
	}
}

type XLSXHyperlinksListResult struct {
	File        string              `json:"file"`
	Sheet       string              `json:"sheet"`
	SheetNumber int                 `json:"sheetNumber"`
	Count       int                 `json:"count"`
	Hyperlinks  []XLSXHyperlinkJSON `json:"hyperlinks"`
}

type XLSXHyperlinkMutationResult struct {
	File                  string             `json:"file"`
	Sheet                 string             `json:"sheet"`
	SheetNumber           int                `json:"sheetNumber"`
	Action                string             `json:"action"`
	Ref                   string             `json:"ref"`
	Hyperlink             *XLSXHyperlinkJSON `json:"hyperlink,omitempty"`
	Output                string             `json:"output,omitempty"`
	DryRun                bool               `json:"dryRun"`
	ValidateCommand       string             `json:"validateCommand,omitempty"`
	HyperlinksListCommand string             `json:"hyperlinksListCommand,omitempty"`
}

var (
	xlsxHyperlinksListSheet  string
	xlsxHyperlinksListBroken bool
	xlsxHyperlinksShowSheet  string
	xlsxHyperlinksShowCell   string
	xlsxHyperlinkSheet       string
	xlsxHyperlinkCell        string
	xlsxHyperlinkURL         string
	xlsxHyperlinkLocation    string
	xlsxHyperlinkDisplay     string
	xlsxHyperlinkTooltip     string
	xlsxHyperlinkReplace     bool
	xlsxHyperlinkExpectURL   string
	xlsxHyperlinkExpectLoc   string
)

// ---- list ----

var xlsxHyperlinksListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List worksheet hyperlinks",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxHyperlinksListSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		links, err := mutate.ListHyperlinks(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list hyperlinks: %v", err)
		}
		result := &XLSXHyperlinksListResult{File: filePath, Sheet: sheetRef.Name, SheetNumber: sheetRef.Number}
		for _, h := range links {
			if xlsxHyperlinksListBroken && !h.Broken {
				continue
			}
			result.Hyperlinks = append(result.Hyperlinks, hyperlinkToJSON(h))
		}
		result.Count = len(result.Hyperlinks)
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "hyperlinks list")
		}
		var b strings.Builder
		fmt.Fprintf(&b, "%d hyperlink(s) on %s:\n", result.Count, sheetRef.Name)
		for _, h := range result.Hyperlinks {
			target := h.URL
			if target == "" {
				target = h.Location
			}
			fmt.Fprintf(&b, "  %s -> %s\n", h.Ref, target)
		}
		return writeXLSXOutput(cmd, []byte(strings.TrimRight(b.String(), "\n")))
	},
}

// ---- show ----

var xlsxHyperlinksShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show a hyperlink on a cell or range",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		normRef, err := mutate.NormalizeHyperlinkRef(xlsxHyperlinksShowCell)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "invalid --cell: %v", err)
		}
		pkg, _, sheetRef, closeFn, err := resolveXLSXWorksheet(filePath, xlsxHyperlinksShowSheet)
		if err != nil {
			return err
		}
		defer closeFn()
		links, err := mutate.ListHyperlinks(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list hyperlinks: %v", err)
		}
		var refs []string
		var candidates []SelectorCandidate
		for _, h := range links {
			if hn, err := mutate.NormalizeHyperlinkRef(h.Ref); err == nil && hn == normRef {
				j := hyperlinkToJSON(h)
				if GetGlobalConfig(cmd).Format == "json" {
					return writeJSONResult(cmd, &j, "hyperlinks show")
				}
				target := j.URL
				if target == "" {
					target = j.Location
				}
				return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%s -> %s", j.Ref, target)))
			}
			refs = append(refs, h.Ref)
			candidates = append(candidates, SelectorCandidate{Primary: h.Ref, Selectors: xlsxCellSelectors(h.Ref)})
		}
		discovery := fmt.Sprintf("ooxml --json xlsx hyperlinks list <file> --sheet %s", pptxXLSXCommandArg(xlsxSheetSelectorForRef(sheetRef)))
		if len(refs) == 0 {
			return SelectorNotFoundError("hyperlink", normRef, nil, discovery)
		}
		return SelectorNotFoundError("hyperlink", normRef, BuildSelectorCandidates(candidates, normRef, maxSelectorCandidates), discovery)
	},
}

// ---- mutation helper ----

func runHyperlinkMutation(cmd *cobra.Command, filePath, sheetSel, action string, apply func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.HyperlinkMutationResult, error)) error {
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return err
	}
	var result *XLSXHyperlinkMutationResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSel)
		if err != nil {
			return err
		}
		if err := requireXLSXWorksheetRef(sheetRef); err != nil {
			return err
		}
		mutResult, err := apply(pkg, sheetRef)
		if err != nil {
			return NewCLIErrorf(ExitInvalidArgs, "failed to %s hyperlink: %v", action, err)
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		result = &XLSXHyperlinkMutationResult{
			File:        filePath,
			Sheet:       sheetRef.Name,
			SheetNumber: sheetRef.Number,
			Action:      action,
			Ref:         mutResult.Ref,
			Output:      destinationFile,
			DryRun:      mutOpts != nil && mutOpts.DryRun,
		}
		if action != "delete" {
			j := hyperlinkToJSON(mutResult.Hyperlink)
			result.Hyperlink = &j
		}
		if destinationFile != "" {
			selector := xlsxSheetSelectorForRef(sheetRef)
			result.ValidateCommand = xlsxValidateCommand(destinationFile)
			result.HyperlinksListCommand = fmt.Sprintf("ooxml --json xlsx hyperlinks list %s --sheet %s", pptxXLSXCommandArg(destinationFile), pptxXLSXCommandArg(selector))
		}
		return nil
	}); err != nil {
		return err
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return writeJSONResult(cmd, result, "hyperlinks "+action)
	}
	return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%sd hyperlink on %s!%s", action, result.Sheet, result.Ref)))
}

// ---- add ----

var xlsxHyperlinksAddCmd = &cobra.Command{
	Use:   "add <file>",
	Short: "Add a hyperlink to a cell or range",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxHyperlinkCell) == "" {
			return InvalidArgsError("--cell is required")
		}
		return runHyperlinkMutation(cmd, filePath, xlsxHyperlinkSheet, "add", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.HyperlinkMutationResult, error) {
			return mutate.AddHyperlink(&mutate.AddHyperlinkRequest{
				Package: pkg, SheetRef: sheet, Ref: xlsxHyperlinkCell,
				URL: xlsxHyperlinkURL, Location: xlsxHyperlinkLocation,
				Display: xlsxHyperlinkDisplay, Tooltip: xlsxHyperlinkTooltip,
				Replace: xlsxHyperlinkReplace,
			})
		})
	},
}

// ---- update ----

var xlsxHyperlinksUpdateCmd = &cobra.Command{
	Use:   "update <file>",
	Short: "Update an existing hyperlink",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxHyperlinkCell) == "" {
			return InvalidArgsError("--cell is required")
		}
		return runHyperlinkMutation(cmd, filePath, xlsxHyperlinkSheet, "update", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.HyperlinkMutationResult, error) {
			return mutate.UpdateHyperlink(&mutate.UpdateHyperlinkRequest{
				Package: pkg, SheetRef: sheet, Ref: xlsxHyperlinkCell,
				URL: xlsxHyperlinkURL, Location: xlsxHyperlinkLocation,
				Display: xlsxHyperlinkDisplay, Tooltip: xlsxHyperlinkTooltip,
				SetURL: cmd.Flags().Changed("url"), SetLocation: cmd.Flags().Changed("location"),
				SetDisplay: cmd.Flags().Changed("display"), SetTooltip: cmd.Flags().Changed("tooltip"),
				ExpectURL: xlsxHyperlinkExpectURL, HasExpectURL: cmd.Flags().Changed("expect-url"),
				ExpectLocation: xlsxHyperlinkExpectLoc, HasExpectLoc: cmd.Flags().Changed("expect-location"),
			})
		})
	},
}

// ---- delete ----

var xlsxHyperlinksDeleteCmd = &cobra.Command{
	Use:   "delete <file>",
	Short: "Delete a hyperlink from a cell or range",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if strings.TrimSpace(xlsxHyperlinkCell) == "" {
			return InvalidArgsError("--cell is required")
		}
		return runHyperlinkMutation(cmd, filePath, xlsxHyperlinkSheet, "delete", func(pkg opc.PackageSession, sheet model.SheetRef) (*mutate.HyperlinkMutationResult, error) {
			return mutate.DeleteHyperlink(&mutate.DeleteHyperlinkRequest{
				Package: pkg, SheetRef: sheet, Ref: xlsxHyperlinkCell,
				ExpectURL: xlsxHyperlinkExpectURL, HasExpectURL: cmd.Flags().Changed("expect-url"),
				ExpectLocation: xlsxHyperlinkExpectLoc, HasExpectLoc: cmd.Flags().Changed("expect-location"),
			})
		})
	},
}

func init() {
	xlsxHyperlinksListCmd.Flags().StringVar(&xlsxHyperlinksListSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxHyperlinksListCmd.Flags().BoolVar(&xlsxHyperlinksListBroken, "include-broken", false, "only list hyperlinks with missing relationship targets")
	xlsxHyperlinksCmd.AddCommand(xlsxHyperlinksListCmd)

	xlsxHyperlinksShowCmd.Flags().StringVar(&xlsxHyperlinksShowSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
	xlsxHyperlinksShowCmd.Flags().StringVar(&xlsxHyperlinksShowCell, "cell", "", "cell or range ref such as A1 or A1:B2")
	xlsxHyperlinksCmd.AddCommand(xlsxHyperlinksShowCmd)

	for _, c := range []*cobra.Command{xlsxHyperlinksAddCmd, xlsxHyperlinksUpdateCmd, xlsxHyperlinksDeleteCmd} {
		c.Flags().StringVar(&xlsxHyperlinkSheet, "sheet", "", "sheet number (1-based) or exact sheet name")
		c.Flags().StringVar(&xlsxHyperlinkCell, "cell", "", "cell or range ref such as A1 or A1:B2")
		AddMutationFlags(c)
	}
	xlsxHyperlinksAddCmd.Flags().StringVar(&xlsxHyperlinkURL, "url", "", "external URL target")
	xlsxHyperlinksAddCmd.Flags().StringVar(&xlsxHyperlinkLocation, "location", "", "internal location target such as Sheet2!A1")
	xlsxHyperlinksAddCmd.Flags().StringVar(&xlsxHyperlinkDisplay, "display", "", "display text hint")
	xlsxHyperlinksAddCmd.Flags().StringVar(&xlsxHyperlinkTooltip, "tooltip", "", "hover tooltip")
	xlsxHyperlinksAddCmd.Flags().BoolVar(&xlsxHyperlinkReplace, "replace", false, "replace an existing hyperlink on the same ref")
	xlsxHyperlinksCmd.AddCommand(xlsxHyperlinksAddCmd)

	xlsxHyperlinksUpdateCmd.Flags().StringVar(&xlsxHyperlinkURL, "url", "", "new external URL target")
	xlsxHyperlinksUpdateCmd.Flags().StringVar(&xlsxHyperlinkLocation, "location", "", "new internal location target")
	xlsxHyperlinksUpdateCmd.Flags().StringVar(&xlsxHyperlinkDisplay, "display", "", "new display text hint")
	xlsxHyperlinksUpdateCmd.Flags().StringVar(&xlsxHyperlinkTooltip, "tooltip", "", "new hover tooltip")
	xlsxHyperlinksUpdateCmd.Flags().StringVar(&xlsxHyperlinkExpectURL, "expect-url", "", "guard: require the current URL to match")
	xlsxHyperlinksUpdateCmd.Flags().StringVar(&xlsxHyperlinkExpectLoc, "expect-location", "", "guard: require the current location to match")
	xlsxHyperlinksCmd.AddCommand(xlsxHyperlinksUpdateCmd)

	xlsxHyperlinksDeleteCmd.Flags().StringVar(&xlsxHyperlinkExpectURL, "expect-url", "", "guard: require the current URL to match")
	xlsxHyperlinksDeleteCmd.Flags().StringVar(&xlsxHyperlinkExpectLoc, "expect-location", "", "guard: require the current location to match")
	xlsxHyperlinksCmd.AddCommand(xlsxHyperlinksDeleteCmd)

	xlsxCmd.AddCommand(xlsxHyperlinksCmd)
}
