package cli

import (
	"encoding/json"
	"net/url"
	"sort"
	"strings"
	"testing"
)

type agentWorkbenchContractGolden struct {
	Capabilities agentWorkbenchCapabilitiesGolden `json:"capabilities"`
	Commands     []agentWorkbenchCommandGolden    `json:"commands"`
	Handles      agentWorkbenchHandlesGolden      `json:"handles"`
	MCP          agentWorkbenchMCPGolden          `json:"mcp"`
}

type agentWorkbenchCapabilitiesGolden struct {
	ContractVersion string   `json:"contractVersion"`
	PackageTypes    []string `json:"packageTypes"`
	CommandCount    int      `json:"commandCount"`
	ObjectKindCount int      `json:"objectKindCount"`
	WorkflowNames   []string `json:"workflowNames"`
}

type agentWorkbenchCommandGolden struct {
	Path                string   `json:"path"`
	Flags               []string `json:"flags"`
	TargetObjectKinds   []string `json:"targetObjectKinds"`
	ExampleCommands     []string `json:"exampleCommands"`
	CommonErrorPatterns []string `json:"commonErrorPatterns"`
}

type agentWorkbenchHandlesGolden struct {
	Field              string                                 `json:"field"`
	Prefix             string                                 `json:"prefix"`
	Accepted           bool                                   `json:"accepted"`
	Issued             bool                                   `json:"issued"`
	EmittedByFindToOps bool                                   `json:"emittedByFindToOps"`
	AcceptedBy         []agentWorkbenchHandleAcceptanceGolden `json:"acceptedBy"`
	Errors             []string                               `json:"errors"`
}

type agentWorkbenchHandleAcceptanceGolden struct {
	Command     string   `json:"command"`
	Flags       []string `json:"flags"`
	HandleKinds []string `json:"handleKinds"`
}

type agentWorkbenchMCPGolden struct {
	ToolNames            []string                                  `json:"toolNames"`
	ResourceURIs         []string                                  `json:"resourceURIs"`
	ResourceTemplates    []string                                  `json:"resourceTemplates"`
	CommandResourceReads []agentWorkbenchCommandResourceReadGolden `json:"commandResourceReads"`
}

type agentWorkbenchCommandResourceReadGolden struct {
	URIPath string   `json:"uriPath"`
	Path    string   `json:"path"`
	Flags   []string `json:"flags"`
}

// TestAgentWorkbenchContractGolden freezes the practical discovery surface an
// agent depends on: command inventory shape, high-value command flags/examples,
// handle acceptance, and MCP resource/tool navigation. It intentionally omits
// the build Version because that may be a commit hash.
func TestAgentWorkbenchContractGolden(t *testing.T) {
	resetFlags()
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v\n%s", err, output)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("decode capabilities JSON: %v\n%s", err, output)
	}

	actual := collectAgentWorkbenchContractGolden(t, doc)
	assertGoldenJSONValue(t, "agent_workbench_contract_summary.json", actual)
}

func collectAgentWorkbenchContractGolden(t *testing.T, doc capabilitiesDocument) agentWorkbenchContractGolden {
	t.Helper()
	workflowNames := make([]string, 0, len(doc.Workflows))
	for _, workflow := range doc.Workflows {
		workflowNames = append(workflowNames, workflow.Name)
	}

	selectedCommands := []string{
		"ooxml find",
		"ooxml apply",
		"ooxml inspect",
		"ooxml validate",
		"ooxml conformance check",
		"ooxml conformance coverage",
		"ooxml verify",
		"ooxml doctor",
		"ooxml serve",
		"ooxml mcp",
		"ooxml pptx slides show",
		"ooxml pptx replace text",
		"ooxml pptx charts update-data",
		"ooxml pptx charts set-title",
		"ooxml pptx charts set-axis",
		"ooxml pptx charts set-series-style",
		"ooxml pptx animations add",
		"ooxml xlsx cells set",
		"ooxml xlsx ranges set-style",
		"ooxml xlsx tables append-records",
		"ooxml xlsx charts update-source",
		"ooxml xlsx charts set-title",
		"ooxml xlsx charts set-series-style",
		"ooxml xlsx pivots create",
		"ooxml vba inspect",
		"ooxml vba inspect-bin",
		"ooxml vba create",
		"ooxml vba add-module",
		"ooxml vba replace-module",
		"ooxml vba remove-module",
		"ooxml docx styles apply",
		"ooxml docx paragraphs set",
		"ooxml docx tables set-cell",
		"ooxml docx headers list",
		"ooxml docx headers show",
		"ooxml docx headers set-text",
		"ooxml docx footers list",
		"ooxml docx footers show",
		"ooxml docx footers set-text",
	}

	commands := make([]agentWorkbenchCommandGolden, 0, len(selectedCommands))
	for _, path := range selectedCommands {
		cmd := findCapabilityCommand(doc.Commands, path)
		if cmd == nil {
			t.Fatalf("selected command %q missing from capabilities", path)
		}
		assertAgentCommandMetadata(t, *cmd)
		commands = append(commands, agentWorkbenchCommandGolden{
			Path:                cmd.Path,
			Flags:               capabilityFlagNames(cmd.LocalFlags),
			TargetObjectKinds:   sortedCopy(cmd.TargetObjectKinds),
			ExampleCommands:     capabilityExampleCommands(cmd.Examples),
			CommonErrorPatterns: capabilityCommonErrorPatterns(cmd.CommonErrors),
		})
	}

	return agentWorkbenchContractGolden{
		Capabilities: agentWorkbenchCapabilitiesGolden{
			ContractVersion: doc.ContractVersion,
			PackageTypes:    append([]string(nil), doc.PackageTypes...),
			CommandCount:    len(doc.Commands),
			ObjectKindCount: len(doc.ObjectKinds),
			WorkflowNames:   workflowNames,
		},
		Commands: commands,
		Handles:  collectAgentWorkbenchHandlesGolden(doc.Handles),
		MCP:      collectAgentWorkbenchMCPGolden(t, selectedCommands),
	}
}

func assertAgentCommandMetadata(t *testing.T, cmd capabilityCommand) {
	t.Helper()
	if len(cmd.TargetObjectKinds) == 0 {
		t.Fatalf("%s has no targetObjectKinds", cmd.Path)
	}
	if len(cmd.Examples) == 0 {
		t.Fatalf("%s has no examples", cmd.Path)
	}
	if len(cmd.CommonErrors) == 0 {
		t.Fatalf("%s has no commonErrors", cmd.Path)
	}
	for _, ex := range cmd.Examples {
		if ex.Command == "" {
			t.Fatalf("%s has an empty example command", cmd.Path)
		}
		if len(ex.Command) < len("ooxml ") || ex.Command[:len("ooxml ")] != "ooxml " {
			t.Fatalf("%s example %q must start with ooxml", cmd.Path, ex.Command)
		}
		for _, flag := range exampleFlags(ex.Command) {
			if !agentExampleFlagAllowed(cmd, flag) {
				t.Fatalf("%s example %q uses unknown flag %s", cmd.Path, ex.Command, flag)
			}
		}
	}
}

var agentExampleGlobalFlags = map[string]bool{
	"--format":    true,
	"--json":      true,
	"--keep-temp": true,
	"--no-color":  true,
	"--output":    true,
	"--pretty":    true,
	"--strict":    true,
	"--temp-dir":  true,
	"--verbosity": true,
}

func exampleFlags(command string) []string {
	fields := strings.Fields(command)
	flags := make([]string, 0)
	for _, field := range fields {
		field = strings.Trim(field, `'"`)
		if !strings.HasPrefix(field, "--") {
			continue
		}
		if idx := strings.IndexByte(field, '='); idx >= 0 {
			field = field[:idx]
		}
		flags = append(flags, field)
	}
	return flags
}

func agentExampleFlagAllowed(cmd capabilityCommand, flag string) bool {
	if agentExampleGlobalFlags[flag] {
		return true
	}
	for _, localFlag := range cmd.LocalFlags {
		if canonicalFlagName(localFlag.Name) == flag {
			return true
		}
	}
	return false
}

func capabilityFlagNames(flags []capabilityFlag) []string {
	names := make([]string, 0, len(flags))
	for _, flag := range flags {
		if flag.Name == "help" || flag.Name == "--help" {
			continue
		}
		names = append(names, canonicalFlagName(flag.Name))
	}
	sort.Strings(names)
	return names
}

func canonicalFlagName(name string) string {
	if len(name) >= 2 && name[:2] == "--" {
		return name
	}
	return "--" + name
}

func capabilityExampleCommands(examples []capabilityExample) []string {
	out := make([]string, 0, len(examples))
	for _, ex := range examples {
		out = append(out, ex.Command)
	}
	sort.Strings(out)
	return out
}

func capabilityCommonErrorPatterns(errors []capabilityCommonError) []string {
	out := make([]string, 0, len(errors))
	for _, err := range errors {
		out = append(out, err.Pattern)
	}
	sort.Strings(out)
	return out
}

func collectAgentWorkbenchHandlesGolden(handles capabilityHandles) agentWorkbenchHandlesGolden {
	acceptedBy := make([]agentWorkbenchHandleAcceptanceGolden, 0, len(handles.AcceptedBy))
	for _, entry := range handles.AcceptedBy {
		acceptedBy = append(acceptedBy, agentWorkbenchHandleAcceptanceGolden{
			Command:     entry.Command,
			Flags:       sortedCopy(entry.Flags),
			HandleKinds: sortedCopy(entry.HandleKinds),
		})
	}
	sort.Slice(acceptedBy, func(i, j int) bool {
		return acceptedBy[i].Command < acceptedBy[j].Command
	})
	return agentWorkbenchHandlesGolden{
		Field:              handles.Field,
		Prefix:             handles.Prefix,
		Accepted:           handles.Accepted,
		Issued:             handles.Issued,
		EmittedByFindToOps: handles.EmittedByFindToOps,
		AcceptedBy:         acceptedBy,
		Errors:             sortedCopy(handles.Errors),
	}
}

func collectAgentWorkbenchMCPGolden(t *testing.T, selectedCommands []string) agentWorkbenchMCPGolden {
	t.Helper()
	resetFlags()
	c := newMCPConn(t)

	toolsRaw := c.mustResult("tools/list", nil)
	var tools mcpToolsListResult
	if err := json.Unmarshal(toolsRaw, &tools); err != nil {
		t.Fatalf("decode tools/list: %v", err)
	}
	toolNames := make([]string, 0, len(tools.Tools))
	for _, tool := range tools.Tools {
		toolNames = append(toolNames, tool.Name)
	}
	sort.Strings(toolNames)

	resourcesRaw := c.mustResult("resources/list", nil)
	var resources mcpResourcesListResult
	if err := json.Unmarshal(resourcesRaw, &resources); err != nil {
		t.Fatalf("decode resources/list: %v", err)
	}
	resourceURIs := make([]string, 0, len(resources.Resources))
	for _, resource := range resources.Resources {
		resourceURIs = append(resourceURIs, resource.URI)
	}
	sort.Strings(resourceURIs)

	templatesRaw := c.mustResult("resources/templates/list", nil)
	var templates mcpResourceTemplatesListResult
	if err := json.Unmarshal(templatesRaw, &templates); err != nil {
		t.Fatalf("decode resources/templates/list: %v", err)
	}
	resourceTemplates := make([]string, 0, len(templates.ResourceTemplates))
	for _, tmpl := range templates.ResourceTemplates {
		resourceTemplates = append(resourceTemplates, tmpl.URITemplate)
	}
	sort.Strings(resourceTemplates)

	commandReads := make([]agentWorkbenchCommandResourceReadGolden, 0, len(selectedCommands))
	for _, path := range selectedCommands {
		uriPath := url.PathEscape(path)
		raw := c.mustResult("resources/read", map[string]interface{}{
			"uri": "resource://command/" + uriPath,
		})
		var read mcpResourcesReadResult
		if err := json.Unmarshal(raw, &read); err != nil {
			t.Fatalf("decode command resource for %q: %v", path, err)
		}
		if len(read.Contents) != 1 {
			t.Fatalf("command resource for %q returned %d contents, want 1", path, len(read.Contents))
		}
		var cmd capabilityCommand
		if err := json.Unmarshal([]byte(read.Contents[0].Text), &cmd); err != nil {
			t.Fatalf("decode command resource body for %q: %v", path, err)
		}
		commandReads = append(commandReads, agentWorkbenchCommandResourceReadGolden{
			URIPath: uriPath,
			Path:    cmd.Path,
			Flags:   capabilityFlagNames(cmd.LocalFlags),
		})
	}

	return agentWorkbenchMCPGolden{
		ToolNames:            toolNames,
		ResourceURIs:         resourceURIs,
		ResourceTemplates:    resourceTemplates,
		CommandResourceReads: commandReads,
	}
}

func sortedCopy(values []string) []string {
	out := append([]string(nil), values...)
	sort.Strings(out)
	return out
}
