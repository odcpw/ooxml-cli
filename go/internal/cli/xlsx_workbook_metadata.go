package cli

import (
	"fmt"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	xlsxinspect "github.com/ooxml-cli/ooxml-cli/pkg/xlsx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/mutate"
	"github.com/spf13/cobra"
)

// ---- result types ----

type xlsxWorkbookMetadataFields struct {
	Title          string `json:"title"`
	Subject        string `json:"subject"`
	Creator        string `json:"creator"`
	Keywords       string `json:"keywords"`
	Description    string `json:"description"`
	LastModifiedBy string `json:"lastModifiedBy"`
	Category       string `json:"category"`
	Company        string `json:"company"`
	Manager        string `json:"manager"`
}

type xlsxWorkbookCalcSettings struct {
	CalcMode       string  `json:"calcMode"`
	FullCalcOnLoad bool    `json:"fullCalcOnLoad"`
	ForceFullCalc  bool    `json:"forceFullCalc"`
	CalcID         string  `json:"calcId"`
	Iterate        bool    `json:"iterate"`
	IterateCount   int     `json:"iterateCount"`
	IterateDelta   float64 `json:"iterateDelta"`
}

type XLSXWorkbookMetadataInspectResult struct {
	File                    string                     `json:"file"`
	Action                  string                     `json:"action"`
	Metadata                xlsxWorkbookMetadataFields `json:"metadata"`
	CalcSettings            xlsxWorkbookCalcSettings   `json:"calcSettings"`
	InspectCommandTemplate  string                     `json:"inspectCommandTemplate,omitempty"`
	ValidateCommandTemplate string                     `json:"validateCommandTemplate,omitempty"`
}

type XLSXWorkbookMetadataUpdateResult struct {
	File            string                     `json:"file"`
	Output          string                     `json:"output,omitempty"`
	DryRun          bool                       `json:"dryRun"`
	Action          string                     `json:"action"`
	Metadata        xlsxWorkbookMetadataFields `json:"metadata"`
	CalcSettings    xlsxWorkbookCalcSettings   `json:"calcSettings"`
	Updated         int                        `json:"updated"`
	UpdatedFields   []string                   `json:"updatedFields"`
	PreviousValues  map[string]string          `json:"previousValues"`
	ValidateCommand string                     `json:"validateCommand,omitempty"`
	InspectCommand  string                     `json:"inspectCommand,omitempty"`
}

func metadataFieldsFrom(m *xlsxinspect.WorkbookMetadata) xlsxWorkbookMetadataFields {
	return xlsxWorkbookMetadataFields{
		Title:          m.Title,
		Subject:        m.Subject,
		Creator:        m.Creator,
		Keywords:       m.Keywords,
		Description:    m.Description,
		LastModifiedBy: m.LastModifiedBy,
		Category:       m.Category,
		Company:        m.Company,
		Manager:        m.Manager,
	}
}

func calcSettingsFrom(m *xlsxinspect.WorkbookMetadata) xlsxWorkbookCalcSettings {
	return xlsxWorkbookCalcSettings{
		CalcMode:       m.CalcMode,
		FullCalcOnLoad: m.FullCalcOnLoad,
		ForceFullCalc:  m.ForceFullCalc,
		CalcID:         m.CalcID,
		Iterate:        m.Iterate,
		IterateCount:   m.IterateCount,
		IterateDelta:   m.IterateDelta,
	}
}

// ---- flags ----

var (
	xlsxWorkbookMetaTitle          string
	xlsxWorkbookMetaSubject        string
	xlsxWorkbookMetaCreator        string
	xlsxWorkbookMetaKeywords       string
	xlsxWorkbookMetaDescription    string
	xlsxWorkbookMetaLastModifiedBy string
	xlsxWorkbookMetaCategory       string
	xlsxWorkbookMetaCompany        string
	xlsxWorkbookMetaManager        string
	xlsxWorkbookMetaCalcMode       string
	xlsxWorkbookMetaFullCalc       bool

	xlsxWorkbookMetaExpectTitle          string
	xlsxWorkbookMetaExpectSubject        string
	xlsxWorkbookMetaExpectCreator        string
	xlsxWorkbookMetaExpectKeywords       string
	xlsxWorkbookMetaExpectDescription    string
	xlsxWorkbookMetaExpectLastModifiedBy string
	xlsxWorkbookMetaExpectCategory       string
	xlsxWorkbookMetaExpectCompany        string
	xlsxWorkbookMetaExpectManager        string
)

// ---- inspect ----

var xlsxWorkbookMetadataInspectCmd = &cobra.Command{
	Use:   "inspect <file>",
	Short: "Inspect workbook core/app properties and calc settings",
	Long:  "Read all core and extended (app) workbook properties together with workbook calculation settings.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		pkg, err := openPackageExpectType(filePath, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		defer pkg.Close()
		meta, err := xlsxinspect.ReadWorkbookMetadata(pkg)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to read workbook metadata: %v", err)
		}
		result := &XLSXWorkbookMetadataInspectResult{
			File:                    filePath,
			Action:                  "inspect",
			Metadata:                metadataFieldsFrom(meta),
			CalcSettings:            calcSettingsFrom(meta),
			InspectCommandTemplate:  "ooxml --json xlsx workbook metadata inspect <placeholder>.xlsx",
			ValidateCommandTemplate: "ooxml validate <placeholder>.xlsx",
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "workbook metadata inspect")
		}
		return writeXLSXOutput(cmd, []byte(formatWorkbookMetadata(result.Metadata, result.CalcSettings)))
	},
}

// ---- update ----

var xlsxWorkbookMetadataUpdateCmd = &cobra.Command{
	Use:   "update <file>",
	Short: "Update workbook metadata fields and calc settings",
	Long:  "Update core/app document properties and workbook calculation settings. Only flags that are set are changed.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		updates, expect := buildMetadataUpdates(cmd)
		if len(updates.fieldFlags()) == 0 {
			return InvalidArgsError("no metadata fields specified; set at least one of --title/--subject/--creator/--keywords/--description/--last-modified-by/--category/--company/--manager/--calc-mode/--full-calc-on-load")
		}
		mutOpts, err := GetValidatedMutationOptions(cmd)
		if err != nil {
			return err
		}
		writer, err := NewMutationWriterForType(filePath, mutOpts, opc.PackageTypeXLSX)
		if err != nil {
			return err
		}
		var result *XLSXWorkbookMetadataUpdateResult
		if err := writer.Write(func(pkg opc.PackageSession) error {
			res, err := mutate.UpdateWorkbookMetadata(&mutate.UpdateWorkbookMetadataRequest{
				Package:      pkg,
				Updates:      updates.toMutateUpdate(),
				ExpectValues: expect,
			})
			if err != nil {
				return NewCLIErrorf(ExitInvalidArgs, "failed to update workbook metadata: %v", err)
			}
			meta, err := xlsxinspect.ReadWorkbookMetadata(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to read updated metadata: %v", err)
			}
			destinationFile := mutationOutputPathForResult(filePath, mutOpts)
			result = &XLSXWorkbookMetadataUpdateResult{
				File:           filePath,
				Output:         destinationFile,
				DryRun:         mutOpts != nil && mutOpts.DryRun,
				Action:         "update",
				Metadata:       metadataFieldsFrom(meta),
				CalcSettings:   calcSettingsFrom(meta),
				Updated:        res.UpdatedCount,
				UpdatedFields:  res.UpdatedFields,
				PreviousValues: res.PreviousValues,
			}
			if destinationFile != "" {
				result.ValidateCommand = xlsxValidateCommand(destinationFile)
				result.InspectCommand = fmt.Sprintf("ooxml --json xlsx workbook metadata inspect %s", pptxXLSXCommandArg(destinationFile))
			}
			return nil
		}); err != nil {
			return err
		}
		if GetGlobalConfig(cmd).Format == "json" {
			return writeJSONResult(cmd, result, "workbook metadata update")
		}
		return writeXLSXOutput(cmd, []byte(fmt.Sprintf("updated %d field(s): %v", result.Updated, result.UpdatedFields)))
	},
}

// metadataUpdateInputs collects the per-flag pointers prepared from the command.
type metadataUpdateInputs struct {
	title          *string
	subject        *string
	creator        *string
	keywords       *string
	description    *string
	lastModifiedBy *string
	category       *string
	company        *string
	manager        *string
	calcMode       *string
	fullCalcOnLoad *bool
}

func (m metadataUpdateInputs) fieldFlags() []string {
	var out []string
	if m.title != nil {
		out = append(out, "title")
	}
	if m.subject != nil {
		out = append(out, "subject")
	}
	if m.creator != nil {
		out = append(out, "creator")
	}
	if m.keywords != nil {
		out = append(out, "keywords")
	}
	if m.description != nil {
		out = append(out, "description")
	}
	if m.lastModifiedBy != nil {
		out = append(out, "lastModifiedBy")
	}
	if m.category != nil {
		out = append(out, "category")
	}
	if m.company != nil {
		out = append(out, "company")
	}
	if m.manager != nil {
		out = append(out, "manager")
	}
	if m.calcMode != nil {
		out = append(out, "calcMode")
	}
	if m.fullCalcOnLoad != nil {
		out = append(out, "fullCalcOnLoad")
	}
	return out
}

func (m metadataUpdateInputs) toMutateUpdate() mutate.WorkbookMetadataUpdate {
	return mutate.WorkbookMetadataUpdate{
		Title:          m.title,
		Subject:        m.subject,
		Creator:        m.creator,
		Keywords:       m.keywords,
		Description:    m.description,
		LastModifiedBy: m.lastModifiedBy,
		Category:       m.category,
		Company:        m.company,
		Manager:        m.manager,
		CalcMode:       m.calcMode,
		FullCalcOnLoad: m.fullCalcOnLoad,
	}
}

func buildMetadataUpdates(cmd *cobra.Command) (metadataUpdateInputs, map[string]string) {
	flags := cmd.Flags()
	in := metadataUpdateInputs{}
	strPtr := func(name string, dst *string) *string {
		if flags.Changed(name) {
			v := *dst
			return &v
		}
		return nil
	}
	in.title = strPtr("title", &xlsxWorkbookMetaTitle)
	in.subject = strPtr("subject", &xlsxWorkbookMetaSubject)
	in.creator = strPtr("creator", &xlsxWorkbookMetaCreator)
	in.keywords = strPtr("keywords", &xlsxWorkbookMetaKeywords)
	in.description = strPtr("description", &xlsxWorkbookMetaDescription)
	in.lastModifiedBy = strPtr("last-modified-by", &xlsxWorkbookMetaLastModifiedBy)
	in.category = strPtr("category", &xlsxWorkbookMetaCategory)
	in.company = strPtr("company", &xlsxWorkbookMetaCompany)
	in.manager = strPtr("manager", &xlsxWorkbookMetaManager)
	in.calcMode = strPtr("calc-mode", &xlsxWorkbookMetaCalcMode)
	if flags.Changed("full-calc-on-load") {
		v := xlsxWorkbookMetaFullCalc
		in.fullCalcOnLoad = &v
	}

	expect := map[string]string{}
	addExpect := func(name, field string, dst *string) {
		if flags.Changed(name) {
			expect[field] = *dst
		}
	}
	addExpect("expect-title", "title", &xlsxWorkbookMetaExpectTitle)
	addExpect("expect-subject", "subject", &xlsxWorkbookMetaExpectSubject)
	addExpect("expect-creator", "creator", &xlsxWorkbookMetaExpectCreator)
	addExpect("expect-keywords", "keywords", &xlsxWorkbookMetaExpectKeywords)
	addExpect("expect-description", "description", &xlsxWorkbookMetaExpectDescription)
	addExpect("expect-last-modified-by", "lastModifiedBy", &xlsxWorkbookMetaExpectLastModifiedBy)
	addExpect("expect-category", "category", &xlsxWorkbookMetaExpectCategory)
	addExpect("expect-company", "company", &xlsxWorkbookMetaExpectCompany)
	addExpect("expect-manager", "manager", &xlsxWorkbookMetaExpectManager)
	if len(expect) == 0 {
		expect = nil
	}
	return in, expect
}

func formatWorkbookMetadata(m xlsxWorkbookMetadataFields, c xlsxWorkbookCalcSettings) string {
	return fmt.Sprintf(`Metadata:
  title:          %s
  subject:        %s
  creator:        %s
  keywords:       %s
  description:    %s
  lastModifiedBy: %s
  category:       %s
  company:        %s
  manager:        %s
Calc settings:
  calcMode:       %s
  fullCalcOnLoad: %t
  iterate:        %t
  iterateCount:   %d
  calcId:         %s`,
		m.Title, m.Subject, m.Creator, m.Keywords, m.Description, m.LastModifiedBy, m.Category, m.Company, m.Manager,
		c.CalcMode, c.FullCalcOnLoad, c.Iterate, c.IterateCount, c.CalcID)
}

func init() {
	xlsxWorkbookMetadataCmd.AddCommand(xlsxWorkbookMetadataInspectCmd)

	uf := xlsxWorkbookMetadataUpdateCmd.Flags()
	uf.StringVar(&xlsxWorkbookMetaTitle, "title", "", "set core property dc:title")
	uf.StringVar(&xlsxWorkbookMetaSubject, "subject", "", "set core property dc:subject")
	uf.StringVar(&xlsxWorkbookMetaCreator, "creator", "", "set core property dc:creator")
	uf.StringVar(&xlsxWorkbookMetaKeywords, "keywords", "", "set core property keywords")
	uf.StringVar(&xlsxWorkbookMetaDescription, "description", "", "set core property dc:description")
	uf.StringVar(&xlsxWorkbookMetaLastModifiedBy, "last-modified-by", "", "set core property lastModifiedBy")
	uf.StringVar(&xlsxWorkbookMetaCategory, "category", "", "set core property category")
	uf.StringVar(&xlsxWorkbookMetaCompany, "company", "", "set app property Company")
	uf.StringVar(&xlsxWorkbookMetaManager, "manager", "", "set app property Manager")
	uf.StringVar(&xlsxWorkbookMetaCalcMode, "calc-mode", "", "set workbook calcMode (auto|manual|autoNoTable)")
	uf.BoolVar(&xlsxWorkbookMetaFullCalc, "full-calc-on-load", false, "set fullCalcOnLoad (and forceFullCalc) so Excel recalculates on open")

	uf.StringVar(&xlsxWorkbookMetaExpectTitle, "expect-title", "", "guard: require current title to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectSubject, "expect-subject", "", "guard: require current subject to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectCreator, "expect-creator", "", "guard: require current creator to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectKeywords, "expect-keywords", "", "guard: require current keywords to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectDescription, "expect-description", "", "guard: require current description to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectLastModifiedBy, "expect-last-modified-by", "", "guard: require current lastModifiedBy to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectCategory, "expect-category", "", "guard: require current category to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectCompany, "expect-company", "", "guard: require current company to equal this value")
	uf.StringVar(&xlsxWorkbookMetaExpectManager, "expect-manager", "", "guard: require current manager to equal this value")

	AddMutationFlags(xlsxWorkbookMetadataUpdateCmd)
	xlsxWorkbookMetadataCmd.AddCommand(xlsxWorkbookMetadataUpdateCmd)
}
