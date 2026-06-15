package output

import (
	"context"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"
)

// Format represents the desired output format.
type Format string

const (
	FormatText Format = "text"
	FormatJSON Format = "json"
)

// Writer provides a unified interface for outputting data in the requested format.
type Writer interface {
	// WriteTable writes tabular data as aligned columns (text) or JSON array.
	// rows should be a slice of structs or a slice of maps[string]interface{}.
	WriteTable(rows interface{}) error

	// WriteObject writes a single object as pretty-printed JSON or formatted text.
	WriteObject(obj interface{}) error

	// WriteRaw writes raw bytes directly to the output.
	WriteRaw(data []byte) error

	// WriteString writes a string to the output.
	WriteString(s string) error

	// Close closes the writer and flushes any buffered output.
	Close() error
}

// Config holds configuration for creating a Writer.
type Config struct {
	Format     Format
	Pretty     bool
	OutputPath string
	Stdout     io.Writer // for testing; defaults to os.Stdout
}

// NewWriter creates a new Writer based on the provided configuration.
func NewWriter(cfg Config) (Writer, error) {
	// Validate format
	if cfg.Format == "" {
		cfg.Format = FormatText
	}

	// Ensure we have an output destination
	if cfg.Stdout == nil {
		cfg.Stdout = os.Stdout
	}

	var w io.Writer = cfg.Stdout
	var closer io.Closer
	var createdFile *os.File
	defer func() {
		if createdFile != nil {
			_ = createdFile.Close()
		}
	}()

	// If output path is specified, create/open the file
	if cfg.OutputPath != "" {
		// Ensure directory exists
		dir := filepath.Dir(cfg.OutputPath)
		if dir != "." && dir != "" {
			if err := os.MkdirAll(dir, 0755); err != nil {
				return nil, fmt.Errorf("failed to create output directory: %w", err)
			}
		}

		// Use regular file write. pkg/core/fsx.AtomicWriter can be used when task-2 is complete.
		file, err := os.Create(cfg.OutputPath)
		if err != nil {
			return nil, fmt.Errorf("failed to open output file: %w", err)
		}
		w = file
		closer = file
		createdFile = file
	}

	// Create appropriate formatter
	var formatter Formatter
	switch cfg.Format {
	case FormatJSON:
		formatter = newJSONFormatter(w, cfg.Pretty)
	case FormatText:
		formatter = newTextFormatter(w)
	default:
		return nil, fmt.Errorf("unsupported format: %s", cfg.Format)
	}

	createdFile = nil
	return &writerImpl{
		formatter: formatter,
		closer:    closer,
	}, nil
}

// writerImpl implements the Writer interface.
type writerImpl struct {
	formatter Formatter
	closer    io.Closer
}

func (w *writerImpl) WriteTable(rows interface{}) error {
	return w.formatter.FormatTable(rows)
}

func (w *writerImpl) WriteObject(obj interface{}) error {
	return w.formatter.FormatObject(obj)
}

func (w *writerImpl) WriteRaw(data []byte) error {
	return w.formatter.FormatRaw(data)
}

func (w *writerImpl) WriteString(s string) error {
	return w.formatter.FormatString(s)
}

func (w *writerImpl) Close() error {
	if w.closer != nil {
		return w.closer.Close()
	}
	return nil
}

// Formatter is the internal interface for different output formats.
type Formatter interface {
	FormatTable(rows interface{}) error
	FormatObject(obj interface{}) error
	FormatRaw(data []byte) error
	FormatString(s string) error
}

// GetWriterFromContext extracts the output Writer from a command's context.
// The root command should have stored GlobalConfig in the context via:
//
//	ctx = context.WithValue(ctx, "config", &GlobalConfig{...})
//	cmd.ExecutionContext = ctx
//
// Commands should call this to obtain a Writer configured with global flags.
func GetWriterFromContext(ctx context.Context) (Writer, error) {
	cfg, ok := ctx.Value("config").(*GlobalConfig)
	if !ok {
		// Fallback to defaults if GlobalConfig not available
		return NewWriter(Config{
			Format: FormatText,
			Pretty: false,
		})
	}

	return NewWriter(Config{
		Format:     Format(cfg.Format),
		Pretty:     cfg.Pretty,
		OutputPath: cfg.OutputPath,
	})
}

// GetWriterFromCommand is a convenience wrapper for commands that have access to *cobra.Command.
// It extracts the context and calls GetWriterFromContext.
func GetWriterFromCommand(cmd *cobra.Command) (Writer, error) {
	return GetWriterFromContext(cmd.Context())
}

// GlobalConfig represents the global CLI configuration.
// This is defined here as a reference type; task-3 (IronEagle) will define the actual version in internal/cli.
// Commands should read config from context rather than directly from flags.
type GlobalConfig struct {
	Format     string
	Pretty     bool
	OutputPath string
	// Add other global flags as task-3 defines them
}

// Formatter implementations are in text.go and json.go
