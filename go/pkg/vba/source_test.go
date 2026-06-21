package vba

import (
	"bytes"
	"encoding/binary"
	"strings"
	"testing"
	"unicode/utf16"

	"github.com/ooxml-cli/ooxml-cli/pkg/vba/cfb"
)

func TestParseSourceProjectSyntheticModules(t *testing.T) {
	project, err := ParseSourceProject(syntheticVBAProjectBinForTest(t))
	if err != nil {
		t.Fatalf("ParseSourceProject failed: %v", err)
	}
	if project.CodePage != 1252 || project.ModuleCount != 2 || len(project.Modules) != 2 {
		t.Fatalf("unexpected project metadata: %+v", project)
	}
	standard := project.Modules[0]
	if standard.Name != "Module1" || standard.StreamName != "Module1" || standard.Kind != "standard" || standard.Extension != ".bas" {
		t.Fatalf("unexpected standard module: %+v", standard)
	}
	if !strings.Contains(standard.Source, "Public Sub HelloWorld()") || standard.LineCount != 3 || standard.SHA256 == "" {
		t.Fatalf("unexpected standard module source: %+v", standard)
	}
	if standard.LineEnding != "crlf" || !standard.TrailingNewline || standard.SHA256Basis != "decoded-source-utf8" {
		t.Fatalf("unexpected standard source metadata: %+v", standard)
	}
	if !containsVBASelectorForTest(standard.Selectors, "module:Module1") || standard.PrimarySelector != "module:Module1" {
		t.Fatalf("missing selectors: %+v", standard)
	}
	classModule := project.Modules[1]
	if classModule.Name != "Class1" || classModule.Kind != "class" || classModule.Extension != ".cls" {
		t.Fatalf("unexpected class module: %+v", classModule)
	}
	if !strings.Contains(classModule.Source, "Public Function Answer()") {
		t.Fatalf("unexpected class source: %q", classModule.Source)
	}
}

func TestSourceLineEndingStyle(t *testing.T) {
	tests := []struct {
		name   string
		source string
		want   string
	}{
		{name: "none", source: "Attribute VB_Name = \"Module1\"", want: "none"},
		{name: "crlf", source: "A\r\nB\r\n", want: "crlf"},
		{name: "lf", source: "A\nB\n", want: "lf"},
		{name: "cr", source: "A\rB\r", want: "cr"},
		{name: "mixed", source: "A\r\nB\nC\r", want: "mixed"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := SourceLineEndingStyle(tt.source); got != tt.want {
				t.Fatalf("SourceLineEndingStyle(%q) = %q, want %q", tt.source, got, tt.want)
			}
		})
	}
}

func TestParseSourceProjectIncludesProjectMetadata(t *testing.T) {
	project, err := ParseSourceProject(syntheticVBAProjectBinWithProjectStreamForTest(t))
	if err != nil {
		t.Fatalf("ParseSourceProject failed: %v", err)
	}
	metadata := project.ProjectMetadata
	if metadata == nil || !metadata.Present || metadata.StreamName != "PROJECT" {
		t.Fatalf("missing project metadata: %+v", metadata)
	}
	if metadata.ID != "{00000000-0000-0000-0000-000000000000}" {
		t.Fatalf("unexpected project ID: %+v", metadata)
	}
	if len(metadata.Modules) != 2 || metadata.Modules[0].Kind != "module" || metadata.Modules[0].Name != "Module1" || metadata.Modules[1].Kind != "class" || metadata.Modules[1].Name != "Class1" {
		t.Fatalf("unexpected project module declarations: %+v", metadata.Modules)
	}
	if len(metadata.References) != 1 || metadata.References[0].Kind != "reference" || !strings.Contains(metadata.References[0].Value, "stdole") {
		t.Fatalf("unexpected project references: %+v", metadata.References)
	}
	if len(metadata.WorkspaceEntries) != 2 || metadata.WorkspaceEntries[0].Name != "Module1" {
		t.Fatalf("unexpected workspace entries: %+v", metadata.WorkspaceEntries)
	}
}

func TestHostCompatibilityWarnings(t *testing.T) {
	tests := []struct {
		name     string
		project  *SourceProject
		wantCode string
	}{
		{
			name: "xlsx accepts normal Excel document modules",
			project: &SourceProject{Family: "xlsx", Modules: []SourceModule{
				{Name: "ThisWorkbook", Kind: "class", Extension: ".cls"},
				{Name: "Sheet1", Kind: "class", Extension: ".cls"},
				{Name: "Module1", Kind: "standard", Extension: ".bas"},
			}},
		},
		{
			name: "pptx warns on Excel document modules",
			project: &SourceProject{Family: "pptx", Modules: []SourceModule{
				{Name: "ThisWorkbook", Kind: "class", Extension: ".cls"},
				{Name: "Sheet1", Kind: "class", Extension: ".cls"},
				{Name: "Module1", Kind: "standard", Extension: ".bas"},
			}},
			wantCode: "VBA_HOST_EXCEL_MODULES_IN_PPTM",
		},
		{
			name: "xlsx warns on PowerPoint document-like modules",
			project: &SourceProject{Family: "xlsx", Modules: []SourceModule{
				{Name: "ThisPresentation", Kind: "class", Extension: ".cls"},
				{Name: "Slide1", Kind: "class", Extension: ".cls"},
				{Name: "Module1", Kind: "standard", Extension: ".bas"},
			}},
			wantCode: "VBA_HOST_POWERPOINT_MODULES_IN_XLSM",
		},
		{
			name: "pptx accepts ordinary standard and class modules",
			project: &SourceProject{Family: "pptx", Modules: []SourceModule{
				{Name: "DeckAutomation", Kind: "standard", Extension: ".bas"},
				{Name: "ClientState", Kind: "class", Extension: ".cls"},
			}},
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			warnings := HostCompatibilityWarnings(tc.project)
			if tc.wantCode == "" {
				if len(warnings) != 0 {
					t.Fatalf("expected no warnings, got %+v", warnings)
				}
				return
			}
			if len(warnings) != 1 || warnings[0].Code != tc.wantCode || warnings[0].Message == "" || len(warnings[0].Modules) == 0 {
				t.Fatalf("unexpected warnings: %+v", warnings)
			}
		})
	}
}

func TestParseDirStreamSkipsRealOfficeReferenceRecords(t *testing.T) {
	modules := []syntheticVBAModule{
		{Name: "ThisWorkbook", StreamName: "ThisWorkbook", Kind: "class"},
		{Name: "Sheet1", StreamName: "Sheet1", Kind: "class"},
		{Name: "modExporttoPPTX", StreamName: "modExporttoPPTX", Kind: "standard"},
	}
	dirData := syntheticDirStreamWithReferencesForTest(modules)

	codePage, parsed, warnings, err := parseDirStream(dirData)
	if err != nil {
		t.Fatalf("parseDirStream failed: %v", err)
	}
	if codePage != 1252 || len(warnings) != 0 {
		t.Fatalf("unexpected codepage/warnings: codePage=%d warnings=%v", codePage, warnings)
	}
	if len(parsed) != len(modules) {
		t.Fatalf("parsed %d modules, want %d: %+v", len(parsed), len(modules), parsed)
	}
	if parsed[0].Name != "ThisWorkbook" || parsed[0].Kind != "class" || parsed[2].Name != "modExporttoPPTX" || parsed[2].Kind != "standard" {
		t.Fatalf("unexpected parsed modules: %+v", parsed)
	}
}

func TestDecompressContainerLiteralChunks(t *testing.T) {
	raw := []byte(strings.Repeat("abcdefghi", 600))
	compressed := compressedLiteralsForTest(raw)
	got, err := DecompressContainer(compressed)
	if err != nil {
		t.Fatalf("DecompressContainer failed: %v", err)
	}
	if !bytes.Equal(got, raw) {
		t.Fatalf("decompressed data mismatch: got %d bytes, want %d", len(got), len(raw))
	}
}

func TestCompressContainerLiteralsRoundTrip(t *testing.T) {
	for _, raw := range [][]byte{
		[]byte("Attribute VB_Name = \"Module1\"\r\nPublic Sub X()\r\nEnd Sub\r\n"),
		[]byte(strings.Repeat("a", 4096)),
		[]byte(strings.Repeat("abcdefghi", 900)),
	} {
		compressed := CompressContainerLiterals(raw)
		got, err := DecompressContainer(compressed)
		if err != nil {
			t.Fatalf("DecompressContainer(CompressContainerLiterals(%d bytes)) failed: %v", len(raw), err)
		}
		if !bytes.Equal(got, raw) {
			t.Fatalf("round trip mismatch for %d bytes", len(raw))
		}
	}
}

func TestReplaceModuleSourceInProjectDataSynthetic(t *testing.T) {
	projectData := syntheticVBAProjectBinForTest(t)
	replacementSource := []byte("Attribute VB_Name = \"Module1\"\nPublic Sub Replaced()\nDebug.Print \"ok\"\nEnd Sub")

	result, rewritten, err := ReplaceModuleSourceInProjectData(projectData, "module:Module1", replacementSource, "", SourceMutationOptions{})
	if err != nil {
		t.Fatalf("ReplaceModuleSourceInProjectData failed: %v", err)
	}
	if result.Action != "replace-module" || result.Module.Name != "Module1" || result.PreviousSHA256 == "" || result.SHA256 == "" || result.PreviousSHA256 == result.SHA256 {
		t.Fatalf("unexpected replacement result: %+v", result)
	}
	if !result.PurgedCaches || !result.RecompilesOnOpen {
		t.Fatalf("expected purge metadata, got %+v", result)
	}
	if !containsWarningForTest(result.Warnings, "CRLF") || !containsWarningForTest(result.Warnings, "MODULEOFFSET 0") {
		t.Fatalf("expected CRLF and offset warnings, got %+v", result.Warnings)
	}

	project, err := ParseSourceProject(rewritten)
	if err != nil {
		t.Fatalf("ParseSourceProject(rewritten) failed: %v", err)
	}
	if !strings.Contains(project.Modules[0].Source, "Public Sub Replaced()") {
		t.Fatalf("Module1 was not replaced:\n%s", project.Modules[0].Source)
	}
	if !strings.Contains(project.Modules[1].Source, "Public Function Answer()") {
		t.Fatalf("Class1 should be unchanged:\n%s", project.Modules[1].Source)
	}
}

func TestReplaceModuleSourceRejectsVBNameMismatch(t *testing.T) {
	projectData := syntheticVBAProjectBinForTest(t)
	source := []byte("Attribute VB_Name = \"Other\"\r\nPublic Sub Replaced()\r\nEnd Sub\r\n")
	_, _, err := ReplaceModuleSourceInProjectData(projectData, "module:Module1", source, "", SourceMutationOptions{SourceKind: "bas"})
	if err == nil || !strings.Contains(err.Error(), "Attribute VB_Name") {
		t.Fatalf("expected VB_Name mismatch, got %v", err)
	}
}

func TestReplaceModuleSourceRejectsKindMismatch(t *testing.T) {
	projectData := syntheticVBAProjectBinForTest(t)
	source := []byte("Attribute VB_Name = \"Module1\"\r\nPublic Sub Replaced()\r\nEnd Sub\r\n")
	_, _, err := ReplaceModuleSourceInProjectData(projectData, "module:Module1", source, "", SourceMutationOptions{SourceKind: "cls"})
	if err == nil || !strings.Contains(err.Error(), "incompatible") {
		t.Fatalf("expected kind mismatch, got %v", err)
	}
}

func TestReplaceModuleSourceInProjectDataPurgesPerformanceCaches(t *testing.T) {
	projectData := syntheticVBAProjectBinWithPCodeForTest(t)
	replacementSource := []byte("Attribute VB_Name = \"Module1\"\r\nPublic Sub Replaced()\r\nEnd Sub\r\n")

	result, rewritten, err := ReplaceModuleSourceInProjectData(projectData, "module:Module1", replacementSource, "", SourceMutationOptions{AllowExperimentalSourceRewrite: true})
	if err != nil {
		t.Fatalf("ReplaceModuleSourceInProjectData failed: %v", err)
	}
	if !result.PurgedCaches || !containsWarningForTest(result.Warnings, "compiled cache") {
		t.Fatalf("expected compiled-cache purge result, got %+v", result)
	}

	rewrittenCFB, err := cfb.Open(rewritten)
	if err != nil {
		t.Fatalf("cfb.Open(rewritten) failed: %v", err)
	}
	if _, err := rewrittenCFB.Stream("VBA/__SRP_0"); err == nil {
		t.Fatal("expected __SRP_0 compiled cache stream to be removed")
	}
	module1Stream, err := rewrittenCFB.Stream("VBA/Module1")
	if err != nil {
		t.Fatalf("failed to read Module1 stream: %v", err)
	}
	if len(module1Stream) == 0 || module1Stream[0] != 0x01 {
		t.Fatalf("edited Module1 should start with compressed source, got %x", module1Stream[:minForTest(len(module1Stream), 8)])
	}
	class1Stream, err := rewrittenCFB.Stream("VBA/Class1")
	if err != nil {
		t.Fatalf("failed to read Class1 stream: %v", err)
	}
	if !strings.HasPrefix(string(class1Stream), "compiled-pcode-class1") {
		t.Fatalf("unchanged Class1 should preserve its performance-cache prefix, got %x", class1Stream[:minForTest(len(class1Stream), 22)])
	}

	project, err := ParseSourceProject(rewritten)
	if err != nil {
		t.Fatalf("ParseSourceProject(rewritten) failed: %v", err)
	}
	module1 := sourceModuleByNameForTest(t, project, "Module1")
	class1 := sourceModuleByNameForTest(t, project, "Class1")
	if module1.SourceOffset != 0 {
		t.Fatalf("edited module source offset = %d, want 0", module1.SourceOffset)
	}
	if want := uint32(len("compiled-pcode-class1")); class1.SourceOffset != want {
		t.Fatalf("unchanged class source offset = %d, want preserved offset %d", class1.SourceOffset, want)
	}
	if !strings.Contains(project.Modules[0].Source, "Public Sub Replaced()") {
		t.Fatalf("Module1 was not replaced:\n%s", project.Modules[0].Source)
	}
	if !strings.Contains(project.Modules[1].Source, "Public Function Answer()") {
		t.Fatalf("Class1 should be unchanged:\n%s", project.Modules[1].Source)
	}
}

func TestReplaceModuleSourceInProjectDataRejectsHashGuard(t *testing.T) {
	projectData := syntheticVBAProjectBinForTest(t)
	_, _, err := ReplaceModuleSourceInProjectData(projectData, "Module1", []byte("Attribute VB_Name = \"Module1\"\r\n"), "sha256:deadbeef", SourceMutationOptions{})
	if err == nil || !strings.Contains(err.Error(), "source hash mismatch") {
		t.Fatalf("expected source hash mismatch, got %v", err)
	}
}

func TestReplaceModuleSourceNoopPreservesVBAProjectBinBytes(t *testing.T) {
	projectData := syntheticVBAProjectBinWithPCodeForTest(t)
	project, err := ParseSourceProject(projectData)
	if err != nil {
		t.Fatalf("ParseSourceProject failed: %v", err)
	}
	current := sourceModuleByNameForTest(t, project, "Module1")

	result, rewritten, err := ReplaceModuleSourceInProjectData(projectData, current.PrimarySelector, []byte(current.Source), current.SHA256, SourceMutationOptions{})
	if err != nil {
		t.Fatalf("no-op replace should not require experimental opt-in: %v", err)
	}
	if !bytes.Equal(rewritten, projectData) {
		t.Fatal("no-op replace changed raw vbaProject.bin bytes")
	}
	if result.PurgedCaches || result.RecompilesOnOpen || result.CompatibilityStatus != "unchanged" || result.SHA256 != current.SHA256 {
		t.Fatalf("unexpected no-op result: %+v", result)
	}
	if !containsWarningForTest(result.Warnings, "unchanged") {
		t.Fatalf("expected unchanged warning, got %+v", result.Warnings)
	}
}

func TestSourceMutationRefusesOfficeShapedProjectWithoutExperimentalFlag(t *testing.T) {
	projectData := syntheticVBAProjectBinWithPCodeForTest(t)
	project, err := ParseSourceProject(projectData)
	if err != nil {
		t.Fatalf("ParseSourceProject failed: %v", err)
	}
	current := sourceModuleByNameForTest(t, project, "Module1")
	replacement := []byte("Attribute VB_Name = \"Module1\"\r\nPublic Sub Changed()\r\nEnd Sub\r\n")
	if _, _, err := ReplaceModuleSourceInProjectData(projectData, current.PrimarySelector, replacement, current.SHA256, SourceMutationOptions{}); err == nil || !strings.Contains(err.Error(), "experimental VBA source rewrite refused") {
		t.Fatalf("expected replace refusal, got %v", err)
	}
	if _, _, err := AddModuleSourceInProjectData(projectData, []byte("Attribute VB_Name = \"Module2\"\r\nPublic Sub Added()\r\nEnd Sub\r\n"), AddModuleOptions{}); err == nil || !strings.Contains(err.Error(), "experimental VBA source rewrite refused") {
		t.Fatalf("expected add refusal, got %v", err)
	}
	if _, _, err := RemoveModuleSourceInProjectData(projectData, current.PrimarySelector, current.SHA256, SourceMutationOptions{}); err == nil || !strings.Contains(err.Error(), "experimental VBA source rewrite refused") {
		t.Fatalf("expected remove refusal, got %v", err)
	}
}

func TestModuleSetMutationRefusesVersionDependentProjectMetadata(t *testing.T) {
	projectData := syntheticVBAProjectBinWithVersionDependentProjectMetadataForTest(t)
	project, err := ParseSourceProject(projectData)
	if err != nil {
		t.Fatalf("ParseSourceProject failed: %v", err)
	}
	current := sourceModuleByNameForTest(t, project, "Module1")
	replacement := []byte("Attribute VB_Name = \"Module1\"\r\nPublic Sub Changed()\r\nEnd Sub\r\n")
	if _, _, err := ReplaceModuleSourceInProjectData(projectData, current.PrimarySelector, replacement, current.SHA256, SourceMutationOptions{AllowExperimentalSourceRewrite: true}); err != nil {
		t.Fatalf("replace should remain supported for version-dependent project metadata: %v", err)
	}
	if _, _, err := AddModuleSourceInProjectData(projectData, []byte("Attribute VB_Name = \"Module2\"\r\nPublic Sub Added()\r\nEnd Sub\r\n"), AddModuleOptions{AllowExperimentalSourceRewrite: true}); err == nil || !strings.Contains(err.Error(), "version-dependent _VBA_PROJECT metadata") {
		t.Fatalf("expected add refusal for version-dependent metadata, got %v", err)
	}
	if _, _, err := RemoveModuleSourceInProjectData(projectData, current.PrimarySelector, current.SHA256, SourceMutationOptions{AllowExperimentalSourceRewrite: true}); err == nil || !strings.Contains(err.Error(), "version-dependent _VBA_PROJECT metadata") {
		t.Fatalf("expected remove refusal for version-dependent metadata, got %v", err)
	}
}

func TestAddModuleSourceInProjectDataSyntheticStandard(t *testing.T) {
	projectData := syntheticVBAProjectBinWithProjectStreamForTest(t)
	source := []byte("Public Sub Added()\r\nDebug.Print \"added\"\r\nEnd Sub\r\n")

	result, rewritten, err := AddModuleSourceInProjectData(projectData, source, AddModuleOptions{
		Name:                           "Module2",
		Kind:                           "standard",
		ExpectModuleCount:              2,
		AllowExperimentalSourceRewrite: true,
	})
	if err != nil {
		t.Fatalf("AddModuleSourceInProjectData failed: %v", err)
	}
	if result.Action != "add-module" || result.Module.Name != "Module2" || result.Module.Kind != "standard" || result.PreviousCount != 2 || result.ModuleCount != 3 || result.SHA256 == "" {
		t.Fatalf("unexpected add result: %+v", result)
	}
	if !result.PurgedCaches || !result.RecompilesOnOpen {
		t.Fatalf("expected purge metadata, got %+v", result)
	}
	if !containsWarningForTest(result.Warnings, "Attribute VB_Name") || !containsWarningForTest(result.Warnings, "PROJECT stream") {
		t.Fatalf("expected attribute and PROJECT warnings, got %+v", result.Warnings)
	}

	updated, err := ParseSourceProject(rewritten)
	if err != nil {
		t.Fatalf("ParseSourceProject(rewritten) failed: %v", err)
	}
	if updated.ModuleCount != 3 || len(updated.Modules) != 3 {
		t.Fatalf("unexpected module count: %+v", updated)
	}
	added := updated.Modules[2]
	if added.Name != "Module2" || added.Kind != "standard" || added.SourceOffset != 0 || !strings.Contains(added.Source, "Public Sub Added()") || !strings.Contains(added.Source, "Attribute VB_Name = \"Module2\"") {
		t.Fatalf("unexpected added module: %+v source=%q", added, added.Source)
	}
	expectedOffsets := map[string]uint32{
		"Module1": uint32(len("compiled-pcode-module1")),
		"Class1":  uint32(len("compiled-pcode-class1")),
		"Module2": 0,
	}
	for _, module := range updated.Modules {
		if module.SourceOffset != expectedOffsets[module.Name] {
			t.Fatalf("module %s source offset = %d, want %d", module.Name, module.SourceOffset, expectedOffsets[module.Name])
		}
	}

	rewrittenCFB, err := cfb.Open(rewritten)
	if err != nil {
		t.Fatalf("cfb.Open(rewritten) failed: %v", err)
	}
	if _, err := rewrittenCFB.Stream("VBA/Module2"); err != nil {
		t.Fatalf("expected Module2 stream: %v", err)
	}
	projectStream, err := rewrittenCFB.Stream("PROJECT")
	if err != nil {
		t.Fatalf("failed to read PROJECT stream: %v", err)
	}
	if !strings.Contains(string(projectStream), "Module=Module2") || !strings.Contains(string(projectStream), "Module2=0, 0, 0, 0, C") {
		t.Fatalf("PROJECT stream missing Module2 metadata:\n%s", string(projectStream))
	}
}

func TestAddModuleSourceInProjectDataSyntheticClass(t *testing.T) {
	projectData := syntheticVBAProjectBinForTest(t)
	source := []byte("Attribute VB_Name = \"Class2\"\r\nPublic Function AddedClass()\r\nAddedClass = 7\r\nEnd Function\r\n")

	result, rewritten, err := AddModuleSourceInProjectData(projectData, source, AddModuleOptions{Kind: "class", ExpectModuleCount: 2})
	if err != nil {
		t.Fatalf("AddModuleSourceInProjectData class failed: %v", err)
	}
	if result.Module.Name != "Class2" || result.Module.Kind != "class" || result.Module.Extension != ".cls" {
		t.Fatalf("unexpected class add result: %+v", result)
	}
	updated, err := ParseSourceProject(rewritten)
	if err != nil {
		t.Fatalf("ParseSourceProject(rewritten) failed: %v", err)
	}
	added := updated.Modules[2]
	if added.Name != "Class2" || added.Kind != "class" || !strings.Contains(added.Source, "Public Function AddedClass()") {
		t.Fatalf("unexpected added class module: %+v source=%q", added, added.Source)
	}
}

func TestAddModuleSourceInProjectDataRejectsGuardsAndDuplicates(t *testing.T) {
	projectData := syntheticVBAProjectBinForTest(t)
	source := []byte("Attribute VB_Name = \"Module2\"\r\nPublic Sub Added()\r\nEnd Sub\r\n")
	if _, _, err := AddModuleSourceInProjectData(projectData, source, AddModuleOptions{ExpectModuleCount: 99}); err == nil || !strings.Contains(err.Error(), "module count mismatch") {
		t.Fatalf("expected module count mismatch, got %v", err)
	}
	if _, _, err := AddModuleSourceInProjectData(projectData, []byte("Attribute VB_Name = \"Module1\"\r\nPublic Sub Duplicate()\r\nEnd Sub\r\n"), AddModuleOptions{}); err == nil || !strings.Contains(err.Error(), "already exists") {
		t.Fatalf("expected duplicate module refusal, got %v", err)
	}
	if _, _, err := AddModuleSourceInProjectData(projectData, source, AddModuleOptions{Name: "Different"}); err == nil || !strings.Contains(err.Error(), "does not match") {
		t.Fatalf("expected VB_Name mismatch, got %v", err)
	}
}

func TestRemoveModuleSourceInProjectDataSynthetic(t *testing.T) {
	projectData := syntheticVBAProjectBinWithProjectStreamForTest(t)
	project, err := ParseSourceProject(projectData)
	if err != nil {
		t.Fatalf("ParseSourceProject failed: %v", err)
	}
	currentHash := project.Modules[0].SHA256

	result, rewritten, err := RemoveModuleSourceInProjectData(projectData, "module:Module1", currentHash, SourceMutationOptions{AllowExperimentalSourceRewrite: true})
	if err != nil {
		t.Fatalf("RemoveModuleSourceInProjectData failed: %v", err)
	}
	if result.Action != "remove-module" || result.Module.Name != "Module1" || result.PreviousSHA256 != currentHash || result.SHA256 != "" {
		t.Fatalf("unexpected remove result: %+v", result)
	}
	if !result.PurgedCaches || !result.RecompilesOnOpen {
		t.Fatalf("expected purge metadata, got %+v", result)
	}
	if !containsWarningForTest(result.Warnings, "PROJECT stream") || !containsWarningForTest(result.Warnings, "remaining module streams were preserved") {
		t.Fatalf("expected PROJECT cleanup and preservation warnings, got %+v", result.Warnings)
	}

	rewrittenCFB, err := cfb.Open(rewritten)
	if err != nil {
		t.Fatalf("cfb.Open(rewritten) failed: %v", err)
	}
	if _, err := rewrittenCFB.Stream("VBA/Module1"); err == nil {
		t.Fatal("expected Module1 stream to be removed")
	}
	if _, err := rewrittenCFB.Stream("VBA/__SRP_0"); err == nil {
		t.Fatal("expected compiled cache stream to be removed")
	}
	projectStream, err := rewrittenCFB.Stream("PROJECT")
	if err != nil {
		t.Fatalf("failed to read PROJECT stream: %v", err)
	}
	if strings.Contains(string(projectStream), "Module=Module1") || strings.Contains(string(projectStream), "Module1=") {
		t.Fatalf("PROJECT stream still references Module1:\n%s", string(projectStream))
	}

	updated, err := ParseSourceProject(rewritten)
	if err != nil {
		t.Fatalf("ParseSourceProject(rewritten) failed: %v", err)
	}
	if updated.ModuleCount != 1 || len(updated.Modules) != 1 || updated.Modules[0].Name != "Class1" {
		t.Fatalf("unexpected updated project: %+v", updated)
	}
	if want := uint32(len("compiled-pcode-class1")); updated.Modules[0].SourceOffset != want {
		t.Fatalf("remaining module source offset = %d, want preserved offset %d", updated.Modules[0].SourceOffset, want)
	}
	if !strings.Contains(updated.Modules[0].Source, "Public Function Answer()") {
		t.Fatalf("Class1 should remain extractable:\n%s", updated.Modules[0].Source)
	}
}

func TestRemoveModuleSourceInProjectDataRejectsHashGuard(t *testing.T) {
	projectData := syntheticVBAProjectBinForTest(t)
	_, _, err := RemoveModuleSourceInProjectData(projectData, "Module1", "sha256:deadbeef", SourceMutationOptions{})
	if err == nil || !strings.Contains(err.Error(), "source hash mismatch") {
		t.Fatalf("expected source hash mismatch, got %v", err)
	}
}

func TestRemoveModuleSourceInProjectDataRefusesLastModule(t *testing.T) {
	projectData := syntheticVBAProjectBinForModulesTest(t, []syntheticVBAModule{{
		Name:       "Module1",
		StreamName: "Module1",
		Kind:       "standard",
		Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub OnlyOne()\r\nEnd Sub\r\n",
	}}, nil, nil)
	_, _, err := RemoveModuleSourceInProjectData(projectData, "Module1", "", SourceMutationOptions{})
	if err == nil || !strings.Contains(err.Error(), "last VBA module") {
		t.Fatalf("expected last-module refusal, got %v", err)
	}
}

func TestRemoveModuleSourceInProjectDataRejectsSharedStream(t *testing.T) {
	projectData := syntheticVBAProjectBinForModulesTest(t, []syntheticVBAModule{
		{
			Name:       "Module1",
			StreamName: "Shared",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub One()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Module2",
			StreamName: "Shared",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module2\"\r\nPublic Sub Two()\r\nEnd Sub\r\n",
		},
	}, nil, map[string][]byte{
		"VBA/Shared": compressedLiteralsForTest([]byte("Attribute VB_Name = \"Module1\"\r\nPublic Sub Shared()\r\nEnd Sub\r\n")),
	})
	_, _, err := RemoveModuleSourceInProjectData(projectData, "Module1", "", SourceMutationOptions{})
	if err == nil || !strings.Contains(err.Error(), "stream") {
		t.Fatalf("expected shared-stream refusal, got %v", err)
	}
}

func TestRemoveModuleSourceInProjectDataRejectsUnmatchedProjectStream(t *testing.T) {
	modules := []syntheticVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
	}
	projectData := syntheticVBAProjectBinForModulesTest(t, modules, nil, map[string][]byte{
		"PROJECT": []byte("ID=\"{00000000-0000-0000-0000-000000000000}\"\r\nModule=OtherModule\r\n"),
	})
	_, _, err := RemoveModuleSourceInProjectData(projectData, "Module1", "", SourceMutationOptions{})
	if err == nil || !strings.Contains(err.Error(), "PROJECT stream") {
		t.Fatalf("expected unmatched PROJECT stream refusal, got %v", err)
	}
}

func TestParseSourceProjectSkipsProjectCookie(t *testing.T) {
	projectData := syntheticVBAProjectBinWithProjectCookieForTest(t)
	project, err := ParseSourceProject(projectData)
	if err != nil {
		t.Fatalf("ParseSourceProject failed: %v", err)
	}
	if project.ModuleCount != 2 || project.Modules[0].Name != "Module1" || project.Modules[1].Name != "Class1" {
		t.Fatalf("unexpected project parsed after PROJECTCOOKIE: %+v", project)
	}
	if _, _, err := RemoveModuleSourceInProjectData(projectData, "Module1", "", SourceMutationOptions{}); err != nil {
		t.Fatalf("RemoveModuleSourceInProjectData with PROJECTCOOKIE failed: %v", err)
	}
}

func syntheticVBAProjectBinWithPCodeForTest(t *testing.T) []byte {
	t.Helper()
	modules := []syntheticVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
	}
	prefixes := map[string][]byte{
		"Module1": []byte("compiled-pcode-module1"),
		"Class1":  []byte("compiled-pcode-class1"),
	}
	return syntheticVBAProjectBinForModulesTest(t, modules, prefixes, map[string][]byte{"VBA/__SRP_0": []byte("compiled cache")})
}

func syntheticVBAProjectBinForTest(t *testing.T) []byte {
	t.Helper()
	modules := []syntheticVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
	}
	return syntheticVBAProjectBinForModulesTest(t, modules, nil, nil)
}

func syntheticVBAProjectBinWithProjectCookieForTest(t *testing.T) []byte {
	t.Helper()
	modules := []syntheticVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
	}
	streams := map[string][]byte{
		"VBA/dir": compressedLiteralsForTest(syntheticDirStreamForTestWithOffsets(modules, nil, true)),
	}
	return syntheticVBAProjectBinForModulesTest(t, modules, nil, streams)
}

func syntheticVBAProjectBinWithProjectStreamForTest(t *testing.T) []byte {
	t.Helper()
	modules := []syntheticVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
	}
	prefixes := map[string][]byte{
		"Module1": []byte("compiled-pcode-module1"),
		"Class1":  []byte("compiled-pcode-class1"),
	}
	extra := map[string][]byte{
		"PROJECT":     []byte("ID=\"{00000000-0000-0000-0000-000000000000}\"\r\nReference=*\\G{00020430-0000-0000-C000-000000000046}#2.0#0#C:\\Windows\\System32\\stdole2.tlb#OLE Automation\r\nModule=Module1\r\nClass=Class1\r\n[Workspace]\r\nModule1=0, 0, 0, 0, C\r\nClass1=0, 0, 0, 0, C\r\n"),
		"VBA/__SRP_0": []byte("compiled cache"),
	}
	return syntheticVBAProjectBinForModulesTest(t, modules, prefixes, extra)
}

func syntheticVBAProjectBinWithVersionDependentProjectMetadataForTest(t *testing.T) []byte {
	t.Helper()
	modules := []syntheticVBAModule{
		{
			Name:       "Module1",
			StreamName: "Module1",
			Kind:       "standard",
			Source:     "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
		},
		{
			Name:       "Class1",
			StreamName: "Class1",
			Kind:       "class",
			Source:     "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
		},
	}
	prefixes := map[string][]byte{
		"Module1": []byte("compiled-pcode-module1"),
		"Class1":  []byte("compiled-pcode-class1"),
	}
	extra := map[string][]byte{
		"PROJECT":          []byte("ID=\"{00000000-0000-0000-0000-000000000000}\"\r\nModule=Module1\r\nClass=Class1\r\n[Workspace]\r\nModule1=0, 0, 0, 0, C\r\nClass1=0, 0, 0, 0, C\r\n"),
		"PROJECTwm":        append(append(append([]byte("Module1\x00"), utf16BytesForTest("Module1")...), 0, 0), 0, 0),
		"VBA/_VBA_PROJECT": bytes.Repeat([]byte{0x42}, 64),
	}
	return syntheticVBAProjectBinForModulesTest(t, modules, prefixes, extra)
}

func syntheticVBAProjectBinForModulesTest(t *testing.T, modules []syntheticVBAModule, prefixes map[string][]byte, extraStreams map[string][]byte) []byte {
	t.Helper()
	offsets := make([]uint32, 0, len(modules))
	streams := map[string][]byte{
		"VBA/dir":          nil,
		"VBA/_VBA_PROJECT": []byte{0xCC, 0x61},
	}
	for path, data := range extraStreams {
		streams[path] = append([]byte(nil), data...)
	}
	for _, module := range modules {
		prefix := prefixes[module.StreamName]
		offsets = append(offsets, uint32(len(prefix)))
		streams["VBA/"+module.StreamName] = append(append([]byte(nil), prefix...), compressedLiteralsForTest([]byte(module.Source))...)
	}
	if _, ok := streams["VBA/dir"]; !ok || streams["VBA/dir"] == nil {
		streams["VBA/dir"] = compressedLiteralsForTest(syntheticDirStreamForTestWithOffsets(modules, offsets, false))
	}
	project, err := cfb.BuildRegularSectorFile(streams)
	if err != nil {
		t.Fatalf("failed to build synthetic CFB: %v", err)
	}
	return project
}

type syntheticVBAModule struct {
	Name       string
	StreamName string
	Kind       string
	Source     string
}

func syntheticDirStreamForTest(modules []syntheticVBAModule) []byte {
	return syntheticDirStreamForTestWithOffsets(modules, nil, false)
}

func syntheticDirStreamForTestWithOffsets(modules []syntheticVBAModule, offsets []uint32, includeProjectCookie bool) []byte {
	var out []byte
	out = append(out, dirRecordForTest(0x0003, le16ForTest(1252))...)
	out = append(out, dirRecordForTest(0x000F, le16ForTest(uint16(len(modules))))...)
	if includeProjectCookie {
		out = append(out, dirRecordForTest(0x0013, le16ForTest(0xFFFF))...)
	}
	for idx, module := range modules {
		sourceOffset := uint32(0)
		if idx < len(offsets) {
			sourceOffset = offsets[idx]
		}
		out = append(out, dirRecordForTest(0x0019, []byte(module.Name))...)
		out = append(out, dirRecordForTest(0x0047, utf16BytesForTest(module.Name))...)
		out = append(out, dirRecordForTest(0x001A, []byte(module.StreamName))...)
		out = append(out, dirRecordForTest(0x0032, utf16BytesForTest(module.StreamName))...)
		out = append(out, dirRecordForTest(0x0031, le32ForTest(sourceOffset))...)
		if module.Kind == "class" {
			out = append(out, dirRecordForTest(0x0022, nil)...)
		} else {
			out = append(out, dirRecordForTest(0x0021, nil)...)
		}
		out = append(out, dirRecordForTest(0x002B, nil)...)
	}
	out = append(out, dirRecordForTest(0x0010, nil)...)
	return out
}

func syntheticDirStreamWithReferencesForTest(modules []syntheticVBAModule) []byte {
	var out []byte
	out = append(out, dirRecordForTest(0x0001, le32ForTest(3))...)
	out = append(out, dirRecordForTest(0x0002, le32ForTest(1033))...)
	out = append(out, dirRecordForTest(0x0014, le32ForTest(1033))...)
	out = append(out, dirRecordForTest(0x0003, le16ForTest(1252))...)
	out = append(out, dirRecordForTest(0x0004, []byte("VBAProject"))...)
	out = append(out, projectDocStringRecordForTest("", "")...)
	out = append(out, projectConstantsRecordForTest("", "")...)
	out = append(out, referenceNameRecordForTest("stdole")...)
	out = append(out, referenceRegisteredRecordForTest(`*\G{00020430-0000-0000-C000-000000000046}#2.0#0#C:\Windows\System32\stdole2.tlb#OLE Automation`)...)
	out = append(out, referenceNameRecordForTest("Office")...)
	out = append(out, referenceRegisteredRecordForTest(`*\G{2DF8D04C-5BFA-101B-BDE5-00AA0044DE52}#2.0#0#C:\Program Files\Common Files\Microsoft Shared\OFFICE16\MSO.DLL#Microsoft Office 16.0 Object Library`)...)
	out = append(out, dirRecordForTest(0x000F, le16ForTest(uint16(len(modules))))...)
	out = append(out, dirRecordForTest(0x0013, le16ForTest(0xECF5))...)
	for _, module := range modules {
		out = append(out, dirRecordForTest(0x0019, []byte(module.Name))...)
		out = append(out, dirRecordForTest(0x0047, utf16BytesForTest(module.Name))...)
		out = append(out, dirRecordForTest(0x001A, []byte(module.StreamName))...)
		out = append(out, dirRecordForTest(0x0032, utf16BytesForTest(module.StreamName))...)
		out = append(out, dirRecordForTest(0x001C, nil)...)
		out = append(out, dirRecordForTest(0x0048, nil)...)
		out = append(out, dirRecordForTest(0x0031, le32ForTest(0))...)
		out = append(out, dirRecordForTest(0x001E, le32ForTest(0))...)
		out = append(out, dirRecordForTest(0x002C, le16ForTest(0xFFFF))...)
		if module.Kind == "class" {
			out = append(out, dirRecordForTest(0x0022, nil)...)
		} else {
			out = append(out, dirRecordForTest(0x0021, nil)...)
		}
		out = append(out, dirRecordForTest(0x002B, nil)...)
	}
	out = append(out, dirRecordForTest(0x0010, nil)...)
	return out
}

func projectDocStringRecordForTest(mbcs, unicode string) []byte {
	var out []byte
	out = append(out, dirRecordForTest(0x0005, []byte(mbcs))...)
	out = binary.LittleEndian.AppendUint16(out, 0x0040)
	out = binary.LittleEndian.AppendUint32(out, uint32(len(utf16BytesForTest(unicode))))
	out = append(out, utf16BytesForTest(unicode)...)
	return out
}

func projectConstantsRecordForTest(mbcs, unicode string) []byte {
	var out []byte
	out = append(out, dirRecordForTest(0x000C, []byte(mbcs))...)
	out = binary.LittleEndian.AppendUint16(out, 0x003C)
	out = binary.LittleEndian.AppendUint32(out, uint32(len(utf16BytesForTest(unicode))))
	out = append(out, utf16BytesForTest(unicode)...)
	return out
}

func referenceNameRecordForTest(name string) []byte {
	var out []byte
	out = append(out, dirRecordForTest(0x0016, []byte(name))...)
	out = binary.LittleEndian.AppendUint16(out, 0x003E)
	out = binary.LittleEndian.AppendUint32(out, uint32(len(utf16BytesForTest(name))))
	out = append(out, utf16BytesForTest(name)...)
	return out
}

func referenceRegisteredRecordForTest(libid string) []byte {
	payload := binary.LittleEndian.AppendUint32(nil, uint32(len([]byte(libid))))
	payload = append(payload, []byte(libid)...)
	payload = binary.LittleEndian.AppendUint32(payload, 0)
	payload = binary.LittleEndian.AppendUint16(payload, 0)
	return dirRecordForTest(0x000D, payload)
}

func dirRecordForTest(id uint16, payload []byte) []byte {
	out := make([]byte, 6+len(payload))
	binary.LittleEndian.PutUint16(out[:2], id)
	binary.LittleEndian.PutUint32(out[2:6], uint32(len(payload)))
	copy(out[6:], payload)
	return out
}

func compressedLiteralsForTest(raw []byte) []byte {
	out := []byte{0x01}
	for len(raw) > 0 {
		chunk := raw
		if len(chunk) > 3600 {
			chunk = raw[:3600]
		}
		var chunkData []byte
		for offset := 0; offset < len(chunk); {
			n := len(chunk) - offset
			if n > 8 {
				n = 8
			}
			chunkData = append(chunkData, 0x00)
			chunkData = append(chunkData, chunk[offset:offset+n]...)
			offset += n
		}
		header := uint16(len(chunkData)-1) | 0x3000 | 0x8000
		out = binary.LittleEndian.AppendUint16(out, header)
		out = append(out, chunkData...)
		raw = raw[len(chunk):]
	}
	return out
}

type cfbEntryForTest struct {
	name        string
	objectType  byte
	left        uint32
	right       uint32
	child       uint32
	startSector uint32
	size        uint64
}

func syntheticCFBForTest(t *testing.T, streams map[string][]byte) []byte {
	t.Helper()
	const sectorSize = 512
	const noStream = uint32(0xFFFFFFFF)
	const endOfChain = uint32(0xFFFFFFFE)
	const fatSector = uint32(0xFFFFFFFD)

	names := []string{"dir", "_VBA_PROJECT", "Module1", "Class1"}
	var sectors [][]byte
	sectors = append(sectors, make([]byte, sectorSize)) // FAT sector placeholder.
	entries := []cfbEntryForTest{
		{name: "Root Entry", objectType: 5, child: 1, left: noStream, right: noStream, startSector: endOfChain},
		{name: "VBA", objectType: 1, child: 2, left: noStream, right: noStream, startSector: endOfChain},
	}
	for idx, name := range names {
		streamPath := "VBA/" + name
		data, ok := streams[streamPath]
		if !ok {
			continue
		}
		start := uint32(len(sectors))
		padded := append([]byte{}, data...)
		for len(padded)%sectorSize != 0 {
			padded = append(padded, 0)
		}
		for len(padded) > 0 {
			sectors = append(sectors, append([]byte{}, padded[:sectorSize]...))
			padded = padded[sectorSize:]
		}
		right := noStream
		if idx < len(names)-1 {
			right = uint32(len(entries) + 1)
		}
		entries = append(entries, cfbEntryForTest{
			name:        name,
			objectType:  2,
			left:        noStream,
			right:       right,
			child:       noStream,
			startSector: start,
			size:        uint64(len(data)),
		})
	}
	dirStart := uint32(len(sectors))
	dirData := make([]byte, 0, ((len(entries)*128+sectorSize-1)/sectorSize)*sectorSize)
	for _, entry := range entries {
		dirData = append(dirData, directoryEntryForTest(entry)...)
	}
	for len(dirData)%sectorSize != 0 {
		dirData = append(dirData, 0)
	}
	for len(dirData) > 0 {
		sectors = append(sectors, append([]byte{}, dirData[:sectorSize]...))
		dirData = dirData[sectorSize:]
	}

	fat := make([]uint32, len(sectors))
	for i := range fat {
		fat[i] = endOfChain
	}
	fat[0] = fatSector
	for _, entry := range entries {
		if entry.objectType != 2 || entry.size == 0 {
			continue
		}
		count := int((entry.size + sectorSize - 1) / sectorSize)
		for i := 0; i < count-1; i++ {
			fat[int(entry.startSector)+i] = entry.startSector + uint32(i) + 1
		}
	}
	dirSectorCount := len(sectors) - int(dirStart)
	for i := 0; i < dirSectorCount-1; i++ {
		fat[int(dirStart)+i] = dirStart + uint32(i) + 1
	}
	for i, value := range fat {
		binary.LittleEndian.PutUint32(sectors[0][i*4:i*4+4], value)
	}
	for i := len(fat); i < sectorSize/4; i++ {
		binary.LittleEndian.PutUint32(sectors[0][i*4:i*4+4], 0xFFFFFFFF)
	}

	header := make([]byte, 512)
	copy(header[:8], []byte{0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1})
	binary.LittleEndian.PutUint16(header[24:26], 0x003E)
	binary.LittleEndian.PutUint16(header[26:28], 0x0003)
	binary.LittleEndian.PutUint16(header[28:30], 0xFFFE)
	binary.LittleEndian.PutUint16(header[30:32], 9)
	binary.LittleEndian.PutUint16(header[32:34], 6)
	binary.LittleEndian.PutUint32(header[44:48], 1)
	binary.LittleEndian.PutUint32(header[48:52], dirStart)
	binary.LittleEndian.PutUint32(header[56:60], 0)
	binary.LittleEndian.PutUint32(header[60:64], endOfChain)
	binary.LittleEndian.PutUint32(header[68:72], endOfChain)
	binary.LittleEndian.PutUint32(header[76:80], 0)
	for offset := 80; offset < 512; offset += 4 {
		binary.LittleEndian.PutUint32(header[offset:offset+4], 0xFFFFFFFF)
	}

	out := append([]byte{}, header...)
	for _, sector := range sectors {
		out = append(out, sector...)
	}
	return out
}

func directoryEntryForTest(entry cfbEntryForTest) []byte {
	const noStream = uint32(0xFFFFFFFF)
	out := make([]byte, 128)
	if entry.left == 0 {
		entry.left = noStream
	}
	if entry.right == 0 {
		entry.right = noStream
	}
	if entry.child == 0 {
		entry.child = noStream
	}
	nameBytes := utf16BytesForTest(entry.name + "\x00")
	copy(out[:64], nameBytes)
	binary.LittleEndian.PutUint16(out[64:66], uint16(len(nameBytes)))
	out[66] = entry.objectType
	out[67] = 1
	binary.LittleEndian.PutUint32(out[68:72], entry.left)
	binary.LittleEndian.PutUint32(out[72:76], entry.right)
	binary.LittleEndian.PutUint32(out[76:80], entry.child)
	binary.LittleEndian.PutUint32(out[116:120], entry.startSector)
	binary.LittleEndian.PutUint32(out[120:124], uint32(entry.size))
	return out
}

func utf16BytesForTest(text string) []byte {
	units := utf16.Encode([]rune(text))
	out := make([]byte, len(units)*2)
	for i, unit := range units {
		binary.LittleEndian.PutUint16(out[i*2:i*2+2], unit)
	}
	return out
}

func le16ForTest(value uint16) []byte {
	out := make([]byte, 2)
	binary.LittleEndian.PutUint16(out, value)
	return out
}

func le32ForTest(value uint32) []byte {
	out := make([]byte, 4)
	binary.LittleEndian.PutUint32(out, value)
	return out
}

func containsVBASelectorForTest(selectors []string, want string) bool {
	for _, selector := range selectors {
		if selector == want {
			return true
		}
	}
	return false
}

func containsWarningForTest(warnings []string, want string) bool {
	for _, warning := range warnings {
		if strings.Contains(warning, want) {
			return true
		}
	}
	return false
}

func sourceModuleByNameForTest(t *testing.T, project *SourceProject, name string) SourceModule {
	t.Helper()
	for _, module := range project.Modules {
		if module.Name == name {
			return module
		}
	}
	t.Fatalf("module %s not found in %+v", name, project.Modules)
	return SourceModule{}
}

func minForTest(a, b int) int {
	if a < b {
		return a
	}
	return b
}
