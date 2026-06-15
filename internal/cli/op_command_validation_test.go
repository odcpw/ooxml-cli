package cli

import (
	"strings"
	"testing"
)

func TestValidateKnownOperationCommandRequiresOpCompatibleMutation(t *testing.T) {
	for _, command := range []string{
		"xlsx cells set",
		"ooxml pptx charts set-title",
		"vba add-module",
	} {
		if err := validateKnownOperationCommand(command); err != nil {
			t.Fatalf("validateKnownOperationCommand(%q) rejected compatible mutation: %v", command, err)
		}
	}

	readErr := validateKnownOperationCommand("xlsx sheets list")
	if readErr == nil {
		t.Fatal("read command should not be op-compatible")
	}
	if !strings.Contains(readErr.Message, "does not accept the mutation output flags") {
		t.Fatalf("read command error should explain mutation flags: %s", readErr.Message)
	}

	positionalErr := validateKnownOperationCommand("pptx slides move")
	if positionalErr == nil {
		t.Fatal("extra-positional mutation should not be op-compatible")
	}
	if !strings.Contains(positionalErr.Message, "op can supply only the package file") {
		t.Fatalf("extra-positional command error should explain positional limit: %s", positionalErr.Message)
	}
}
