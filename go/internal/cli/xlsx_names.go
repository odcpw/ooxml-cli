package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/address"
	xlsxhandle "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/handle"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

type XLSXNamesListResult struct {
	File            string                `json:"file"`
	ValidateCommand string                `json:"validateCommand,omitempty"`
	Names           []XLSXDefinedNameItem `json:"names"`
}

type XLSXNamesShowResult struct {
	File            string              `json:"file"`
	ValidateCommand string              `json:"validateCommand,omitempty"`
	Name            XLSXDefinedNameItem `json:"name"`
}

type XLSXDefinedNameItem struct {
	Number          int      `json:"number"`
	Name            string   `json:"name"`
	Scope           string   `json:"scope"`
	LocalSheetID    *int     `json:"localSheetId,omitempty"`
	SheetNumber     int      `json:"sheetNumber,omitempty"`
	SheetName       string   `json:"sheetName,omitempty"`
	Ref             string   `json:"ref"`
	Hidden          bool     `json:"hidden,omitempty"`
	Comment         string   `json:"comment,omitempty"`
	Description     string   `json:"description,omitempty"`
	Handle          string   `json:"handle,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	ShowCommand     string   `json:"showCommand,omitempty"`
}

type XLSXNameMutationResult struct {
	File         string               `json:"file"`
	Output       string               `json:"output,omitempty"`
	DryRun       bool                 `json:"dryRun"`
	Action       string               `json:"action"`
	Name         *XLSXDefinedNameItem `json:"name,omitempty"`
	Deleted      *XLSXDefinedNameItem `json:"deleted,omitempty"`
	PreviousName string               `json:"previousName,omitempty"`
	PreviousRef  string               `json:"previousRef,omitempty"`
	XLSXNameMutationReadbackCommands
}

type XLSXNameMutationReadbackCommands struct {
	ValidateCommand          string `json:"validateCommand,omitempty"`
	NamesListCommand         string `json:"namesListCommand,omitempty"`
	NameShowCommand          string `json:"nameShowCommand,omitempty"`
	ValidateCommandTemplate  string `json:"validateCommandTemplate,omitempty"`
	NamesListCommandTemplate string `json:"namesListCommandTemplate,omitempty"`
	NameShowCommandTemplate  string `json:"nameShowCommandTemplate,omitempty"`
}

var xlsxNamesCmd = &cobra.Command{
	Use:     "names",
	Aliases: []string{"defined-names"},
	Short:   "Inspect and mutate workbook defined names",
	Long:    "Commands for workbook defined names and named ranges, including workbook-scope and sheet-local names.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

var xlsxNamesListScopeSheet string

var xlsxNamesListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List workbook defined names",
	Long:  "List workbook defined names, including workbook-scope and sheet-local named ranges.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()
		workbook, err := xlsxinspect.ParseWorkbook(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
		}
		names, err := xlsxinspect.ListDefinedNames(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to list defined names: %v", err)
		}
		names, err = filterDefinedNamesByScopeSheet(workbook, names, xlsxNamesListScopeSheet)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXNamesListJSON(cmd, filePath, names)
		}
		return outputXLSXNamesListText(cmd, names)
	},
}

var (
	xlsxNamesShowName       string
	xlsxNamesShowScopeSheet string
)

var xlsxNamesShowCmd = &cobra.Command{
	Use:   "show <file>",
	Short: "Show one workbook defined name",
	Long:  "Show one workbook defined name by name or published selector. Use --scope-sheet for sheet-local names.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		if strings.TrimSpace(xlsxNamesShowName) == "" {
			return InvalidArgsError("--name is required")
		}
		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()
		workbook, names, err := loadWorkbookDefinedNames(pkg)
		if err != nil {
			return err
		}
		name, err := selectXLSXDefinedName(workbook, names, xlsxNamesShowName, xlsxNamesShowScopeSheet)
		if err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return outputXLSXNamesShowJSON(cmd, filePath, name)
		}
		return outputXLSXNamesShowText(cmd, name)
	},
}

var (
	xlsxNamesAddName       string
	xlsxNamesAddRef        string
	xlsxNamesAddSheet      string
	xlsxNamesAddRange      string
	xlsxNamesAddScopeSheet string
	xlsxNamesAddHidden     bool
	xlsxNamesAddComment    string
)

var xlsxNamesAddCmd = &cobra.Command{
	Use:     "add <file>",
	Aliases: []string{"create"},
	Short:   "Add a workbook defined name",
	Long:    "Add a workbook-scope or sheet-local defined name from an exact --ref or from --sheet plus --range.",
	Args:    cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runXLSXNameMutation(cmd, args[0], "add")
	},
}

var (
	xlsxNamesUpdateName       string
	xlsxNamesUpdateRef        string
	xlsxNamesUpdateSheet      string
	xlsxNamesUpdateRange      string
	xlsxNamesUpdateScopeSheet string
	xlsxNamesUpdateExpectRef  string
)

var xlsxNamesUpdateCmd = &cobra.Command{
	Use:   "update <file>",
	Short: "Update a workbook defined name reference",
	Long:  "Update the ref/formula text for a workbook defined name. Use --expect-ref as a stale-target guard.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runXLSXNameMutation(cmd, args[0], "update")
	},
}

var (
	xlsxNamesRenameName       string
	xlsxNamesRenameNewName    string
	xlsxNamesRenameScopeSheet string
	xlsxNamesRenameExpectRef  string
)

var xlsxNamesRenameCmd = &cobra.Command{
	Use:   "rename <file>",
	Short: "Rename a workbook defined name",
	Long:  "Rename a workbook defined name while preserving its ref and scope. Use --expect-ref as a stale-target guard.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runXLSXNameMutation(cmd, args[0], "rename")
	},
}

var (
	xlsxNamesDeleteName       string
	xlsxNamesDeleteScopeSheet string
	xlsxNamesDeleteExpectRef  string
)

var xlsxNamesDeleteCmd = &cobra.Command{
	Use:     "delete <file>",
	Aliases: []string{"remove"},
	Short:   "Delete a workbook defined name",
	Long:    "Delete a workbook defined name. Use --expect-ref as a stale-target guard.",
	Args:    cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runXLSXNameMutation(cmd, args[0], "delete")
	},
}

func runXLSXNameMutation(cmd *cobra.Command, filePath, action string) error {
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	mutOpts, err := GetValidatedMutationOptions(cmd)
	if err != nil {
		return err
	}
	result, err := performXLSXNameMutation(filePath, action, mutOpts)
	if err != nil {
		return err
	}
	if GetGlobalConfig(cmd).Format == "json" {
		return outputXLSXNameMutationJSON(cmd, result)
	}
	return outputXLSXNameMutationText(cmd, result)
}

func performXLSXNameMutation(filePath, action string, mutOpts *MutationOptions) (*XLSXNameMutationResult, error) {
	writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
	if err != nil {
		return nil, err
	}
	var result *XLSXNameMutationResult
	if err := writer.Write(func(pkg opc.PackageSession) error {
		workbook, names, err := loadWorkbookDefinedNames(pkg)
		if err != nil {
			return err
		}
		destinationFile := mutationOutputPathForResult(filePath, mutOpts)
		switch action {
		case "add":
			name, ref, scope, err := resolveAddDefinedNameInputs(workbook)
			if err != nil {
				return err
			}
			added, err := mutate.AddDefinedName(&mutate.AddDefinedNameRequest{
				Package:     pkg,
				WorkbookURI: workbook.PartURI,
				Name:        name,
				Ref:         ref,
				Scope:       scope,
				Hidden:      xlsxNamesAddHidden,
				Comment:     xlsxNamesAddComment,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to add defined name: %v", err)
			}
			readback, err := readbackDefinedName(pkg, added.Name, added.LocalSheetID)
			if err != nil {
				return err
			}
			result = xlsxNameMutationResult(filePath, destinationFile, mutOpts, action, readback, nil, "", "")
		case "update":
			target, err := selectXLSXDefinedName(workbook, names, xlsxNamesUpdateName, xlsxNamesUpdateScopeSheet)
			if err != nil {
				return err
			}
			ref, err := resolveDefinedNameRefFromFlags(workbook, xlsxNamesUpdateRef, xlsxNamesUpdateSheet, xlsxNamesUpdateRange)
			if err != nil {
				return err
			}
			updated, err := mutate.UpdateDefinedName(&mutate.UpdateDefinedNameRequest{
				Package:     pkg,
				WorkbookURI: workbook.PartURI,
				Target:      target,
				Ref:         ref,
				ExpectRef:   xlsxNamesUpdateExpectRef,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to update defined name: %v", err)
			}
			readback, err := readbackDefinedName(pkg, updated.Name, updated.LocalSheetID)
			if err != nil {
				return err
			}
			result = xlsxNameMutationResult(filePath, destinationFile, mutOpts, action, readback, nil, "", updated.PreviousRef)
		case "rename":
			target, err := selectXLSXDefinedName(workbook, names, xlsxNamesRenameName, xlsxNamesRenameScopeSheet)
			if err != nil {
				return err
			}
			renamed, err := mutate.RenameDefinedName(&mutate.RenameDefinedNameRequest{
				Package:     pkg,
				WorkbookURI: workbook.PartURI,
				Target:      target,
				NewName:     xlsxNamesRenameNewName,
				ExpectRef:   xlsxNamesRenameExpectRef,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to rename defined name: %v", err)
			}
			readback, err := readbackDefinedName(pkg, renamed.Name, renamed.LocalSheetID)
			if err != nil {
				return err
			}
			result = xlsxNameMutationResult(filePath, destinationFile, mutOpts, action, readback, nil, renamed.PreviousName, "")
		case "delete":
			target, err := selectXLSXDefinedName(workbook, names, xlsxNamesDeleteName, xlsxNamesDeleteScopeSheet)
			if err != nil {
				return err
			}
			deleted, err := mutate.DeleteDefinedName(&mutate.DeleteDefinedNameRequest{
				Package:     pkg,
				WorkbookURI: workbook.PartURI,
				Target:      target,
				ExpectRef:   xlsxNamesDeleteExpectRef,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to delete defined name: %v", err)
			}
			deletedItem := xlsxDefinedNameItem(filePath, target)
			deletedItem.Ref = deleted.Ref
			result = xlsxNameMutationResult(filePath, destinationFile, mutOpts, action, nil, &deletedItem, "", "")
		default:
			return NewCLIErrorf(ExitUnexpected, "unknown name mutation action %q", action)
		}
		return nil
	}); err != nil {
		return nil, err
	}
	return result, nil
}

func loadWorkbookDefinedNames(pkg opc.PackageSession) (*model.Workbook, []model.DefinedName, error) {
	workbook, err := xlsxinspect.ParseWorkbook(pkg)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to parse workbook: %v", err)
	}
	names, err := xlsxinspect.ListDefinedNames(pkg)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to list defined names: %v", err)
	}
	return workbook, names, nil
}

func readbackDefinedName(pkg opc.PackageSession, name string, localSheetID *int) (*XLSXDefinedNameItem, error) {
	names, err := xlsxinspect.ListDefinedNames(pkg)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to read back defined names: %v", err)
	}
	for _, candidate := range names {
		if strings.EqualFold(candidate.Name, name) && sameIntPtr(candidate.LocalSheetID, localSheetID) {
			item := xlsxDefinedNameItem("", candidate)
			return &item, nil
		}
	}
	return nil, NewCLIErrorf(ExitUnexpected, "changed defined name %q did not read back", name)
}

func resolveAddDefinedNameInputs(workbook *model.Workbook) (string, string, mutate.DefinedNameScope, error) {
	name := strings.TrimSpace(xlsxNamesAddName)
	if name == "" {
		return "", "", mutate.DefinedNameScope{}, InvalidArgsError("--name is required")
	}
	ref, err := resolveDefinedNameRefFromFlags(workbook, xlsxNamesAddRef, xlsxNamesAddSheet, xlsxNamesAddRange)
	if err != nil {
		return "", "", mutate.DefinedNameScope{}, err
	}
	scope, err := resolveDefinedNameScope(workbook, xlsxNamesAddScopeSheet)
	if err != nil {
		return "", "", mutate.DefinedNameScope{}, err
	}
	return name, ref, scope, nil
}

func resolveDefinedNameRefFromFlags(workbook *model.Workbook, exactRef, sheetSelector, rangeText string) (string, error) {
	exactRef = strings.TrimSpace(exactRef)
	rangeText = strings.TrimSpace(rangeText)
	if (exactRef == "") == (rangeText == "") {
		return "", InvalidArgsError("must specify exactly one of --ref or --range")
	}
	if exactRef != "" {
		ref, err := mutate.NormalizeDefinedNameRef(exactRef)
		if err != nil {
			return "", NewCLIErrorf(ExitInvalidArgs, "%v", err)
		}
		return ref, nil
	}
	if strings.TrimSpace(sheetSelector) == "" {
		return "", InvalidArgsError("--sheet is required when using --range")
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, sheetSelector)
	if err != nil {
		return "", err
	}
	rangeRef, err := address.ParseRange(rangeText)
	if err != nil {
		return "", NewCLIErrorf(ExitInvalidArgs, "invalid --range: %v", err)
	}
	return mutate.DefinedNameRangeRef(sheetRef.Name, rangeRef), nil
}

func resolveDefinedNameScope(workbook *model.Workbook, scopeSheet string) (mutate.DefinedNameScope, error) {
	scopeSheet = strings.TrimSpace(scopeSheet)
	if scopeSheet == "" {
		return mutate.DefinedNameScope{}, nil
	}
	sheetRef, err := selectXLSXSheet(workbook.Sheets, scopeSheet)
	if err != nil {
		return mutate.DefinedNameScope{}, err
	}
	localSheetID := sheetRef.Number - 1
	return mutate.DefinedNameScope{LocalSheetID: &localSheetID}, nil
}

func filterDefinedNamesByScopeSheet(workbook *model.Workbook, names []model.DefinedName, scopeSheet string) ([]model.DefinedName, error) {
	scope, err := resolveDefinedNameScope(workbook, scopeSheet)
	if err != nil {
		return nil, err
	}
	if scope.LocalSheetID == nil {
		return names, nil
	}
	filtered := make([]model.DefinedName, 0, len(names))
	for _, name := range names {
		if sameIntPtr(name.LocalSheetID, scope.LocalSheetID) {
			filtered = append(filtered, name)
		}
	}
	return filtered, nil
}

func selectXLSXDefinedName(workbook *model.Workbook, names []model.DefinedName, selector, scopeSheet string) (model.DefinedName, error) {
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return model.DefinedName{}, InvalidArgsError("--name is required")
	}
	// Handle-first branch (additive): a workbook-scoped defined-name handle is a
	// native, position-independent address resolved by SEARCHING for the name,
	// refusing on a duplicate. Non-handle selectors fall through unchanged.
	if xlsxhandle.IsHandle(selector) {
		h, perr := xlsxhandle.Parse(selector)
		if perr != nil {
			return model.DefinedName{}, mapXLSXHandleError(perr)
		}
		if h.Kind != xlsxhandle.KindDefinedName {
			return model.DefinedName{}, InvalidArgsError("expected a defined-name handle (H:xlsx/wb/name:n:<name>)")
		}
		resolved, rerr := xlsxhandle.ResolveDefinedName(names, h)
		if rerr != nil {
			return model.DefinedName{}, mapXLSXHandleError(rerr)
		}
		return resolved, nil
	}
	scope, err := resolveDefinedNameScope(workbook, scopeSheet)
	if err != nil {
		return model.DefinedName{}, err
	}
	matches := []model.DefinedName{}
	for _, name := range names {
		if scope.LocalSheetID != nil && !sameIntPtr(name.LocalSheetID, scope.LocalSheetID) {
			continue
		}
		if model.SelectorMatches(name.Selectors, selector) || strings.EqualFold(name.Name, selector) {
			matches = append(matches, name)
		}
	}
	switch len(matches) {
	case 0:
		candidates := definedNameSelectorCandidates(names)
		return model.DefinedName{}, SelectorNotFoundError("defined name", selector, BuildSelectorCandidates(candidates, selector, maxSelectorCandidates), "ooxml --json xlsx names list <file>")
	case 1:
		return matches[0], nil
	default:
		return model.DefinedName{}, NewCLIErrorf(ExitInvalidArgs, "defined name %q is ambiguous; use --scope-sheet or one of: %s", selector, definedNameCandidateSelectors(matches))
	}
}

func definedNameSelectorCandidates(names []model.DefinedName) []SelectorCandidate {
	out := make([]SelectorCandidate, 0, len(names))
	for _, name := range names {
		out = append(out, SelectorCandidate{Primary: name.PrimarySelector, Selectors: name.Selectors})
	}
	return out
}

func definedNameCandidateSelectors(names []model.DefinedName) string {
	values := make([]string, 0, len(names))
	for _, name := range names {
		values = append(values, name.PrimarySelector)
	}
	return strings.Join(values, ", ")
}

func outputXLSXNamesListJSON(cmd *cobra.Command, filePath string, names []model.DefinedName) error {
	result := XLSXNamesListResult{
		File:            filePath,
		ValidateCommand: xlsxValidateCommand(filePath),
		Names:           xlsxDefinedNameItems(filePath, names),
	}
	data, err := marshalXLSXJSON(cmd, result, "names list")
	if err != nil {
		return err
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXNamesShowJSON(cmd *cobra.Command, filePath string, name model.DefinedName) error {
	result := XLSXNamesShowResult{
		File:            filePath,
		ValidateCommand: xlsxValidateCommand(filePath),
		Name:            xlsxDefinedNameItem(filePath, name),
	}
	data, err := marshalXLSXJSON(cmd, result, "names show")
	if err != nil {
		return err
	}
	return writeXLSXOutput(cmd, data)
}

func outputXLSXNameMutationJSON(cmd *cobra.Command, result *XLSXNameMutationResult) error {
	data, err := marshalXLSXJSON(cmd, result, "name mutation")
	if err != nil {
		return err
	}
	return writeXLSXOutput(cmd, data)
}

func marshalXLSXJSON(cmd *cobra.Command, value any, label string) ([]byte, error) {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(value, "", "  ")
	} else {
		data, err = json.Marshal(value)
	}
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to marshal %s JSON: %v", label, err)
	}
	return data, nil
}

func outputXLSXNamesListText(cmd *cobra.Command, names []model.DefinedName) error {
	lines := []string{fmt.Sprintf("%-4s %-28s %-10s %-20s %s", "[N]", "Name", "Scope", "Sheet", "Ref")}
	lines = append(lines, strings.Repeat("-", 100))
	for _, name := range names {
		sheet := ""
		if name.Scope == "sheet" {
			sheet = fmt.Sprintf("%d:%s", name.SheetNumber, name.SheetName)
		}
		lines = append(lines, fmt.Sprintf("[%-2d] %-28s %-10s %-20s %s", name.Number, truncateStr(name.Name, 28), name.Scope, truncateStr(sheet, 20), name.Ref))
	}
	return writeXLSXOutput(cmd, []byte(strings.Join(lines, "\n")))
}

func outputXLSXNamesShowText(cmd *cobra.Command, name model.DefinedName) error {
	lines := []string{
		fmt.Sprintf("Name: %s", name.Name),
		fmt.Sprintf("Scope: %s", name.Scope),
		fmt.Sprintf("Ref: %s", name.Ref),
	}
	if name.Scope == "sheet" {
		lines = append(lines, fmt.Sprintf("Sheet: %d %s", name.SheetNumber, name.SheetName))
	}
	if name.Hidden {
		lines = append(lines, "Hidden: true")
	}
	return writeXLSXOutput(cmd, []byte(strings.Join(lines, "\n")))
}

func outputXLSXNameMutationText(cmd *cobra.Command, result *XLSXNameMutationResult) error {
	switch result.Action {
	case "delete":
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("deleted defined name %s", result.Deleted.Name)))
	default:
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("%s defined name %s -> %s", result.Action, result.Name.Name, result.Name.Ref)))
	}
}

func xlsxNameMutationResult(filePath, destinationFile string, mutOpts *MutationOptions, action string, name *XLSXDefinedNameItem, deleted *XLSXDefinedNameItem, previousName, previousRef string) *XLSXNameMutationResult {
	if name != nil && destinationFile != "" {
		nameCopy := *name
		nameCopy.ShowCommand = xlsxNameShowCommand(destinationFile, nameCopy)
		name = &nameCopy
	}
	result := &XLSXNameMutationResult{
		File:         filePath,
		Output:       destinationFile,
		DryRun:       mutOpts != nil && mutOpts.DryRun,
		Action:       action,
		Name:         name,
		Deleted:      deleted,
		PreviousName: previousName,
		PreviousRef:  previousRef,
	}
	result.XLSXNameMutationReadbackCommands = xlsxNameMutationReadbackCommands(destinationFile, name)
	return result
}

func xlsxNameMutationReadbackCommands(filePath string, name *XLSXDefinedNameItem) XLSXNameMutationReadbackCommands {
	if filePath == "" {
		placeholder := xlsxOutputPlaceholder()
		commands := XLSXNameMutationReadbackCommands{
			ValidateCommandTemplate:  xlsxValidateCommand(placeholder),
			NamesListCommandTemplate: xlsxNamesListCommand(placeholder),
		}
		if name != nil {
			commands.NameShowCommandTemplate = xlsxNameShowCommand(placeholder, *name)
		}
		return commands
	}
	commands := XLSXNameMutationReadbackCommands{
		ValidateCommand:  xlsxValidateCommand(filePath),
		NamesListCommand: xlsxNamesListCommand(filePath),
	}
	if name != nil {
		commands.NameShowCommand = xlsxNameShowCommand(filePath, *name)
	}
	return commands
}

func xlsxDefinedNameItems(filePath string, names []model.DefinedName) []XLSXDefinedNameItem {
	counts := workbookScopedDefinedNameCounts(names)
	items := make([]XLSXDefinedNameItem, 0, len(names))
	for _, name := range names {
		items = append(items, xlsxDefinedNameItemWithCounts(filePath, name, counts))
	}
	return items
}

func xlsxDefinedNameItem(filePath string, name model.DefinedName) XLSXDefinedNameItem {
	return xlsxDefinedNameItemWithCounts(filePath, name, nil)
}

func xlsxDefinedNameItemWithCounts(filePath string, name model.DefinedName, workbookNameCounts map[string]int) XLSXDefinedNameItem {
	item := XLSXDefinedNameItem{
		Number:          name.Number,
		Name:            name.Name,
		Scope:           name.Scope,
		LocalSheetID:    name.LocalSheetID,
		SheetNumber:     name.SheetNumber,
		SheetName:       name.SheetName,
		Ref:             name.Ref,
		Hidden:          name.Hidden,
		Comment:         name.Comment,
		Description:     name.Description,
		PrimarySelector: name.PrimarySelector,
		Selectors:       append([]string{}, name.Selectors...),
	}
	// A workbook-scoped defined name is a native, position-independent handle.
	// Sheet-scoped names are not minted (the handle grammar is workbook-scoped).
	// Corrupt workbooks can contain duplicate workbook-scoped names; omit a
	// handle in that case because resolution correctly returns HANDLE_AMBIGUOUS.
	unique := true
	if workbookNameCounts != nil {
		unique = workbookNameCounts[name.Name] == 1
	}
	if unique && name.Scope == "workbook" && strings.TrimSpace(name.Name) != "" {
		item.Handle = xlsxhandle.FormatDefinedName(name.Name)
	}
	if filePath != "" {
		item.ShowCommand = xlsxNameShowCommand(filePath, item)
	}
	return item
}

func workbookScopedDefinedNameCounts(names []model.DefinedName) map[string]int {
	counts := make(map[string]int)
	for _, name := range names {
		if name.Scope == "workbook" && strings.TrimSpace(name.Name) != "" {
			counts[name.Name]++
		}
	}
	return counts
}

func xlsxNamesListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json xlsx names list %s", pptxXLSXCommandArg(filePath))
}

func xlsxNameShowCommand(filePath string, name XLSXDefinedNameItem) string {
	command := fmt.Sprintf("ooxml --json xlsx names show %s --name %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(name.Name))
	if name.Scope == "sheet" && name.SheetNumber > 0 {
		command += fmt.Sprintf(" --scope-sheet %s", pptxXLSXCommandArg(fmt.Sprintf("sheet:%d", name.SheetNumber)))
	}
	return command
}

func sameIntPtr(a, b *int) bool {
	if a == nil || b == nil {
		return a == nil && b == nil
	}
	return *a == *b
}

func init() {
	xlsxNamesListCmd.Flags().StringVar(&xlsxNamesListScopeSheet, "scope-sheet", "", "only list sheet-local names for this sheet selector")

	xlsxNamesShowCmd.Flags().StringVar(&xlsxNamesShowName, "name", "", "defined name or published selector")
	xlsxNamesShowCmd.Flags().StringVar(&xlsxNamesShowScopeSheet, "scope-sheet", "", "sheet selector for sheet-local names")

	xlsxNamesAddCmd.Flags().StringVar(&xlsxNamesAddName, "name", "", "defined name to add")
	xlsxNamesAddCmd.Flags().StringVar(&xlsxNamesAddRef, "ref", "", "exact defined-name ref/formula text")
	xlsxNamesAddCmd.Flags().StringVar(&xlsxNamesAddSheet, "sheet", "", "sheet selector used with --range")
	xlsxNamesAddCmd.Flags().StringVar(&xlsxNamesAddRange, "range", "", "A1 range used with --sheet")
	xlsxNamesAddCmd.Flags().StringVar(&xlsxNamesAddScopeSheet, "scope-sheet", "", "create a sheet-local name scoped to this sheet selector")
	xlsxNamesAddCmd.Flags().BoolVar(&xlsxNamesAddHidden, "hidden", false, "mark the defined name hidden")
	xlsxNamesAddCmd.Flags().StringVar(&xlsxNamesAddComment, "comment", "", "optional defined-name comment")
	AddMutationFlags(xlsxNamesAddCmd)

	xlsxNamesUpdateCmd.Flags().StringVar(&xlsxNamesUpdateName, "name", "", "defined name or published selector")
	xlsxNamesUpdateCmd.Flags().StringVar(&xlsxNamesUpdateRef, "ref", "", "exact replacement ref/formula text")
	xlsxNamesUpdateCmd.Flags().StringVar(&xlsxNamesUpdateSheet, "sheet", "", "sheet selector used with --range")
	xlsxNamesUpdateCmd.Flags().StringVar(&xlsxNamesUpdateRange, "range", "", "replacement A1 range used with --sheet")
	xlsxNamesUpdateCmd.Flags().StringVar(&xlsxNamesUpdateScopeSheet, "scope-sheet", "", "sheet selector for sheet-local names")
	xlsxNamesUpdateCmd.Flags().StringVar(&xlsxNamesUpdateExpectRef, "expect-ref", "", "expected current ref/formula text")
	AddMutationFlags(xlsxNamesUpdateCmd)

	xlsxNamesRenameCmd.Flags().StringVar(&xlsxNamesRenameName, "name", "", "defined name or published selector")
	xlsxNamesRenameCmd.Flags().StringVar(&xlsxNamesRenameNewName, "new-name", "", "new defined name")
	xlsxNamesRenameCmd.Flags().StringVar(&xlsxNamesRenameScopeSheet, "scope-sheet", "", "sheet selector for sheet-local names")
	xlsxNamesRenameCmd.Flags().StringVar(&xlsxNamesRenameExpectRef, "expect-ref", "", "expected current ref/formula text")
	AddMutationFlags(xlsxNamesRenameCmd)

	xlsxNamesDeleteCmd.Flags().StringVar(&xlsxNamesDeleteName, "name", "", "defined name or published selector")
	xlsxNamesDeleteCmd.Flags().StringVar(&xlsxNamesDeleteScopeSheet, "scope-sheet", "", "sheet selector for sheet-local names")
	xlsxNamesDeleteCmd.Flags().StringVar(&xlsxNamesDeleteExpectRef, "expect-ref", "", "expected current ref/formula text")
	AddMutationFlags(xlsxNamesDeleteCmd)

	xlsxNamesCmd.AddCommand(xlsxNamesListCmd)
	xlsxNamesCmd.AddCommand(xlsxNamesShowCmd)
	xlsxNamesCmd.AddCommand(xlsxNamesAddCmd)
	xlsxNamesCmd.AddCommand(xlsxNamesUpdateCmd)
	xlsxNamesCmd.AddCommand(xlsxNamesRenameCmd)
	xlsxNamesCmd.AddCommand(xlsxNamesDeleteCmd)
	xlsxCmd.AddCommand(xlsxNamesCmd)
}
