package cli

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	vbapkg "github.com/ooxml-cli/ooxml-cli/pkg/vba"
)

type vbaAgentWorkflowGolden struct {
	Workflow  string                    `json:"workflow"`
	Initial   vbaAgentModuleStateGolden `json:"initial"`
	Extract   vbaAgentExtractGolden     `json:"extract"`
	Mutations []vbaAgentMutationGolden  `json:"mutations"`
	Commands  vbaAgentCommandsGolden    `json:"commands"`
	Final     vbaAgentFinalGolden       `json:"final"`
}

type vbaAgentModuleStateGolden struct {
	MacroEnabled bool     `json:"macroEnabled"`
	ModuleCount  int      `json:"moduleCount"`
	Modules      []string `json:"modules"`
}

type vbaAgentExtractGolden struct {
	Module1BasWritten      bool `json:"module1BasWritten"`
	Class1ClsWritten       bool `json:"class1ClsWritten"`
	Module1ContainsHello   bool `json:"module1ContainsHelloWorld"`
	Class1ContainsAnswer   bool `json:"class1ContainsAnswer"`
	ModuleCount            int  `json:"moduleCount"`
	ListCommandPresent     bool `json:"listCommandPresent"`
	OfficeCheckCommand     bool `json:"officeCheckCommandPresent"`
	ValidateCommandPresent bool `json:"validateCommandPresent"`
}

type vbaAgentMutationGolden struct {
	Action             string `json:"action"`
	Module             string `json:"module"`
	Kind               string `json:"kind"`
	PreviousCount      int    `json:"previousCount,omitempty"`
	ModuleCount        int    `json:"moduleCount,omitempty"`
	PreviousSHAPresent bool   `json:"previousShaPresent,omitempty"`
	SHAPresent         bool   `json:"shaPresent"`
	SHAChanged         bool   `json:"shaChanged,omitempty"`
	Removed            bool   `json:"removed,omitempty"`
	PurgedCaches       bool   `json:"purgedCaches"`
	RecompilesOnOpen   bool   `json:"recompilesOnOpen"`
	Compatibility      string `json:"compatibility"`
	ValidationStatus   string `json:"validationStatus"`
}

type vbaAgentCommandsGolden struct {
	InspectPresent                bool `json:"inspectPresent"`
	ValidatePresent               bool `json:"validatePresent"`
	OfficeCheckPresent            bool `json:"officeCheckPresent"`
	PackageReadbackPresent        bool `json:"packageReadbackPresent"`
	ListPresent                   bool `json:"listPresent"`
	ExtractPresentAfterAddReplace bool `json:"extractPresentAfterAddReplace"`
	ExtractAbsentAfterRemove      bool `json:"extractAbsentAfterRemove"`
}

type vbaAgentFinalGolden struct {
	MacroEnabled                      bool     `json:"macroEnabled"`
	ModuleCount                       int      `json:"moduleCount"`
	Modules                           []string `json:"modules"`
	RemovedClassAbsent                bool     `json:"removedClassAbsent"`
	ReplacementExtractContainsNewText bool     `json:"replacementExtractContainsNewText"`
	ValidationStatus                  string   `json:"validationStatus"`
}

// TestVBAAgentWorkflowGolden freezes the practical source-module automation
// path an agent needs: list/extract .bas+.cls modules, add a standard module,
// add a class module, replace with a SHA guard, remove with a SHA guard, then
// validate and read back the final macro-enabled workbook. It snapshots semantic
// facts rather than temp paths, raw vbaProject.bin bytes, or full source text.
func TestVBAAgentWorkflowGolden(t *testing.T) {
	dir := t.TempDir()
	inputPath := filepath.Join("..", "..", "testdata", "xlsx", "minimal-workbook", "workbook.xlsx")
	binPath := filepath.Join(dir, "vbaProject.bin")
	if err := os.WriteFile(binPath, syntheticCLIVBAProjectBinForTest(t), 0o644); err != nil {
		t.Fatalf("write synthetic VBA project: %v", err)
	}

	attachedPath := filepath.Join(dir, "attached.xlsm")
	runVBACommand(t, "--format", "json", "vba", "attach", inputPath, "--bin", binPath, "--out", attachedPath)
	validateVBAAgentWorkflowPackage(t, attachedPath)

	initial := parseVBAModuleListResult(t, runVBACommand(t, "--format", "json", "vba", "list", attachedPath))
	if initial.Project == nil || initial.Project.ModuleCount != 2 {
		t.Fatalf("unexpected initial module list: %+v", initial.Project)
	}

	extractDir := filepath.Join(dir, "initial-extract")
	extract := parseVBAModuleExtractResult(t, runVBACommand(t, "--format", "json", "vba", "extract", attachedPath, "--out-dir", extractDir))
	module1Source := readTextFileForVBATest(t, filepath.Join(extractDir, "Module1.bas"))
	class1Source := readTextFileForVBATest(t, filepath.Join(extractDir, "Class1.cls"))

	module2Path := filepath.Join(dir, "Module2.bas")
	writeVBASourceForTest(t, module2Path, strings.Join([]string{
		`Attribute VB_Name = "Module2"`,
		"Public Sub Added()",
		`    Debug.Print "added"`,
		"End Sub",
		"",
	}, "\r\n"))
	addedStandardPath := filepath.Join(dir, "added-standard.xlsm")
	addStandard := parseVBAModuleReplaceResult(t, runVBACommand(t,
		"--format", "json",
		"vba", "add-module", attachedPath,
		"--source", module2Path,
		"--expect-module-count", "2",
		"--out", addedStandardPath,
	))
	validateVBAAgentWorkflowPackage(t, addedStandardPath)

	class2Path := filepath.Join(dir, "Class2.cls")
	writeVBASourceForTest(t, class2Path, strings.Join([]string{
		`Attribute VB_Name = "Class2"`,
		"Public Function AddedClass()",
		"    AddedClass = 9",
		"End Function",
		"",
	}, "\r\n"))
	addedClassPath := filepath.Join(dir, "added-class.xlsm")
	addClass := parseVBAModuleReplaceResult(t, runVBACommand(t,
		"--format", "json",
		"vba", "add-module", addedStandardPath,
		"--source", class2Path,
		"--expect-module-count", "3",
		"--out", addedClassPath,
	))
	validateVBAAgentWorkflowPackage(t, addedClassPath)

	afterAdd := parseVBAModuleListResult(t, runVBACommand(t, "--format", "json", "vba", "list", addedClassPath))
	module2 := vbaModuleByNameForTest(t, afterAdd.Project.Modules, "Module2")
	class2 := vbaModuleByNameForTest(t, afterAdd.Project.Modules, "Class2")

	writeVBASourceForTest(t, module2Path, strings.Join([]string{
		`Attribute VB_Name = "Module2"`,
		"Public Sub Added()",
		`    Debug.Print "replaced"`,
		"End Sub",
		"",
	}, "\r\n"))
	replacedPath := filepath.Join(dir, "replaced-standard.xlsm")
	replaced := parseVBAModuleReplaceResult(t, runVBACommand(t,
		"--format", "json",
		"vba", "replace-module", addedClassPath,
		"--module", module2.PrimarySelector,
		"--source", module2Path,
		"--expect-sha256", module2.SHA256,
		"--out", replacedPath,
	))
	validateVBAAgentWorkflowPackage(t, replacedPath)

	removedClassPath := filepath.Join(dir, "removed-class.xlsm")
	removed := parseVBAModuleReplaceResult(t, runVBACommand(t,
		"--format", "json",
		"vba", "remove-module", replacedPath,
		"--module", class2.PrimarySelector,
		"--expect-sha256", class2.SHA256,
		"--out", removedClassPath,
	))
	validateVBAAgentWorkflowPackage(t, removedClassPath)

	finalList := parseVBAModuleListResult(t, runVBACommand(t, "--format", "json", "vba", "list", removedClassPath))
	replacedExtractDir := filepath.Join(dir, "replaced-extract")
	replacedExtract := parseVBAModuleExtractResult(t, runVBACommand(t, "--format", "json", "vba", "extract", removedClassPath, "--out-dir", replacedExtractDir, "--module", "Module2"))
	replacedSource := readTextFileForVBATest(t, filepath.Join(replacedExtractDir, "Module2.bas"))

	actual := vbaAgentWorkflowGolden{
		Workflow: "vba-source-module-manage-validate",
		Initial:  summarizeVBAAgentState(initial),
		Extract: vbaAgentExtractGolden{
			Module1BasWritten:      fileExists(filepath.Join(extractDir, "Module1.bas")),
			Class1ClsWritten:       fileExists(filepath.Join(extractDir, "Class1.cls")),
			Module1ContainsHello:   strings.Contains(module1Source, "Public Sub HelloWorld()"),
			Class1ContainsAnswer:   strings.Contains(class1Source, "Public Function Answer()"),
			ModuleCount:            len(extract.Modules),
			ListCommandPresent:     extract.ListCommand != "",
			OfficeCheckCommand:     extract.OfficeCheckCommand != "",
			ValidateCommandPresent: extract.ValidateCommand != "",
		},
		Mutations: []vbaAgentMutationGolden{
			summarizeVBAAgentMutation(addStandard, "valid"),
			summarizeVBAAgentMutation(addClass, "valid"),
			summarizeVBAAgentMutation(replaced, "valid"),
			summarizeVBAAgentMutation(removed, "valid"),
		},
		Commands: vbaAgentCommandsGolden{
			InspectPresent:                addStandard.InspectCommand != "" && addClass.InspectCommand != "" && replaced.InspectCommand != "" && removed.InspectCommand != "",
			ValidatePresent:               addStandard.ValidateCommand != "" && addClass.ValidateCommand != "" && replaced.ValidateCommand != "" && removed.ValidateCommand != "",
			OfficeCheckPresent:            addStandard.OfficeCheckCommand != "" && addClass.OfficeCheckCommand != "" && replaced.OfficeCheckCommand != "" && removed.OfficeCheckCommand != "",
			PackageReadbackPresent:        addStandard.PackageReadbackCommand != "" && addClass.PackageReadbackCommand != "" && replaced.PackageReadbackCommand != "" && removed.PackageReadbackCommand != "",
			ListPresent:                   addStandard.ListCommand != "" && addClass.ListCommand != "" && replaced.ListCommand != "" && removed.ListCommand != "",
			ExtractPresentAfterAddReplace: addStandard.ExtractCommand != "" && addClass.ExtractCommand != "" && replaced.ExtractCommand != "" && replacedExtract.ListCommand != "",
			ExtractAbsentAfterRemove:      removed.ExtractCommand == "" && removed.ExtractCommandTemplate == "",
		},
		Final: vbaAgentFinalGolden{
			MacroEnabled:                      finalList.VBA != nil && finalList.VBA.MacroEnabled,
			ModuleCount:                       finalList.Project.ModuleCount,
			Modules:                           summarizeVBAAgentModules(finalList.Project.Modules),
			RemovedClassAbsent:                !vbaModuleExistsForTest(finalList.Project.Modules, "Class2"),
			ReplacementExtractContainsNewText: strings.Contains(replacedSource, `Debug.Print "replaced"`),
			ValidationStatus:                  "valid",
		},
	}
	assertGoldenJSONValue(t, "vba_agent_workflow_summary.json", actual)
}

func validateVBAAgentWorkflowPackage(t *testing.T, path string) {
	t.Helper()
	runVBACommand(t, "validate", "--strict", path)
}

func summarizeVBAAgentState(result VBAModuleListResult) vbaAgentModuleStateGolden {
	return vbaAgentModuleStateGolden{
		MacroEnabled: result.VBA != nil && result.VBA.MacroEnabled,
		ModuleCount:  result.Project.ModuleCount,
		Modules:      summarizeVBAAgentModules(result.Project.Modules),
	}
}

func summarizeVBAAgentModules(modules []vbapkg.SourceModule) []string {
	out := make([]string, 0, len(modules))
	for _, module := range modules {
		out = append(out, module.Name+":"+module.Kind+":"+module.Extension)
	}
	sort.Strings(out)
	return out
}

func summarizeVBAAgentMutation(result VBAModuleReplaceResult, validationStatus string) vbaAgentMutationGolden {
	mutation := result.Result
	return vbaAgentMutationGolden{
		Action:             mutation.Action,
		Module:             mutation.Module.Name,
		Kind:               mutation.Module.Kind,
		PreviousCount:      mutation.PreviousCount,
		ModuleCount:        mutation.ModuleCount,
		PreviousSHAPresent: mutation.PreviousSHA256 != "",
		SHAPresent:         mutation.SHA256 != "",
		SHAChanged:         mutation.PreviousSHA256 != "" && mutation.SHA256 != "" && mutation.PreviousSHA256 != mutation.SHA256,
		Removed:            mutation.Action == "remove-module" && mutation.SHA256 == "",
		PurgedCaches:       mutation.PurgedCaches,
		RecompilesOnOpen:   mutation.RecompilesOnOpen,
		Compatibility:      mutation.CompatibilityStatus,
		ValidationStatus:   validationStatus,
	}
}
