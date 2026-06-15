package cli

import (
	"context"
	"os"
	"sync"

	"github.com/spf13/cobra"
)

var globalConfig *GlobalConfig

// exampleMetadataOnce ensures cobra Example help text is derived from the shared
// capabilities metadata exactly once, after the full command tree is assembled.
// init-func ordering across files cannot be relied on to have every subcommand
// attached, so this runs lazily at the first Execute/GetRootCmd instead.
var exampleMetadataOnce sync.Once

func ensureExampleMetadata() {
	exampleMetadataOnce.Do(func() {
		applyExampleMetadata(rootCmd)
		installAgentErrorHints(rootCmd)
	})
}

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "ooxml",
	Short: "Inspect, validate, render, and mutate OOXML packages",
	Long: `ooxml is a CLI tool for inspecting and manipulating OOXML packages.

It can reverse-engineer package structure, validate content, extract resources,
apply mutations, render to other formats, and compute diffs.`,
	Args: cobra.NoArgs,
	PersistentPreRunE: func(cmd *cobra.Command, args []string) error {
		// Load global flags into config
		var err error
		globalConfig, err = loadGlobalFlags(cmd)
		if err != nil {
			return err
		}

		// Store config in context for subcommands to access
		cmd.SetContext(context.WithValue(cmd.Context(), "config", globalConfig))

		return nil
	},
	RunE: showHelp,
}

func showHelp(cmd *cobra.Command, _ []string) error {
	return cmd.Help()
}

// Execute adds all child commands to the root command and sets flags appropriately.
func Execute() {
	rootCmd.SilenceUsage = true
	rootCmd.SilenceErrors = true
	ensureExampleMetadata()

	if err := rootCmd.Execute(); err != nil {
		os.Exit(renderError(err, errorFormat(), errorPretty(), os.Stderr))
	}
}

func errorFormat() string {
	format := flagFormat
	if globalConfig != nil {
		format = globalConfig.Format
	}
	if format == "json" || flagJSON || argsRequestJSON(os.Args[1:]) {
		return "json"
	}
	return "text"
}

func errorPretty() bool {
	if globalConfig != nil {
		return globalConfig.Pretty
	}
	return flagPretty
}

// init sets up the root command's persistent flags
func init() {
	rootCmd.PersistentFlags().StringVarP(
		&flagFormat,
		"format",
		"f",
		"text",
		`output format: "text" or "json"`,
	)

	rootCmd.PersistentFlags().BoolVar(
		&flagJSON,
		"json",
		false,
		"emit JSON output (same as --format json)",
	)

	rootCmd.PersistentFlags().StringVarP(
		&flagVerbosity,
		"verbosity",
		"v",
		"normal",
		`logging level: "quiet", "normal", "detailed", or "debug"`,
	)

	rootCmd.PersistentFlags().BoolVar(
		&flagNoColor,
		"no-color",
		false,
		"disable colored output",
	)

	rootCmd.PersistentFlags().BoolVar(
		&flagPretty,
		"pretty",
		false,
		"pretty-print output (JSON and other formats)",
	)

	rootCmd.PersistentFlags().StringVarP(
		&flagOutput,
		"output",
		"o",
		"",
		"output file path (empty means stdout)",
	)

	rootCmd.PersistentFlags().StringVar(
		&flagTempDir,
		"temp-dir",
		"",
		"temporary directory for scratch space",
	)

	rootCmd.PersistentFlags().BoolVar(
		&flagKeepTemp,
		"keep-temp",
		false,
		"preserve temporary files after command completion",
	)

	rootCmd.PersistentFlags().BoolVar(
		&flagStrict,
		"strict",
		false,
		"enable strict validation mode",
	)

	rootCmd.PersistentFlags().StringVar(
		&flagOut,
		"out",
		"",
		"output file path (for mutating commands; mutually exclusive with --in-place)",
	)

	rootCmd.PersistentFlags().StringVar(
		&flagInPlace,
		"in-place",
		"",
		"modify the input file in place (for mutating commands; mutually exclusive with --out)",
	)

	rootCmd.PersistentFlags().StringVar(
		&flagBackup,
		"backup",
		"",
		"backup suffix for in-place mutations (e.g., '.bak'); only used with --in-place",
	)
	_ = rootCmd.PersistentFlags().MarkHidden("out")
	_ = rootCmd.PersistentFlags().MarkHidden("in-place")
	_ = rootCmd.PersistentFlags().MarkHidden("backup")
}

// Flag variables for Cobra to bind to
var (
	flagFormat    string
	flagJSON      bool
	flagVerbosity string
	flagNoColor   bool
	flagPretty    bool
	flagOutput    string
	flagTempDir   string
	flagKeepTemp  bool
	flagStrict    bool
	flagOut       string
	flagInPlace   string
	flagBackup    string
)

// loadGlobalFlags loads all global flags into a GlobalConfig
func loadGlobalFlags(cmd *cobra.Command) (*GlobalConfig, error) {
	format := flagFormat
	if flagJSON {
		format = "json"
	}

	config := &GlobalConfig{
		Format:    format,
		Verbosity: flagVerbosity,
		NoColor:   flagNoColor,
		Pretty:    flagPretty,
		Output:    flagOutput,
		TempDir:   flagTempDir,
		KeepTemp:  flagKeepTemp,
		Strict:    flagStrict,
		Out:       flagOut,
		InPlace:   flagInPlace,
		Backup:    flagBackup,
	}

	// Validate format
	if config.Format != "text" && config.Format != "json" {
		return nil, NewCLIErrorf(ExitInvalidArgs, "invalid format: %s (expected 'text' or 'json')", config.Format)
	}

	// Validate verbosity
	validVerbosities := map[string]bool{
		"quiet":    true,
		"normal":   true,
		"detailed": true,
		"debug":    true,
	}
	if !validVerbosities[config.Verbosity] {
		return nil, NewCLIErrorf(ExitInvalidArgs, "invalid verbosity: %s (expected 'quiet', 'normal', 'detailed', or 'debug')", config.Verbosity)
	}

	return config, nil
}

// loadGlobalFlagsFromCommand loads global flags from the command's flag set
// This is used when a subcommand is executed directly without the root command's PersistentPreRunE
func loadGlobalFlagsFromCommand(cmd *cobra.Command) *GlobalConfig {
	config := &GlobalConfig{
		Format:    "text",
		Verbosity: "normal",
		NoColor:   false,
		Pretty:    false,
		Output:    "",
		TempDir:   "",
		KeepTemp:  false,
		Strict:    false,
		Out:       "",
		InPlace:   "",
		Backup:    "",
	}

	// Helper to read a string flag, walking up parent commands if needed
	readStringFlag := func(flagName string, defaultValue string) string {
		current := cmd
		for current != nil {
			if flag := current.Flags().Lookup(flagName); flag != nil {
				value := flag.Value.String()
				if value != "" {
					return value
				}
			}
			current = current.Parent()
		}
		return defaultValue
	}

	// Helper to read a boolean flag, walking up parent commands if needed
	readBoolFlag := func(flagName string) bool {
		current := cmd
		for current != nil {
			if flag := current.Flags().Lookup(flagName); flag != nil {
				return flag.Value.String() == "true"
			}
			current = current.Parent()
		}
		return false
	}

	// Read all global flags from the command tree
	config.Format = readStringFlag("format", flagFormat)
	if readBoolFlag("json") || flagJSON {
		config.Format = "json"
	}
	config.Verbosity = readStringFlag("verbosity", flagVerbosity)
	config.NoColor = readBoolFlag("no-color") || flagNoColor
	config.Pretty = readBoolFlag("pretty") || flagPretty
	config.Output = readStringFlag("output", flagOutput)
	config.TempDir = readStringFlag("temp-dir", flagTempDir)
	config.KeepTemp = readBoolFlag("keep-temp") || flagKeepTemp
	config.Strict = readBoolFlag("strict") || flagStrict
	config.Out = readStringFlag("out", flagOut)
	config.InPlace = readStringFlag("in-place", flagInPlace)
	config.Backup = readStringFlag("backup", flagBackup)

	return config
}

func argsRequestJSON(args []string) bool {
	for i, arg := range args {
		switch {
		case arg == "--json":
			return true
		case arg == "--format=json":
			return true
		case arg == "-f=json":
			return true
		case arg == "--format" || arg == "-f":
			if i+1 < len(args) && args[i+1] == "json" {
				return true
			}
		}
	}
	return false
}

// GetGlobalConfig returns the current global config from the command context
// or by reading flags from the command (for direct subcommand execution)
func GetGlobalConfig(cmd *cobra.Command) *GlobalConfig {
	if cfg, ok := cmd.Context().Value("config").(*GlobalConfig); ok {
		return cfg
	}

	// Fallback: Load config from command flags (for direct subcommand execution)
	return loadGlobalFlagsFromCommand(cmd)
}

// GetRootCmd returns the root command (used by tests and main)
func GetRootCmd() *cobra.Command {
	return rootCmd
}
