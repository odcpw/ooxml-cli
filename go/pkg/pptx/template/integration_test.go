package template

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// findFixture locates a fixture file in expected locations
func findFixture(filename string) (string, bool) {
	baseName := filepath.Join("testdata", "pptx", "template-branded", filename)

	// go test runs this package from pkg/pptx/template/, so walk up to the repo root.
	paths := []string{
		baseName,
		filepath.Join("..", "..", "..", baseName),
		filepath.Join(".", baseName),
	}

	for _, path := range paths {
		if _, err := os.Stat(path); err == nil {
			return path, true
		}
	}

	return paths[0], false
}

// TestBrandedTemplateManifestStructure verifies the branded template manifest is properly structured
func TestBrandedTemplateManifestStructure(t *testing.T) {
	path, exists := findFixture("manifest.json")
	if !exists {
		t.Skipf("manifest fixture not found (checked: %s)", path)
	}

	// Read manifest file
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read manifest: %v", err)
	}

	// Parse as JSON to verify structure
	var manifest map[string]interface{}
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatalf("failed to unmarshal manifest JSON: %v", err)
	}

	// Verify required top-level fields
	requiredFields := []string{
		"manifestVersion",
		"name",
		"version",
		"createdAt",
		"modifiedAt",
		"archetypes",
	}

	for _, field := range requiredFields {
		if _, ok := manifest[field]; !ok {
			t.Errorf("missing required field: %q", field)
		}
	}

	// Verify archetypes array
	archetypesRaw, ok := manifest["archetypes"]
	if !ok {
		t.Fatal("manifest has no archetypes field")
	}

	archetypesArray, ok := archetypesRaw.([]interface{})
	if !ok {
		t.Fatalf("archetypes is not an array, got %T", archetypesRaw)
	}

	if len(archetypesArray) < 2 {
		t.Errorf("expected at least 2 archetypes, got %d", len(archetypesArray))
	}

	// Verify each archetype has required fields
	for i, archRaw := range archetypesArray {
		arch, ok := archRaw.(map[string]interface{})
		if !ok {
			t.Errorf("archetype %d is not an object", i)
			continue
		}

		requiredArchFields := []string{"id", "name", "slots"}
		for _, field := range requiredArchFields {
			if _, ok := arch[field]; !ok {
				t.Errorf("archetype %d missing required field: %q", i, field)
			}
		}

		// Verify slots array
		slotsRaw, ok := arch["slots"]
		if !ok {
			t.Errorf("archetype %d has no slots", i)
			continue
		}

		slotsArray, ok := slotsRaw.([]interface{})
		if !ok {
			t.Errorf("archetype %d slots is not an array", i)
			continue
		}

		if len(slotsArray) == 0 {
			t.Errorf("archetype %d has no slots (must have at least one)", i)
		}

		// Verify each slot has required fields
		for j, slotRaw := range slotsArray {
			slot, ok := slotRaw.(map[string]interface{})
			if !ok {
				t.Errorf("archetype %d slot %d is not an object", i, j)
				continue
			}

			requiredSlotFields := []string{"id", "name", "kind", "required"}
			for _, field := range requiredSlotFields {
				if _, ok := slot[field]; !ok {
					t.Errorf("archetype %d slot %d missing required field: %q", i, j, field)
				}
			}
		}
	}

	t.Logf("✓ Manifest structure verified: %d archetypes", len(archetypesArray))
}

// TestBrandedTemplateSimpleSpec verifies the simple compilation spec is valid
func TestBrandedTemplateSimpleSpec(t *testing.T) {
	path, exists := findFixture("spec-simple.yaml")
	if !exists {
		t.Skipf("spec fixture not found (checked: %s)", path)
	}

	// Read spec file
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read spec: %v", err)
	}

	// Verify content is not empty
	if len(data) == 0 {
		t.Fatal("spec file is empty")
	}

	// Verify it contains expected YAML structure markers
	content := string(data)
	if !contains(content, "version:") || !contains(content, "slides:") {
		t.Error("spec file does not contain expected YAML structure (missing 'version' or 'slides')")
	}

	// Verify at least one slide is defined
	if !contains(content, "archetype:") {
		t.Error("spec file does not contain any slides (missing 'archetype')")
	}

	t.Log("✓ Simple spec structure verified")
}

// TestBrandedTemplateComplexSpec verifies the complex compilation spec is valid
func TestBrandedTemplateComplexSpec(t *testing.T) {
	path, exists := findFixture("spec-complex.yaml")
	if !exists {
		t.Skipf("spec fixture not found (checked: %s)", path)
	}

	// Read spec file
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read spec: %v", err)
	}

	// Verify content is not empty
	if len(data) == 0 {
		t.Fatal("spec file is empty")
	}

	// Verify it contains expected YAML structure markers
	content := string(data)
	if !contains(content, "version:") || !contains(content, "slides:") {
		t.Error("spec file does not contain expected YAML structure (missing 'version' or 'slides')")
	}

	// Verify multiple slides are defined
	slideCount := countOccurrences(content, "- archetype:")
	if slideCount < 3 {
		t.Errorf("complex spec should have multiple slides, found %d", slideCount)
	}

	// Verify theme overrides are present
	if !contains(content, "themeOverrides:") {
		t.Error("complex spec missing themeOverrides section")
	}

	t.Logf("✓ Complex spec structure verified: %d slides", slideCount)
}

// TestBrandedTemplateInvalidSpec verifies the invalid spec structure
func TestBrandedTemplateInvalidSpec(t *testing.T) {
	path, exists := findFixture("spec-invalid.yaml")
	if !exists {
		t.Skipf("invalid spec fixture not found (checked: %s)", path)
	}

	// Read spec file
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read spec: %v", err)
	}

	// Verify content is not empty
	if len(data) == 0 {
		t.Fatal("spec file is empty")
	}

	// Verify the spec contains problematic content
	content := string(data)
	hasProblems := false

	if contains(content, "unknown-archetype") {
		hasProblems = true
	}
	if contains(content, "missing") && contains(content, "content") {
		hasProblems = true
	}

	if !hasProblems {
		t.Error("invalid spec should contain problematic archetype or content references")
	}

	t.Log("✓ Invalid spec structure verified (contains expected invalid content)")
}

// TestBrandedTemplatePPTXExists verifies the branded template PPTX fixture exists
func TestBrandedTemplatePPTXExists(t *testing.T) {
	path, exists := findFixture("presentation.pptx")
	if !exists {
		t.Skipf("PPTX fixture not found (checked: %s)", path)
	}

	// Verify file exists
	stat, err := os.Stat(path)
	if err != nil {
		t.Fatalf("error checking PPTX fixture: %v", err)
	}

	// Verify it's a file (not directory)
	if stat.IsDir() {
		t.Error("presentation.pptx is a directory, not a file")
	}

	// Verify it has reasonable size (valid PPTX files are typically >10KB)
	if stat.Size() < 10000 {
		t.Errorf("presentation.pptx appears too small (%d bytes), likely not a valid PPTX", stat.Size())
	}

	t.Logf("✓ PPTX fixture exists and has valid size (%d bytes)", stat.Size())
}

// TestTemplateManifestRoundtrip verifies manifest can be parsed and re-marshaled
func TestTemplateManifestRoundtrip(t *testing.T) {
	path, exists := findFixture("manifest.json")
	if !exists {
		t.Skipf("manifest fixture not found (checked: %s)", path)
	}

	// Read original manifest
	originalData, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read manifest: %v", err)
	}

	// Parse into struct
	var manifest TemplateManifest
	if err := json.Unmarshal(originalData, &manifest); err != nil {
		t.Fatalf("failed to unmarshal manifest: %v", err)
	}

	// Validate manifest
	if err := manifest.ValidateManifest(); err != nil {
		t.Fatalf("manifest validation failed: %v", err)
	}

	// Re-marshal to JSON
	remarshaled, err := json.MarshalIndent(&manifest, "", "  ")
	if err != nil {
		t.Fatalf("failed to re-marshal manifest: %v", err)
	}

	// Parse re-marshaled data to verify structure
	var remarshaledManifest TemplateManifest
	if err := json.Unmarshal(remarshaled, &remarshaledManifest); err != nil {
		t.Fatalf("failed to unmarshal re-marshaled manifest: %v", err)
	}

	// Verify key fields match
	if manifest.Name != remarshaledManifest.Name {
		t.Errorf("name mismatch: %s != %s", manifest.Name, remarshaledManifest.Name)
	}

	if len(manifest.Archetypes) != len(remarshaledManifest.Archetypes) {
		t.Errorf("archetype count mismatch: %d != %d", len(manifest.Archetypes), len(remarshaledManifest.Archetypes))
	}

	t.Log("✓ Manifest roundtrip successful")
}

// TestBrandedTemplateMetadata verifies manifest metadata is correct
func TestBrandedTemplateMetadata(t *testing.T) {
	path, exists := findFixture("manifest.json")
	if !exists {
		t.Skipf("manifest fixture not found (checked: %s)", path)
	}

	// Read and parse manifest
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("failed to read manifest: %v", err)
	}

	var manifest TemplateManifest
	if err := json.Unmarshal(data, &manifest); err != nil {
		t.Fatalf("failed to unmarshal manifest: %v", err)
	}

	// Verify basic metadata
	if manifest.Name == "" {
		t.Error("manifest name is empty")
	}

	if manifest.Version == nil {
		t.Error("manifest version is nil")
	} else {
		if manifest.Version.Major < 0 || manifest.Version.Minor < 0 || manifest.Version.Patch < 0 {
			t.Errorf("invalid version: %d.%d.%d", manifest.Version.Major, manifest.Version.Minor, manifest.Version.Patch)
		}

		if manifest.Version.CreatedAt.IsZero() {
			t.Error("version CreatedAt is zero")
		}
	}

	// Verify dates are not zero
	if manifest.CreatedAt.IsZero() {
		t.Error("manifest CreatedAt is zero")
	}

	if manifest.ModifiedAt.IsZero() {
		t.Error("manifest ModifiedAt is zero")
	}

	// Verify CreatedAt is not in the future by more than 1 minute
	now := time.Now()
	if manifest.CreatedAt.After(now.Add(1 * time.Minute)) {
		t.Errorf("CreatedAt is in the future: %v (now: %v)", manifest.CreatedAt, now)
	}

	t.Logf("✓ Manifest metadata verified: %s v%s created by %s",
		manifest.Name, manifest.Version.String(), manifest.Author)
}

// TestFixtureIntegration verifies all fixtures can be loaded together
func TestFixtureIntegration(t *testing.T) {
	fixtures := []string{
		"manifest.json",
		"spec-simple.yaml",
		"spec-complex.yaml",
		"spec-invalid.yaml",
		"presentation.pptx",
	}

	allExist := true
	for _, filename := range fixtures {
		if _, exists := findFixture(filename); !exists {
			t.Logf("⚠ Fixture %q not found", filename)
			allExist = false
		}
	}

	if !allExist {
		t.Skip("Some fixtures missing (run 'make fixtures')")
	}

	// Read manifest and verify it parses
	path, _ := findFixture("manifest.json")
	manifestData, _ := os.ReadFile(path)
	var manifest TemplateManifest
	if err := json.Unmarshal(manifestData, &manifest); err != nil {
		t.Fatalf("failed to parse manifest: %v", err)
	}

	t.Logf("✓ All fixtures available and readable (%d fixtures)", len(fixtures))
}

// Helper functions

func contains(s, substr string) bool {
	for i := 0; i+len(substr) <= len(s); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func countOccurrences(s, substr string) int {
	count := 0
	for i := 0; i+len(substr) <= len(s); i++ {
		if s[i:i+len(substr)] == substr {
			count++
			i += len(substr) - 1
		}
	}
	return count
}
