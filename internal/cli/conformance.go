package cli

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/conformance"
	"github.com/spf13/cobra"
)

var conformanceOfficeCheck bool
var conformanceOfficeCheckOutDir string
var conformanceCheckPackage = conformance.CheckPackage

var conformanceCmd = &cobra.Command{
	Use:   "conformance",
	Short: "Run OOXML repair-focused conformance checks",
	Long:  "Run repair-focused OOXML checks that combine repo validation, Office-sensitive XML invariants, and optional LibreOffice/soffice open evidence.",
}

var conformanceCheckCmd = &cobra.Command{
	Use:           "check <file>",
	Short:         "Check a PPTX/XLSX package for Office repair risks",
	SilenceUsage:  true,
	SilenceErrors: true,
	Args:          cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		filePath := args[0]
		if _, err := os.Stat(filePath); err != nil {
			return FileNotFoundError(filePath)
		}
		report, err := conformanceCheckPackage(filePath, conformance.Options{RunOfficeCheck: conformanceOfficeCheck, OfficeCheckOutDir: conformanceOfficeCheckOutDir})
		if err != nil && report == nil {
			return NewCLIErrorf(ExitUnexpected, "conformance check failed: %v", err)
		}
		config := GetGlobalConfig(cmd)
		if config.Format == "json" {
			if err := outputConformanceJSON(cmd, report); err != nil {
				return err
			}
		} else if err := outputConformanceText(cmd, report); err != nil {
			return err
		}
		if report.HasFailures() {
			e := NewCLIError(ExitValidationFailed, "")
			e.Reported = true
			return e
		}
		return nil
	},
}

var conformanceCoverageCmd = &cobra.Command{
	Use:           "coverage",
	Short:         "Show Office-repair conformance harness coverage",
	SilenceUsage:  true,
	SilenceErrors: true,
	Args:          cobra.NoArgs,
	RunE: func(cmd *cobra.Command, args []string) error {
		report := conformance.RepairCoverageReport()
		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, report)
		}
		return outputConformanceCoverageText(cmd, report)
	},
}

func outputConformanceJSON(cmd *cobra.Command, report *conformance.Report) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(report, "", "  ")
	} else {
		data, err = json.Marshal(report)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal conformance report: %v", err)
	}
	_, err = fmt.Fprintf(cmd.OutOrStdout(), "%s\n", data)
	return err
}

func outputConformanceText(cmd *cobra.Command, report *conformance.Report) error {
	out := cmd.OutOrStdout()
	fmt.Fprintf(out, "File: %s\n", report.File)
	fmt.Fprintf(out, "Family: %s\n", report.Family)
	fmt.Fprintf(out, "Status: %s\n", report.Status)
	fmt.Fprintln(out)
	fmt.Fprintln(out, "Checks:")
	for _, check := range report.Checks {
		fmt.Fprintf(out, "  [%s] %s\n", check.Status, check.Name)
		for _, d := range check.Diagnostics {
			fmt.Fprintf(out, "    [%s] %s: %s\n", d.Severity, d.Code, d.Message)
		}
		if check.OfficeCheck != nil && check.OfficeCheck.Error != "" {
			fmt.Fprintf(out, "    office-check: %s\n", check.OfficeCheck.Error)
		}
		if check.OfficeCheck != nil && check.OfficeCheck.OutputPath != "" {
			fmt.Fprintf(out, "    office-check-output: %s\n", check.OfficeCheck.OutputPath)
		}
	}
	return nil
}

func outputConformanceCoverageText(cmd *cobra.Command, report conformance.CoverageReport) error {
	out := cmd.OutOrStdout()
	fmt.Fprintf(out, "Scope: %s\n", report.Scope)
	fmt.Fprintf(out, "Status: %s\n", report.Status)
	fmt.Fprintln(out)
	fmt.Fprintln(out, "Harness stages:")
	for _, stage := range report.HarnessStages {
		fmt.Fprintf(out, "  [%s] %s\n", stage.Status, stage.Name)
	}
	fmt.Fprintln(out)
	fmt.Fprintln(out, "Repair classes:")
	for _, class := range report.RepairClasses {
		fmt.Fprintf(out, "  [%s] %s (%s)\n", class.Status, class.ID, strings.Join(class.Families, ","))
		if len(class.DiagnosticCodes) > 0 {
			fmt.Fprintf(out, "    diagnostics: %s\n", strings.Join(class.DiagnosticCodes, ", "))
		}
		if len(class.Evidence) > 0 {
			fmt.Fprintf(out, "    evidence: %s\n", strings.Join(class.Evidence, ", "))
		}
	}
	if len(report.FixtureSets) > 0 {
		fmt.Fprintln(out)
		fmt.Fprintln(out, "Fixture sets:")
		for _, fixtureSet := range report.FixtureSets {
			fmt.Fprintf(out, "  [%s] %s", fixtureSet.Status, fixtureSet.Name)
			if len(fixtureSet.Families) > 0 {
				fmt.Fprintf(out, " (%s)", strings.Join(fixtureSet.Families, ","))
			}
			fmt.Fprintln(out)
			if len(fixtureSet.Evidence) > 0 {
				fmt.Fprintf(out, "    evidence: %s\n", strings.Join(fixtureSet.Evidence, ", "))
			}
		}
	}
	if len(report.KnownLimitations) > 0 {
		fmt.Fprintln(out)
		fmt.Fprintln(out, "Known limitations:")
		for _, limitation := range report.KnownLimitations {
			fmt.Fprintf(out, "  - %s\n", limitation)
		}
	}
	return nil
}

func init() {
	conformanceCheckCmd.Flags().BoolVar(&conformanceOfficeCheck, "office-check", false, "also run local LibreOffice/soffice open-check evidence when available")
	conformanceCheckCmd.Flags().StringVar(&conformanceOfficeCheckOutDir, "office-check-out-dir", "", "optional directory to keep LibreOffice/soffice conversion output for inspection")
	conformanceCmd.AddCommand(conformanceCheckCmd, conformanceCoverageCmd)
	GetRootCmd().AddCommand(conformanceCmd)
}
