package cli

import (
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

var tablesCmd = &cobra.Command{
	Use:     "tables",
	Aliases: []string{"table"},
	Short:   "Inspect and mutate PPTX tables",
	Long:    "Commands for inspecting and mutating PowerPoint table graphic frames.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

type PPTXTableSummary struct {
	File            string           `json:"file,omitempty"`
	Slide           int              `json:"slide"`
	ShapeID         int              `json:"shapeId"`
	ShapeName       string           `json:"shapeName"`
	TargetKind      string           `json:"targetKind"`
	PrimarySelector string           `json:"primarySelector"`
	Selectors       []string         `json:"selectors"`
	Rows            int              `json:"rows"`
	Cols            int              `json:"cols"`
	Cells           [][]string       `json:"cells"`
	Bounds          *model.Bounds    `json:"bounds,omitempty"`
	TableInfo       *model.TableInfo `json:"tableInfo,omitempty"`
}

func resolvePPTXSlideRef(pkg opc.PackageSession, slideNumber int) (*inspect.SlideRef, error) {
	graph, err := inspect.ParsePresentation(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
	}
	if slideNumber < 1 || slideNumber > len(graph.Slides) {
		return nil, InvalidArgsError(fmt.Sprintf("slide number %d out of range (1-%d)", slideNumber, len(graph.Slides)))
	}
	return &graph.Slides[slideNumber-1], nil
}

func collectPPTXTables(pkg opc.PackageSession, slideRef *inspect.SlideRef, tableID int, includeDetails bool) ([]PPTXTableSummary, error) {
	slideDoc, err := pkg.ReadXMLPart(slideRef.PartURI)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read slide %d: %v", slideRef.SlideNumber, err)
	}
	spTree := findPPTXShapeTree(slideDoc.Root())
	if spTree == nil {
		return nil, NewCLIErrorf(ExitUnexpected, "slide %d has no shape tree", slideRef.SlideNumber)
	}

	catalog, err := pptselectors.BuildSlideCatalog(pkg, slideRef.SlideNumber)
	if err != nil {
		return nil, mapPPTXShapeCatalogError(err)
	}
	targetsByShapeID := make(map[int]pptselectors.SlideSelectorTarget, len(catalog.Targets))
	for _, target := range catalog.Targets {
		targetsByShapeID[target.ShapeID] = target
	}

	var tables []PPTXTableSummary
	for _, shape := range inspect.EnumerateShapes(spTree) {
		if shape.TableInfo == nil {
			continue
		}
		if tableID > 0 && shape.ID != tableID {
			continue
		}
		target := targetsByShapeID[shape.ID]
		summary := PPTXTableSummary{
			Slide:           slideRef.SlideNumber,
			ShapeID:         shape.ID,
			ShapeName:       shape.Name,
			TargetKind:      nonEmpty(target.TargetKind, "table"),
			PrimarySelector: nonEmpty(target.PrimarySelector, fmt.Sprintf("shape:%d", shape.ID)),
			Selectors:       append([]string{}, target.Selectors...),
			Rows:            shape.TableInfo.Rows,
			Cols:            shape.TableInfo.Cols,
			Cells:           shape.TableInfo.Cells,
			Bounds:          shape.Bounds,
		}
		if includeDetails {
			summary.TableInfo = shape.TableInfo
		}
		tables = append(tables, summary)
	}
	if tableID > 0 && len(tables) == 0 {
		return nil, TargetNotFoundError(fmt.Sprintf("table shape ID %d on slide %d", tableID, slideRef.SlideNumber))
	}
	return tables, nil
}

func collectPPTXSingleTable(pkg opc.PackageSession, slideRef *inspect.SlideRef, tableID int) (*PPTXTableSummary, error) {
	tables, err := collectPPTXTables(pkg, slideRef, tableID, false)
	if err != nil {
		return nil, err
	}
	if len(tables) != 1 {
		return nil, TargetNotFoundError(fmt.Sprintf("table shape ID %d on slide %d", tableID, slideRef.SlideNumber))
	}
	return &tables[0], nil
}

func collectPPTXTableDestination(pkg opc.PackageSession, slideRef *inspect.SlideRef, tableID int, destinationFile string) (*PPTXTableSummary, error) {
	table, err := collectPPTXSingleTable(pkg, slideRef, tableID)
	if err != nil {
		return nil, err
	}
	table.File = destinationFile
	return table, nil
}

func resolvePPTXTableTarget(pkg opc.PackageSession, slideNumber, tableID int, target string) (int, error) {
	target = strings.TrimSpace(target)
	if tableID < 0 {
		return 0, InvalidArgsError("--table-id must be a positive integer")
	}
	if tableID > 0 && target != "" {
		return 0, InvalidArgsError("specify only one of --target or --table-id")
	}
	if target == "" {
		return tableID, nil
	}
	catalog, err := pptselectors.BuildSlideCatalog(pkg, slideNumber)
	if err != nil {
		return 0, mapPPTXShapeCatalogError(err)
	}
	resolved, err := catalog.ResolveTarget(target)
	if err != nil {
		return 0, mapPPTXShapeCatalogError(err)
	}
	if resolved.TargetKind != "table" {
		return 0, InvalidArgsError(fmt.Sprintf("target %q resolves to %s, not a table", target, resolved.PrimarySelector))
	}
	return resolved.ShapeID, nil
}

func resolveRequiredPPTXTableTarget(pkg opc.PackageSession, slideNumber, tableID int, target string) (int, error) {
	resolvedID, err := resolvePPTXTableTarget(pkg, slideNumber, tableID, target)
	if err != nil {
		return 0, err
	}
	if resolvedID < 1 {
		return 0, InvalidArgsError("must specify --target or --table-id")
	}
	return resolvedID, nil
}

func findPPTXShapeTree(root *etree.Element) *etree.Element {
	if root == nil {
		return nil
	}
	if spTree := root.FindElement(".//spTree"); spTree != nil {
		return spTree
	}
	if spTree := root.FindElement(".//p:spTree"); spTree != nil {
		return spTree
	}
	return nil
}

func resolveRequiredPPTXTableText(cmd *cobra.Command, textFlag, textFileFlag, textValue, textFileValue string) (string, error) {
	textChanged := cmd.Flags().Lookup(textFlag).Changed
	textFileChanged := cmd.Flags().Lookup(textFileFlag).Changed
	if textChanged == textFileChanged {
		return "", InvalidArgsError("must specify exactly one of --text or --text-file")
	}
	if textChanged {
		return textValue, nil
	}
	data, err := os.ReadFile(textFileValue)
	if err != nil {
		return "", FileNotFoundError(textFileValue)
	}
	return string(data), nil
}

func mapPPTXTableMutationError(err error) error {
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	msg := err.Error()
	switch {
	case strings.Contains(msg, "merge"), strings.Contains(msg, "cannot delete"):
		return InvalidArgsError(msg)
	case strings.Contains(msg, "dimension mismatch"), strings.Contains(msg, "source matrix"), strings.Contains(msg, "rectangular"):
		return InvalidArgsError(msg)
	case strings.Contains(msg, "not found"), strings.Contains(msg, "out of range"):
		return TargetNotFoundError(msg)
	default:
		return NewCLIErrorf(ExitUnexpected, "failed to mutate table: %v", err)
	}
}

func outputPPTXTablesJSON(cmd *cobra.Command, value any, label string) error {
	return writeLabeledJSON(cmd, value, label)
}

func writePPTXTablesText(cmd *cobra.Command, result *PPTXTablesShowResult) error {
	config := GetGlobalConfig(cmd)
	var out io.Writer
	if config.Output != "" {
		file, err := os.Create(config.Output)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
		}
		defer file.Close()
		out = file
	} else {
		out = cmd.OutOrStdout()
	}

	if len(result.Tables) == 0 {
		fmt.Fprintf(out, "No tables found on slide %d.\n", result.Slide)
		return nil
	}

	for _, table := range result.Tables {
		selector := nonEmpty(table.PrimarySelector, fmt.Sprintf("shape:%d", table.ShapeID))
		fmt.Fprintf(out, "Slide %d table %s %q id=%d: %dx%d\n", table.Slide, selector, table.ShapeName, table.ShapeID, table.Rows, table.Cols)
		for rowIndex, row := range table.Cells {
			fmt.Fprintf(out, "  %d: %s\n", rowIndex+1, strings.Join(row, "\t"))
		}
	}
	return nil
}

func parsePositiveIntFlag(value int, name string) error {
	if value < 1 {
		return InvalidArgsError("--" + name + " must be >= 1")
	}
	return nil
}

func init() {
	pptxCmd.AddCommand(tablesCmd)
}
