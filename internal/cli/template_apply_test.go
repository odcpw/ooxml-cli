package cli

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/capabilities"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/spf13/pflag"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// runTemplateApply resets the apply-specific package-level flags (cobra BoolVar
// state persists across the shared root command) and runs the command.
func runTemplateApply(t *testing.T, args ...string) (string, error) {
	t.Helper()
	templateApplyFrom = ""
	templateApplyTokens = ""
	templateApplyProfile = ""
	templateApplyFor = "auto"
	templateApplyColors = false
	templateApplyFonts = false
	templateApplyCharts = false
	templateApplyTextStyles = false
	templateApplyRanges = false
	// Reset this command's local flags (--out/--in-place/--dry-run/--backup etc.)
	// which otherwise persist on the shared root command across test invocations.
	templateApplyCmd.Flags().VisitAll(func(f *pflag.Flag) {
		_ = templateApplyCmd.Flags().Set(f.Name, f.DefValue)
	})
	return executeRootForXLSXTest(t, args...)
}

func writeTemplateTokensProfile(t *testing.T, body string) string {
	t.Helper()
	profile := filepath.Join(t.TempDir(), "tokens.json")
	require.NoError(t, os.WriteFile(profile, []byte(body), 0o644))
	return profile
}

func copyFixture(t *testing.T, src string) string {
	t.Helper()
	data, err := os.ReadFile(src)
	require.NoError(t, err)
	dst := filepath.Join(t.TempDir(), filepath.Base(src))
	require.NoError(t, os.WriteFile(dst, data, 0o644))
	return dst
}

func TestTemplateApply_FromTokens_ColorsAndFonts(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.pptx")
	profile := writeTemplateTokensProfile(t, `{
		"schemaVersion":"1.0",
		"type":"pptx",
		"source":"brand",
		"pptx":{
			"theme":{
				"colorScheme":{
					"name":"Brand",
					"dark1":"111111",
					"light1":"F7F7F7",
					"dark2":"222222",
					"light2":"EEEEEE",
					"accent1":"AABBCC",
					"accent2":"DDEEFF",
					"accent3":"123456",
					"accent4":"654321",
					"accent5":"ABCDEF",
					"accent6":"FEDCBA",
					"hypLink":"13579B",
					"folLink":"2468AC"
				},
				"fontScheme":{"name":"Brand","majorFont":"Arial","minorFont":"Arial"}
			},
			"defaultTextStyles":[],
			"tableStyles":[],
			"chartStyles":[]
		}
	}`)

	output, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--tokens", profile, "--out", out)
	require.NoError(t, err)

	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(output), &res))
	assert.Equal(t, "pptx", res.TargetType)
	assert.False(t, res.DryRun)
	assert.Len(t, res.Applied.Colors, 12)
	require.NotNil(t, res.Applied.Fonts)
	assert.Equal(t, "Arial", res.Applied.Fonts.MajorFont)
	assert.Len(t, res.Applied.FontParts, 1)
	assert.Equal(t, 13, res.TotalUpdates)

	// Output must exist and strict-validate.
	require.FileExists(t, out)
	vout, verr := executeRootForXLSXTest(t, "validate", "--strict", out)
	require.NoError(t, verr)
	assert.Contains(t, vout, "valid")

	// Readback: applied accent1 from source.
	tokOut, terr := runTemplateApply(t, "--json", "template", "tokens", out)
	require.NoError(t, terr)
	var tok map[string]interface{}
	require.NoError(t, json.Unmarshal([]byte(tokOut), &tok))
	pptx := tok["pptx"].(map[string]interface{})
	theme := pptx["theme"].(map[string]interface{})
	cs := theme["colorScheme"].(map[string]interface{})
	fs := theme["fontScheme"].(map[string]interface{})
	assert.Equal(t, "AABBCC", cs["accent1"])
	assert.Equal(t, "Arial", fs["majorFont"])
}

func TestTemplateApply_IdempotentReapplyReportsNoUpdates(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	first := filepath.Join(t.TempDir(), "first.pptx")
	second := filepath.Join(t.TempDir(), "second.pptx")
	profile := writeTemplateTokensProfile(t, `{
		"schemaVersion":"1.0",
		"type":"pptx",
		"source":"brand",
		"pptx":{
			"theme":{
				"colorScheme":{
					"dark1":"111111",
					"light1":"F7F7F7",
					"dark2":"222222",
					"light2":"EEEEEE",
					"accent1":"AABBCC",
					"accent2":"DDEEFF",
					"accent3":"123456",
					"accent4":"654321",
					"accent5":"ABCDEF",
					"accent6":"FEDCBA",
					"hypLink":"13579B",
					"folLink":"2468AC"
				},
				"fontScheme":{"majorFont":"Arial","minorFont":"Arial"}
			},
			"defaultTextStyles":[],
			"tableStyles":[],
			"chartStyles":[]
		}
	}`)

	_, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--tokens", profile, "--out", first)
	require.NoError(t, err)

	output, err := runTemplateApply(t, "--json", "template", "apply", first,
		"--tokens", profile, "--out", second)
	require.NoError(t, err)

	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(output), &res))
	assert.Equal(t, 0, res.TotalUpdates)
	assert.Empty(t, res.Applied.Colors)
	assert.Nil(t, res.Applied.Fonts)
	assert.Empty(t, res.Applied.FontParts)
	assert.NotEmpty(t, res.Skipped)
	require.FileExists(t, second)
}

func TestTemplateApply_PPTXTextStyles(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.pptx")
	second := filepath.Join(t.TempDir(), "second.pptx")
	profile := writeTemplateTokensProfile(t, `{
		"schemaVersion":"1.0",
		"type":"pptx",
		"source":"text-styles",
		"pptx":{
			"theme":null,
			"defaultTextStyles":[
				{"masterRef":"/ppt/slideMasters/slideMaster9.xml","role":"title","fontRef":"major","sizePt":24,"colorRef":"accent1"},
				{"masterRef":"/ppt/slideMasters/slideMaster9.xml","role":"body","fontRef":"minor","sizePt":20,"color":"112233"}
			],
			"tableStyles":[],
			"chartStyles":[]
		}
	}`)

	output, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--tokens", profile, "--target-text-styles", "--out", out)
	require.NoError(t, err)

	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(output), &res))
	assert.Equal(t, 2, res.TotalUpdates)
	assert.Len(t, res.Applied.TextStyles, 2)

	tokOut, terr := runTemplateApply(t, "--json", "template", "tokens", out)
	require.NoError(t, terr)
	var tok tmplTokensForTest
	require.NoError(t, json.Unmarshal([]byte(tokOut), &tok))
	require.NotNil(t, tok.PPTX)
	styles := map[string]defaultTextStyleForTest{}
	for _, style := range tok.PPTX.DefaultTextStyles {
		styles[style.Role] = style
	}
	assert.Equal(t, 24.0, styles["title"].SizePt)
	assert.Equal(t, "major", styles["title"].FontRef)
	assert.Equal(t, "accent1", styles["title"].ColorRef)
	assert.Equal(t, 20.0, styles["body"].SizePt)
	assert.Equal(t, "minor", styles["body"].FontRef)
	assert.Equal(t, "112233", styles["body"].Color)
	assertBodyMasterHierarchyPreserved(t, out)

	vout, verr := executeRootForXLSXTest(t, "validate", "--strict", out)
	require.NoError(t, verr)
	assert.Contains(t, vout, "valid")

	reapply, err := runTemplateApply(t, "--json", "template", "apply", out,
		"--tokens", profile, "--target-text-styles", "--out", second)
	require.NoError(t, err)
	var reapplyRes TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(reapply), &reapplyRes))
	assert.Equal(t, 0, reapplyRes.TotalUpdates)
	assert.Empty(t, reapplyRes.Applied.TextStyles)
}

func assertBodyMasterHierarchyPreserved(t *testing.T, path string) {
	t.Helper()
	pkg, err := opc.Open(path)
	require.NoError(t, err)
	defer pkg.Close()

	doc, err := pkg.ReadXMLPart("/ppt/slideMasters/slideMaster1.xml")
	require.NoError(t, err)
	txStyles := findDescendantByLocalForTest(doc.Root(), "txStyles")
	require.NotNil(t, txStyles)
	bodyStyle := findChildByLocalForTest(txStyles, "bodyStyle")
	require.NotNil(t, bodyStyle)
	lvl1 := findChildByLocalForTest(bodyStyle, "lvl1pPr")
	require.NotNil(t, lvl1)
	assert.Equal(t, "342900", lvl1.SelectAttrValue("marL", ""))
	assert.Equal(t, "-342900", lvl1.SelectAttrValue("indent", ""))
	require.NotNil(t, findDescendantByLocalForTest(lvl1, "buChar"))
	lvl2 := findChildByLocalForTest(bodyStyle, "lvl2pPr")
	require.NotNil(t, lvl2)
	defRPr2 := findChildByLocalForTest(lvl2, "defRPr")
	require.NotNil(t, defRPr2)
	assert.Equal(t, "2800", defRPr2.SelectAttrValue("sz", ""))
	require.NotNil(t, findDescendantByLocalForTest(lvl2, "buChar"))
}

func findChildByLocalForTest(elem interface{ ChildElements() []*etree.Element }, local string) *etree.Element {
	if elem == nil {
		return nil
	}
	for _, child := range elem.ChildElements() {
		if localNameForTest(child.Tag) == local {
			return child
		}
	}
	return nil
}

func findDescendantByLocalForTest(elem *etree.Element, local string) *etree.Element {
	if elem == nil {
		return nil
	}
	if localNameForTest(elem.Tag) == local {
		return elem
	}
	for _, child := range elem.ChildElements() {
		if found := findDescendantByLocalForTest(child, local); found != nil {
			return found
		}
	}
	return nil
}

func localNameForTest(tag string) string {
	if idx := strings.IndexByte(tag, '}'); idx >= 0 {
		return tag[idx+1:]
	}
	if idx := strings.IndexByte(tag, ':'); idx >= 0 {
		return tag[idx+1:]
	}
	return tag
}

type tmplTokensForTest struct {
	PPTX *struct {
		DefaultTextStyles []defaultTextStyleForTest `json:"defaultTextStyles"`
	} `json:"pptx"`
}

type defaultTextStyleForTest struct {
	Role     string  `json:"role"`
	FontRef  string  `json:"fontRef"`
	SizePt   float64 `json:"sizePt"`
	Color    string  `json:"color"`
	ColorRef string  `json:"colorRef"`
}

func TestTemplateApply_DryRunNoWrite(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	profile := writeTemplateTokensProfile(t, `{"schemaVersion":"1.0","type":"pptx","source":"prof","pptx":{"theme":{"colorScheme":{"accent1":"FF0000"},"fontScheme":{"majorFont":"Arial"}},"defaultTextStyles":[],"tableStyles":[],"chartStyles":[]}}`)

	output, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--tokens", profile, "--dry-run")
	require.NoError(t, err)

	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(output), &res))
	assert.True(t, res.DryRun)
	assert.Empty(t, res.Output)
	assert.Len(t, res.Applied.Colors, 1)
	require.NotNil(t, res.Applied.Fonts)
}

func TestTemplateApply_ChartsFromTokensProfile(t *testing.T) {
	tgtFixture := "../../testdata/pptx/chart-simple/presentation.pptx"
	if _, err := os.Stat(tgtFixture); err != nil {
		t.Skipf("fixture not found: %s", tgtFixture)
	}
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.pptx")

	profile := filepath.Join(t.TempDir(), "prof.json")
	require.NoError(t, os.WriteFile(profile, []byte(
		`{"schemaVersion":"1.0","type":"pptx","source":"prof","pptx":{"theme":null,"defaultTextStyles":[],"tableStyles":[],"chartStyles":[{"partUri":"/x","seriesFillColor":"FF0000","seriesLineColor":"00FF00"}]}}`), 0o644))

	output, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--tokens", profile, "--target-charts", "--out", out)
	require.NoError(t, err)

	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(output), &res))
	require.NotEmpty(t, res.Applied.Charts)
	for _, c := range res.Applied.Charts {
		assert.Equal(t, "FF0000", c.SeriesFillColor)
		assert.Equal(t, "00FF00", c.SeriesLineColor)
	}
	// charts-only selection must not touch colors/fonts.
	assert.Empty(t, res.Applied.Colors)
	assert.Nil(t, res.Applied.Fonts)

	vout, verr := executeRootForXLSXTest(t, "validate", "--strict", out)
	require.NoError(t, verr)
	assert.Contains(t, vout, "valid")
}

func TestTemplateApply_XLSX_ChartsFromTokensProfile(t *testing.T) {
	tgtFixture := "../../testdata/xlsx/chart-workbook/workbook.xlsx"
	if _, err := os.Stat(tgtFixture); err != nil {
		t.Skipf("fixture not found: %s", tgtFixture)
	}
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.xlsx")

	profile := filepath.Join(t.TempDir(), "prof.json")
	require.NoError(t, os.WriteFile(profile, []byte(
		`{"schemaVersion":"1.0","type":"xlsx","source":"prof","xlsx":{"theme":null,"namedCellStyles":[],"chartStyles":[{"partUri":"/x","seriesFillColor":"FF0000","seriesLineColor":"00FF00"}]}}`), 0o644))

	output, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--tokens", profile, "--target-charts", "--out", out)
	require.NoError(t, err)

	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(output), &res))
	assert.Equal(t, "xlsx", res.TargetType)
	require.NotEmpty(t, res.Applied.Charts, "xlsx chart series styling must apply")
	for _, c := range res.Applied.Charts {
		assert.Contains(t, c.PartURI, "/xl/charts/chart")
		assert.Equal(t, "FF0000", c.SeriesFillColor)
	}

	vout, verr := executeRootForXLSXTest(t, "validate", "--strict", out)
	require.NoError(t, verr)
	assert.Contains(t, vout, "valid")
}

func TestTemplateApply_RejectsDecorativeTokens(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.pptx")

	profile := filepath.Join(t.TempDir(), "bad.json")
	require.NoError(t, os.WriteFile(profile, []byte(
		`{"schemaVersion":"1.0","type":"pptx","pptx":{"theme":{}},"gradients":{"x":1}}`), 0o644))

	_, err := runTemplateApply(t, "template", "apply", target, "--tokens", profile, "--out", out)
	require.Error(t, err)
	assert.Contains(t, strings.ToLower(err.Error()), "decorative effects")
	assert.Contains(t, err.Error(), "gradients")
}

func TestTemplateApply_RequiresExactlyOneSource(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.pptx")

	// Neither source.
	_, err := runTemplateApply(t, "template", "apply", target, "--out", out)
	require.Error(t, err)
	assert.Contains(t, strings.ToLower(err.Error()), "exactly one token source")

	// Both sources.
	_, err = runTemplateApply(t, "template", "apply", target,
		"--from", tgtFixture, "--tokens", "x.json", "--out", out)
	require.Error(t, err)
	assert.Contains(t, strings.ToLower(err.Error()), "exactly one token source")
}

func TestTemplateApply_RangesReportedSkipped(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.pptx")
	profile := writeTemplateTokensProfile(t, `{"schemaVersion":"1.0","type":"pptx","source":"prof","pptx":{"theme":{"colorScheme":{"accent1":"FF0000"}},"defaultTextStyles":[],"tableStyles":[],"chartStyles":[]}}`)

	output, err := runTemplateApply(t, "--json", "template", "apply", target,
		"--tokens", profile, "--target-colors", "--target-ranges", "--out", out)
	require.NoError(t, err)

	var res TemplateApplyResult
	require.NoError(t, json.Unmarshal([]byte(output), &res))
	found := false
	for _, s := range res.Skipped {
		if strings.Contains(s, "ranges") {
			found = true
		}
	}
	assert.True(t, found, "expected a skipped reason for ranges, got %v", res.Skipped)
	// --target-ranges alone with --target-colors still applies colors.
	assert.NotEmpty(t, res.Applied.Colors)
}

func TestTemplateApply_DryRunCannotCombineWithOut(t *testing.T) {
	tgtFixture := "../../testdata/pptx/minimal-title/presentation.pptx"
	target := copyFixture(t, tgtFixture)
	out := filepath.Join(t.TempDir(), "out.pptx")

	_, err := runTemplateApply(t, "template", "apply", target,
		"--from", tgtFixture, "--dry-run", "--out", out)
	require.Error(t, err)
	assert.Contains(t, strings.ToLower(err.Error()), "dry-run")
}

func TestTemplateApply_CapabilitiesRegistered(t *testing.T) {
	meta, ok := capabilities.MetadataFor("ooxml template apply")
	require.True(t, ok, "template apply must have capabilities metadata")
	assert.NotEmpty(t, meta.Examples)
	for _, kind := range meta.TargetObjectKinds {
		assert.True(t, capabilities.IsObjectKind(kind), "target kind %q must be in the closed vocabulary", kind)
	}
}
