package cli

import (
	"bytes"
	"testing"
)

func resetFlags() {
	flagFormat = "text"
	flagJSON = false
	flagVerbosity = "normal"
	flagNoColor = false
	flagPretty = false
	flagOutput = ""
	flagTempDir = ""
	flagKeepTemp = false
	flagStrict = false
	flagOut = ""
	flagInPlace = ""
	flagBackup = ""
	conformanceOfficeCheck = false
	conformanceOfficeCheckOutDir = ""
}

func TestRootCommandHelp(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--help"})

	var output bytes.Buffer
	cmd.SetOut(&output)

	err := cmd.Execute()
	// --help doesn't return an error, it just exits 0
	if err != nil && err.Error() != "help requested" {
		t.Errorf("expected no error or help requested, got %v", err)
	}

	outStr := output.String()
	if outStr == "" {
		t.Errorf("expected help output, got empty string")
	}

	// Check that all flags are documented
	expectedFlags := []string{
		"--format",
		"--json",
		"--verbosity",
		"--no-color",
		"--pretty",
		"--output",
		"--temp-dir",
		"--keep-temp",
		"--strict",
	}

	for _, flag := range expectedFlags {
		if !bytes.Contains([]byte(outStr), []byte(flag)) {
			t.Errorf("expected flag %s in help output", flag)
		}
	}
}

func TestInvalidFormat(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--format", "invalid", "version"})

	var errBuf bytes.Buffer
	cmd.SetErr(&errBuf)

	err := cmd.Execute()
	if err == nil {
		t.Errorf("expected error for invalid format")
	}

	if cliErr, ok := err.(*CLIError); ok {
		if cliErr.ExitCode != ExitInvalidArgs {
			t.Errorf("expected exit code %d, got %d", ExitInvalidArgs, cliErr.ExitCode)
		}
	} else {
		t.Errorf("expected CLIError, got %T", err)
	}
}

func TestInvalidVerbosity(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--verbosity", "invalid", "version"})

	var errBuf bytes.Buffer
	cmd.SetErr(&errBuf)

	err := cmd.Execute()
	if err == nil {
		t.Errorf("expected error for invalid verbosity")
	}

	if cliErr, ok := err.(*CLIError); ok {
		if cliErr.ExitCode != ExitInvalidArgs {
			t.Errorf("expected exit code %d, got %d", ExitInvalidArgs, cliErr.ExitCode)
		}
	} else {
		t.Errorf("expected CLIError, got %T", err)
	}
}

func TestValidVerbosities(t *testing.T) {
	verbosities := []string{"quiet", "normal", "detailed", "debug"}

	for _, verbosity := range verbosities {
		t.Run(verbosity, func(t *testing.T) {
			resetFlags()
			cmd := GetRootCmd()
			cmd.SetArgs([]string{"--verbosity", verbosity, "version"})

			var output bytes.Buffer
			cmd.SetOut(&output)

			err := cmd.Execute()
			if err != nil {
				t.Errorf("expected no error for verbosity %s, got %v", verbosity, err)
			}
		})
	}
}

func TestValidFormats(t *testing.T) {
	formats := []string{"text", "json"}

	for _, format := range formats {
		t.Run(format, func(t *testing.T) {
			resetFlags()
			cmd := GetRootCmd()
			cmd.SetArgs([]string{"--format", format, "version"})

			var output bytes.Buffer
			cmd.SetOut(&output)

			err := cmd.Execute()
			if err != nil {
				t.Errorf("expected no error for format %s, got %v", format, err)
			}
		})
	}
}

func TestFlagsLoaded(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"--format", "json", "--verbosity", "debug", "--no-color", "--pretty", "--strict", "version"})

	var output bytes.Buffer
	cmd.SetOut(&output)

	err := cmd.Execute()
	if err != nil {
		t.Errorf("expected no error, got %v", err)
	}

	// Check that config was loaded (we can't directly test this, but if we got here without errors, flags were valid)
}

// TestValidateSubcommandRegistration tests that validate command is registered at root level
func TestValidateSubcommandRegistration(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()

	// Check that validate command is in the list of available commands
	validateFound := false
	for _, c := range cmd.Commands() {
		if c.Name() == "validate" {
			validateFound = true
			break
		}
	}

	if !validateFound {
		t.Error("validate command is not registered as a subcommand of root")
	}
}

// TestValidateFromRoot tests the validate command can be invoked from the root command
func TestValidateFromRoot(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", "../../testdata/pptx/minimal-title/presentation.pptx"})

	var output bytes.Buffer
	cmd.SetOut(&output)

	err := cmd.Execute()
	// For a valid file, the command should succeed
	if err != nil && err.Error() != "" {
		// CLIError with exit code 0 is acceptable for success
		if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitSuccess {
			t.Errorf("expected success or CLIError with exit code 0, got %v", err)
		}
	}
}

// TestValidateFromRootWithJSON tests the validate command with JSON output from root
func TestValidateFromRootWithJSON(t *testing.T) {
	resetFlags()
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"validate", "../../testdata/pptx/minimal-title/presentation.pptx", "--format", "json"})

	err := cmd.Execute()
	// Command should succeed or return CLIError with exit code 0
	if err != nil && err.Error() != "" {
		if cliErr, ok := err.(*CLIError); !ok || cliErr.ExitCode != ExitSuccess {
			t.Errorf("expected success, got %v", err)
		}
	}

	// If we got here without a panic, the command executed successfully
	// The JSON output was written to stdout by the validate command
}
