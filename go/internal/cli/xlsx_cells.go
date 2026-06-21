package cli

import "github.com/spf13/cobra"

var xlsxCellsCmd = &cobra.Command{
	Use:     "cells",
	Aliases: []string{"cell"},
	Short:   "Read and mutate worksheet cells",
	Long:    "Commands for reading and mutating worksheet cell values.",
	Args:    cobra.NoArgs,
	RunE:    showHelp,
}

func init() {
	xlsxCmd.AddCommand(xlsxCellsCmd)
}
