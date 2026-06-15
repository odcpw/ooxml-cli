package cli

import (
	"strings"
	"testing"
)

func assertPPTXBridgeSavedCommandsForTest(t *testing.T, commands PPTXBridgeReadbackCommands, outPath, readbackNeedle string) string {
	t.Helper()

	if commands.ReadbackCommand == "" || commands.SlideReadbackCommand == "" || commands.ValidateCommand == "" || commands.RenderCommand == "" {
		t.Fatalf("saved bridge commands are incomplete: %+v", commands)
	}
	if commands.ReadbackCommandTemplate != "" || commands.SlideReadbackCommandTemplate != "" || commands.ValidateCommandTemplate != "" || commands.RenderCommandTemplate != "" {
		t.Fatalf("saved bridge result should not include templates: %+v", commands)
	}
	if !strings.Contains(commands.ReadbackCommand, readbackNeedle) || !strings.Contains(commands.ReadbackCommand, outPath) {
		t.Fatalf("unexpected readbackCommand: %s", commands.ReadbackCommand)
	}
	if !strings.Contains(commands.SlideReadbackCommand, "pptx slides show") || !strings.Contains(commands.SlideReadbackCommand, outPath) {
		t.Fatalf("unexpected slideReadbackCommand: %s", commands.SlideReadbackCommand)
	}
	if !strings.Contains(commands.ValidateCommand, "validate --strict") || !strings.Contains(commands.ValidateCommand, outPath) {
		t.Fatalf("unexpected validateCommand: %s", commands.ValidateCommand)
	}
	if !strings.Contains(commands.RenderCommand, "pptx render") || !strings.Contains(commands.RenderCommand, outPath) {
		t.Fatalf("unexpected renderCommand: %s", commands.RenderCommand)
	}

	executeGeneratedOOXMLCommandForXLSXTest(t, commands.ValidateCommand)
	readback := executeGeneratedOOXMLCommandForXLSXTest(t, commands.ReadbackCommand)
	if strings.TrimSpace(readback) == "" {
		t.Fatalf("generated readbackCommand returned empty output: %s", commands.ReadbackCommand)
	}
	slideReadback := executeGeneratedOOXMLCommandForXLSXTest(t, commands.SlideReadbackCommand)
	if strings.TrimSpace(slideReadback) == "" {
		t.Fatalf("generated slideReadbackCommand returned empty output: %s", commands.SlideReadbackCommand)
	}
	return readback
}

func assertPPTXBridgeDryRunTemplatesForTest(t *testing.T, commands PPTXBridgeReadbackCommands, readbackNeedle string) {
	t.Helper()

	if commands.ReadbackCommand != "" || commands.SlideReadbackCommand != "" || commands.ValidateCommand != "" || commands.RenderCommand != "" {
		t.Fatalf("dry-run bridge result should not include saved-output commands: %+v", commands)
	}
	for label, command := range map[string]string{
		"readback":       commands.ReadbackCommandTemplate,
		"slide readback": commands.SlideReadbackCommandTemplate,
		"validate":       commands.ValidateCommandTemplate,
		"render":         commands.RenderCommandTemplate,
	} {
		if command == "" || !strings.Contains(command, "<out.pptx>") {
			t.Fatalf("%s template missing output placeholder: %s", label, command)
		}
	}
	if !strings.Contains(commands.ReadbackCommandTemplate, readbackNeedle) {
		t.Fatalf("unexpected readbackCommandTemplate: %s", commands.ReadbackCommandTemplate)
	}
	if !strings.Contains(commands.SlideReadbackCommandTemplate, "pptx slides show") {
		t.Fatalf("unexpected slideReadbackCommandTemplate: %s", commands.SlideReadbackCommandTemplate)
	}
	if !strings.Contains(commands.ValidateCommandTemplate, "validate --strict") {
		t.Fatalf("unexpected validateCommandTemplate: %s", commands.ValidateCommandTemplate)
	}
	if !strings.Contains(commands.RenderCommandTemplate, "pptx render") {
		t.Fatalf("unexpected renderCommandTemplate: %s", commands.RenderCommandTemplate)
	}
}

func assertPPTXBridgeOutputVerificationCommandsForTest(t *testing.T, commands PPTXBridgeReadbackCommands, outPath string) {
	t.Helper()

	if commands.ReadbackCommand != "" || commands.SlideReadbackCommand != "" || commands.ReadbackCommandTemplate != "" || commands.SlideReadbackCommandTemplate != "" {
		t.Fatalf("output verification commands should not include object readback fields: %+v", commands)
	}
	if commands.ValidateCommand == "" || commands.RenderCommand == "" {
		t.Fatalf("output verification commands are incomplete: %+v", commands)
	}
	if commands.ValidateCommandTemplate != "" || commands.RenderCommandTemplate != "" {
		t.Fatalf("saved output verification should not include templates: %+v", commands)
	}
	if !strings.Contains(commands.ValidateCommand, "validate --strict") || !strings.Contains(commands.ValidateCommand, outPath) {
		t.Fatalf("unexpected validateCommand: %s", commands.ValidateCommand)
	}
	if !strings.Contains(commands.RenderCommand, "pptx render") || !strings.Contains(commands.RenderCommand, outPath) {
		t.Fatalf("unexpected renderCommand: %s", commands.RenderCommand)
	}
	executeGeneratedOOXMLCommandForXLSXTest(t, commands.ValidateCommand)
}

func assertPPTXBridgeOutputVerificationTemplatesForTest(t *testing.T, commands PPTXBridgeReadbackCommands) {
	t.Helper()

	if commands.ReadbackCommand != "" || commands.SlideReadbackCommand != "" || commands.ValidateCommand != "" || commands.RenderCommand != "" {
		t.Fatalf("dry-run output verification should not include saved-output commands: %+v", commands)
	}
	if commands.ReadbackCommandTemplate != "" || commands.SlideReadbackCommandTemplate != "" {
		t.Fatalf("dry-run output verification should not include object readback templates: %+v", commands)
	}
	if commands.ValidateCommandTemplate == "" || commands.RenderCommandTemplate == "" {
		t.Fatalf("dry-run output verification templates are incomplete: %+v", commands)
	}
	if !strings.Contains(commands.ValidateCommandTemplate, "validate --strict") || !strings.Contains(commands.ValidateCommandTemplate, "<out.pptx>") {
		t.Fatalf("unexpected validateCommandTemplate: %s", commands.ValidateCommandTemplate)
	}
	if !strings.Contains(commands.RenderCommandTemplate, "pptx render") || !strings.Contains(commands.RenderCommandTemplate, "<out.pptx>") {
		t.Fatalf("unexpected renderCommandTemplate: %s", commands.RenderCommandTemplate)
	}
}
