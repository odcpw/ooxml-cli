package cli

import (
	"encoding/json"
	"fmt"
	"strings"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/capabilities"
	"github.com/spf13/cobra"
)

// findCobraCommandByPath walks the tree for a command by its full path.
func findCobraCommandByPath(path string) *cobra.Command {
	var found *cobra.Command
	var walk func(*cobra.Command)
	walk = func(c *cobra.Command) {
		if c.CommandPath() == path {
			found = c
			return
		}
		for _, ch := range c.Commands() {
			walk(ch)
		}
	}
	walk(GetRootCmd())
	return found
}

// TestCapabilitiesOpCompatibleMatchesGuard closes the discoverability drift: the
// manifest's opCompatible field must agree with the op-validation guard for every
// command, so an agent can predict an apply/serve/MCP rejection from the manifest
// instead of hitting it at runtime. It also spot-checks the headline cases.
func TestCapabilitiesOpCompatibleMatchesGuard(t *testing.T) {
	doc := buildCapabilitiesDocument()

	for _, c := range doc.Commands {
		guardErr := validateKnownOperationCommand(c.Path)
		if c.OpCompatible && guardErr != nil {
			t.Errorf("%s: manifest opCompatible=true but guard rejects it: %s", c.Path, guardErr.Message)
		}
		if !c.OpCompatible {
			if guardErr == nil {
				t.Errorf("%s: manifest opCompatible=false but guard accepts it", c.Path)
			}
			if c.OpIneligibleReason == "" {
				t.Errorf("%s: opCompatible=false must carry an opIneligibleReason", c.Path)
			}
		}
	}

	// Spot checks: a single-positional mutator is op-compatible; a multi-positional
	// slide mutator and a read command are not.
	if c := findCapabilityCommand(doc.Commands, "ooxml xlsx cells set"); c == nil || !c.OpCompatible {
		t.Fatalf("xlsx cells set should be op-compatible, got %+v", c)
	}
	if c := findCapabilityCommand(doc.Commands, "ooxml pptx slides reorder"); c == nil || c.OpCompatible || !strings.Contains(c.OpIneligibleReason, "positional") {
		t.Fatalf("pptx slides reorder should be op-INcompatible (positional), got %+v", c)
	}
}

func TestApplyExampleMetadataPopulatesEmptyExamples(t *testing.T) {
	// Trigger cobra.OnInitialize (which derives Example help) by running a command.
	if _, err := executeRootForXLSXTest(t, "capabilities", "--json"); err != nil {
		t.Fatalf("warm-up execute failed: %v", err)
	}
	c := findCobraCommandByPath("ooxml pptx shapes show")
	if c == nil {
		t.Fatalf("could not find 'ooxml pptx shapes show'")
	}
	if c.Example == "" {
		t.Fatalf("expected derived Example help on 'ooxml pptx shapes show'")
	}
	if !strings.Contains(c.Example, "ooxml --json pptx shapes show") {
		t.Fatalf("derived Example missing expected command:\n%s", c.Example)
	}
}

func TestApplyExampleMetadataPreservesHandAuthored(t *testing.T) {
	if _, err := executeRootForXLSXTest(t, "capabilities", "--json"); err != nil {
		t.Fatalf("warm-up execute failed: %v", err)
	}
	// 'ooxml pptx clone-slide' hand-authors a cobra Example and also has authored
	// metadata; metadata derivation must NOT overwrite the existing Example.
	c := findCobraCommandByPath("ooxml pptx clone-slide")
	if c == nil {
		t.Fatalf("could not find 'ooxml pptx clone-slide'")
	}
	if !strings.Contains(c.Example, "--insert-after") {
		t.Fatalf("hand-authored 'ooxml pptx clone-slide' Example was clobbered:\n%s", c.Example)
	}
}

// findCapabilityCommand returns the command entry for a path, or nil.
func findCapabilityCommand(commands []capabilityCommand, path string) *capabilityCommand {
	for i := range commands {
		if commands[i].Path == path {
			return &commands[i]
		}
	}
	return nil
}

func TestCapabilitiesJSONContractV4(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}

	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("capabilities output is not valid JSON: %v\n%s", err, output)
	}

	if doc.ContractVersion != "ooxml-cli.agent-capabilities.v4" {
		t.Fatalf("contractVersion = %q, want v4", doc.ContractVersion)
	}
	if len(doc.ObjectKinds) < 10 {
		t.Fatalf("expected >=10 object kinds, got %d", len(doc.ObjectKinds))
	}
	if doc.ObjectKindIndex == nil {
		t.Fatalf("objectKindsIndex is nil")
	}
	if len(doc.ObjectKindIndex["shape"]) == 0 {
		t.Fatalf("objectKindsIndex missing shape entries")
	}
	cmd := findCapabilityCommand(doc.Commands, "ooxml xlsx cells set")
	if cmd == nil {
		t.Fatal("capabilities missing xlsx cells set")
	}
	argNames := map[string]bool{}
	for _, flag := range cmd.LocalFlags {
		if flag.ArgName != "" {
			argNames[flag.ArgName] = true
		}
	}
	for _, want := range []string{"sheet", "cell", "value"} {
		if !argNames[want] {
			t.Fatalf("capabilities missing dashless argName %q in xlsx cells set flags: %+v", want, cmd.LocalFlags)
		}
	}
}

func TestCapabilitiesArgNamesAreJSONFriendly(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("capabilities output is not valid JSON: %v\n%s", err, output)
	}
	checkFlags := func(scope string, flags []capabilityFlag) {
		t.Helper()
		for _, flag := range flags {
			if flag.ArgName == "" {
				continue
			}
			if strings.HasPrefix(flag.ArgName, "-") || strings.Contains(flag.ArgName, "-") || strings.Contains(flag.ArgName, "_") || strings.Contains(flag.ArgName, " ") {
				t.Fatalf("%s flag %s advertised non-JSON-friendly argName %q", scope, flag.Name, flag.ArgName)
			}
		}
	}
	checkFlags("global", doc.GlobalFlags)
	for _, cmd := range doc.Commands {
		checkFlags(cmd.Path, cmd.LocalFlags)
	}

	for _, tc := range []struct {
		path string
		want []string
	}{
		{path: "ooxml apply", want: []string{"dryRun", "inPlace", "noValidate"}},
		{path: "ooxml pptx replace text-occurrences", want: []string{"expectCount", "expectPlanHash", "forSlides", "forShape"}},
		{path: "ooxml xlsx ranges export", want: []string{"dataOut", "includeTypes"}},
	} {
		cmd := findCapabilityCommand(doc.Commands, tc.path)
		if cmd == nil {
			t.Fatalf("capabilities missing %s", tc.path)
		}
		got := map[string]bool{}
		for _, flag := range cmd.LocalFlags {
			got[flag.ArgName] = true
		}
		for _, want := range tc.want {
			if !got[want] {
				t.Fatalf("%s missing argName %q in flags: %+v", tc.path, want, cmd.LocalFlags)
			}
		}
	}
}

func TestCapabilitiesForLayoutMasterReverseLookup(t *testing.T) {
	for _, tc := range []struct {
		filter string
		want   []string
	}{
		{filter: "layout", want: []string{"ooxml pptx layouts list", "ooxml pptx layouts show", "ooxml pptx layouts rename"}},
		{filter: "master", want: []string{"ooxml pptx masters list", "ooxml pptx masters show", "ooxml pptx masters add-placeholder"}},
	} {
		output, err := executeRootForXLSXTest(t, "capabilities", "--json", "--for", tc.filter)
		if err != nil {
			t.Fatalf("capabilities --for %s failed: %v", tc.filter, err)
		}
		var doc capabilitiesDocument
		if err := json.Unmarshal([]byte(output), &doc); err != nil {
			t.Fatalf("capabilities --for %s output is not valid JSON: %v\n%s", tc.filter, err, output)
		}
		paths := map[string]bool{}
		for _, cmd := range doc.Commands {
			paths[cmd.Path] = true
		}
		for _, want := range tc.want {
			if !paths[want] {
				t.Fatalf("capabilities --for %s missing %q; got %+v", tc.filter, want, doc.Commands)
			}
		}
	}
}

// TestCapabilitiesAdvertisesHandles asserts the additive handles section reflects
// that some commands accept handles and read surfaces issue them, so an agent
// can discover the stable-handle contract from the machine-readable document.
func TestCapabilitiesAdvertisesHandles(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("capabilities output is not valid JSON: %v\n%s", err, output)
	}
	h := doc.Handles
	if h.Field != "handle" {
		t.Errorf("handles.field = %q, want handle", h.Field)
	}
	if !h.Accepted || !h.Issued || !h.EmittedByFindToOps {
		t.Errorf("handles flags = %+v, want accepted/issued/emittedByFindToOps all true", h)
	}
	if len(h.Grammar) == 0 || len(h.Errors) == 0 {
		t.Errorf("handles grammar/errors should be populated: %+v", h)
	}
	grammar := strings.Join(h.Grammar, "\n")
	if !strings.Contains(grammar, "H:docx/pt:styles/style:n:<styleId>") {
		t.Errorf("handles.grammar missing DOCX style handle grammar; got:\n%s", grammar)
	}
	if strings.Contains(grammar, "H:docx/pt:doc/style:n:<styleId>") {
		t.Errorf("handles.grammar contains stale DOCX style handle grammar; got:\n%s", grammar)
	}
	notes := strings.Join(h.Notes, "\n")
	if strings.Contains(notes, "accepted wherever a selector is accepted") {
		t.Errorf("handles.notes overclaim universal selector support: %q", notes)
	}
	wantErr := map[string]bool{"HANDLE_STALE": false, "HANDLE_AMBIGUOUS": false}
	for _, e := range h.Errors {
		if _, ok := wantErr[e]; ok {
			wantErr[e] = true
		}
	}
	for code, seen := range wantErr {
		if !seen {
			t.Errorf("handles.errors missing %s", code)
		}
	}
}

func TestCapabilitiesHandleAcceptanceIsSpecific(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("capabilities output is not valid JSON: %v\n%s", err, output)
	}
	acceptedBy := doc.Handles.AcceptedBy
	if len(acceptedBy) == 0 {
		t.Fatalf("handles.acceptedBy is empty")
	}
	for _, tc := range []struct {
		command string
		flag    string
		kind    string
	}{
		{command: "ooxml pptx replace text", flag: "--target", kind: "pptx.shape"},
		{command: "ooxml pptx animations add", flag: "--shape", kind: "pptx.shape"},
		{command: "ooxml pptx comments edit", flag: "--handle", kind: "pptx.comment"},
		{command: "ooxml pptx comments remove", flag: "--handle", kind: "pptx.comment"},
		{command: "ooxml xlsx cells set", flag: "--cell", kind: "xlsx.cell"},
		{command: "ooxml xlsx comments remove", flag: "--handle", kind: "xlsx.comment"},
		{command: "ooxml xlsx comments update", flag: "--handle", kind: "xlsx.comment"},
		{command: "ooxml docx styles apply", flag: "--style", kind: "docx.style"},
		{command: "ooxml docx paragraphs set", flag: "--handle", kind: "docx.paragraph"},
	} {
		if !hasHandleAcceptance(acceptedBy, tc.command, tc.flag, tc.kind) {
			t.Fatalf("handles.acceptedBy missing command=%q flag=%q kind=%q: %+v", tc.command, tc.flag, tc.kind, acceptedBy)
		}
	}
	if hasHandleAcceptanceCommand(acceptedBy, "ooxml pptx shapes set-bounds") {
		t.Fatalf("handles.acceptedBy should not list pptx shapes set-bounds until that command accepts handles")
	}
}

func TestCapabilitiesHandleAcceptanceCommandsAndFlagsExist(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("capabilities output is not valid JSON: %v\n%s", err, output)
	}
	for _, entry := range doc.Handles.AcceptedBy {
		cmd := findCobraCommandByPath(entry.Command)
		if cmd == nil {
			t.Fatalf("handles.acceptedBy advertises missing command %q", entry.Command)
		}
		for _, flag := range entry.Flags {
			name := strings.TrimPrefix(flag, "--")
			if cmd.Flags().Lookup(name) == nil && cmd.InheritedFlags().Lookup(name) == nil && cmd.PersistentFlags().Lookup(name) == nil {
				t.Fatalf("handles.acceptedBy advertises missing flag %q on %q", flag, entry.Command)
			}
		}
	}
}

func hasHandleAcceptance(entries []capabilityHandleAcceptance, command, flag, kind string) bool {
	for _, entry := range entries {
		if entry.Command != command {
			continue
		}
		if containsString(entry.Flags, flag) && containsString(entry.HandleKinds, kind) {
			return true
		}
	}
	return false
}

func hasHandleAcceptanceCommand(entries []capabilityHandleAcceptance, command string) bool {
	for _, entry := range entries {
		if entry.Command == command {
			return true
		}
	}
	return false
}

func TestCapabilitiesExamplesPresentOnChosenCommands(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}

	withExamples := 0
	for _, c := range doc.Commands {
		if len(c.Examples) > 0 {
			withExamples++
		}
	}
	if withExamples < 10 {
		t.Fatalf("expected >=10 commands with examples, got %d", withExamples)
	}

	// Spot-check a few high-use commands and their example shape.
	for _, path := range []string{
		"ooxml pptx shapes show",
		"ooxml xlsx cells set",
		"ooxml pptx charts update-data",
		"ooxml inspect",
		"ooxml validate",
	} {
		c := findCapabilityCommand(doc.Commands, path)
		if c == nil {
			t.Fatalf("command %q missing from capabilities", path)
		}
		if len(c.Examples) == 0 {
			t.Fatalf("command %q has no examples", path)
		}
		for i, ex := range c.Examples {
			if !strings.HasPrefix(ex.Command, "ooxml ") {
				t.Fatalf("%q example %d does not start with 'ooxml ': %q", path, i, ex.Command)
			}
			if ex.Description == "" {
				t.Fatalf("%q example %d has empty description", path, i)
			}
		}
	}
}

func TestCapabilitiesCommonErrorsPresent(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}
	c := findCapabilityCommand(doc.Commands, "ooxml pptx shapes show")
	if c == nil || len(c.CommonErrors) == 0 {
		t.Fatalf("expected commonErrors on 'ooxml pptx shapes show'")
	}
	for _, ce := range c.CommonErrors {
		if ce.Pattern == "" || ce.Solution == "" {
			t.Fatalf("commonError has empty pattern or solution: %+v", ce)
		}
	}
}

func TestCapabilitiesPracticalChartCommandsHaveAgentMetadata(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}

	for _, path := range []string{
		"ooxml pptx charts create",
		"ooxml pptx charts set-title",
		"ooxml pptx charts set-legend",
		"ooxml pptx charts set-axis",
		"ooxml pptx charts set-series-style",
		"ooxml pptx charts convert-type",
		"ooxml pptx charts set-plot-area-fill",
		"ooxml pptx charts set-chart-area-fill",
		"ooxml pptx charts copy-style",
		"ooxml xlsx charts create",
		"ooxml xlsx charts set-title",
		"ooxml xlsx charts set-legend",
		"ooxml xlsx charts set-axis",
		"ooxml xlsx charts set-series-style",
		"ooxml xlsx charts convert-type",
		"ooxml xlsx charts set-plot-area-fill",
		"ooxml xlsx charts set-chart-area-fill",
		"ooxml xlsx charts copy-style",
	} {
		c := findCapabilityCommand(doc.Commands, path)
		if c == nil {
			t.Fatalf("capabilities missing practical chart command %q", path)
		}
		if len(c.Examples) == 0 {
			t.Fatalf("%q has no runnable examples in capabilities", path)
		}
		if len(c.CommonErrors) == 0 {
			t.Fatalf("%q has no commonErrors in capabilities", path)
		}
		if !containsString(c.TargetObjectKinds, "chart") {
			t.Fatalf("%q should target object kind chart, got %v", path, c.TargetObjectKinds)
		}
		if !strings.Contains(path, " create") && !containsString(c.TargetObjectKinds, "style") {
			t.Fatalf("%q should target object kind style, got %v", path, c.TargetObjectKinds)
		}
	}
}

func TestCapabilitiesTargetObjectKinds(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}
	c := findCapabilityCommand(doc.Commands, "ooxml pptx shapes show")
	if c == nil {
		t.Fatalf("missing 'ooxml pptx shapes show'")
	}
	found := false
	for _, k := range c.TargetObjectKinds {
		if k == "shape" {
			found = true
		}
	}
	if !found {
		t.Fatalf("'ooxml pptx shapes show' should target 'shape', got %v", c.TargetObjectKinds)
	}
}

func TestCapabilitiesForReverseLookup(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json", "--for", "shape")
	if err != nil {
		t.Fatalf("capabilities --for shape failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v\n%s", err, output)
	}
	if len(doc.Commands) == 0 {
		t.Fatalf("--for shape returned no commands")
	}
	for _, c := range doc.Commands {
		matches := false
		for _, k := range c.TargetObjectKinds {
			if k == "shape" {
				matches = true
			}
		}
		if !matches {
			t.Fatalf("--for shape returned non-shape command %q (kinds %v)", c.Path, c.TargetObjectKinds)
		}
	}
	if got := doc.ObjectKindIndex["shape"]; len(got) != len(doc.Commands) {
		t.Fatalf("objectKindsIndex[shape] (%d) should match returned commands (%d)", len(got), len(doc.Commands))
	}
}

func TestCapabilitiesForCommentAndImageReverseLookup(t *testing.T) {
	tests := []struct {
		kind string
		want []string
	}{
		{
			kind: "comment",
			want: []string{
				"ooxml pptx comments list",
				"ooxml xlsx comments list",
				"ooxml docx comments list",
			},
		},
		{
			kind: "image",
			want: []string{
				"ooxml pptx replace images",
				"ooxml pptx place image",
				"ooxml docx images list",
			},
		},
		{
			kind: "hyperlink",
			want: []string{
				"ooxml xlsx hyperlinks list",
				"ooxml xlsx hyperlinks show",
				"ooxml xlsx hyperlinks add",
				"ooxml xlsx hyperlinks update",
				"ooxml xlsx hyperlinks delete",
			},
		},
		{
			kind: "name",
			want: []string{
				"ooxml xlsx names list",
				"ooxml xlsx names show",
				"ooxml xlsx names add",
				"ooxml xlsx names update",
				"ooxml xlsx names rename",
				"ooxml xlsx names delete",
			},
		},
		{
			kind: "data-validation",
			want: []string{
				"ooxml xlsx data-validations list",
				"ooxml xlsx data-validations show",
				"ooxml xlsx data-validations create",
				"ooxml xlsx data-validations update",
				"ooxml xlsx data-validations delete",
			},
		},
		{
			kind: "conditional-format",
			want: []string{
				"ooxml xlsx conditional-formats list",
				"ooxml xlsx conditional-formats show",
				"ooxml xlsx conditional-formats add",
				"ooxml xlsx conditional-formats delete",
			},
		},
		{
			kind: "header",
			want: []string{
				"ooxml docx headers list",
				"ooxml docx headers show",
				"ooxml docx headers set-text",
			},
		},
		{
			kind: "footer",
			want: []string{
				"ooxml docx footers list",
				"ooxml docx footers show",
				"ooxml docx footers set-text",
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.kind, func(t *testing.T) {
			output, err := executeRootForXLSXTest(t, "capabilities", "--json", "--for", tt.kind)
			if err != nil {
				t.Fatalf("capabilities --for %s failed: %v", tt.kind, err)
			}
			var doc capabilitiesDocument
			if err := json.Unmarshal([]byte(output), &doc); err != nil {
				t.Fatalf("invalid JSON: %v\n%s", err, output)
			}
			if len(doc.Commands) == 0 {
				t.Fatalf("--for %s returned no commands", tt.kind)
			}
			for _, path := range tt.want {
				cmd := findCapabilityCommand(doc.Commands, path)
				if cmd == nil {
					t.Fatalf("--for %s missing expected command %q", tt.kind, path)
				}
				if !containsString(cmd.TargetObjectKinds, tt.kind) {
					t.Fatalf("%s targetObjectKinds = %v, want %q", path, cmd.TargetObjectKinds, tt.kind)
				}
			}
			if got := doc.ObjectKindIndex[tt.kind]; len(got) != len(doc.Commands) {
				t.Fatalf("objectKindsIndex[%s] (%d) should match returned commands (%d)", tt.kind, len(got), len(doc.Commands))
			}
		})
	}
}

func TestCapabilitiesExamplesAndWorkflowsAvoidShellActiveAnglePlaceholders(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v\n%s", err, output)
	}
	check := func(scope, command string) {
		t.Helper()
		if containsAnglePlaceholderForCapabilitiesTest(command) {
			t.Fatalf("%s command uses shell-active angle placeholder: %q", scope, command)
		}
	}
	for _, cmd := range doc.Commands {
		for i, ex := range cmd.Examples {
			check(fmt.Sprintf("%s example %d", cmd.Path, i), ex.Command)
		}
	}
	for _, workflow := range doc.Workflows {
		for i, command := range workflow.Commands {
			check(fmt.Sprintf("workflow %s command %d", workflow.Name, i), command)
		}
	}
}

func containsAnglePlaceholderForCapabilitiesTest(command string) bool {
	start := strings.Index(command, "<")
	if start == -1 {
		return false
	}
	return strings.Contains(command[start+1:], ">")
}

func TestCapabilitiesForFamilyReverseLookup(t *testing.T) {
	tests := []struct {
		filter string
		prefix string
		want   []string
	}{
		{
			filter: "pptx",
			prefix: "ooxml pptx",
			want:   []string{"ooxml pptx slides list", "ooxml pptx shapes show", "ooxml pptx animations list"},
		},
		{
			filter: "xlsx",
			prefix: "ooxml xlsx",
			want:   []string{"ooxml xlsx sheets list", "ooxml xlsx cells set", "ooxml xlsx charts list"},
		},
		{
			filter: "docx",
			prefix: "ooxml docx",
			want:   []string{"ooxml docx styles list", "ooxml docx comments list", "ooxml docx tables show"},
		},
		{
			filter: "vba",
			prefix: "ooxml vba",
			want:   []string{"ooxml vba list", "ooxml vba add-module", "ooxml vba office-check"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.filter, func(t *testing.T) {
			output, err := executeRootForXLSXTest(t, "capabilities", "--json", "--for", tt.filter)
			if err != nil {
				t.Fatalf("capabilities --for %s failed: %v", tt.filter, err)
			}
			var doc capabilitiesDocument
			if err := json.Unmarshal([]byte(output), &doc); err != nil {
				t.Fatalf("invalid JSON: %v\n%s", err, output)
			}
			if len(doc.Commands) == 0 {
				t.Fatalf("--for %s returned no commands", tt.filter)
			}
			for _, c := range doc.Commands {
				if c.Path != tt.prefix && !strings.HasPrefix(c.Path, tt.prefix+" ") {
					t.Fatalf("--for %s returned command outside family prefix %q: %q", tt.filter, tt.prefix, c.Path)
				}
			}
			for _, path := range tt.want {
				if findCapabilityCommand(doc.Commands, path) == nil {
					t.Fatalf("--for %s missing expected command %q", tt.filter, path)
				}
			}
		})
	}
}

func TestCapabilitiesFamilyFilterDoesNotPolluteObjectKinds(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json", "--for", "vba")
	if err != nil {
		t.Fatalf("capabilities --for vba failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v\n%s", err, output)
	}
	if len(doc.Commands) == 0 {
		t.Fatalf("--for vba returned no commands")
	}
	if _, ok := doc.ObjectKindIndex["vba"]; ok {
		t.Fatalf("vba is a command family, not an object kind; objectKindsIndex should not contain it")
	}
	if got := doc.ObjectKindIndex["module"]; len(got) == 0 {
		t.Fatalf("--for vba should still expose module object-kind entries")
	}
}

func TestCapabilitiesForUnknownKindIsGraceful(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json", "--for", "definitely-not-a-kind")
	if err != nil {
		t.Fatalf("--for unknown should exit 0, got error: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v\n%s", err, output)
	}
	if len(doc.Commands) != 0 {
		t.Fatalf("--for unknown should return no commands, got %d", len(doc.Commands))
	}
	if got, ok := doc.ObjectKindIndex["definitely-not-a-kind"]; !ok || len(got) != 0 {
		t.Fatalf("objectKindsIndex for unknown kind should be empty array, got %v (ok=%v)", got, ok)
	}
}

func TestCapabilitiesForTextMode(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--for", "sheet")
	if err != nil {
		t.Fatalf("capabilities --for sheet (text) failed: %v", err)
	}
	if !strings.Contains(output, "ooxml xlsx sheets list") {
		t.Fatalf("text --for sheet missing 'ooxml xlsx sheets list':\n%s", output)
	}
}

func TestCapabilitiesForFamilyTextMode(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--for", "vba")
	if err != nil {
		t.Fatalf("capabilities --for vba (text) failed: %v", err)
	}
	if !strings.Contains(output, `Commands in command family "vba"`) {
		t.Fatalf("text --for vba missing family heading:\n%s", output)
	}
	if !strings.Contains(output, "ooxml vba add-module") {
		t.Fatalf("text --for vba missing 'ooxml vba add-module':\n%s", output)
	}
}

// TestCapabilityMetadataPathsResolveToLiveCommands is the staleness guard:
// every authored metadata key must correspond to a real command in the tree.
func TestCapabilityMetadataPathsResolveToLiveCommands(t *testing.T) {
	output, err := executeRootForXLSXTest(t, "capabilities", "--json")
	if err != nil {
		t.Fatalf("capabilities --json failed: %v", err)
	}
	var doc capabilitiesDocument
	if err := json.Unmarshal([]byte(output), &doc); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}
	live := map[string]bool{}
	for _, c := range doc.Commands {
		live[c.Path] = true
	}
	for _, path := range capabilities.CommandPaths() {
		if !live[path] {
			t.Fatalf("metadata references command path %q that is not a live command (renamed/removed?)", path)
		}
	}
}
