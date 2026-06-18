package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/spf13/pflag"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	tmpl "github.com/ooxml-cli/ooxml-cli/pkg/template"
)

const (
	profileSrcFixture = "../../testdata/pptx/theme-custom-colors/presentation.pptx"
	profileTgtFixture = "../../testdata/pptx/minimal-title/presentation.pptx"
)

// runTemplateProfile resets profile-command flag state (cobra persists package
// vars across the shared root command) and runs the command.
func runTemplateProfile(t *testing.T, args ...string) (string, error) {
	t.Helper()
	templateProfileFor = "auto"
	templateProfileName = ""
	templateProfileDescription = ""
	templateProfileOut = ""
	templateProfileSaveCmd.Flags().VisitAll(func(f *pflag.Flag) {
		_ = templateProfileSaveCmd.Flags().Set(f.Name, f.DefValue)
	})
	return executeRootForXLSXTest(t, args...)
}

func TestTemplateProfileSave_PPTX(t *testing.T) {
	if _, err := os.Stat(profileSrcFixture); err != nil {
		t.Skipf("fixture not found: %s", profileSrcFixture)
	}
	out := filepath.Join(t.TempDir(), "brand.json")
	output, err := runTemplateProfile(t, "--json", "template", "profile", "save",
		profileSrcFixture, "--out", out, "--name", "Acme")
	require.NoError(t, err)

	// JSON confirmation echoes the profile.
	var p tmpl.DesignProfile
	require.NoError(t, json.Unmarshal([]byte(output), &p))
	assert.Equal(t, tmpl.ProfileFormat, p.Format)
	assert.Equal(t, "Acme", p.Metadata.Name)
	require.NotNil(t, p.Design.Theme)
	require.NotNil(t, p.Design.Theme.ColorScheme)
	assert.Equal(t, "4F81BD", p.Design.Theme.ColorScheme.Accent1)

	// File written and re-loadable via inspect.
	require.FileExists(t, out)
	insp, ierr := runTemplateProfile(t, "--json", "template", "profile", "inspect", out)
	require.NoError(t, ierr)
	var p2 tmpl.DesignProfile
	require.NoError(t, json.Unmarshal([]byte(insp), &p2))
	assert.Equal(t, p.Design.Theme.ColorScheme.Accent1, p2.Design.Theme.ColorScheme.Accent1)
}

func TestTemplateProfileSave_ToStdout(t *testing.T) {
	if _, err := os.Stat(profileSrcFixture); err != nil {
		t.Skipf("fixture not found: %s", profileSrcFixture)
	}
	output, err := runTemplateProfile(t, "--json", "template", "profile", "save", profileSrcFixture)
	require.NoError(t, err)
	var p tmpl.DesignProfile
	require.NoError(t, json.Unmarshal([]byte(output), &p))
	assert.Equal(t, tmpl.ProfileSchemaVersion, p.SchemaVersion)
}

func TestTemplateProfileInspect_RejectsNonProfile(t *testing.T) {
	bad := filepath.Join(t.TempDir(), "bad.json")
	require.NoError(t, os.WriteFile(bad, []byte(`{"schemaVersion":"1.0","type":"pptx"}`), 0o644))
	_, err := runTemplateProfile(t, "template", "profile", "inspect", bad)
	require.Error(t, err)
}

// TestTemplateProfile_RoundTripMatchesFrom is the core invariant: applying a
// SAVED profile produces the same Applied set (and TotalUpdates) as applying
// directly --from the original template. Uses --dry-run --json for a clean,
// fast comparison under the default colors+fonts selection.
func TestTemplateProfile_RoundTripMatchesFrom(t *testing.T) {
	if _, err := os.Stat(profileSrcFixture); err != nil {
		t.Skipf("fixture not found: %s", profileSrcFixture)
	}
	target := copyFixture(t, profileTgtFixture)

	// Apply directly from the source template (dry-run).
	fromOut, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--from", profileSrcFixture, "--dry-run")
	require.NoError(t, err)
	var fromRes TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(fromOut), &fromRes))

	// Save a profile from the same source, then apply via --profile (dry-run).
	prof := filepath.Join(t.TempDir(), "brand.json")
	_, serr := runTemplateProfile(t, "template", "profile", "save", profileSrcFixture, "--out", prof)
	require.NoError(t, serr)

	profOut, perr := runTemplateApply(t, "--json", "template", "apply", target,
		"--profile", prof, "--dry-run")
	require.NoError(t, perr)
	var profRes TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(profOut), &profRes))

	// Applied colors must match exactly.
	require.Equal(t, len(fromRes.Applied.Colors), len(profRes.Applied.Colors))
	for i := range fromRes.Applied.Colors {
		assert.Equal(t, fromRes.Applied.Colors[i], profRes.Applied.Colors[i])
	}
	// Fonts must match.
	if fromRes.Applied.Fonts != nil {
		require.NotNil(t, profRes.Applied.Fonts)
		assert.Equal(t, *fromRes.Applied.Fonts, *profRes.Applied.Fonts)
	} else {
		assert.Nil(t, profRes.Applied.Fonts)
	}
	assert.Equal(t, fromRes.TotalUpdates, profRes.TotalUpdates)
	assert.Equal(t, fromRes.Skipped, profRes.Skipped)
}

// TestTemplateProfile_ApplyWritesValidOutput verifies a profile apply produces a
// strict-valid output whose theme readback matches the profile intent.
func TestTemplateProfile_ApplyWritesValidOutput(t *testing.T) {
	if _, err := os.Stat(profileSrcFixture); err != nil {
		t.Skipf("fixture not found: %s", profileSrcFixture)
	}
	target := copyFixture(t, profileTgtFixture)
	prof := filepath.Join(t.TempDir(), "brand.json")
	_, serr := runTemplateProfile(t, "template", "profile", "save", profileSrcFixture, "--out", prof)
	require.NoError(t, serr)

	out := filepath.Join(t.TempDir(), "branded.pptx")
	applyOut, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--profile", prof, "--out", out)
	require.NoError(t, err)
	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(applyOut), &res))
	assert.Equal(t, len(res.Applied.Colors)+len(res.Applied.FontParts), res.TotalUpdates)

	require.FileExists(t, out)
	vout, verr := executeRootForXLSXTest(t, "validate", "--strict", out)
	require.NoError(t, verr)
	assert.Contains(t, vout, "valid")

	// Readback: accent1 from the profile is present in the output theme.
	tokOut, terr := runTemplateApply(t, "--json", "template", "tokens", out)
	require.NoError(t, terr)
	var tok map[string]interface{}
	require.NoError(t, json.Unmarshal([]byte(tokOut), &tok))
	cs := tok["pptx"].(map[string]interface{})["theme"].(map[string]interface{})["colorScheme"].(map[string]interface{})
	assert.Equal(t, "4F81BD", cs["accent1"])
}

// TestTemplateProfile_ThreeSourcesMutuallyExclusive verifies the apply guard.
func TestTemplateProfile_SourcesMutuallyExclusive(t *testing.T) {
	target := copyFixture(t, profileTgtFixture)
	prof := filepath.Join(t.TempDir(), "p.json")
	require.NoError(t, os.WriteFile(prof, []byte(
		`{"schemaVersion":"1.0","format":"ooxml-design-profile","metadata":{},"design":{"theme":{"colorScheme":{"accent1":"112233"}}}}`), 0o644))

	// --from and --profile together: error.
	_, err := runTemplateApply(t, "template", "apply", target,
		"--from", profileSrcFixture, "--profile", prof, "--dry-run")
	require.Error(t, err)

	// No source at all: error.
	_, err = runTemplateApply(t, "template", "apply", target, "--dry-run")
	require.Error(t, err)
}
