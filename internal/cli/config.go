package cli

// GlobalConfig holds all global flag values that are shared across commands
type GlobalConfig struct {
	// Format specifies the output format: "text" or "json"
	Format string

	// Verbosity specifies the logging level: "quiet", "normal", "detailed", or "debug"
	Verbosity string

	// NoColor disables colored output
	NoColor bool

	// Pretty enables pretty-printed output (for JSON and other formats)
	Pretty bool

	// Output specifies the output file path (empty means stdout)
	Output string

	// TempDir specifies the temporary directory for scratch space
	TempDir string

	// KeepTemp preserves temporary files after command completion
	KeepTemp bool

	// Strict enables strict validation mode
	Strict bool

	// Out specifies the output file path for mutating commands (--out flag)
	Out string

	// InPlace specifies in-place mutation (--in-place flag)
	InPlace string

	// Backup specifies the backup suffix for in-place mutations (--backup flag)
	Backup string
}
