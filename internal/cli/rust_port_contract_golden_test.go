package cli

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"testing"

	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
	"github.com/spf13/cobra"
)

type rustPortContractGolden struct {
	SchemaVersion           string                     `json:"schemaVersion"`
	ReferenceImplementation string                     `json:"referenceImplementation"`
	Generator               string                     `json:"generator"`
	Fixtures                []string                   `json:"fixtures"`
	CLI                     []rustPortCLICommandGolden `json:"cli"`
	Mutation                rustPortMutationGolden     `json:"mutation"`
	Serve                   rustPortProtocolGolden     `json:"serve"`
	MCP                     rustPortMCPGolden          `json:"mcp"`
	WebSmoke                rustPortWebSmokeGolden     `json:"webSmoke"`
	Coverage                []rustPortCoverageGolden   `json:"coverage"`
}

type rustPortCLICommandGolden struct {
	Name       string   `json:"name"`
	Args       []string `json:"args"`
	ExitCode   int      `json:"exitCode"`
	StdoutJSON any      `json:"stdoutJson,omitempty"`
	StdoutText string   `json:"stdoutText,omitempty"`
	StderrJSON any      `json:"stderrJson,omitempty"`
	StderrText string   `json:"stderrText,omitempty"`
}

type rustPortMutationGolden struct {
	EditedFilePublished bool                     `json:"editedFilePublished"`
	Edit                rustPortCLICommandGolden `json:"edit"`
	Validate            rustPortCLICommandGolden `json:"validate"`
	Render              rustPortCLICommandGolden `json:"render"`
	Verify              rustPortCLICommandGolden `json:"verify"`
}

type rustPortProtocolGolden struct {
	Transport string                `json:"transport"`
	Flow      []rustPortRPCExchange `json:"flow"`
}

type rustPortRPCExchange struct {
	Method   string `json:"method"`
	Request  any    `json:"request"`
	Response any    `json:"response"`
}

type rustPortMCPGolden struct {
	Discovery rustPortMCPDiscoveryGolden `json:"discovery"`
	Flow      rustPortProtocolGolden     `json:"flow"`
}

type rustPortMCPDiscoveryGolden struct {
	Initialize        any                           `json:"initialize"`
	Tools             []rustPortMCPToolGolden       `json:"tools"`
	Resources         []mcpResource                 `json:"resources"`
	ResourceTemplates []mcpResourceTemplate         `json:"resourceTemplates"`
	CommandResource   rustPortCommandResourceGolden `json:"commandResource"`
}

type rustPortMCPToolGolden struct {
	Name                 string   `json:"name"`
	Properties           []string `json:"properties"`
	Required             []string `json:"required"`
	AdditionalProperties bool     `json:"additionalProperties"`
}

type rustPortCommandResourceGolden struct {
	URI          string   `json:"uri"`
	Path         string   `json:"path"`
	OpCompatible bool     `json:"opCompatible"`
	Flags        []string `json:"flags"`
	ArgNames     []string `json:"argNames"`
}

type rustPortWebSmokeGolden struct {
	PackageScripts       map[string]string `json:"packageScripts"`
	AgentScript          string            `json:"agentScript"`
	NonPPTXScript        string            `json:"nonPptxScript"`
	BinaryEnv            string            `json:"binaryEnv"`
	SummaryIncludesBin   bool              `json:"summaryIncludesBin"`
	AgentDefaultFixture  string            `json:"agentDefaultFixture"`
	DOCXDefaultFixture   string            `json:"docxDefaultFixture"`
	XLSXDefaultFixture   string            `json:"xlsxDefaultFixture"`
	BinaryReadbackChecks []string          `json:"binaryReadbackChecks"`
}

type rustPortCoverageGolden struct {
	Surface     string `json:"surface"`
	Requirement string `json:"requirement"`
	Evidence    string `json:"evidence"`
	Status      string `json:"status"`
}

// TestRustPortContractGolden freezes the current Go implementation as the
// reference behavior a future Rust ooxml-cli must diff against. It covers the
// user-facing binary contract, a real mutation plus validation/render/verify
// readback, serve JSON-RPC sessions, MCP discovery/session flows, and the web
// smoke path that injects the binary through OOXML_BIN.
func TestRustPortContractGolden(t *testing.T) {
	resetRustPortContractCommandState()
	t.Cleanup(resetRustPortContractCommandState)

	repoRoot := serveRepoRoot(t)
	pptxFixture := "testdata/pptx/minimal-title/presentation.pptx"
	xlsxFixture := "testdata/xlsx/minimal-workbook/workbook.xlsx"
	docxFixture := "testdata/docx/minimal/document.docx"

	actual := rustPortContractGolden{
		SchemaVersion:           "1.0",
		ReferenceImplementation: "go-ooxml-cli-current",
		Generator:               "UPDATE_GOLDENS=1 go test ./internal/cli -run TestRustPortContractGolden -count=1",
		Fixtures:                []string{pptxFixture, xlsxFixture, docxFixture},
		CLI: []rustPortCLICommandGolden{
			runContractBinary(t, repoRoot, "version-json", nil, "--json", "version"),
			runContractBinary(t, repoRoot, "inspect-pptx-json", nil, "--json", "inspect", pptxFixture),
			runContractBinary(t, repoRoot, "pptx-slide-show-json", nil, "--json", "pptx", "slides", "show", pptxFixture, "--slide", "1", "--include-text"),
			runContractBinary(t, repoRoot, "xlsx-range-export-json", nil, "--json", "xlsx", "ranges", "export", xlsxFixture, "--sheet", "1", "--range", "A1:B2", "--include-types"),
			runContractBinary(t, repoRoot, "docx-text-json", nil, "--json", "docx", "text", docxFixture),
			runContractBinary(t, repoRoot, "invalid-slide-json-error", nil, "--json", "pptx", "slides", "show", pptxFixture, "--slide", "99"),
		},
		Mutation: buildRustPortMutationGolden(t, repoRoot, pptxFixture),
		Serve:    buildRustPortServeGolden(t),
		MCP:      buildRustPortMCPGolden(t, repoRoot),
		WebSmoke: buildRustPortWebSmokeGolden(t, repoRoot),
		Coverage: rustPortCoverageRows(),
	}

	assertGoldenJSONValue(t, "rust-port-contract/baseline.json", actual)
}

func runContractBinary(t *testing.T, repoRoot, name string, replacements map[string]string, args ...string) rustPortCLICommandGolden {
	t.Helper()
	cmd := exec.Command(serveBinary, args...)
	cmd.Dir = repoRoot
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	exitCode := ExitSuccess
	if err != nil {
		var exitErr *exec.ExitError
		if errors.As(err, &exitErr) {
			exitCode = exitErr.ExitCode()
		} else {
			exitCode = -1
		}
	}
	stdoutJSON, stdoutText := decodeContractStream(t, stdout.String(), replacements)
	stderrJSON, stderrText := decodeContractStream(t, stderr.String(), replacements)
	return rustPortCLICommandGolden{
		Name:       name,
		Args:       scrubContractArgs(args, replacements),
		ExitCode:   exitCode,
		StdoutJSON: stdoutJSON,
		StdoutText: stdoutText,
		StderrJSON: stderrJSON,
		StderrText: stderrText,
	}
}

func buildRustPortMutationGolden(t *testing.T, repoRoot, pptxFixture string) rustPortMutationGolden {
	t.Helper()
	editedPath := filepath.Join(t.TempDir(), "edited.pptx")
	baseAbs := filepath.Join(repoRoot, pptxFixture)
	renderDir := filepath.Join(t.TempDir(), "rendered")
	replacements := map[string]string{
		editedPath: "[EDITED_PPTX]",
		renderDir:  "[RENDER_DIR]",
		baseAbs:    pptxFixture,
	}

	edit := runContractBinary(t, repoRoot, "pptx-replace-text", replacements,
		"--json", "pptx", "replace", "text", pptxFixture,
		"--slide", "1",
		"--target", "title",
		"--text", "Rust Port Contract",
		"--out", editedPath,
	)
	requireContractExit(t, edit, ExitSuccess)

	validate := runContractBinary(t, repoRoot, "validate-edited-pptx-strict", replacements,
		"--json", "--strict", "validate", editedPath,
	)
	requireContractExit(t, validate, ExitSuccess)

	render := runMockedRenderContract(t, editedPath, renderDir, replacements)
	requireContractExit(t, render, ExitSuccess)

	verify := runMockedVerifyContract(t, editedPath, baseAbs, replacements)
	requireContractExit(t, verify, ExitSuccess)

	return rustPortMutationGolden{
		EditedFilePublished: fileExists(editedPath),
		Edit:                edit,
		Validate:            validate,
		Render:              render,
		Verify:              verify,
	}
}

func runMockedRenderContract(t *testing.T, editedPath, renderDir string, replacements map[string]string) rustPortCLICommandGolden {
	t.Helper()
	resetRustPortContractCommandState()
	origRender := renderToPDFFn
	origRaster := rasterizeFn
	t.Cleanup(func() {
		renderToPDFFn = origRender
		rasterizeFn = origRaster
	})
	renderToPDFFn = func(string, string) (string, error) {
		if err := os.MkdirAll(renderDir, 0o755); err != nil {
			return "", err
		}
		pdfPath := filepath.Join(renderDir, "edited.pdf")
		if err := os.WriteFile(pdfPath, []byte("pdf"), 0o644); err != nil {
			return "", err
		}
		return pdfPath, nil
	}
	rasterizeFn = func(_ string, outDir string, opts pkgrender.RasterizeOptions) ([]string, error) {
		imagePath := filepath.Join(outDir, "slide-1.png")
		if err := os.WriteFile(imagePath, []byte("png"), 0o644); err != nil {
			return nil, err
		}
		if len(opts.Pages) != 1 || opts.Pages[0] != 1 {
			t.Fatalf("render pages = %v, want [1]", opts.Pages)
		}
		return []string{imagePath}, nil
	}

	renderThumbnails = false
	renderThumbDPI = 96
	cmd := newTestRootCmd(t)
	args := []string{"pptx", "render", editedPath, "--out", renderDir, "--slides", "1", "--format", "json"}
	cmd.SetArgs(args)
	var stdout, stderr bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetErr(&stderr)
	err := cmd.Execute()
	exitCode := cliExitCode(err)
	stdoutJSON, stdoutText := decodeContractStream(t, stdout.String(), replacements)
	stderrJSON, stderrText := decodeContractStream(t, stderr.String(), replacements)
	return rustPortCLICommandGolden{
		Name:       "render-edited-pptx-mocked",
		Args:       scrubContractArgs(args, replacements),
		ExitCode:   exitCode,
		StdoutJSON: stdoutJSON,
		StdoutText: stdoutText,
		StderrJSON: stderrJSON,
		StderrText: stderrText,
	}
}

func runMockedVerifyContract(t *testing.T, editedPath, baseAbs string, replacements map[string]string) rustPortCLICommandGolden {
	t.Helper()
	resetRustPortContractCommandState()
	resetVerifyFlags()
	resetFamilyDiffFlags()
	origRender := renderToPDFFn
	t.Cleanup(func() {
		resetVerifyFlags()
		resetFamilyDiffFlags()
		renderToPDFFn = origRender
	})
	renderToPDFFn = func(string, string) (string, error) {
		return "", &pkgrender.MissingDependencyError{Tool: "soffice"}
	}

	cmd := newTestRootCmd(t)
	args := []string{"--format", "json", "verify", editedPath, "--baseline", baseAbs}
	cmd.SetArgs(args)
	var stdout, stderr bytes.Buffer
	cmd.SetOut(&stdout)
	cmd.SetErr(&stderr)
	err := cmd.Execute()
	exitCode := cliExitCode(err)
	stdoutJSON, stdoutText := decodeContractStream(t, stdout.String(), replacements)
	stderrJSON, stderrText := decodeContractStream(t, stderr.String(), replacements)
	return rustPortCLICommandGolden{
		Name:       "verify-edited-pptx-with-render-gate",
		Args:       scrubContractArgs(args, replacements),
		ExitCode:   exitCode,
		StdoutJSON: stdoutJSON,
		StdoutText: stdoutText,
		StderrJSON: stderrJSON,
		StderrText: stderrText,
	}
}

func buildRustPortServeGolden(t *testing.T) rustPortProtocolGolden {
	t.Helper()
	resetRustPortContractCommandState()
	resetFlags()
	input := stageServeXLSX(t)
	outPath := filepath.Join(t.TempDir(), "serve-out.xlsx")
	replacements := map[string]string{
		input:   "[SERVE_INPUT_XLSX]",
		outPath: "[SERVE_OUT_XLSX]",
	}
	c := newRPCConn(t)
	flow := make([]rustPortRPCExchange, 0, 8)
	record := func(method string, params map[string]interface{}) rpcResponse {
		t.Helper()
		requestID := c.id + 1
		resp := c.call(method, params)
		flow = append(flow, rustPortRPCExchange{
			Method: method,
			Request: scrubContractValue(map[string]any{
				"jsonrpc": "2.0",
				"id":      requestID,
				"method":  method,
				"params":  params,
			}, replacements),
			Response: scrubRPCResponse(t, resp, replacements),
		})
		return resp
	}

	openResp := record("open", map[string]interface{}{"file": input, "out": outPath})
	session := sessionIDFromRPCResult(t, openResp)
	replacements[session] = "[SESSION]"
	flow[len(flow)-1].Response = scrubRPCResponse(t, openResp, replacements)

	record("op", map[string]interface{}{"session": session, "command": "xlsx cells set", "args": map[string]interface{}{"sheet": "1", "cell": "A1", "value": "serve-contract"}})
	record("inspect", map[string]interface{}{"session": session, "command": "xlsx ranges export", "args": map[string]interface{}{"sheet": "1", "range": "A1", "include-types": true}})
	record("validate", map[string]interface{}{"session": session})
	record("plan", map[string]interface{}{"session": session})
	record("commit", map[string]interface{}{"session": session})
	if got := readCellViaBinary(t, outPath, "1", "A1"); got != "serve-contract" {
		t.Fatalf("serve committed A1 = %q, want serve-contract", got)
	}

	dryResp := record("open", map[string]interface{}{"file": input, "dryRun": true})
	drySession := sessionIDFromRPCResult(t, dryResp)
	replacements[drySession] = "[DRY_RUN_SESSION]"
	flow[len(flow)-1].Response = scrubRPCResponse(t, dryResp, replacements)
	record("abort", map[string]interface{}{"session": drySession})

	return rustPortProtocolGolden{Transport: "json-rpc-2.0-stdio", Flow: flow}
}

func buildRustPortMCPGolden(t *testing.T, repoRoot string) rustPortMCPGolden {
	t.Helper()
	resetRustPortContractCommandState()
	resetFlags()
	c := newMCPConn(t)
	discovery := collectRustPortMCPDiscovery(t, c)

	input := mcpStageXLSX(t)
	outPath := filepath.Join(t.TempDir(), "mcp-out.xlsx")
	replacements := map[string]string{
		input:   "[MCP_INPUT_XLSX]",
		outPath: "[MCP_OUT_XLSX]",
	}
	flow := make([]rustPortRPCExchange, 0, 8)
	recordTool := func(name string, args map[string]interface{}) rpcResponse {
		t.Helper()
		params := map[string]interface{}{"name": name, "arguments": args}
		requestID := c.id + 1
		resp := c.call("tools/call", params)
		flow = append(flow, rustPortRPCExchange{
			Method: "tools/call",
			Request: scrubContractValue(map[string]any{
				"jsonrpc": "2.0",
				"id":      requestID,
				"method":  "tools/call",
				"params":  params,
			}, replacements),
			Response: scrubRPCResponse(t, resp, replacements),
		})
		return resp
	}

	openResp := recordTool("open", map[string]interface{}{"file": input, "out": outPath})
	session := sessionIDFromMCPToolResult(t, openResp)
	replacements[session] = "[MCP_SESSION]"
	flow[len(flow)-1].Response = scrubRPCResponse(t, openResp, replacements)
	recordTool("op", map[string]interface{}{"session": session, "command": "xlsx cells set", "args": map[string]interface{}{"sheet": "1", "cell": "A1", "value": "mcp-contract"}})
	recordTool("inspect", map[string]interface{}{"session": session, "command": "xlsx ranges export", "args": map[string]interface{}{"sheet": "1", "range": "A1", "include-types": true}})
	recordTool("validate", map[string]interface{}{"session": session})
	recordTool("plan", map[string]interface{}{"session": session})
	recordTool("commit", map[string]interface{}{"session": session})
	if got := readCellViaBinary(t, outPath, "1", "A1"); got != "mcp-contract" {
		t.Fatalf("mcp committed A1 = %q, want mcp-contract", got)
	}

	dryResp := recordTool("open", map[string]interface{}{"file": input, "dryRun": true})
	drySession := sessionIDFromMCPToolResult(t, dryResp)
	replacements[drySession] = "[MCP_DRY_RUN_SESSION]"
	flow[len(flow)-1].Response = scrubRPCResponse(t, dryResp, replacements)
	recordTool("abort", map[string]interface{}{"session": drySession})

	_ = repoRoot
	return rustPortMCPGolden{
		Discovery: discovery,
		Flow:      rustPortProtocolGolden{Transport: "mcp-json-rpc-2.0-stdio", Flow: flow},
	}
}

func collectRustPortMCPDiscovery(t *testing.T, c *mcpConn) rustPortMCPDiscoveryGolden {
	t.Helper()
	initialize := c.mustResult("initialize", map[string]interface{}{
		"protocolVersion": "2025-06-18",
		"clientInfo":      map[string]interface{}{"name": "rust-port-contract", "version": "1"},
	})
	var initValue any
	if err := json.Unmarshal(initialize, &initValue); err != nil {
		t.Fatalf("decode MCP initialize: %v", err)
	}

	toolsRaw := c.mustResult("tools/list", nil)
	var tools mcpToolsListResult
	if err := json.Unmarshal(toolsRaw, &tools); err != nil {
		t.Fatalf("decode tools/list: %v", err)
	}
	toolSummaries := make([]rustPortMCPToolGolden, 0, len(tools.Tools))
	for _, tool := range tools.Tools {
		var schema struct {
			Properties           map[string]any `json:"properties"`
			Required             []string       `json:"required"`
			AdditionalProperties bool           `json:"additionalProperties"`
		}
		if err := json.Unmarshal(tool.InputSchema, &schema); err != nil {
			t.Fatalf("decode input schema for %s: %v", tool.Name, err)
		}
		toolSummaries = append(toolSummaries, rustPortMCPToolGolden{
			Name:                 tool.Name,
			Properties:           sortedMapKeys(schema.Properties),
			Required:             sortedCopy(schema.Required),
			AdditionalProperties: schema.AdditionalProperties,
		})
	}
	sort.Slice(toolSummaries, func(i, j int) bool { return toolSummaries[i].Name < toolSummaries[j].Name })

	resourcesRaw := c.mustResult("resources/list", nil)
	var resources mcpResourcesListResult
	if err := json.Unmarshal(resourcesRaw, &resources); err != nil {
		t.Fatalf("decode resources/list: %v", err)
	}
	sort.Slice(resources.Resources, func(i, j int) bool { return resources.Resources[i].URI < resources.Resources[j].URI })

	templatesRaw := c.mustResult("resources/templates/list", nil)
	var templates mcpResourceTemplatesListResult
	if err := json.Unmarshal(templatesRaw, &templates); err != nil {
		t.Fatalf("decode resources/templates/list: %v", err)
	}
	sort.Slice(templates.ResourceTemplates, func(i, j int) bool {
		return templates.ResourceTemplates[i].URITemplate < templates.ResourceTemplates[j].URITemplate
	})

	commandURI := "resource://command/" + url.PathEscape("xlsx cells set")
	commandRaw := c.mustResult("resources/read", map[string]interface{}{"uri": commandURI})
	var commandRead mcpResourcesReadResult
	if err := json.Unmarshal(commandRaw, &commandRead); err != nil {
		t.Fatalf("decode command resource read: %v", err)
	}
	if len(commandRead.Contents) != 1 {
		t.Fatalf("command resource returned %d contents, want 1", len(commandRead.Contents))
	}
	var command capabilityCommand
	if err := json.Unmarshal([]byte(commandRead.Contents[0].Text), &command); err != nil {
		t.Fatalf("decode command resource body: %v", err)
	}

	return rustPortMCPDiscoveryGolden{
		Initialize:        scrubContractValue(initValue, nil),
		Tools:             toolSummaries,
		Resources:         resources.Resources,
		ResourceTemplates: templates.ResourceTemplates,
		CommandResource: rustPortCommandResourceGolden{
			URI:          commandURI,
			Path:         command.Path,
			OpCompatible: command.OpCompatible,
			Flags:        capabilityFlagNames(command.LocalFlags),
			ArgNames:     capabilityArgNames(command.LocalFlags),
		},
	}
}

func buildRustPortWebSmokeGolden(t *testing.T, repoRoot string) rustPortWebSmokeGolden {
	t.Helper()
	packageJSONPath := filepath.Join(repoRoot, "web", "package.json")
	packageData, err := os.ReadFile(packageJSONPath)
	if err != nil {
		t.Fatalf("read web package.json: %v", err)
	}
	var packageJSON struct {
		Scripts map[string]string `json:"scripts"`
	}
	if err := json.Unmarshal(packageData, &packageJSON); err != nil {
		t.Fatalf("decode web package.json: %v", err)
	}
	scripts := map[string]string{
		"build":         packageJSON.Scripts["build"],
		"smoke:agent":   packageJSON.Scripts["smoke:agent"],
		"smoke:nonpptx": packageJSON.Scripts["smoke:nonpptx"],
		"typecheck":     packageJSON.Scripts["typecheck"],
	}

	agentScript := filepath.Join(repoRoot, "web", "scripts", "smoke-agent-edit.mjs")
	nonPPTXScript := filepath.Join(repoRoot, "web", "scripts", "smoke-nonpptx.mjs")
	agentSource := readTextFile(t, agentScript)
	nonPPTXSource := readTextFile(t, nonPPTXScript)
	for path, source := range map[string]string{agentScript: agentSource, nonPPTXScript: nonPPTXSource} {
		if strings.Contains(source, "../../../testdata/") {
			t.Fatalf("%s points outside repo-local testdata", path)
		}
		if !strings.Contains(source, "process.env.OOXML_BIN") || !strings.Contains(source, "execFileAsync(ooxmlBin") {
			t.Fatalf("%s does not route validation/readback through OOXML_BIN", path)
		}
	}
	if !strings.Contains(agentSource, "ooxmlBin,") {
		t.Fatalf("agent smoke summary must include selected OOXML_BIN")
	}

	fixtures := map[string]string{
		"agent": filepath.Join(repoRoot, "testdata", "pptx", "minimal-title", "presentation.pptx"),
		"docx":  filepath.Join(repoRoot, "testdata", "docx", "minimal", "document.docx"),
		"xlsx":  filepath.Join(repoRoot, "testdata", "xlsx", "minimal-workbook", "workbook.xlsx"),
	}
	for name, path := range fixtures {
		if _, err := os.Stat(path); err != nil {
			t.Fatalf("%s smoke fixture missing at %s: %v", name, path, err)
		}
	}

	return rustPortWebSmokeGolden{
		PackageScripts:       scripts,
		AgentScript:          "web/scripts/smoke-agent-edit.mjs",
		NonPPTXScript:        "web/scripts/smoke-nonpptx.mjs",
		BinaryEnv:            "OOXML_BIN",
		SummaryIncludesBin:   true,
		AgentDefaultFixture:  "testdata/pptx/minimal-title/presentation.pptx",
		DOCXDefaultFixture:   "testdata/docx/minimal/document.docx",
		XLSXDefaultFixture:   "testdata/xlsx/minimal-workbook/workbook.xlsx",
		BinaryReadbackChecks: []string{"validate --strict", "pptx slides show", "docx text", "xlsx sheets list"},
	}
}

func rustPortCoverageRows() []rustPortCoverageGolden {
	return []rustPortCoverageGolden{
		{Surface: "cli", Requirement: "stdout/stderr/exit codes for success and JSON errors", Evidence: "CLI binary cases in rust-port-contract/baseline.json", Status: "frozen"},
		{Surface: "mutation", Requirement: "PPTX edit publishes output and strict validation passes", Evidence: "pptx replace text plus validate --strict", Status: "frozen"},
		{Surface: "render", Requirement: "PPTX render command emits manifest deterministically", Evidence: "mocked pptx render on edited output", Status: "frozen"},
		{Surface: "verify", Requirement: "validation/render/diff envelope is stable when render dependency is unavailable", Evidence: "verify --baseline with mocked missing soffice", Status: "frozen"},
		{Surface: "serve", Requirement: "JSON-RPC open/op/inspect/validate/plan/commit/abort envelopes", Evidence: "scrubbed serve flow", Status: "frozen"},
		{Surface: "mcp", Requirement: "MCP initialize/tool/resource discovery and session tools", Evidence: "scrubbed MCP discovery and tools/call flow", Status: "frozen"},
		{Surface: "web", Requirement: "web smoke scripts use repo-selected binary through OOXML_BIN", Evidence: "script/package contract checks", Status: "frozen"},
	}
}

func requireContractExit(t *testing.T, result rustPortCLICommandGolden, want int) {
	t.Helper()
	if result.ExitCode != want {
		t.Fatalf("%s exitCode = %d, want %d\nstdout=%v\nstderr=%v%s%s", result.Name, result.ExitCode, want, result.StdoutJSON, result.StderrJSON, result.StdoutText, result.StderrText)
	}
}

func cliExitCode(err error) int {
	if err == nil {
		return ExitSuccess
	}
	if cliErr, ok := AsCLIError(err); ok {
		return cliErr.ExitCode
	}
	return -1
}

func decodeContractStream(t *testing.T, data string, replacements map[string]string) (any, string) {
	t.Helper()
	trimmed := strings.TrimSpace(data)
	if trimmed == "" {
		return nil, ""
	}
	var value any
	if err := json.Unmarshal([]byte(trimmed), &value); err == nil {
		return scrubContractValue(value, replacements), ""
	}
	return nil, scrubContractString(strings.TrimRight(data, "\n"), replacements)
}

func scrubRPCResponse(t *testing.T, resp rpcResponse, replacements map[string]string) any {
	t.Helper()
	data, err := json.Marshal(resp)
	if err != nil {
		t.Fatalf("marshal rpc response: %v", err)
	}
	var value any
	if err := json.Unmarshal(data, &value); err != nil {
		t.Fatalf("decode rpc response for scrubbing: %v", err)
	}
	return scrubContractValue(value, replacements)
}

func scrubContractValue(value any, replacements map[string]string) any {
	switch v := value.(type) {
	case map[string]any:
		out := make(map[string]any, len(v))
		for key, item := range v {
			out[key] = scrubContractValue(item, replacements)
		}
		return out
	case []any:
		out := make([]any, len(v))
		for i, item := range v {
			out[i] = scrubContractValue(item, replacements)
		}
		return out
	case string:
		return scrubContractString(v, replacements)
	default:
		return value
	}
}

func scrubContractArgs(args []string, replacements map[string]string) []string {
	out := make([]string, len(args))
	for i, arg := range args {
		out[i] = scrubContractString(arg, replacements)
	}
	return out
}

func scrubContractString(value string, replacements map[string]string) string {
	out := filepath.ToSlash(value)
	for _, rule := range contractScrubPatternRules {
		out = rule.pattern.ReplaceAllString(out, rule.replacement)
	}
	keys := make([]string, 0, len(replacements))
	for key := range replacements {
		keys = append(keys, filepath.ToSlash(key))
	}
	sort.Slice(keys, func(i, j int) bool { return len(keys[i]) > len(keys[j]) })
	for _, key := range keys {
		out = strings.ReplaceAll(out, key, replacements[key])
	}
	return out
}

var contractScrubPatternRules = []struct {
	pattern     *regexp.Regexp
	replacement string
}{
	{
		pattern:     regexp.MustCompile(`/tmp/[^\s"']*ooxml-serve-[^\s"']*/working-\d+\.(pptx|pptm|xlsx|xlsm|docx|docm)`),
		replacement: "[SESSION_WORKING_PACKAGE]",
	},
}

func sessionIDFromRPCResult(t *testing.T, resp rpcResponse) string {
	t.Helper()
	raw, err := json.Marshal(resp.Result)
	if err != nil {
		t.Fatalf("marshal open result: %v", err)
	}
	var body struct {
		SessionID string `json:"sessionId"`
	}
	if err := json.Unmarshal(raw, &body); err != nil {
		t.Fatalf("decode open result: %v", err)
	}
	if body.SessionID == "" {
		t.Fatalf("open result missing sessionId: %s", raw)
	}
	return body.SessionID
}

func sessionIDFromMCPToolResult(t *testing.T, resp rpcResponse) string {
	t.Helper()
	raw, err := json.Marshal(resp.Result)
	if err != nil {
		t.Fatalf("marshal MCP tool result: %v", err)
	}
	var toolResult mcpCallToolResult
	if err := json.Unmarshal(raw, &toolResult); err != nil {
		t.Fatalf("decode MCP tool result: %v", err)
	}
	if toolResult.IsError {
		t.Fatalf("MCP open returned isError: %s", toolResult.StructuredContent)
	}
	var body struct {
		SessionID string `json:"sessionId"`
	}
	if err := json.Unmarshal(toolResult.StructuredContent, &body); err != nil {
		t.Fatalf("decode MCP open structuredContent: %v", err)
	}
	if body.SessionID == "" {
		t.Fatalf("MCP open result missing sessionId: %s", toolResult.StructuredContent)
	}
	return body.SessionID
}

func capabilityArgNames(flags []capabilityFlag) []string {
	names := make([]string, 0, len(flags))
	for _, flag := range flags {
		if flag.ArgName == "" || flag.ArgName == "help" {
			continue
		}
		names = append(names, flag.ArgName)
	}
	sort.Strings(names)
	return names
}

func sortedMapKeys(values map[string]any) []string {
	keys := make([]string, 0, len(values))
	for key := range values {
		keys = append(keys, key)
	}
	sort.Strings(keys)
	return keys
}

func readTextFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return string(data)
}

func resetRustPortContractCommandState() {
	cmd := GetRootCmd()
	resetFlagsRecursive(cmd)
	resetFlags()
	resetTestGlobals()
	resetVerifyFlags()
	resetFamilyDiffFlags()
	resetCommandContexts(cmd)
	globalConfig = nil
}

func resetCommandContexts(cmd *cobra.Command) {
	cmd.SetContext(context.Background())
	for _, child := range cmd.Commands() {
		resetCommandContexts(child)
	}
}
