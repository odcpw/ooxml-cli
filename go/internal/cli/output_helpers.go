package cli

import (
	"io"
	"os"

	"github.com/spf13/cobra"
)

func writeGlobalOutput(cmd *cobra.Command, data []byte) error {
	config := GetGlobalConfig(cmd)
	out, cleanup, err := globalOutputWriter(cmd, config)
	if err != nil {
		return err
	}
	if cleanup != nil {
		defer cleanup()
	}

	if len(data) == 0 || data[len(data)-1] != '\n' {
		data = append(data, '\n')
	}
	if _, err := out.Write(data); err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to write output: %v", err)
	}
	return nil
}

func writeGlobalJSON(cmd *cobra.Command, value any) error {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), value)
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal JSON: %v", err)
	}
	return writeGlobalOutput(cmd, data)
}

func marshalLabeledJSON(cmd *cobra.Command, value any, label string) ([]byte, error) {
	data, err := marshalWithConfig(GetGlobalConfig(cmd), value)
	if err != nil {
		return nil, NewCLIErrorf(ExitUnexpected, "failed to marshal %s JSON: %v", label, err)
	}
	return data, nil
}

func writeLabeledJSON(cmd *cobra.Command, value any, label string) error {
	data, err := marshalLabeledJSON(cmd, value, label)
	if err != nil {
		return err
	}
	return writeCLIOutput(cmd, data)
}

func globalOutputWriter(cmd *cobra.Command, config *GlobalConfig) (io.Writer, func() error, error) {
	if config == nil || config.Output == "" {
		return cmd.OutOrStdout(), nil, nil
	}

	file, err := os.Create(config.Output)
	if err != nil {
		return nil, nil, NewCLIErrorf(ExitUnexpected, "failed to create output file: %v", err)
	}
	return file, file.Close, nil
}
