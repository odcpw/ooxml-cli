package cli

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"unicode/utf8"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	pptselectors "github.com/ooxml-cli/ooxml-cli/pkg/pptx/selectors"
	"github.com/spf13/cobra"
)

type PPTXXLSXBindingsResult struct {
	File          string                     `json:"file"`
	Output        string                     `json:"output,omitempty"`
	DryRun        bool                       `json:"dryRun,omitempty"`
	BindingSource XLSXRangeSource            `json:"bindingSource"`
	Operations    []PPTXXLSXBindingOperation `json:"operations"`
}

type PPTXXLSXBindingOperation struct {
	ID                string                       `json:"id,omitempty"`
	SourceRow         int                          `json:"sourceRow"`
	Op                string                       `json:"op"`
	Status            string                       `json:"status"`
	EquivalentCommand string                       `json:"equivalentCommand,omitempty"`
	ReadbackCommand   string                       `json:"readbackCommand,omitempty"`
	Source            *XLSXRangeSource             `json:"source,omitempty"`
	Text              *ReplaceTextFromXLSXText     `json:"text,omitempty"`
	Update            *PPTXTablesUpdateFromXLSXRun `json:"update,omitempty"`
	Image             *PPTXXLSXBindingImage        `json:"image,omitempty"`
	Bounds            *PPTXXLSXBindingBounds       `json:"bounds,omitempty"`
	Destination       any                          `json:"destination,omitempty"`
}

type PPTXXLSXBindingPlacePlan struct {
	Slide int    `json:"slide"`
	Name  string `json:"name,omitempty"`
	Rows  int    `json:"rows"`
	Cols  int    `json:"cols"`
	X     int64  `json:"x"`
	Y     int64  `json:"y"`
	CX    int64  `json:"cx"`
	CY    int64  `json:"cy,omitempty"`
}

type PPTXXLSXBindingImage struct {
	Path           string `json:"path"`
	ResolvedPath   string `json:"resolvedPath,omitempty"`
	ContentType    string `json:"contentType"`
	FitMode        string `json:"fitMode"`
	Bytes          int    `json:"bytes"`
	RelationshipID string `json:"relationshipId,omitempty"`
	TargetURI      string `json:"targetUri,omitempty"`
	OldTargetURI   string `json:"oldTargetUri,omitempty"`
	OldContentType string `json:"oldContentType,omitempty"`
	NewTargetURI   string `json:"newTargetUri,omitempty"`
	NewContentType string `json:"newContentType,omitempty"`
}

type PPTXXLSXBindingImagePlan struct {
	Slide  int    `json:"slide"`
	Target string `json:"target,omitempty"`
	Name   string `json:"name,omitempty"`
	X      int64  `json:"x,omitempty"`
	Y      int64  `json:"y,omitempty"`
	CX     int64  `json:"cx,omitempty"`
	CY     int64  `json:"cy,omitempty"`
}

type PPTXXLSXBindingBounds struct {
	X  int64 `json:"x"`
	Y  int64 `json:"y"`
	CX int64 `json:"cx"`
	CY int64 `json:"cy"`
}

type pptxXLSXBindingRow struct {
	SourceRow         int
	ID                string
	Op                string
	Slide             int
	Target            string
	SourceSheet       string
	SourceRange       string
	SourceTable       string
	ExpectSourceRange string
	FormulaMode       string
	Mode              string
	RowSep            string
	ColSep            string
	FitMode           string
	ImagePath         string
	ResolvedImagePath string
	X                 int64
	Y                 int64
	CX                int64
	CY                int64
	HasX              bool
	HasY              bool
	HasCX             bool
	HasCY             bool
	Name              string
	Header            bool
}

type preparedPPTXXLSXBinding struct {
	Row         pptxXLSXBindingRow
	Source      *XLSXRangeSource
	Matrix      [][]string
	Text        string
	TextOptions ReplaceTextFromXLSXText
	ImageData   []byte
	Image       PPTXXLSXBindingImage
	Plan        PPTXXLSXBindingOperation
}

var pptxXLSXBindingsCmd = &cobra.Command{
	Use:     "xlsx-bindings",
	Aliases: []string{"xlsx-binding"},
	Short:   "Plan and apply workbook-driven PPTX updates",
	Long:    "Plan and apply mixed PPTX updates from XLSX binding rows.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var (
	pptxXLSXBindingsWorkbook string
	pptxXLSXBindingsSheet    string
	pptxXLSXBindingsRange    string
	pptxXLSXBindingsTable    string
	pptxXLSXBindingsMaxCells int
)

var pptxXLSXBindingsPlanCmd = &cobra.Command{
	Use:   "plan <file>",
	Short: "Resolve an XLSX binding table without writing",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requirePPTXXLSXBindingsWorkbook(); err != nil {
			return err
		}
		result, err := planPPTXXLSXBindings(filePath)
		if err != nil {
			return err
		}
		return outputPPTXXLSXBindingsJSON(cmd, result)
	},
}

var pptxXLSXBindingsApplyCmd = &cobra.Command{
	Use:   "apply <file>",
	Short: "Apply an XLSX binding table to a PPTX deck",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if err := requirePPTXXLSXBindingsWorkbook(); err != nil {
			return err
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		result, err := applyPPTXXLSXBindings(filePath, mutOpts)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputPPTXXLSXBindingsJSON(cmd, result)
		}
		return writeCLIOutput(cmd, []byte(fmt.Sprintf("applied %d XLSX bindings", len(result.Operations))))
	},
}

func requirePPTXXLSXBindingsWorkbook() error {
	if strings.TrimSpace(pptxXLSXBindingsWorkbook) == "" {
		return InvalidArgsError("--workbook is required")
	}
	if _, err := os.Stat(pptxXLSXBindingsWorkbook); err != nil {
		return FileNotFoundError(pptxXLSXBindingsWorkbook)
	}
	return nil
}

func planPPTXXLSXBindings(filePath string) (*PPTXXLSXBindingsResult, error) {
	bindingSource, prepared, err := preparePPTXXLSXBindings(filePath)
	if err != nil {
		return nil, err
	}
	operations := make([]PPTXXLSXBindingOperation, len(prepared))
	for i, item := range prepared {
		operations[i] = item.Plan
	}
	return &PPTXXLSXBindingsResult{
		File:          filePath,
		BindingSource: *bindingSource,
		Operations:    operations,
	}, nil
}

func applyPPTXXLSXBindings(filePath string, mutOpts *MutationOptions) (*PPTXXLSXBindingsResult, error) {
	bindingSource, prepared, err := preparePPTXXLSXBindings(filePath)
	if err != nil {
		return nil, err
	}
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypePPTX)
	if err != nil {
		return nil, err
	}

	destinationFile := mutationOutputPathForResult(filePath, mutOpts)
	var operations []PPTXXLSXBindingOperation
	if err := writer.Write(func(pkg opc.PackageSession) error {
		operations = make([]PPTXXLSXBindingOperation, 0, len(prepared))
		for _, item := range prepared {
			applied, err := applyPreparedPPTXXLSXBinding(pkg, item, destinationFile, mutOpts.DryRun)
			if err != nil {
				return err
			}
			operations = append(operations, applied)
		}
		return nil
	}); err != nil {
		if cliErr, ok := AsCLIError(err); ok {
			return nil, cliErr
		}
		return nil, NewCLIErrorf(ExitUnexpected, "failed to apply XLSX bindings: %v", err)
	}

	return &PPTXXLSXBindingsResult{
		File:          filePath,
		Output:        destinationFile,
		DryRun:        mutOpts.DryRun,
		BindingSource: *bindingSource,
		Operations:    operations,
	}, nil
}

func preparePPTXXLSXBindings(filePath string) (*XLSXRangeSource, []preparedPPTXXLSXBinding, error) {
	bindingSource, rows, err := loadPPTXXLSXBindingRows()
	if err != nil {
		return nil, nil, err
	}
	pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
	if err != nil {
		return nil, nil, err
	}
	defer pkg.Close()

	prepared := make([]preparedPPTXXLSXBinding, 0, len(rows))
	seenTargets := map[string]int{}
	for _, row := range rows {
		item, err := preparePPTXXLSXBinding(pkg, filePath, row)
		if err != nil {
			return nil, nil, err
		}
		if key := duplicateTargetKey(item.Plan.Destination); key != "" {
			if previousRow, ok := seenTargets[key]; ok {
				return nil, nil, InvalidArgsError(fmt.Sprintf("row %d duplicates destination target from row %d: %s", row.SourceRow, previousRow, key))
			}
			seenTargets[key] = row.SourceRow
		}
		prepared = append(prepared, item)
	}
	return bindingSource, prepared, nil
}

func preparePPTXXLSXBinding(pkg opc.PackageSession, deckPath string, row pptxXLSXBindingRow) (preparedPPTXXLSXBinding, error) {
	formulaMode, err := normalizeXLSXFormulaMode(row.FormulaMode, "formulaMode")
	if err != nil {
		return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
	}
	row.FormulaMode = formulaMode

	item := preparedPPTXXLSXBinding{
		Row: row,
	}
	plan := PPTXXLSXBindingOperation{
		ID:        row.ID,
		SourceRow: row.SourceRow,
		Op:        row.Op,
		Status:    "planned",
	}

	switch row.Op {
	case "replace-text":
		source, stringsMatrix, err := loadPPTXXLSXBindingStringMatrix(row, formulaMode)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		item.Source = source
		item.Matrix = stringsMatrix
		plan.Source = source
		mode, err := normalizeReplaceTextFromXLSXMode(row.Mode)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		row.Mode = mode
		rowSep, err := decodeTextSeparatorFlag(defaultString(row.RowSep, "\n"), "rowSep")
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		colSep, err := decodeTextSeparatorFlag(defaultString(row.ColSep, "\t"), "colSep")
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		text := joinXLSXTextMatrix(stringsMatrix, rowSep, colSep)
		destination, err := collectPPTXTextShapeDestination(pkg, row.Slide, row.Target, "", true, true)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		item.Text = text
		item.TextOptions = ReplaceTextFromXLSXText{
			Mode:         mode,
			FormulaMode:  formulaMode,
			RowSeparator: rowSep,
			ColSeparator: colSep,
			Chars:        utf8.RuneCountInString(text),
			Value:        text,
		}
		plan.Text = &item.TextOptions
		plan.Destination = destination
		plan.ReadbackCommand = fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s --include-text", outputPlaceholder(), row.Slide, destination.PrimarySelector)
		plan.EquivalentCommand = equivalentPPTXXLSXBindingCommand(deckPath, row)
	case "update-table":
		source, stringsMatrix, err := loadPPTXXLSXBindingStringMatrix(row, formulaMode)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		item.Source = source
		item.Matrix = stringsMatrix
		plan.Source = source
		slideRef, destination, err := resolveBindingTableDestination(pkg, row)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		_ = slideRef
		if destination.Rows != source.Rows || destination.Cols != source.Cols {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError(fmt.Sprintf("source/destination dimension mismatch: source is %dx%d, destination table is %dx%d", source.Rows, source.Cols, destination.Rows, destination.Cols)))
		}
		plan.Update = &PPTXTablesUpdateFromXLSXRun{FormulaMode: formulaMode, UpdatedCells: source.Rows * source.Cols}
		plan.Destination = destination
		plan.ReadbackCommand = fmt.Sprintf("ooxml --json pptx tables show %s --slide %d --target %s", outputPlaceholder(), row.Slide, destination.PrimarySelector)
		plan.EquivalentCommand = equivalentPPTXXLSXBindingCommand(deckPath, row)
	case "place-table":
		source, stringsMatrix, err := loadPPTXXLSXBindingStringMatrix(row, formulaMode)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		item.Source = source
		item.Matrix = stringsMatrix
		plan.Source = source
		if _, err := resolvePPTXSlideRef(pkg, row.Slide); err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		if len(stringsMatrix) == 0 || len(stringsMatrix[0]) == 0 {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("source range is empty"))
		}
		if row.CX <= 0 {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("cx must be positive for place-table"))
		}
		plan.Destination = &PPTXXLSXBindingPlacePlan{
			Slide: row.Slide,
			Name:  row.Name,
			Rows:  source.Rows,
			Cols:  source.Cols,
			X:     row.X,
			Y:     row.Y,
			CX:    row.CX,
			CY:    row.CY,
		}
		plan.EquivalentCommand = equivalentPPTXXLSXBindingCommand(deckPath, row)
	case "place-image":
		if _, err := resolvePPTXSlideRef(pkg, row.Slide); err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		if row.CX <= 0 || row.CY <= 0 {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("cx and cy must be positive for place-image"))
		}
		image, imageData, err := preparePPTXXLSXBindingImage(row)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		row.ImagePath = image.Path
		row.ResolvedImagePath = image.ResolvedPath
		row.FitMode = image.FitMode
		item.Image = *image
		item.ImageData = imageData
		plan.Image = image
		plan.Destination = &PPTXXLSXBindingImagePlan{
			Slide: row.Slide,
			Name:  row.Name,
			X:     row.X,
			Y:     row.Y,
			CX:    row.CX,
			CY:    row.CY,
		}
		plan.EquivalentCommand = equivalentPPTXXLSXBindingCommand(deckPath, row)
	case "replace-image":
		if row.Target == "" {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("target is required for replace-image"))
		}
		destination, err := collectPPTXShapeDestination(pkg, row.Slide, row.Target, "", false, true)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		if destination.ImageRef == nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError(fmt.Sprintf("target %s is not an image shape", row.Target)))
		}
		row.Target = fmt.Sprintf("shape:%d", destination.ShapeID)
		image, imageData, err := preparePPTXXLSXBindingImage(row)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		row.ImagePath = image.Path
		row.ResolvedImagePath = image.ResolvedPath
		row.FitMode = image.FitMode
		item.Image = *image
		item.ImageData = imageData
		plan.Image = image
		plan.Destination = destination
		plan.ReadbackCommand = fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s --include-bounds", outputPlaceholder(), row.Slide, destination.PrimarySelector)
		plan.EquivalentCommand = equivalentPPTXXLSXBindingCommand(deckPath, row)
	case "set-bounds":
		if row.Target == "" {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("target is required for set-bounds"))
		}
		if !row.HasX || !row.HasY || !row.HasCX || !row.HasCY {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("x, y, cx, and cy are required for set-bounds"))
		}
		if row.CX <= 0 || row.CY <= 0 {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("cx and cy must be positive for set-bounds"))
		}
		destination, err := collectPPTXShapeDestination(pkg, row.Slide, row.Target, "", false, true)
		if err != nil {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, err)
		}
		if destination.TargetKind == "group" {
			return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError(fmt.Sprintf("group shape bounds mutation is not supported: %s", destination.PrimarySelector)))
		}
		row.Target = destination.PrimarySelector
		plan.Bounds = &PPTXXLSXBindingBounds{X: row.X, Y: row.Y, CX: row.CX, CY: row.CY}
		plan.Destination = destination
		plan.ReadbackCommand = fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s --include-bounds", outputPlaceholder(), row.Slide, destination.PrimarySelector)
		plan.EquivalentCommand = equivalentPPTXXLSXBindingCommand(deckPath, row)
	default:
		return preparedPPTXXLSXBinding{}, bindingRowError(row, InvalidArgsError("op must be replace-text, update-table, place-table, place-image, replace-image, or set-bounds"))
	}
	item.Row = row
	item.Plan = plan
	return item, nil
}

func loadPPTXXLSXBindingStringMatrix(row pptxXLSXBindingRow, formulaMode string) (*XLSXRangeSource, [][]string, error) {
	source, matrix, err := loadXLSXRangeOrTableSourceForCLI(pptxXLSXBindingsWorkbook, row.SourceSheet, row.SourceRange, row.SourceTable, pptxXLSXBindingsMaxCells)
	if err != nil {
		return nil, nil, err
	}
	if err := checkExpectedXLSXSourceRange(source.Range, row.ExpectSourceRange); err != nil {
		return nil, nil, err
	}
	return source, xlsxRangeStringsFromMatrix(matrix, formulaMode), nil
}

func preparePPTXXLSXBindingImage(row pptxXLSXBindingRow) (*PPTXXLSXBindingImage, []byte, error) {
	imagePath := strings.TrimSpace(row.ImagePath)
	if imagePath == "" {
		return nil, nil, InvalidArgsError("imagePath is required for image bindings")
	}
	resolvedPath := imagePath
	if !filepath.IsAbs(resolvedPath) {
		resolvedPath = filepath.Join(filepath.Dir(pptxXLSXBindingsWorkbook), imagePath)
	}
	if _, err := os.Stat(resolvedPath); err != nil {
		return nil, nil, FileNotFoundError(resolvedPath)
	}
	data, err := os.ReadFile(resolvedPath)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to read image file: %v", err)
	}
	fitMode, err := mutate.ParseFitMode(defaultString(row.FitMode, "contain"))
	if err != nil {
		return nil, nil, InvalidArgsError(err.Error())
	}
	contentType, err := getImageContentType(resolvedPath)
	if err != nil {
		return nil, nil, err
	}
	return &PPTXXLSXBindingImage{
		Path:         imagePath,
		ResolvedPath: resolvedPath,
		ContentType:  contentType,
		FitMode:      string(fitMode),
		Bytes:        len(data),
	}, data, nil
}

func collectPPTXTextShapeDestination(pkg opc.PackageSession, slide int, targetSelector string, destinationFile string, includeText, includeBounds bool) (*PPTXShapeDestination, error) {
	catalog, err := pptselectors.BuildSlideCatalog(pkg, slide)
	if err != nil {
		return nil, mapPPTXShapeCatalogError(err)
	}
	target, err := catalog.ResolveTarget(targetSelector)
	if err != nil {
		return nil, mapPPTXShapeCatalogError(err)
	}
	if !target.TextCapable {
		return nil, InvalidArgsError(fmt.Sprintf("target %s resolves to a non-text %s shape", targetSelector, target.TargetKind))
	}
	return collectPPTXShapeDestination(pkg, slide, targetSelector, destinationFile, includeText, includeBounds)
}

func applyPreparedPPTXXLSXBinding(pkg opc.PackageSession, item preparedPPTXXLSXBinding, destinationFile string, dryRun bool) (PPTXXLSXBindingOperation, error) {
	row := item.Row
	op := item.Plan
	op.Status = "applied"
	if dryRun {
		op.Status = "dry-run"
	}
	op.Destination = nil
	op.ReadbackCommand = ""

	switch row.Op {
	case "replace-text":
		request := &mutate.ReplaceTextRequest{
			Package:     pkg,
			SlideNumber: row.Slide,
			Target:      row.Target,
			NewText:     item.Text,
			Mode:        item.TextOptions.Mode,
		}
		if err := mutate.ReplaceText(request); err != nil {
			return op, bindingRowError(row, mapReplaceTextFromXLSXMutationError(err, row.Target))
		}
		destination, err := collectPPTXShapeDestination(pkg, row.Slide, row.Target, destinationFile, true, true)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		op.Text = &item.TextOptions
		op.Destination = destination
		op.ReadbackCommand = fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s --include-text", destinationFileOrPlaceholder(destinationFile), row.Slide, destination.PrimarySelector)
	case "update-table":
		slideRef, destination, err := resolveBindingTableDestination(pkg, row)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		update, err := mutate.SetTableTextMatrix(&mutate.SetTableTextMatrixRequest{
			Package:  pkg,
			SlideRef: slideRef,
			TableID:  destination.ShapeID,
			Data:     item.Matrix,
		})
		if err != nil {
			return op, bindingRowError(row, mapPPTXTableMutationError(err))
		}
		updatedDestination, err := collectPPTXTableDestination(pkg, slideRef, destination.ShapeID, destinationFile)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		op.Update = &PPTXTablesUpdateFromXLSXRun{
			FormulaMode:  item.Plan.Update.FormulaMode,
			UpdatedCells: update.UpdatedCells,
			ChangedCells: update.ChangedCells,
		}
		op.Destination = updatedDestination
		op.ReadbackCommand = fmt.Sprintf("ooxml --json pptx tables show %s --slide %d --target %s", destinationFileOrPlaceholder(destinationFile), row.Slide, updatedDestination.PrimarySelector)
	case "place-table":
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return op, NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}
		if row.Slide > len(graph.Slides) {
			return op, bindingRowError(row, InvalidArgsError(fmt.Sprintf("slide %d out of range (1-%d)", row.Slide, len(graph.Slides))))
		}
		slideRef := graph.Slides[row.Slide-1]
		inserted, err := mutate.InsertTable(&mutate.InsertTableRequest{
			Package:         pkg,
			SlideRef:        &slideRef,
			Data:            item.Matrix,
			X:               row.X,
			Y:               row.Y,
			Width:           row.CX,
			Height:          row.CY,
			HasHeader:       row.Header,
			HeaderFillColor: "4472C4",
			BandFill1Color:  "D9E1F2",
			DefaultFontSize: 18,
			BorderColor:     "000000",
			BorderWidth:     19050,
			ShapeName:       row.Name,
		})
		if err != nil {
			return op, bindingRowError(row, NewCLIErrorf(ExitUnexpected, "failed to insert table: %v", err))
		}
		summary, err := collectPPTXSingleTable(pkg, &slideRef, inserted.ShapeID)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		destination := PlaceTableDestination{
			File:            destinationFile,
			Slide:           row.Slide,
			ShapeID:         inserted.ShapeID,
			ShapeName:       inserted.ShapeName,
			PrimarySelector: summary.PrimarySelector,
			Selectors:       append([]string{}, summary.Selectors...),
			Rows:            summary.Rows,
			Cols:            summary.Cols,
			Cells:           summary.Cells,
			X:               row.X,
			Y:               row.Y,
			CX:              inserted.Width,
			CY:              inserted.Height,
		}
		op.Destination = destination
		op.ReadbackCommand = fmt.Sprintf("ooxml --json pptx tables show %s --slide %d --target %s", destinationFileOrPlaceholder(destinationFile), row.Slide, destination.PrimarySelector)
	case "place-image":
		graph, err := inspect.ParsePresentation(pkg)
		if err != nil {
			return op, NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
		}
		if row.Slide > len(graph.Slides) {
			return op, bindingRowError(row, InvalidArgsError(fmt.Sprintf("slide %d out of range (1-%d)", row.Slide, len(graph.Slides))))
		}
		slideRef := graph.Slides[row.Slide-1]
		inserted, err := mutate.InsertImage(&mutate.InsertImageRequest{
			Package:     pkg,
			SlideRef:    &slideRef,
			ImageData:   item.ImageData,
			ContentType: item.Image.ContentType,
			FitMode:     mutate.FitMode(item.Image.FitMode),
			X:           row.X,
			Y:           row.Y,
			CX:          row.CX,
			CY:          row.CY,
			Name:        row.Name,
		})
		if err != nil {
			return op, bindingRowError(row, NewCLIErrorf(ExitUnexpected, "failed to insert image: %v", err))
		}
		destination, err := collectPPTXShapeDestination(pkg, row.Slide, fmt.Sprintf("shape:%d", inserted.ShapeID), destinationFile, false, true)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		image := item.Image
		image.RelationshipID = inserted.RelationshipID
		image.TargetURI = inserted.TargetURI
		op.Image = &image
		op.Destination = destination
		op.ReadbackCommand = fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s --include-bounds", destinationFileOrPlaceholder(destinationFile), row.Slide, destination.PrimarySelector)
	case "replace-image":
		slideRef, err := resolvePPTXSlideRef(pkg, row.Slide)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		selector, err := pptselectors.Parse(row.Target)
		if err != nil {
			return op, bindingRowError(row, InvalidArgsError(fmt.Sprintf("invalid target selector: %v", err)))
		}
		replaced, err := mutate.ReplaceImage(selector, slideRef, pkg, mutate.ImageReplaceOptions{
			FitMode:             mutate.FitMode(item.Image.FitMode),
			NewImageData:        item.ImageData,
			NewImageContentType: item.Image.ContentType,
		})
		if err != nil {
			return op, bindingRowError(row, mapPPTXXLSXBindingReplaceImageError(err, row.Target))
		}
		destination, err := collectPPTXShapeDestination(pkg, row.Slide, row.Target, destinationFile, false, true)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		image := item.Image
		image.RelationshipID = replaced.RelationshipID
		image.OldTargetURI = replaced.OldTargetURI
		image.OldContentType = replaced.OldContentType
		image.NewTargetURI = replaced.NewTargetURI
		image.NewContentType = replaced.NewContentType
		image.TargetURI = replaced.NewTargetURI
		op.Image = &image
		op.Destination = destination
		op.ReadbackCommand = fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s --include-bounds", destinationFileOrPlaceholder(destinationFile), row.Slide, destination.PrimarySelector)
	case "set-bounds":
		updated, err := mutate.SetSlideShapeBounds(&mutate.SetSlideShapeBoundsRequest{
			Package:     pkg,
			SlideNumber: row.Slide,
			Target:      row.Target,
			X:           row.X,
			Y:           row.Y,
			CX:          row.CX,
			CY:          row.CY,
		})
		if err != nil {
			return op, bindingRowError(row, mapPPTXShapesMutationError(err))
		}
		destination, err := collectPPTXShapeDestination(pkg, updated.Slide, updated.Target, destinationFile, false, true)
		if err != nil {
			return op, bindingRowError(row, err)
		}
		op.Bounds = &PPTXXLSXBindingBounds{X: updated.NewX, Y: updated.NewY, CX: updated.NewCX, CY: updated.NewCY}
		op.Destination = destination
		op.ReadbackCommand = fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s --include-bounds", destinationFileOrPlaceholder(destinationFile), updated.Slide, destination.PrimarySelector)
	}
	return op, nil
}

func mapPPTXXLSXBindingReplaceImageError(err error, target string) error {
	if err == nil {
		return nil
	}
	msg := err.Error()
	if isReplaceImageSearchMiss(err) || strings.Contains(msg, "target not found") {
		return TargetNotFoundError(target)
	}
	if strings.Contains(msg, "not supported for image replacement") || strings.Contains(msg, "not an image") {
		return InvalidArgsError(msg)
	}
	return err
}

func resolveBindingTableDestination(pkg opc.PackageSession, row pptxXLSXBindingRow) (*inspect.SlideRef, *PPTXTableSummary, error) {
	slideRef, err := resolvePPTXSlideRef(pkg, row.Slide)
	if err != nil {
		return nil, nil, err
	}
	tableID, err := resolveRequiredPPTXTableTarget(pkg, row.Slide, 0, row.Target)
	if err != nil {
		return nil, nil, err
	}
	destination, err := collectPPTXSingleTable(pkg, slideRef, tableID)
	if err != nil {
		return nil, nil, err
	}
	return slideRef, destination, nil
}

func loadPPTXXLSXBindingRows() (*XLSXRangeSource, []pptxXLSXBindingRow, error) {
	source, matrix, err := loadXLSXRangeOrTableSourceForCLI(pptxXLSXBindingsWorkbook, pptxXLSXBindingsSheet, pptxXLSXBindingsRange, pptxXLSXBindingsTable, pptxXLSXBindingsMaxCells)
	if err != nil {
		return nil, nil, err
	}
	values := xlsxRangeStringsFromMatrix(matrix, "value")
	if len(values) < 2 {
		return nil, nil, InvalidArgsError("binding source must include a header row and at least one operation row")
	}
	columns := indexPPTXXLSXBindingHeaders(values[0])
	rows := make([]pptxXLSXBindingRow, 0, len(values)-1)
	for rowIdx := 1; rowIdx < len(values); rowIdx++ {
		row, err := parsePPTXXLSXBindingRow(values[rowIdx], columns, rowIdx+1)
		if err != nil {
			return nil, nil, err
		}
		rows = append(rows, row)
	}
	return source, rows, nil
}

func parsePPTXXLSXBindingRow(values []string, columns map[string]int, sourceRow int) (pptxXLSXBindingRow, error) {
	row := pptxXLSXBindingRow{
		SourceRow:         sourceRow,
		ID:                bindingColumnValue(values, columns, "id"),
		Op:                normalizePPTXXLSXBindingOp(bindingColumnValue(values, columns, "op")),
		Target:            bindingColumnValue(values, columns, "target"),
		SourceSheet:       firstBindingColumnValue(values, columns, "sourceSheet", "sheet"),
		SourceRange:       firstBindingColumnValue(values, columns, "sourceRange", "range"),
		SourceTable:       firstBindingColumnValue(values, columns, "sourceTable", "table"),
		ExpectSourceRange: firstBindingColumnValue(values, columns, "expectSourceRange", "expectRange"),
		FormulaMode:       firstBindingColumnValue(values, columns, "formulaMode", "formula"),
		Mode:              bindingColumnValue(values, columns, "mode"),
		RowSep:            firstRawBindingColumnValue(values, columns, "rowSep", "rowSeparator"),
		ColSep:            firstRawBindingColumnValue(values, columns, "colSep", "colSeparator"),
		FitMode:           firstBindingColumnValue(values, columns, "fitMode", "imageFit"),
		ImagePath:         firstBindingColumnValue(values, columns, "imagePath", "image", "imageFile", "path"),
		Name:              bindingColumnValue(values, columns, "name"),
	}
	if row.FitMode == "" && (row.Op == "place-image" || row.Op == "replace-image") {
		row.FitMode = row.Mode
	}
	var err error
	row.Slide, err = parseRequiredBindingInt(values, columns, "slide", sourceRow)
	if err != nil {
		return row, err
	}
	row.X, row.HasX, err = parseOptionalBindingInt64WithPresence(values, columns, "x", sourceRow)
	if err != nil {
		return row, err
	}
	row.Y, row.HasY, err = parseOptionalBindingInt64WithPresence(values, columns, "y", sourceRow)
	if err != nil {
		return row, err
	}
	row.CX, row.HasCX, err = parseOptionalBindingInt64WithPresence(values, columns, "cx", sourceRow)
	if err != nil {
		return row, err
	}
	row.CY, row.HasCY, err = parseOptionalBindingInt64WithPresence(values, columns, "cy", sourceRow)
	if err != nil {
		return row, err
	}
	row.Header, err = parseOptionalBindingBool(values, columns, "header", sourceRow)
	if err != nil {
		return row, err
	}
	if row.Op == "" {
		return row, InvalidArgsError(fmt.Sprintf("row %d: op is required", sourceRow))
	}
	if row.Target == "" && (row.Op == "replace-text" || row.Op == "update-table" || row.Op == "replace-image" || row.Op == "set-bounds") {
		return row, InvalidArgsError(fmt.Sprintf("row %d: target is required for %s", sourceRow, row.Op))
	}
	return row, nil
}

func indexPPTXXLSXBindingHeaders(header []string) map[string]int {
	out := map[string]int{}
	for idx, value := range header {
		key := normalizePPTXXLSXBindingHeader(value)
		if key == "" {
			continue
		}
		if _, exists := out[key]; !exists {
			out[key] = idx
		}
	}
	return out
}

func normalizePPTXXLSXBindingHeader(value string) string {
	value = strings.ToLower(strings.TrimSpace(value))
	value = strings.ReplaceAll(value, "_", "")
	value = strings.ReplaceAll(value, "-", "")
	value = strings.ReplaceAll(value, " ", "")
	return value
}

func normalizePPTXXLSXBindingOp(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "replace-text", "replacetext", "text":
		return "replace-text"
	case "update-table", "updatetable", "table-update":
		return "update-table"
	case "place-table", "placetable", "table-place":
		return "place-table"
	case "place-image", "placeimage", "image-place":
		return "place-image"
	case "replace-image", "replaceimage", "image-replace", "image":
		return "replace-image"
	case "set-bounds", "setbounds", "set-shape-bounds", "shapebounds", "shape-bounds", "bounds":
		return "set-bounds"
	default:
		return strings.ToLower(strings.TrimSpace(value))
	}
}

func bindingColumnValue(values []string, columns map[string]int, name string) string {
	return strings.TrimSpace(rawBindingColumnValue(values, columns, name))
}

func rawBindingColumnValue(values []string, columns map[string]int, name string) string {
	idx, ok := columns[normalizePPTXXLSXBindingHeader(name)]
	if !ok || idx >= len(values) {
		return ""
	}
	return values[idx]
}

func firstBindingColumnValue(values []string, columns map[string]int, names ...string) string {
	for _, name := range names {
		value := bindingColumnValue(values, columns, name)
		if value != "" {
			return value
		}
	}
	return ""
}

func firstRawBindingColumnValue(values []string, columns map[string]int, names ...string) string {
	for _, name := range names {
		value := rawBindingColumnValue(values, columns, name)
		if value != "" {
			return value
		}
	}
	return ""
}

func parseRequiredBindingInt(values []string, columns map[string]int, name string, sourceRow int) (int, error) {
	value := bindingColumnValue(values, columns, name)
	if value == "" {
		return 0, InvalidArgsError(fmt.Sprintf("row %d: %s is required", sourceRow, name))
	}
	parsed, err := strconv.Atoi(value)
	if err != nil || parsed < 1 {
		return 0, InvalidArgsError(fmt.Sprintf("row %d: %s must be a positive integer", sourceRow, name))
	}
	return parsed, nil
}

func parseOptionalBindingInt64WithPresence(values []string, columns map[string]int, name string, sourceRow int) (int64, bool, error) {
	value := bindingColumnValue(values, columns, name)
	if value == "" {
		return 0, false, nil
	}
	parsed, err := strconv.ParseInt(value, 10, 64)
	if err != nil {
		return 0, true, InvalidArgsError(fmt.Sprintf("row %d: %s must be an integer", sourceRow, name))
	}
	return parsed, true, nil
}

func parseOptionalBindingBool(values []string, columns map[string]int, name string, sourceRow int) (bool, error) {
	value := strings.ToLower(bindingColumnValue(values, columns, name))
	switch value {
	case "":
		return false, nil
	case "1", "true", "yes", "y":
		return true, nil
	case "0", "false", "no", "n":
		return false, nil
	default:
		return false, InvalidArgsError(fmt.Sprintf("row %d: %s must be true or false", sourceRow, name))
	}
}

func duplicateTargetKey(destination any) string {
	switch dest := destination.(type) {
	case *PPTXShapeDestination:
		return fmt.Sprintf("slide:%d:%s", dest.Slide, dest.PrimarySelector)
	case *PPTXTableSummary:
		return fmt.Sprintf("slide:%d:%s", dest.Slide, dest.PrimarySelector)
	default:
		return ""
	}
}

func bindingRowError(row pptxXLSXBindingRow, err error) error {
	if err == nil {
		return nil
	}
	if cliErr, ok := AsCLIError(err); ok {
		return NewCLIErrorf(cliErr.ExitCode, "row %d: %s", row.SourceRow, cliErr.Message)
	}
	return NewCLIErrorf(ExitInvalidArgs, "row %d: %v", row.SourceRow, err)
}

func equivalentPPTXXLSXBindingCommand(deckPath string, row pptxXLSXBindingRow) string {
	sourceArgs := []string{"--workbook", pptxXLSXCommandArg(pptxXLSXBindingsWorkbook)}
	if row.SourceTable != "" {
		sourceArgs = append(sourceArgs, "--table", pptxXLSXCommandArg(row.SourceTable))
	} else {
		sourceArgs = append(sourceArgs, "--sheet", pptxXLSXCommandArg(row.SourceSheet), "--range", pptxXLSXCommandArg(row.SourceRange))
	}
	if row.ExpectSourceRange != "" {
		sourceArgs = append(sourceArgs, "--expect-source-range", pptxXLSXCommandArg(row.ExpectSourceRange))
	}
	if pptxXLSXBindingsMaxCells != 100000 {
		sourceArgs = append(sourceArgs, "--max-cells", strconv.Itoa(pptxXLSXBindingsMaxCells))
	}
	if row.FormulaMode != "" && row.FormulaMode != "value" {
		sourceArgs = append(sourceArgs, "--formula-mode", pptxXLSXCommandArg(row.FormulaMode))
	}
	source := strings.Join(sourceArgs, " ")
	switch row.Op {
	case "replace-text":
		args := []string{"ooxml", "--json", "pptx", "replace", "text-from-xlsx", pptxXLSXCommandArg(deckPath), source, "--slide", strconv.Itoa(row.Slide), "--target", pptxXLSXCommandArg(row.Target)}
		if row.Mode != "" && row.Mode != "plain-text" {
			args = append(args, "--mode", pptxXLSXCommandArg(row.Mode))
		}
		if row.RowSep != "" {
			args = append(args, "--row-sep", pptxXLSXCommandArg(row.RowSep))
		}
		if row.ColSep != "" {
			args = append(args, "--col-sep", pptxXLSXCommandArg(row.ColSep))
		}
		args = append(args, "--out", "<out.pptx>")
		return strings.Join(args, " ")
	case "update-table":
		return strings.Join([]string{"ooxml", "--json", "pptx", "tables", "update-from-xlsx", pptxXLSXCommandArg(deckPath), source, "--slide", strconv.Itoa(row.Slide), "--target", pptxXLSXCommandArg(row.Target), "--out", "<out.pptx>"}, " ")
	case "place-table":
		args := []string{"ooxml", "--json", "pptx", "place", "table-from-xlsx", pptxXLSXCommandArg(deckPath), source, "--slide", strconv.Itoa(row.Slide), "--x", strconv.FormatInt(row.X, 10), "--y", strconv.FormatInt(row.Y, 10), "--cx", strconv.FormatInt(row.CX, 10)}
		if row.CY > 0 {
			args = append(args, "--cy", strconv.FormatInt(row.CY, 10))
		}
		if row.Name != "" {
			args = append(args, "--name", pptxXLSXCommandArg(row.Name))
		}
		if row.Header {
			args = append(args, "--header")
		}
		args = append(args, "--out", "<out.pptx>")
		return strings.Join(args, " ")
	case "place-image":
		args := []string{"ooxml", "--json", "pptx", "place", "image", pptxXLSXCommandArg(deckPath), "--slide", strconv.Itoa(row.Slide), "--image", pptxXLSXCommandArg(defaultString(row.ResolvedImagePath, row.ImagePath)), "--x", strconv.FormatInt(row.X, 10), "--y", strconv.FormatInt(row.Y, 10), "--cx", strconv.FormatInt(row.CX, 10), "--cy", strconv.FormatInt(row.CY, 10)}
		if row.FitMode != "" && row.FitMode != "contain" {
			args = append(args, "--fit-mode", pptxXLSXCommandArg(row.FitMode))
		}
		if row.Name != "" {
			args = append(args, "--name", pptxXLSXCommandArg(row.Name))
		}
		args = append(args, "--out", "<out.pptx>")
		return strings.Join(args, " ")
	case "replace-image":
		args := []string{"ooxml", "--json", "pptx", "replace", "images", pptxXLSXCommandArg(deckPath), "--slide", strconv.Itoa(row.Slide), "--target", pptxXLSXCommandArg(row.Target), "--image", pptxXLSXCommandArg(defaultString(row.ResolvedImagePath, row.ImagePath))}
		if row.FitMode != "" && row.FitMode != "contain" {
			args = append(args, "--fit-mode", pptxXLSXCommandArg(row.FitMode))
		}
		args = append(args, "--out", "<out.pptx>")
		return strings.Join(args, " ")
	case "set-bounds":
		return strings.Join([]string{"ooxml", "--json", "pptx", "shapes", "set-bounds", pptxXLSXCommandArg(deckPath), "--slide", strconv.Itoa(row.Slide), "--target", pptxXLSXCommandArg(row.Target), "--bounds", fmt.Sprintf("%d,%d,%d,%d", row.X, row.Y, row.CX, row.CY), "--out", "<out.pptx>"}, " ")
	default:
		return ""
	}
}

func pptxXLSXCommandArg(value string) string {
	if value == "" {
		return "''"
	}
	if !strings.ContainsAny(value, " \t\r\n'\"\\$`<>|&;()[]{}*?!") {
		return value
	}
	return "'" + strings.ReplaceAll(value, "'", "'\"'\"'") + "'"
}

func destinationFileOrPlaceholder(path string) string {
	if path == "" {
		return outputPlaceholder()
	}
	return path
}

func outputPlaceholder() string {
	return "<out.pptx>"
}

func defaultString(value, fallback string) string {
	if strings.TrimSpace(value) == "" {
		return fallback
	}
	return value
}

func outputPPTXXLSXBindingsJSON(cmd *cobra.Command, result *PPTXXLSXBindingsResult) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal xlsx-bindings JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func init() {
	for _, cmd := range []*cobra.Command{pptxXLSXBindingsPlanCmd, pptxXLSXBindingsApplyCmd} {
		cmd.Flags().StringVar(&pptxXLSXBindingsWorkbook, "workbook", "", "XLSX workbook containing binding rows (required)")
		cmd.Flags().StringVar(&pptxXLSXBindingsSheet, "sheet", "", "binding sheet selector")
		cmd.Flags().StringVar(&pptxXLSXBindingsRange, "range", "", "binding A1 range")
		cmd.Flags().StringVar(&pptxXLSXBindingsTable, "table", "", "binding workbook table selector")
		cmd.Flags().IntVar(&pptxXLSXBindingsMaxCells, "max-cells", 100000, "maximum binding/source cells to read (0 for unlimited)")
	}
	AddMutationFlags(pptxXLSXBindingsApplyCmd)
	pptxXLSXBindingsCmd.AddCommand(pptxXLSXBindingsPlanCmd)
	pptxXLSXBindingsCmd.AddCommand(pptxXLSXBindingsApplyCmd)
	pptxCmd.AddCommand(pptxXLSXBindingsCmd)
}
