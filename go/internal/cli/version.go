package cli

import "github.com/spf13/cobra"

// Version is the current version of ooxml.
var Version = "0.0.1"

type versionOutput struct {
	Tool    string `json:"tool"`
	Version string `json:"version"`
}

// versionCmd represents the version command
var versionCmd = &cobra.Command{
	Use:   "version",
	Short: "Print the version of ooxml",
	RunE: func(cmd *cobra.Command, args []string) error {
		if GetGlobalConfig(cmd).Format == "json" {
			return writeGlobalJSON(cmd, versionOutput{
				Tool:    "ooxml",
				Version: Version,
			})
		}
		return writeGlobalOutput(cmd, []byte(Version))
	},
}

func init() {
	rootCmd.AddCommand(versionCmd)
}
