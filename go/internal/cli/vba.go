package cli

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/officecheck"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/validate"
	"github.com/ooxml-cli/ooxml-cli/pkg/vba"
	"github.com/spf13/cobra"
)

var vbaCmd = &cobra.Command{
	Use:   "vba",
	Short: "Inspect and edit VBA macro projects",
	Long:  "Commands for package-level vbaProject.bin operations, Office-authored macro-enabled file creation, and guarded source-module replacement/removal in PPTX/PPTM and XLSX/XLSM files.",
	Args:  cobra.NoArgs,
	RunE:  showHelp,
}

var vbaInspectCmd = &cobra.Command{
	Use:   "inspect <file>",
	Short: "Inspect opaque VBA package state",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		pkg, err := opc.Open(args[0])
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()

		info, err := vba.Inspect(pkg)
		if err != nil {
			return mapVBAError(err)
		}
		return outputVBAInspect(cmd, args[0], info)
	},
}

var vbaOfficeCheckOutDir string
var vbaOfficeCheckToolsFactory = func() *officecheck.Tools { return officecheck.NewTools() }

var vbaOfficeCheckCmd = &cobra.Command{
	Use:           "office-check <file>",
	Short:         "Check whether a local Office-compatible engine opens the macro package",
	Long:          "Validate a PPTM/XLSM package, then ask the best local Office-compatible engine (LibreOffice/soffice) to open it via headless conversion. This is compatibility evidence, not Microsoft Office, macro execution, or macro compile proof.",
	SilenceUsage:  true,
	SilenceErrors: true,
	Args:          cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runVBAOfficeCheck(cmd, args[0])
	},
}

var vbaExtractBinCmd = &cobra.Command{
	Use:   "extract-bin <file>",
	Short: "Extract opaque vbaProject.bin",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		outPath, err := cmd.Flags().GetString("out")
		if err != nil {
			return err
		}
		if outPath == "" {
			return InvalidArgsError("--out is required")
		}

		pkg, err := opc.Open(args[0])
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()

		data, info, err := vba.ExtractBin(pkg)
		if err != nil {
			return mapVBAError(err)
		}
		if err := os.WriteFile(outPath, data, 0o644); err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to write VBA binary: %v", err)
		}
		return outputVBAExtract(cmd, args[0], outPath, len(data), info)
	},
}

var vbaInspectBinFamily string

var vbaInspectBinCmd = &cobra.Command{
	Use:   "inspect-bin <vbaProject.bin>",
	Short: "Inspect a standalone VBA binary before attach",
	Long:  "Inspect a standalone vbaProject.bin seed before attaching it to a PPTX/PPTM or XLSX/XLSM package. Use --family pptx|xlsx to surface host-family compatibility risks.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		data, err := os.ReadFile(args[0])
		if err != nil {
			return NewCLIErrorf(ExitFileNotFound, "failed to read VBA binary: %v", err)
		}
		family, err := normalizeVBAInspectBinFamily(vbaInspectBinFamily)
		if err != nil {
			return err
		}
		project, err := vba.ParseSourceProjectForFamily(data, family)
		if err != nil {
			return mapVBAError(err)
		}
		return outputVBAInspectBin(cmd, args[0], data, project)
	},
}

var vbaListCmd = &cobra.Command{
	Use:   "list <file>",
	Short: "List VBA source modules",
	Long:  "Parse vbaProject.bin and list source modules without mutating the package.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		pkg, err := opc.Open(args[0])
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()

		info, err := vba.Inspect(pkg)
		if err != nil {
			return mapVBAError(err)
		}
		project, err := vba.InspectSourceProject(pkg)
		if err != nil {
			return mapVBASourceProjectError(args[0], info, err)
		}
		return outputVBAModuleList(cmd, args[0], info, project)
	},
}

var (
	vbaCreateFamily                string
	vbaCreateSources               []string
	vbaCreateExtractBinPath        string
	vbaCreateOfficeScriptPath      string
	vbaCreateEnableVBOMAccess      bool
	vbaCreateVisible               bool
	vbaCreateForce                 bool
	vbaCreateScriptRunner          = invokeVBACreateOfficeScript
	vbaOfficeCreateScriptFileName  = "windows-office-vba-create.ps1"
	vbaOfficeCreateScriptDirectory = "tools"
)

var vbaCreateCmd = &cobra.Command{
	Use:   "create <output.xlsm|output.pptm>",
	Short: "Create an Office-authored XLSM/PPTM from VBA source files",
	Long:  "Create a fresh macro-enabled Excel workbook or PowerPoint presentation from .bas/.cls sources by driving desktop Microsoft Office on Windows. This is the first-class CLI wrapper around the repo's Office COM helper; agents should use this command instead of calling the helper script directly.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runVBACreate(cmd, args[0])
	},
}

var vbaExtractModuleSelector string
var (
	vbaAddModuleSourcePath        string
	vbaAddModuleName              string
	vbaAddModuleKind              string
	vbaAddModuleExpectModuleCount int
	vbaReplaceModuleSelector      string
	vbaReplaceModuleSourcePath    string
	vbaReplaceModuleExpectSHA256  string
	vbaRemoveModuleSelector       string
	vbaRemoveModuleExpectSHA256   string
	vbaAllowExperimentalRewrite   bool
	vbaAttachAllowHostFamilyRisk  bool
)

var vbaExtractCmd = &cobra.Command{
	Use:   "extract <file>",
	Short: "Extract VBA source modules",
	Long:  "Extract .bas and .cls source modules from vbaProject.bin without mutating the package.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		outDir, err := cmd.Flags().GetString("out-dir")
		if err != nil {
			return err
		}
		if strings.TrimSpace(outDir) == "" {
			return InvalidArgsError("--out-dir is required")
		}
		pkg, err := opc.Open(args[0])
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
		}
		defer pkg.Close()

		info, err := vba.Inspect(pkg)
		if err != nil {
			return mapVBAError(err)
		}
		project, err := vba.InspectSourceProject(pkg)
		if err != nil {
			return mapVBASourceProjectError(args[0], info, err)
		}
		modules, err := selectVBAModules(args[0], project.Modules, vbaExtractModuleSelector)
		if err != nil {
			return err
		}
		extracted, err := writeVBAModules(outDir, modules)
		if err != nil {
			return err
		}
		return outputVBAModuleExtract(cmd, args[0], outDir, info, project, extracted)
	},
}

var vbaAddModuleCmd = &cobra.Command{
	Use:   "add-module <file>",
	Short: "Add a new VBA source module",
	Long:  "Add one new .bas or .cls source module to a parseable source-only/synthetic vbaProject.bin. The rewrite updates module metadata, adds the module stream, removes compiled caches, and preserves untouched module streams. Real Office-shaped projects with version-dependent _VBA_PROJECT metadata are refused; create or obtain an Office-authored vbaProject.bin with the desired modules and attach it instead. This does not preserve signatures, execute macros, or prove Office-load compatibility.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if strings.TrimSpace(vbaAddModuleSourcePath) == "" {
			return InvalidArgsError("--source is required")
		}
		source, err := vba.ReadModuleSourceFile(vbaAddModuleSourcePath)
		if err != nil {
			if os.IsNotExist(err) {
				return NewCLIErrorf(ExitFileNotFound, "failed to read VBA source: %v", err)
			}
			return NewCLIErrorf(ExitInvalidArgs, "failed to read VBA source: %v", err)
		}
		opts := vba.AddModuleOptions{
			Name:                           vbaAddModuleName,
			Kind:                           vbaAddModuleKind,
			ExpectModuleCount:              vbaAddModuleExpectModuleCount,
			AllowExperimentalSourceRewrite: vbaAllowExperimentalRewrite,
		}
		if strings.TrimSpace(opts.Name) == "" {
			opts.Name = strings.TrimSuffix(filepath.Base(vbaAddModuleSourcePath), filepath.Ext(vbaAddModuleSourcePath))
		}
		if strings.TrimSpace(opts.Kind) == "" {
			opts.Kind = strings.TrimPrefix(strings.ToLower(filepath.Ext(vbaAddModuleSourcePath)), ".")
		}
		return runVBASourceMutation(cmd, args[0], "add-module", func(session opc.PackageSession) (*vba.SourceMutationResult, *vba.SourceProject, error) {
			return vba.AddModuleSource(session, source, opts)
		})
	},
}

var vbaReplaceModuleCmd = &cobra.Command{
	Use:   "replace-module <file>",
	Short: "Replace an existing VBA source module",
	Long:  "Replace one existing .bas or .cls module source stream in a parseable vbaProject.bin. Exact no-op replacement preserves raw vbaProject.bin bytes; source-changing rewrites of Office-shaped projects require --allow-experimental-vba-source-rewrite. This does not add modules, preserve signatures, execute macros, or prove macro execution/compile compatibility; the Windows office-vba-smoke gate provides Microsoft Office COM open proof for Office-generated XLSM/PPTM replacement outputs.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if strings.TrimSpace(vbaReplaceModuleSelector) == "" {
			return InvalidArgsError("--module is required")
		}
		if strings.TrimSpace(vbaReplaceModuleSourcePath) == "" {
			return InvalidArgsError("--source is required")
		}
		source, err := vba.ReadModuleSourceFile(vbaReplaceModuleSourcePath)
		if err != nil {
			if os.IsNotExist(err) {
				return NewCLIErrorf(ExitFileNotFound, "failed to read VBA source: %v", err)
			}
			return NewCLIErrorf(ExitInvalidArgs, "failed to read VBA source: %v", err)
		}
		if err := preflightVBAModuleSelector(args[0], vbaReplaceModuleSelector); err != nil {
			return err
		}
		sourceKind := strings.TrimPrefix(strings.ToLower(filepath.Ext(vbaReplaceModuleSourcePath)), ".")
		return runVBASourceMutation(cmd, args[0], "replace-module", func(session opc.PackageSession) (*vba.SourceMutationResult, *vba.SourceProject, error) {
			return vba.ReplaceModuleSource(session, vbaReplaceModuleSelector, source, vbaReplaceModuleExpectSHA256, vba.SourceMutationOptions{
				AllowExperimentalSourceRewrite: vbaAllowExperimentalRewrite,
				SourceKind:                     sourceKind,
			})
		})
	},
}

var vbaRemoveModuleCmd = &cobra.Command{
	Use:   "remove-module <file>",
	Short: "Remove an existing VBA source module",
	Long:  "Remove one existing .bas or .cls module source stream from a parseable source-only/synthetic vbaProject.bin. The rewrite removes the module metadata/stream, removes compiled caches, and preserves remaining module streams. Real Office-shaped projects with version-dependent _VBA_PROJECT metadata are refused; remove the whole macro project with vba remove, or attach an Office-authored vbaProject.bin with the desired module set. This does not preserve signatures, execute macros, or prove Office-load compatibility.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if strings.TrimSpace(vbaRemoveModuleSelector) == "" {
			return InvalidArgsError("--module is required")
		}
		if err := preflightVBAModuleSelector(args[0], vbaRemoveModuleSelector); err != nil {
			return err
		}
		return runVBASourceMutation(cmd, args[0], "remove-module", func(session opc.PackageSession) (*vba.SourceMutationResult, *vba.SourceProject, error) {
			return vba.RemoveModuleSource(session, vbaRemoveModuleSelector, vbaRemoveModuleExpectSHA256, vba.SourceMutationOptions{AllowExperimentalSourceRewrite: vbaAllowExperimentalRewrite})
		})
	},
}

var vbaAttachCmd = &cobra.Command{
	Use:   "attach <file>",
	Short: "Attach or replace opaque vbaProject.bin",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		binPath, err := cmd.Flags().GetString("bin")
		if err != nil {
			return err
		}
		if binPath == "" {
			return InvalidArgsError("--bin is required")
		}
		projectData, err := os.ReadFile(binPath)
		if err != nil {
			return NewCLIErrorf(ExitFileNotFound, "failed to read VBA binary: %v", err)
		}

		return runVBAMutation(cmd, args[0], "attach", func(session opc.PackageSession) (*vba.MutationResult, error) {
			return vba.AttachWithOptions(session, projectData, vba.AttachOptions{AllowHostFamilyRisk: vbaAttachAllowHostFamilyRisk})
		})
	},
}

var vbaRemoveCmd = &cobra.Command{
	Use:   "remove <file>",
	Short: "Remove opaque VBA macro package artifacts",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		return runVBAMutation(cmd, args[0], "remove", func(session opc.PackageSession) (*vba.MutationResult, error) {
			return vba.Remove(session)
		})
	},
}

type VBAInspectResult struct {
	File                   string    `json:"file"`
	VBA                    *vba.Info `json:"vba"`
	ValidateCommand        string    `json:"validateCommand,omitempty"`
	OfficeCheckCommand     string    `json:"officeCheckCommand,omitempty"`
	PackageReadbackCommand string    `json:"packageReadbackCommand,omitempty"`
	ExtractBinCommand      string    `json:"extractBinCommand,omitempty"`
	NextMutationTemplate   string    `json:"nextMutationTemplate,omitempty"`
}

type VBAExtractResult struct {
	File                   string    `json:"file"`
	Output                 string    `json:"output"`
	BytesWritten           int       `json:"bytesWritten"`
	VBA                    *vba.Info `json:"vba"`
	InspectCommand         string    `json:"inspectCommand,omitempty"`
	ValidateCommand        string    `json:"validateCommand,omitempty"`
	OfficeCheckCommand     string    `json:"officeCheckCommand,omitempty"`
	PackageReadbackCommand string    `json:"packageReadbackCommand,omitempty"`
	AttachCommandTemplate  string    `json:"attachCommandTemplate,omitempty"`
}

type VBAModuleListResult struct {
	File                   string             `json:"file"`
	VBA                    *vba.Info          `json:"vba"`
	Project                *vba.SourceProject `json:"project"`
	InspectCommand         string             `json:"inspectCommand,omitempty"`
	ValidateCommand        string             `json:"validateCommand,omitempty"`
	OfficeCheckCommand     string             `json:"officeCheckCommand,omitempty"`
	PackageReadbackCommand string             `json:"packageReadbackCommand,omitempty"`
	ExtractCommandTemplate string             `json:"extractCommandTemplate,omitempty"`
}

type VBAInspectBinResult struct {
	File                  string             `json:"file"`
	SizeBytes             int                `json:"sizeBytes"`
	SHA256                string             `json:"sha256"`
	Family                string             `json:"family,omitempty"`
	Project               *vba.SourceProject `json:"project"`
	AttachCommandTemplate string             `json:"attachCommandTemplate,omitempty"`
}

type VBACreateOptions struct {
	Family                      string   `json:"family"`
	OutputPath                  string   `json:"outputPath"`
	SourcePaths                 []string `json:"sourcePaths"`
	ExtractBinPath              string   `json:"extractBinPath,omitempty"`
	OfficeCreateScriptPath      string   `json:"officeCreateScriptPath"`
	EnableVBOMAccess            bool     `json:"enableVbaObjectModelAccess"`
	Visible                     bool     `json:"visible"`
	Force                       bool     `json:"force"`
	Backend                     string   `json:"backend"`
	RequestedFamily             string   `json:"requestedFamily,omitempty"`
	InferredFamilyFromExtension bool     `json:"inferredFamilyFromExtension"`
}

type VBACreateImportedModule struct {
	Source string `json:"source"`
	Name   string `json:"name"`
	Type   string `json:"type,omitempty"`
}

type VBACreateNextCommands struct {
	Inspect     string `json:"inspect,omitempty"`
	List        string `json:"list,omitempty"`
	Validate    string `json:"validate,omitempty"`
	OfficeCheck string `json:"officeCheck,omitempty"`
	ExtractBin  string `json:"extractBin,omitempty"`
	AttachSeed  string `json:"attachSeed,omitempty"`
	Readback    string `json:"readback,omitempty"`
}

type VBACreateResult struct {
	Family                 string                    `json:"family"`
	Output                 string                    `json:"output"`
	OutputSHA256           string                    `json:"outputSha256,omitempty"`
	VBAProjectBin          string                    `json:"vbaProjectBin,omitempty"`
	VBAProjectBinSHA256    string                    `json:"vbaProjectBinSha256,omitempty"`
	Sources                []string                  `json:"sources"`
	ImportedModules        []VBACreateImportedModule `json:"importedModules,omitempty"`
	ProofLevel             string                    `json:"proofLevel"`
	Backend                string                    `json:"backend"`
	OfficeCreateScriptPath string                    `json:"officeCreateScriptPath,omitempty"`
	NextCommands           VBACreateNextCommands     `json:"nextCommands"`
	Limitations            []string                  `json:"limitations,omitempty"`
}

type VBAModuleExtractResult struct {
	File                   string                 `json:"file"`
	OutputDir              string                 `json:"outputDir"`
	VBA                    *vba.Info              `json:"vba"`
	Project                *vba.SourceProject     `json:"project"`
	Modules                []VBAModuleExtractItem `json:"modules"`
	InspectCommand         string                 `json:"inspectCommand,omitempty"`
	ValidateCommand        string                 `json:"validateCommand,omitempty"`
	OfficeCheckCommand     string                 `json:"officeCheckCommand,omitempty"`
	PackageReadbackCommand string                 `json:"packageReadbackCommand,omitempty"`
	ListCommand            string                 `json:"listCommand,omitempty"`
}

type VBAModuleReplaceResult struct {
	File                           string                    `json:"file"`
	Output                         string                    `json:"output,omitempty"`
	DryRun                         bool                      `json:"dryRun"`
	Result                         *vba.SourceMutationResult `json:"result"`
	VBA                            *vba.Info                 `json:"vba,omitempty"`
	Project                        *vba.SourceProject        `json:"project,omitempty"`
	InspectCommand                 string                    `json:"inspectCommand,omitempty"`
	ValidateCommand                string                    `json:"validateCommand,omitempty"`
	OfficeCheckCommand             string                    `json:"officeCheckCommand,omitempty"`
	PackageReadbackCommand         string                    `json:"packageReadbackCommand,omitempty"`
	ListCommand                    string                    `json:"listCommand,omitempty"`
	ExtractCommand                 string                    `json:"extractCommand,omitempty"`
	InspectCommandTemplate         string                    `json:"inspectCommandTemplate,omitempty"`
	ValidateCommandTemplate        string                    `json:"validateCommandTemplate,omitempty"`
	OfficeCheckCommandTemplate     string                    `json:"officeCheckCommandTemplate,omitempty"`
	PackageReadbackCommandTemplate string                    `json:"packageReadbackCommandTemplate,omitempty"`
	ListCommandTemplate            string                    `json:"listCommandTemplate,omitempty"`
	ExtractCommandTemplate         string                    `json:"extractCommandTemplate,omitempty"`
}

type VBAModuleExtractItem struct {
	Number          int      `json:"number"`
	Name            string   `json:"name"`
	StreamName      string   `json:"streamName"`
	Kind            string   `json:"kind"`
	Extension       string   `json:"extension"`
	OutputPath      string   `json:"outputPath"`
	BytesWritten    int      `json:"bytesWritten"`
	LineCount       int      `json:"lineCount,omitempty"`
	SHA256          string   `json:"sha256,omitempty"`
	SHA256Basis     string   `json:"sha256Basis,omitempty"`
	LineEnding      string   `json:"lineEnding,omitempty"`
	TrailingNewline bool     `json:"trailingNewline"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
	Warnings        []string `json:"warnings,omitempty"`
}

type VBAMutationCLIResult struct {
	File                           string              `json:"file"`
	Output                         string              `json:"output,omitempty"`
	DryRun                         bool                `json:"dryRun,omitempty"`
	Result                         *vba.MutationResult `json:"result"`
	VBA                            *vba.Info           `json:"vba,omitempty"`
	InspectCommand                 string              `json:"inspectCommand,omitempty"`
	ValidateCommand                string              `json:"validateCommand,omitempty"`
	OfficeCheckCommand             string              `json:"officeCheckCommand,omitempty"`
	PackageReadbackCommand         string              `json:"packageReadbackCommand,omitempty"`
	ListCommand                    string              `json:"listCommand,omitempty"`
	ExtractBinCommand              string              `json:"extractBinCommand,omitempty"`
	NextMutationTemplate           string              `json:"nextMutationTemplate,omitempty"`
	InspectCommandTemplate         string              `json:"inspectCommandTemplate,omitempty"`
	ValidateCommandTemplate        string              `json:"validateCommandTemplate,omitempty"`
	OfficeCheckCommandTemplate     string              `json:"officeCheckCommandTemplate,omitempty"`
	PackageReadbackCommandTemplate string              `json:"packageReadbackCommandTemplate,omitempty"`
	ListCommandTemplate            string              `json:"listCommandTemplate,omitempty"`
}

type VBAOfficeCheckResult struct {
	File             string                 `json:"file"`
	Family           string                 `json:"family"`
	PackageValid     bool                   `json:"packageValid"`
	Validation       VerifyValidationResult `json:"validation"`
	OpenCheck        *officecheck.Result    `json:"openCheck"`
	InspectCommand   string                 `json:"inspectCommand,omitempty"`
	ValidateCommand  string                 `json:"validateCommand,omitempty"`
	VBAListCommand   string                 `json:"vbaListCommand,omitempty"`
	Limitations      []string               `json:"limitations,omitempty"`
	OverallStatus    string                 `json:"overallStatus"`
	OverallVerified  bool                   `json:"overallVerified"`
	Compatibility    string                 `json:"compatibility"`
	MicrosoftOffice  bool                   `json:"microsoftOfficeVerified"`
	MacroExecution   bool                   `json:"macroExecutionVerified"`
	MacroCompilation bool                   `json:"macroCompilationVerified"`
}

func runVBACreate(cmd *cobra.Command, outputPath string) error {
	outputPath = strings.TrimSpace(outputPath)
	if outputPath == "" {
		return InvalidArgsError("output path is required")
	}
	family, inferred, err := normalizeVBACreateFamily(vbaCreateFamily, outputPath)
	if err != nil {
		return err
	}
	if err := validateVBACreateOutputExtension(family, outputPath); err != nil {
		return err
	}
	sources, err := normalizeVBACreateSources(vbaCreateSources)
	if err != nil {
		return err
	}
	if err := validateVBACreateSourceFiles(sources); err != nil {
		return err
	}
	scriptPath, err := resolveVBACreateScriptPath(vbaCreateOfficeScriptPath)
	if err != nil {
		return err
	}
	opts := VBACreateOptions{
		Family:                      family,
		OutputPath:                  outputPath,
		SourcePaths:                 sources,
		ExtractBinPath:              strings.TrimSpace(vbaCreateExtractBinPath),
		OfficeCreateScriptPath:      scriptPath,
		EnableVBOMAccess:            vbaCreateEnableVBOMAccess,
		Visible:                     vbaCreateVisible,
		Force:                       vbaCreateForce,
		Backend:                     "windows-office-com",
		RequestedFamily:             strings.TrimSpace(vbaCreateFamily),
		InferredFamilyFromExtension: inferred,
	}
	result, err := vbaCreateScriptRunner(opts)
	if err != nil {
		return err
	}
	completeVBACreateResult(result, opts)
	return outputVBACreate(cmd, result)
}

func normalizeVBACreateFamily(value, outputPath string) (string, bool, error) {
	value = strings.ToLower(strings.TrimSpace(value))
	if value == "" {
		switch strings.ToLower(filepath.Ext(outputPath)) {
		case ".xlsm":
			return "xlsx", true, nil
		case ".pptm":
			return "pptx", true, nil
		default:
			return "", false, NewCLIErrorf(ExitInvalidArgs, "--family is required when output extension is not .xlsm or .pptm")
		}
	}
	switch value {
	case "xlsx", "xlsm", "excel", "workbook":
		return "xlsx", false, nil
	case "pptx", "pptm", "powerpoint", "presentation", "deck":
		return "pptx", false, nil
	default:
		return "", false, NewCLIErrorf(ExitInvalidArgs, "--family must be xlsx or pptx")
	}
}

func validateVBACreateOutputExtension(family, outputPath string) error {
	ext := strings.ToLower(filepath.Ext(outputPath))
	switch family {
	case "xlsx":
		if ext != ".xlsm" {
			return NewCLIErrorf(ExitInvalidArgs, "output for family xlsx must end with .xlsm")
		}
	case "pptx":
		if ext != ".pptm" {
			return NewCLIErrorf(ExitInvalidArgs, "output for family pptx must end with .pptm")
		}
	default:
		return NewCLIErrorf(ExitInvalidArgs, "--family must be xlsx or pptx")
	}
	return nil
}

func normalizeVBACreateSources(values []string) ([]string, error) {
	var out []string
	for _, value := range values {
		out = append(out, expandVBACreateSourceValue(value)...)
	}
	if len(out) == 0 {
		return nil, InvalidArgsError("--source is required (repeat it for each .bas/.cls file)")
	}
	return out, nil
}

func expandVBACreateSourceValue(value string) []string {
	value = strings.TrimSpace(value)
	if value == "" {
		return nil
	}
	if _, err := os.Stat(value); err == nil {
		return []string{value}
	}
	var out []string
	for _, part := range strings.Split(value, ",") {
		part = strings.TrimSpace(part)
		if part != "" {
			out = append(out, part)
		}
	}
	return out
}

func validateVBACreateSourceFiles(paths []string) error {
	for _, path := range paths {
		if _, err := os.Stat(path); err != nil {
			if os.IsNotExist(err) {
				return NewCLIErrorf(ExitFileNotFound, "VBA source file not found: %s", path)
			}
			return NewCLIErrorf(ExitUnexpected, "failed to stat VBA source file %s: %v", path, err)
		}
		switch strings.ToLower(filepath.Ext(path)) {
		case ".bas", ".cls":
		default:
			return NewCLIErrorf(ExitInvalidArgs, "VBA source must be .bas or .cls: %s", path)
		}
	}
	return nil
}

func resolveVBACreateScriptPath(override string) (string, error) {
	if strings.TrimSpace(override) != "" {
		abs, err := filepath.Abs(override)
		if err != nil {
			abs = override
		}
		info, err := os.Stat(abs)
		if err != nil {
			if os.IsNotExist(err) {
				return "", NewCLIErrorf(ExitFileNotFound, "--office-create-script not found: %s", override)
			}
			return "", NewCLIErrorf(ExitUnexpected, "failed to stat --office-create-script %s: %v", override, err)
		}
		if info.IsDir() {
			return "", NewCLIErrorf(ExitInvalidArgs, "--office-create-script must be a file: %s", override)
		}
		return abs, nil
	}
	var candidates []string
	if cwd, err := os.Getwd(); err == nil {
		candidates = append(candidates, vbaOfficeCreateScriptCandidatesFrom(cwd)...)
	}
	if exe, err := os.Executable(); err == nil {
		candidates = append(candidates, vbaOfficeCreateScriptCandidatesFrom(filepath.Dir(exe))...)
	}
	seen := map[string]bool{}
	for _, candidate := range candidates {
		if strings.TrimSpace(candidate) == "" {
			continue
		}
		abs, err := filepath.Abs(candidate)
		if err != nil {
			abs = candidate
		}
		key := strings.ToLower(abs)
		if seen[key] {
			continue
		}
		seen[key] = true
		info, err := os.Stat(abs)
		if err == nil && !info.IsDir() {
			return abs, nil
		}
	}
	return "", NewCLIErrorf(ExitFileNotFound, "%s not found; run from the ooxml-cli checkout or pass --office-create-script .\\tools\\%s", vbaOfficeCreateScriptFileName, vbaOfficeCreateScriptFileName)
}

func vbaOfficeCreateScriptCandidatesFrom(start string) []string {
	var out []string
	dir := filepath.Clean(start)
	for {
		out = append(out, filepath.Join(dir, vbaOfficeCreateScriptDirectory, vbaOfficeCreateScriptFileName))
		parent := filepath.Dir(dir)
		if parent == dir {
			break
		}
		dir = parent
	}
	return out
}

func invokeVBACreateOfficeScript(opts VBACreateOptions) (*VBACreateResult, error) {
	if runtime.GOOS != "windows" {
		return nil, NewCLIErrorf(ExitUnsupportedType, "vba create requires Windows desktop Microsoft Office; on other platforms create or obtain an Office-authored vbaProject.bin and use ooxml vba attach")
	}
	powerShellPath, err := exec.LookPath("powershell.exe")
	if err != nil {
		return nil, NewCLIErrorf(ExitUnsupportedType, "vba create requires powershell.exe and desktop Microsoft Office on Windows")
	}
	args := buildVBACreateOfficeScriptArgs(opts)
	command := exec.Command(powerShellPath, args...)
	var stdout, stderr bytes.Buffer
	command.Stdout = &stdout
	command.Stderr = &stderr
	if err := command.Run(); err != nil {
		detail := strings.TrimSpace(stderr.String())
		if detail == "" {
			detail = strings.TrimSpace(stdout.String())
		}
		if detail != "" {
			return nil, NewCLIErrorf(ExitUnexpected, "vba create Office automation failed: %v: %s", err, detail)
		}
		return nil, NewCLIErrorf(ExitUnexpected, "vba create Office automation failed: %v", err)
	}
	result, err := parseVBACreateScriptResult(stdout.Bytes())
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "vba create helper returned invalid JSON: %v", err)
	}
	return result, nil
}

func buildVBACreateOfficeScriptArgs(opts VBACreateOptions) []string {
	sourceJSON, _ := json.Marshal(opts.SourcePaths)
	args := []string{
		"-NoProfile",
		"-ExecutionPolicy", "Bypass",
		"-File", opts.OfficeCreateScriptPath,
		"-Family", opts.Family,
		"-OutputPath", opts.OutputPath,
		"-SourcePathJson", string(sourceJSON),
	}
	if strings.TrimSpace(opts.ExtractBinPath) != "" {
		args = append(args, "-ExtractBinPath", opts.ExtractBinPath)
	}
	if opts.EnableVBOMAccess {
		args = append(args, "-EnableVbaObjectModelAccess")
	}
	if opts.Visible {
		args = append(args, "-Visible")
	}
	if opts.Force {
		args = append(args, "-Force")
	}
	return args
}

func parseVBACreateScriptResult(data []byte) (*VBACreateResult, error) {
	var scriptResult struct {
		Family              string                `json:"family"`
		Output              string                `json:"output"`
		OutputSHA256        string                `json:"outputSha256"`
		VBAProjectBin       string                `json:"vbaProjectBin"`
		VBAProjectBinSHA256 string                `json:"vbaProjectBinSha256"`
		Sources             json.RawMessage       `json:"sources"`
		ImportedModules     json.RawMessage       `json:"importedModules"`
		ProofLevel          string                `json:"proofLevel"`
		NextCommands        VBACreateNextCommands `json:"nextCommands"`
	}
	if err := json.Unmarshal(data, &scriptResult); err != nil {
		return nil, err
	}
	sources, err := decodeVBACreateStringList(scriptResult.Sources)
	if err != nil {
		return nil, fmt.Errorf("sources: %w", err)
	}
	importedModules, err := decodeVBACreateImportedModules(scriptResult.ImportedModules)
	if err != nil {
		return nil, fmt.Errorf("importedModules: %w", err)
	}
	return &VBACreateResult{
		Family:              scriptResult.Family,
		Output:              scriptResult.Output,
		OutputSHA256:        scriptResult.OutputSHA256,
		VBAProjectBin:       scriptResult.VBAProjectBin,
		VBAProjectBinSHA256: scriptResult.VBAProjectBinSHA256,
		Sources:             sources,
		ImportedModules:     importedModules,
		ProofLevel:          scriptResult.ProofLevel,
		NextCommands:        scriptResult.NextCommands,
	}, nil
}

func decodeVBACreateStringList(data json.RawMessage) ([]string, error) {
	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 || bytes.Equal(trimmed, []byte("null")) {
		return nil, nil
	}
	var list []string
	if err := json.Unmarshal(trimmed, &list); err == nil {
		return list, nil
	}
	var single string
	if err := json.Unmarshal(trimmed, &single); err != nil {
		return nil, err
	}
	return []string{single}, nil
}

func decodeVBACreateImportedModules(data json.RawMessage) ([]VBACreateImportedModule, error) {
	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 || bytes.Equal(trimmed, []byte("null")) {
		return nil, nil
	}
	var list []VBACreateImportedModule
	if err := json.Unmarshal(trimmed, &list); err == nil {
		return list, nil
	}
	var single VBACreateImportedModule
	if err := json.Unmarshal(trimmed, &single); err != nil {
		return nil, err
	}
	return []VBACreateImportedModule{single}, nil
}

func completeVBACreateResult(result *VBACreateResult, opts VBACreateOptions) {
	if result == nil {
		return
	}
	if result.Family == "" {
		result.Family = opts.Family
	}
	if result.Output == "" {
		result.Output = opts.OutputPath
	}
	if len(result.Sources) == 0 {
		result.Sources = append([]string{}, opts.SourcePaths...)
	}
	if result.VBAProjectBin == "" {
		result.VBAProjectBin = opts.ExtractBinPath
	}
	if result.ProofLevel == "" {
		result.ProofLevel = "microsoft-office-authored"
	}
	result.Backend = opts.Backend
	result.OfficeCreateScriptPath = opts.OfficeCreateScriptPath
	result.NextCommands = VBACreateNextCommands{
		Inspect:     vbaInspectCommand(result.Output),
		List:        vbaListCommand(result.Output),
		Validate:    vbaValidateCommand(result.Output),
		OfficeCheck: fmt.Sprintf("ooxml --json vba office-check %s", pptxXLSXCommandArg(result.Output)),
		Readback:    vbaPackageReadbackCommandForFamily(result.Output, result.Family),
	}
	if strings.TrimSpace(result.VBAProjectBin) == "" {
		result.NextCommands.ExtractBin = fmt.Sprintf("ooxml --json vba extract-bin %s --out vbaProject.bin", pptxXLSXCommandArg(result.Output))
	} else {
		result.NextCommands.AttachSeed = vbaAttachTemplateForStandaloneBin(result.VBAProjectBin, result.Family)
	}
	result.Limitations = []string{
		"Desktop Office authored and saved the package through COM; macros were imported but not executed.",
		"Macro execution, VBE compile proof, signatures/resigning, forms, and password/protection editing are not verified by vba create.",
	}
}

func outputVBACreate(cmd *cobra.Command, result *VBACreateResult) error {
	if GetGlobalConfig(cmd).Format == "json" {
		return outputVBAJSON(cmd, result)
	}
	lines := []string{
		fmt.Sprintf("created: %s", result.Output),
		fmt.Sprintf("family: %s", result.Family),
		fmt.Sprintf("proofLevel: %s", result.ProofLevel),
	}
	if result.VBAProjectBin != "" {
		lines = append(lines, fmt.Sprintf("vbaProjectBin: %s", result.VBAProjectBin))
	}
	if len(result.ImportedModules) > 0 {
		lines = append(lines, "modules:")
		for _, module := range result.ImportedModules {
			lines = append(lines, fmt.Sprintf("  - %s (%s)", module.Name, module.Source))
		}
	}
	return writeCLIOutput(cmd, []byte(strings.Join(lines, "\n")+"\n"))
}

func runVBAMutation(cmd *cobra.Command, inputPath, action string, mutate func(opc.PackageSession) (*vba.MutationResult, error)) error {
	mutOpts, err := GetMutationOptions(cmd)
	if err != nil {
		return err
	}
	expectedType, err := detectVBAPackageType(inputPath)
	if err != nil {
		return err
	}

	writer, err := NewMutationWriterForType(inputPath, mutOpts, expectedType)
	if err != nil {
		return err
	}

	var mutationResult *vba.MutationResult
	var stagedInfo *vba.Info
	if err := writer.Write(func(session opc.PackageSession) error {
		result, err := mutate(session)
		if err != nil {
			return mapVBAError(err)
		}
		mutationResult = result
		info, err := vba.Inspect(session)
		if err != nil {
			return mapVBAError(err)
		}
		stagedInfo = info
		return nil
	}); err != nil {
		return mutationWriteError(err, "failed to "+action+" VBA project")
	}

	outputPath := mutOpts.OutPath
	if mutOpts.InPlace {
		outputPath = inputPath
	}
	mutationInfo := stagedInfo
	if !mutOpts.DryRun {
		info, err := inspectVBAFileForCLI(outputPath)
		if err != nil {
			return err
		}
		mutationInfo = info
	}
	return outputVBAMutation(cmd, inputPath, outputPath, mutOpts.DryRun, mutationResult, mutationInfo)
}

func runVBASourceMutation(cmd *cobra.Command, inputPath, action string, mutate func(opc.PackageSession) (*vba.SourceMutationResult, *vba.SourceProject, error)) error {
	mutOpts, err := GetMutationOptions(cmd)
	if err != nil {
		return err
	}
	expectedType, err := detectVBAPackageType(inputPath)
	if err != nil {
		return err
	}

	writer, err := NewMutationWriterForType(inputPath, mutOpts, expectedType)
	if err != nil {
		return err
	}

	var mutationResult *vba.SourceMutationResult
	var stagedInfo *vba.Info
	var stagedProject *vba.SourceProject
	if err := writer.Write(func(session opc.PackageSession) error {
		result, project, err := mutate(session)
		if err != nil {
			return mapVBAError(err)
		}
		mutationResult = result
		info, err := vba.Inspect(session)
		if err != nil {
			return mapVBAError(err)
		}
		stagedInfo = info
		stagedProject = project
		return nil
	}); err != nil {
		return mutationWriteError(err, "failed to "+action+" VBA module")
	}

	outputPath := mutOpts.OutPath
	if mutOpts.InPlace {
		outputPath = inputPath
	}
	mutationInfo := stagedInfo
	mutationProject := stagedProject
	if !mutOpts.DryRun {
		info, project, err := inspectVBASourceFileForCLI(outputPath)
		if err != nil {
			return err
		}
		mutationInfo = info
		mutationProject = project
	}
	return outputVBAModuleReplace(cmd, inputPath, outputPath, mutOpts.DryRun, mutationResult, mutationInfo, mutationProject)
}

func inspectVBAFileForCLI(path string) (*vba.Info, error) {
	pkg, err := opc.Open(path)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to open VBA mutation output: %v", err)
	}
	defer pkg.Close()
	info, err := vba.Inspect(pkg)
	if err != nil {
		return nil, mapVBAError(err)
	}
	return info, nil
}

func inspectVBASourceFileForCLI(path string) (*vba.Info, *vba.SourceProject, error) {
	pkg, err := opc.Open(path)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to open VBA mutation output: %v", err)
	}
	defer pkg.Close()
	info, err := vba.Inspect(pkg)
	if err != nil {
		return nil, nil, mapVBAError(err)
	}
	project, err := vba.InspectSourceProject(pkg)
	if err != nil {
		return nil, nil, mapVBAError(err)
	}
	return info, project, nil
}

func runVBAOfficeCheck(cmd *cobra.Command, filePath string) error {
	if _, err := os.Stat(filePath); err != nil {
		return FileNotFoundError(filePath)
	}
	pkg, err := opc.Open(filePath)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
	}
	packageType := opc.DetectType(pkg)
	if packageType != opc.PackageTypePPTX && packageType != opc.PackageTypeXLSX {
		pkg.Close()
		return NewCLIErrorf(ExitUnsupportedType, "vba office-check supports PPTM/XLSM packages only (detected: %s)", packageType)
	}
	info, inspectErr := vba.Inspect(pkg)
	diags, err := validate.ValidatePackage(pkg)
	if closeErr := pkg.Close(); err == nil && closeErr != nil {
		err = closeErr
	}
	if inspectErr != nil {
		return mapVBAError(inspectErr)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "validation error: %v", err)
	}

	validation, packageValid := summarizeValidation(diags, true)
	if info == nil || !info.HasVBAProject {
		result := vbaOfficeCheckResult(filePath, packageType, false, validation, &officecheck.Result{
			Status:                 "skipped",
			Checked:                false,
			ErrorCode:              "missing_vba_project",
			Error:                  "package has no vbaProject.bin part; vba office-check only applies to macro packages",
			MacroExecutionVerified: false,
			Limitations:            vbaOfficeCheckLimitations(),
		})
		if outErr := outputVBAOfficeCheck(cmd, result); outErr != nil {
			return outErr
		}
		err := NewCLIError(ExitUnsupportedType, "")
		err.Reported = true
		return err
	}
	if !packageValid {
		result := vbaOfficeCheckResult(filePath, packageType, packageValid, validation, &officecheck.Result{
			Status:                 "skipped",
			Checked:                false,
			ErrorCode:              "package_validation_failed",
			Error:                  "package validation failed; fix validation diagnostics before running an Office-compatible open check",
			MacroExecutionVerified: false,
			Limitations:            vbaOfficeCheckLimitations(),
		})
		if outErr := outputVBAOfficeCheck(cmd, result); outErr != nil {
			return outErr
		}
		err := NewCLIError(ExitValidationFailed, "")
		err.Reported = true
		return err
	}

	check, checkErr := vbaOfficeCheckToolsFactory().Check(filePath, officecheck.Options{Family: packageType.String(), OutDir: vbaOfficeCheckOutDir})
	result := vbaOfficeCheckResult(filePath, packageType, packageValid, validation, check)
	if outErr := outputVBAOfficeCheck(cmd, result); outErr != nil {
		return outErr
	}
	if checkErr != nil {
		err := NewCLIError(ExitRenderFailed, "")
		err.Reported = true
		return err
	}
	return nil
}

func vbaOfficeCheckResult(filePath string, packageType opc.PackageType, packageValid bool, validation VerifyValidationResult, check *officecheck.Result) VBAOfficeCheckResult {
	if check == nil {
		check = &officecheck.Result{Status: "skipped", ErrorCode: "not_run", Error: "office open check did not run", Limitations: vbaOfficeCheckLimitations()}
	}
	status := "failed"
	if packageValid && check.Status == "passed" && check.OfficeOpenVerified {
		status = "passed"
	} else if check.Status == "skipped" {
		status = "skipped"
	}
	return VBAOfficeCheckResult{
		File:             filePath,
		Family:           packageType.String(),
		PackageValid:     packageValid,
		Validation:       validation,
		OpenCheck:        check,
		InspectCommand:   vbaInspectCommand(filePath),
		ValidateCommand:  vbaValidateCommand(filePath),
		VBAListCommand:   vbaListCommand(filePath),
		Limitations:      vbaOfficeCheckLimitations(),
		OverallStatus:    status,
		OverallVerified:  packageValid && check.OfficeOpenVerified,
		Compatibility:    "local-engine-open-check",
		MicrosoftOffice:  false,
		MacroExecution:   false,
		MacroCompilation: false,
	}
}

func outputVBAOfficeCheck(cmd *cobra.Command, result VBAOfficeCheckResult) error {
	if GetGlobalConfig(cmd).Format == "json" {
		return outputVBAJSON(cmd, result)
	}
	lines := []string{
		fmt.Sprintf("file: %s", result.File),
		fmt.Sprintf("family: %s", result.Family),
		fmt.Sprintf("packageValid: %t", result.PackageValid),
		fmt.Sprintf("officeOpenCheck: %s", result.OpenCheck.Status),
		fmt.Sprintf("overallStatus: %s", result.OverallStatus),
	}
	if result.OpenCheck.Engine != "" {
		lines = append(lines, fmt.Sprintf("engine: %s", result.OpenCheck.Engine))
	}
	if result.OpenCheck.Error != "" {
		lines = append(lines, fmt.Sprintf("error: %s", result.OpenCheck.Error))
	}
	if len(result.Limitations) > 0 {
		lines = append(lines, "limitations:")
		for _, limitation := range result.Limitations {
			lines = append(lines, "  - "+limitation)
		}
	}
	return writeCLIOutput(cmd, []byte(strings.Join(lines, "\n")+"\n"))
}

func detectVBAPackageType(path string) (opc.PackageType, error) {
	pkg, err := opc.Open(path)
	if err != nil {
		return opc.PackageTypeUnknown, NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
	}
	defer pkg.Close()
	packageType := opc.DetectType(pkg)
	switch packageType {
	case opc.PackageTypePPTX, opc.PackageTypeXLSX:
		return packageType, nil
	default:
		return packageType, NewCLIErrorf(ExitUnsupportedType, "VBA package operations support PPTX/PPTM and XLSX/XLSM only (detected: %s)", packageType)
	}
}

func outputVBAInspect(cmd *cobra.Command, filePath string, info *vba.Info) error {
	config := GetGlobalConfig(cmd)
	result := VBAInspectResult{
		File:                   filePath,
		VBA:                    info,
		ValidateCommand:        vbaValidateCommand(filePath),
		OfficeCheckCommand:     vbaOfficeCheckCommand(filePath, info),
		PackageReadbackCommand: vbaPackageReadbackCommand(filePath, info),
		ExtractBinCommand:      vbaExtractBinCommand(filePath, info),
		NextMutationTemplate:   vbaNextMutationTemplate(filePath, info),
	}
	if config.Format == "json" {
		return outputVBAJSON(cmd, result)
	}

	text := fmt.Sprintf("family: %s\nmacroEnabled: %t\nmain: %s\nmainContentType: %s\n", info.Family, info.MacroEnabled, info.MainPartURI, info.MainContentType)
	if info.VBAProject != nil {
		text += fmt.Sprintf("vbaProject: %s\nexists: %t\n", info.VBAProject.PartURI, info.VBAProject.Exists)
		if info.VBAProject.SizeBytes > 0 {
			text += fmt.Sprintf("sizeBytes: %d\nsha256: %s\n", info.VBAProject.SizeBytes, info.VBAProject.SHA256)
		}
		if info.VBAProject.RelationshipID != "" {
			text += fmt.Sprintf("relationship: %s -> %s\n", info.VBAProject.RelationshipID, info.VBAProject.RelationshipTarget)
		}
	} else {
		text += "vbaProject: none\n"
	}
	text += formatVBAWarnings(info)
	return writeCLIOutput(cmd, []byte(text))
}

func outputVBAExtract(cmd *cobra.Command, filePath, outPath string, bytesWritten int, info *vba.Info) error {
	config := GetGlobalConfig(cmd)
	result := VBAExtractResult{
		File:                   filePath,
		Output:                 outPath,
		BytesWritten:           bytesWritten,
		VBA:                    info,
		InspectCommand:         vbaInspectCommand(filePath),
		ValidateCommand:        vbaValidateCommand(filePath),
		OfficeCheckCommand:     vbaOfficeCheckCommand(filePath, info),
		PackageReadbackCommand: vbaPackageReadbackCommand(filePath, info),
		AttachCommandTemplate:  vbaAttachTemplateForBin(outPath, info),
	}
	if config.Format == "json" {
		return outputVBAJSON(cmd, result)
	}
	text := fmt.Sprintf("extracted: %s\nbytesWritten: %d\n", outPath, bytesWritten)
	return writeCLIOutput(cmd, []byte(text))
}

func outputVBAModuleList(cmd *cobra.Command, filePath string, info *vba.Info, project *vba.SourceProject) error {
	config := GetGlobalConfig(cmd)
	project = summarizeVBASourceProject(project)
	result := VBAModuleListResult{
		File:                   filePath,
		VBA:                    info,
		Project:                project,
		InspectCommand:         vbaInspectCommand(filePath),
		ValidateCommand:        vbaValidateCommand(filePath),
		OfficeCheckCommand:     vbaOfficeCheckCommand(filePath, info),
		PackageReadbackCommand: vbaPackageReadbackCommand(filePath, info),
		ExtractCommandTemplate: vbaExtractModulesTemplate(filePath),
	}
	if config.Format == "json" {
		return outputVBAJSON(cmd, result)
	}
	lines := []string{fmt.Sprintf("%-4s %-28s %-10s %-20s %s", "[N]", "Name", "Kind", "Stream", "Lines")}
	lines = append(lines, strings.Repeat("-", 80))
	for _, module := range project.Modules {
		lines = append(lines, fmt.Sprintf("[%-2d] %-28s %-10s %-20s %d", module.Number, truncateStr(module.Name, 28), module.Kind, truncateStr(module.StreamName, 20), module.LineCount))
	}
	if len(project.Warnings) > 0 {
		lines = append(lines, "", "Warnings:")
		for _, warning := range project.Warnings {
			lines = append(lines, "  - "+warning)
		}
	}
	return writeCLIOutput(cmd, []byte(strings.Join(lines, "\n")))
}

func outputVBAInspectBin(cmd *cobra.Command, filePath string, data []byte, project *vba.SourceProject) error {
	config := GetGlobalConfig(cmd)
	project = summarizeVBASourceProject(project)
	sum := sha256.Sum256(data)
	result := VBAInspectBinResult{
		File:                  filePath,
		SizeBytes:             len(data),
		SHA256:                fmt.Sprintf("%x", sum[:]),
		Family:                project.Family,
		Project:               project,
		AttachCommandTemplate: vbaAttachTemplateForStandaloneBin(filePath, project.Family),
	}
	if config.Format == "json" {
		return outputVBAJSON(cmd, result)
	}
	lines := []string{
		fmt.Sprintf("file: %s", filePath),
		fmt.Sprintf("sizeBytes: %d", len(data)),
		fmt.Sprintf("sha256: %s", result.SHA256),
		fmt.Sprintf("modules: %d", project.ModuleCount),
	}
	if project.Family != "" {
		lines = append(lines, fmt.Sprintf("family: %s", project.Family))
	}
	if project.OfficeCompatibility != nil {
		lines = append(lines, fmt.Sprintf("officeCompatibility: %s", project.OfficeCompatibility.Status))
	}
	if len(project.Warnings) > 0 {
		lines = append(lines, "warnings:")
		for _, warning := range project.Warnings {
			lines = append(lines, "  - "+warning)
		}
	}
	return writeCLIOutput(cmd, []byte(strings.Join(lines, "\n")))
}

func outputVBAModuleExtract(cmd *cobra.Command, filePath, outDir string, info *vba.Info, project *vba.SourceProject, modules []VBAModuleExtractItem) error {
	config := GetGlobalConfig(cmd)
	result := VBAModuleExtractResult{
		File:                   filePath,
		OutputDir:              outDir,
		VBA:                    info,
		Project:                summarizeVBASourceProject(project),
		Modules:                modules,
		InspectCommand:         vbaInspectCommand(filePath),
		ValidateCommand:        vbaValidateCommand(filePath),
		OfficeCheckCommand:     vbaOfficeCheckCommand(filePath, info),
		PackageReadbackCommand: vbaPackageReadbackCommand(filePath, info),
		ListCommand:            vbaListCommand(filePath),
	}
	if config.Format == "json" {
		return outputVBAJSON(cmd, result)
	}
	lines := make([]string, 0, len(modules))
	for _, module := range modules {
		lines = append(lines, fmt.Sprintf("extracted: %s", module.OutputPath))
	}
	return writeCLIOutput(cmd, []byte(strings.Join(lines, "\n")))
}

func outputVBAModuleReplace(cmd *cobra.Command, filePath, outPath string, dryRun bool, result *vba.SourceMutationResult, info *vba.Info, project *vba.SourceProject) error {
	config := GetGlobalConfig(cmd)
	project = summarizeVBASourceProject(project)
	cliResult := VBAModuleReplaceResult{File: filePath, Output: outPath, DryRun: dryRun, Result: result, VBA: info, Project: project}
	action := "replace-module"
	if result != nil && strings.TrimSpace(result.Action) != "" {
		action = result.Action
	}
	moduleSelector := ""
	if result != nil && result.Module.PrimarySelector != "" {
		moduleSelector = result.Module.PrimarySelector
	} else if action == "remove-module" {
		moduleSelector = vbaRemoveModuleSelector
	} else {
		moduleSelector = vbaReplaceModuleSelector
	}
	if dryRun {
		placeholder := vbaMutationOutputPlaceholder(&vba.MutationResult{Action: action, MacroEnabled: true}, info)
		cliResult.InspectCommandTemplate = vbaInspectCommand(placeholder)
		cliResult.ValidateCommandTemplate = vbaValidateCommand(placeholder)
		cliResult.OfficeCheckCommandTemplate = vbaOfficeCheckCommand(placeholder, info)
		cliResult.PackageReadbackCommandTemplate = vbaPackageReadbackCommand(placeholder, info)
		cliResult.ListCommandTemplate = vbaListCommand(placeholder)
		if action != "remove-module" {
			cliResult.ExtractCommandTemplate = vbaExtractModuleCommand(placeholder, moduleSelector)
		}
	} else if outPath != "" {
		cliResult.InspectCommand = vbaInspectCommand(outPath)
		cliResult.ValidateCommand = vbaValidateCommand(outPath)
		cliResult.OfficeCheckCommand = vbaOfficeCheckCommand(outPath, info)
		cliResult.PackageReadbackCommand = vbaPackageReadbackCommand(outPath, info)
		cliResult.ListCommand = vbaListCommand(outPath)
		if action != "remove-module" {
			cliResult.ExtractCommand = vbaExtractModuleCommand(outPath, moduleSelector)
		}
	}
	if config.Format == "json" {
		return outputVBAJSON(cmd, cliResult)
	}
	if dryRun {
		action = "would " + action
	}
	moduleName := ""
	if result != nil {
		moduleName = result.Module.Name
	}
	text := fmt.Sprintf("%s: %s\nmodule: %s\n", action, filePath, moduleName)
	if !dryRun {
		text += fmt.Sprintf("output: %s\n", outPath)
	}
	return writeCLIOutput(cmd, []byte(text))
}

func outputVBAMutation(cmd *cobra.Command, filePath, outPath string, dryRun bool, result *vba.MutationResult, info *vba.Info) error {
	config := GetGlobalConfig(cmd)
	cliResult := VBAMutationCLIResult{File: filePath, Output: outPath, DryRun: dryRun, Result: result, VBA: info}
	if dryRun {
		placeholder := vbaMutationOutputPlaceholder(result, info)
		cliResult.InspectCommandTemplate = vbaInspectCommand(placeholder)
		cliResult.ValidateCommandTemplate = vbaValidateCommand(placeholder)
		cliResult.OfficeCheckCommandTemplate = vbaOfficeCheckCommand(placeholder, info)
		cliResult.PackageReadbackCommandTemplate = vbaPackageReadbackCommand(placeholder, info)
	} else if outPath != "" {
		cliResult.InspectCommand = vbaInspectCommand(outPath)
		cliResult.ValidateCommand = vbaValidateCommand(outPath)
		cliResult.OfficeCheckCommand = vbaOfficeCheckCommand(outPath, info)
		cliResult.PackageReadbackCommand = vbaPackageReadbackCommand(outPath, info)
		if info != nil && info.HasVBAProject && vbaFileHasParseableSourceProject(outPath) {
			cliResult.ListCommand = vbaListCommand(outPath)
		}
		cliResult.ExtractBinCommand = vbaExtractBinCommand(outPath, info)
		cliResult.NextMutationTemplate = vbaNextMutationTemplate(outPath, info)
	}
	if config.Format == "json" {
		return outputVBAJSON(cmd, cliResult)
	}
	action := "updated"
	if result != nil {
		action = result.Action
	}
	text := fmt.Sprintf("%s: %s\n", action, filePath)
	if dryRun {
		text += "dryRun: true\n"
	} else {
		text += fmt.Sprintf("output: %s\n", outPath)
	}
	if result != nil && result.VBAPartURI != "" {
		text += fmt.Sprintf("vbaProject: %s\n", result.VBAPartURI)
	}
	return writeCLIOutput(cmd, []byte(text))
}

func vbaFileHasParseableSourceProject(filePath string) bool {
	pkg, err := opc.Open(filePath)
	if err != nil {
		return false
	}
	defer pkg.Close()
	_, err = vba.InspectSourceProject(pkg)
	return err == nil
}

func outputVBAJSON(cmd *cobra.Command, value any) error {
	return writeLabeledJSON(cmd, value, "VBA")
}

func formatVBAWarnings(info *vba.Info) string {
	if info == nil || len(info.Warnings) == 0 {
		return ""
	}
	text := "warnings:\n"
	for _, warning := range info.Warnings {
		text += "  - " + warning + "\n"
	}
	return text
}

func vbaInspectCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json vba inspect %s", pptxXLSXCommandArg(filePath))
}

func vbaListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json vba list %s", pptxXLSXCommandArg(filePath))
}

func vbaValidateCommand(filePath string) string {
	return fmt.Sprintf("ooxml validate --strict %s", pptxXLSXCommandArg(filePath))
}

func vbaOfficeCheckCommand(filePath string, info *vba.Info) string {
	if info == nil || (!info.HasVBAProject && !info.MacroEnabled) {
		return ""
	}
	return fmt.Sprintf("ooxml --json vba office-check %s", pptxXLSXCommandArg(filePath))
}

func vbaExtractBinCommand(filePath string, info *vba.Info) string {
	if info == nil || !info.HasVBAProject {
		return ""
	}
	return fmt.Sprintf("ooxml --json vba extract-bin %s --out vbaProject.bin", pptxXLSXCommandArg(filePath))
}

func vbaExtractModulesTemplate(filePath string) string {
	return fmt.Sprintf("ooxml --json vba extract %s --out-dir macros", pptxXLSXCommandArg(filePath))
}

func vbaExtractModuleCommand(filePath, moduleSelector string) string {
	command := fmt.Sprintf("ooxml --json vba extract %s --out-dir macros", pptxXLSXCommandArg(filePath))
	if strings.TrimSpace(moduleSelector) != "" {
		command += " --module " + pptxXLSXCommandArg(moduleSelector)
	}
	return command
}

func vbaPackageReadbackCommand(filePath string, info *vba.Info) string {
	if info == nil {
		return ""
	}
	return vbaPackageReadbackCommandForFamily(filePath, info.Family)
}

func vbaPackageReadbackCommandForFamily(filePath, family string) string {
	switch family {
	case "pptx":
		return fmt.Sprintf("ooxml --json pptx slides list %s", pptxXLSXCommandArg(filePath))
	case "xlsx":
		return fmt.Sprintf("ooxml --json xlsx sheets list %s", pptxXLSXCommandArg(filePath))
	default:
		return ""
	}
}

func vbaNextMutationTemplate(filePath string, info *vba.Info) string {
	if info == nil {
		return ""
	}
	if info.HasVBAProject || info.MacroEnabled {
		return fmt.Sprintf("ooxml --json vba remove %s --out %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(vbaOutputPlaceholder(info.NonMacroExtension)))
	}
	return fmt.Sprintf("ooxml --json vba attach %s --bin vbaProject.bin --out %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(vbaOutputPlaceholder(info.MacroExtension)))
}

func vbaAttachTemplateForBin(binPath string, info *vba.Info) string {
	if info == nil {
		return fmt.Sprintf("ooxml --json vba attach <target> --bin %s --out <macro-output>", pptxXLSXCommandArg(binPath))
	}
	return fmt.Sprintf("ooxml --json vba attach %s --bin %s --out %s", pptxXLSXCommandArg(vbaOutputPlaceholder(info.NonMacroExtension)), pptxXLSXCommandArg(binPath), pptxXLSXCommandArg(vbaOutputPlaceholder(info.MacroExtension)))
}

func vbaAttachTemplateForStandaloneBin(binPath, family string) string {
	switch strings.ToLower(strings.TrimSpace(family)) {
	case "pptx":
		return fmt.Sprintf("ooxml --json vba attach deck.pptx --bin %s --out deck.pptm", pptxXLSXCommandArg(binPath))
	case "xlsx":
		return fmt.Sprintf("ooxml --json vba attach workbook.xlsx --bin %s --out workbook.xlsm", pptxXLSXCommandArg(binPath))
	default:
		return fmt.Sprintf("ooxml --json vba attach <target.pptx|target.xlsx> --bin %s --out <macro-output.pptm|macro-output.xlsm>", pptxXLSXCommandArg(binPath))
	}
}

func vbaMutationOutputPlaceholder(result *vba.MutationResult, info *vba.Info) string {
	extension := ""
	if result != nil && result.MacroEnabled && info != nil {
		extension = info.MacroExtension
	}
	if result != nil && !result.MacroEnabled && info != nil {
		extension = info.NonMacroExtension
	}
	if extension == "" && info != nil {
		if info.MacroEnabled {
			extension = info.MacroExtension
		} else {
			extension = info.NonMacroExtension
		}
	}
	return vbaOutputPlaceholder(extension)
}

func vbaOutputPlaceholder(extension string) string {
	if extension == "" {
		return "<out>"
	}
	return "<out" + extension + ">"
}

func vbaOfficeCheckLimitations() []string {
	return []string{
		"LibreOffice/soffice load and conversion is compatibility evidence, not Microsoft Office proof.",
		"Macros are not executed, compiled, or security-reviewed by this check.",
	}
}

func mapVBAError(err error) error {
	if err == nil {
		return nil
	}
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr
	}
	message := err.Error()
	switch {
	case strings.Contains(message, "support PPTX/PPTM and XLSX/XLSM only"):
		return NewCLIErrorf(ExitUnsupportedType, "%s", message)
	case strings.Contains(message, "no vbaProject.bin"):
		return NewCLIErrorf(ExitTargetNotFound, "%s", message)
	case strings.Contains(message, "VBA module not found"):
		return NewCLIErrorf(ExitTargetNotFound, "%s", message)
	case strings.Contains(message, "source hash mismatch") ||
		strings.Contains(message, "module count mismatch") ||
		strings.Contains(message, "module name") ||
		strings.Contains(message, "module kind") ||
		strings.Contains(message, "already exists") ||
		strings.Contains(message, "is ambiguous") ||
		strings.Contains(message, "cannot be encoded") ||
		strings.Contains(message, "Attribute VB_Name") ||
		strings.Contains(message, "incompatible with target module") ||
		strings.Contains(message, "experimental VBA source rewrite refused") ||
		strings.Contains(message, "version-dependent _VBA_PROJECT metadata") ||
		strings.Contains(message, "VBA host-family risk refused") ||
		strings.Contains(message, "source file is empty") ||
		strings.Contains(message, "module selector is required"):
		return NewCLIErrorf(ExitInvalidArgs, "%s", message)
	case strings.Contains(message, "Compound File Binary") ||
		strings.Contains(message, "VBA dir stream") ||
		strings.Contains(message, "compressed") ||
		strings.Contains(message, "PROJECTMODULES"):
		return NewCLIErrorf(ExitInvalidArgs, "%s", message)
	case strings.Contains(message, "is empty"):
		return NewCLIErrorf(ExitInvalidArgs, "%s", message)
	default:
		return NewCLIErrorf(ExitUnexpected, "%s", message)
	}
}

func mapVBASourceProjectError(filePath string, info *vba.Info, err error) error {
	if err == nil {
		return nil
	}
	if strings.Contains(err.Error(), "no vbaProject.bin") {
		inspect := vbaInspectCommand(filePath)
		attach := vbaNextMutationTemplate(filePath, info)
		if attach != "" {
			return NewCLIErrorf(ExitTargetNotFound, "%s; inspect macro state with `%s`; attach a VBA project with `%s`", err.Error(), inspect, attach)
		}
		return NewCLIErrorf(ExitTargetNotFound, "%s; inspect macro state with `%s`", err.Error(), inspect)
	}
	return mapVBAError(err)
}

func normalizeVBAInspectBinFamily(value string) (string, error) {
	value = strings.ToLower(strings.TrimSpace(value))
	switch value {
	case "":
		return "", NewCLIErrorf(ExitInvalidArgs, "--family is required for inspect-bin (pptx or xlsx)")
	case "pptx", "pptm", "powerpoint", "presentation":
		return "pptx", nil
	case "xlsx", "xlsm", "excel", "workbook":
		return "xlsx", nil
	default:
		return "", NewCLIErrorf(ExitInvalidArgs, "--family must be pptx or xlsx")
	}
}

func summarizeVBASourceProject(project *vba.SourceProject) *vba.SourceProject {
	return vba.SummarizeSourceProject(project)
}

func preflightVBAModuleSelector(filePath, selector string) error {
	pkg, err := opc.Open(filePath)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to open package: %v", err)
	}
	defer pkg.Close()
	project, err := vba.InspectSourceProject(pkg)
	if err != nil {
		return mapVBAError(err)
	}
	_, err = selectVBAModules(filePath, project.Modules, selector)
	return err
}

func selectVBAModules(filePath string, modules []vba.SourceModule, selector string) ([]vba.SourceModule, error) {
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return modules, nil
	}
	var matches []vba.SourceModule
	for _, module := range modules {
		for _, candidate := range module.Selectors {
			if strings.EqualFold(candidate, selector) {
				matches = append(matches, module)
				break
			}
		}
	}
	switch len(matches) {
	case 0:
		candidates := BuildSelectorCandidates(vbaModuleSelectorCandidates(modules), selector, maxSelectorCandidates)
		return nil, SelectorNotFoundError("VBA module", selector, candidates, vbaListCommand(filePath))
	case 1:
		return matches, nil
	default:
		selectors := vbaAmbiguousModuleSelectors(matches)
		return nil, NewCLIErrorf(ExitInvalidArgs, "VBA module selector %q matched multiple modules (%s); use a more specific selector; discover with `%s`", selector, strings.Join(selectors, ", "), vbaListCommand(filePath))
	}
}

func vbaModuleSelectorCandidates(modules []vba.SourceModule) []SelectorCandidate {
	out := make([]SelectorCandidate, 0, len(modules))
	for _, module := range modules {
		out = append(out, SelectorCandidate{Primary: module.PrimarySelector, Selectors: module.Selectors})
	}
	return out
}

func vbaAmbiguousModuleSelectors(modules []vba.SourceModule) []string {
	counts := map[string]int{}
	for _, module := range modules {
		primary := strings.TrimSpace(module.PrimarySelector)
		if primary != "" {
			counts[strings.ToLower(primary)]++
		}
	}
	out := make([]string, 0, len(modules))
	for _, module := range modules {
		primary := strings.TrimSpace(module.PrimarySelector)
		if primary != "" && counts[strings.ToLower(primary)] == 1 {
			out = append(out, primary)
			continue
		}
		if module.Number > 0 {
			out = append(out, fmt.Sprintf("module:%d", module.Number))
			continue
		}
		if primary != "" {
			out = append(out, primary)
		}
	}
	return out
}

func writeVBAModules(outDir string, modules []vba.SourceModule) ([]VBAModuleExtractItem, error) {
	if len(modules) == 0 {
		return nil, TargetNotFoundError("no VBA modules to extract")
	}
	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to create module output directory: %v", err)
	}
	var results []VBAModuleExtractItem
	used := map[string]int{}
	for _, module := range modules {
		name := vba.ModuleOutputName(module)
		if used[name] > 0 {
			ext := filepath.Ext(name)
			base := strings.TrimSuffix(name, ext)
			name = fmt.Sprintf("%s-%d%s", base, used[name]+1, ext)
		}
		used[name]++
		outputPath := filepath.Join(outDir, name)
		data := []byte(module.Source)
		if err := os.WriteFile(outputPath, data, 0o644); err != nil {
			return nil, NewCLIErrorf(ExitUnexpected, "failed to write VBA module %s: %v", module.Name, err)
		}
		results = append(results, VBAModuleExtractItem{
			Number:          module.Number,
			Name:            module.Name,
			StreamName:      module.StreamName,
			Kind:            module.Kind,
			Extension:       module.Extension,
			OutputPath:      outputPath,
			BytesWritten:    len(data),
			LineCount:       module.LineCount,
			SHA256:          module.SHA256,
			SHA256Basis:     module.SHA256Basis,
			LineEnding:      module.LineEnding,
			TrailingNewline: module.TrailingNewline,
			PrimarySelector: module.PrimarySelector,
			Selectors:       append([]string{}, module.Selectors...),
			Warnings:        append([]string{}, module.Warnings...),
		})
	}
	return results, nil
}

func init() {
	vbaOfficeCheckCmd.Flags().StringVar(&vbaOfficeCheckOutDir, "out-dir", "", "optional directory to keep LibreOffice conversion output for inspection")
	vbaExtractBinCmd.Flags().String("out", "", "output vbaProject.bin path")
	vbaInspectBinCmd.Flags().StringVar(&vbaInspectBinFamily, "family", "", "target host family for compatibility checks: pptx or xlsx")
	vbaCreateCmd.Flags().StringVar(&vbaCreateFamily, "family", "", "target Office family: xlsx or pptx; inferred from .xlsm/.pptm output when omitted")
	vbaCreateCmd.Flags().StringArrayVar(&vbaCreateSources, "source", nil, "repeatable .bas or .cls source file to import (comma-separated lists are also accepted)")
	vbaCreateCmd.Flags().StringVar(&vbaCreateExtractBinPath, "extract-bin", "", "optional path to write the created vbaProject.bin seed")
	vbaCreateCmd.Flags().StringVar(&vbaCreateOfficeScriptPath, "office-create-script", "", "path to windows-office-vba-create.ps1 when not running from the repo checkout")
	vbaCreateCmd.Flags().BoolVar(&vbaCreateEnableVBOMAccess, "enable-vba-object-model-access", false, "temporarily enable Trust access to the VBA project object model while Office imports sources")
	vbaCreateCmd.Flags().BoolVar(&vbaCreateVisible, "visible", false, "show the Office application window during creation")
	vbaCreateCmd.Flags().BoolVar(&vbaCreateForce, "force", false, "overwrite existing output/extract-bin paths")
	vbaExtractCmd.Flags().String("out-dir", "", "directory for extracted .bas/.cls modules")
	vbaExtractCmd.Flags().StringVar(&vbaExtractModuleSelector, "module", "", "optional module selector from vba list")
	vbaAddModuleCmd.Flags().StringVar(&vbaAddModuleSourcePath, "source", "", ".bas or .cls source file to add")
	vbaAddModuleCmd.Flags().StringVar(&vbaAddModuleName, "name", "", "module name; defaults to Attribute VB_Name or source file base name")
	vbaAddModuleCmd.Flags().StringVar(&vbaAddModuleKind, "kind", "", "module kind: standard or class; defaults to source extension")
	vbaAddModuleCmd.Flags().IntVar(&vbaAddModuleExpectModuleCount, "expect-module-count", 0, "expected current module count from vba list")
	vbaReplaceModuleCmd.Flags().StringVar(&vbaReplaceModuleSelector, "module", "", "module selector from vba list")
	vbaReplaceModuleCmd.Flags().StringVar(&vbaReplaceModuleSourcePath, "source", "", "replacement .bas or .cls source file")
	vbaReplaceModuleCmd.Flags().StringVar(&vbaReplaceModuleExpectSHA256, "expect-sha256", "", "expected current module source SHA-256 from vba list")
	vbaRemoveModuleCmd.Flags().StringVar(&vbaRemoveModuleSelector, "module", "", "module selector from vba list")
	vbaRemoveModuleCmd.Flags().StringVar(&vbaRemoveModuleExpectSHA256, "expect-sha256", "", "expected current module source SHA-256 from vba list")
	for _, cmd := range []*cobra.Command{vbaAddModuleCmd, vbaReplaceModuleCmd, vbaRemoveModuleCmd} {
		cmd.Flags().BoolVar(&vbaAllowExperimentalRewrite, "allow-experimental-vba-source-rewrite", false, "allow guarded Office-shaped replacement and experimental synthetic/source-only rewrites; real Office-shaped add/remove remain refused")
	}
	vbaAttachCmd.Flags().String("bin", "", "vbaProject.bin to attach")
	vbaAttachCmd.Flags().BoolVar(&vbaAttachAllowHostFamilyRisk, "allow-host-family-risk", false, "allow attaching a parseable VBA seed whose modules look risky for the target Office family")
	AddMutationFlags(vbaAddModuleCmd)
	AddMutationFlags(vbaReplaceModuleCmd)
	AddMutationFlags(vbaRemoveModuleCmd)
	AddMutationFlags(vbaAttachCmd)
	AddMutationFlags(vbaRemoveCmd)

	vbaCmd.AddCommand(vbaInspectCmd)
	vbaCmd.AddCommand(vbaOfficeCheckCmd)
	vbaCmd.AddCommand(vbaExtractBinCmd)
	vbaCmd.AddCommand(vbaInspectBinCmd)
	vbaCmd.AddCommand(vbaCreateCmd)
	vbaCmd.AddCommand(vbaListCmd)
	vbaCmd.AddCommand(vbaExtractCmd)
	vbaCmd.AddCommand(vbaAddModuleCmd)
	vbaCmd.AddCommand(vbaReplaceModuleCmd)
	vbaCmd.AddCommand(vbaRemoveModuleCmd)
	vbaCmd.AddCommand(vbaAttachCmd)
	vbaCmd.AddCommand(vbaRemoveCmd)
	rootCmd.AddCommand(vbaCmd)
}
