package cli

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/mutate"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestLayoutsImportCLI(t *testing.T) {
	resetFlags()
	resetLayoutFlags()

	targetPath := getTestFilePath("minimal-title", "presentation.pptx")
	sourceFixture := getTestFilePath("minimal-title", "presentation.pptx")
	if _, err := os.Stat(targetPath); err != nil {
		t.Skipf("target fixture not found: %v", err)
	}
	if _, err := os.Stat(sourceFixture); err != nil {
		t.Skipf("source fixture not found: %v", err)
	}

	sourcePath := filepath.Join(t.TempDir(), "source-layout-import.pptx")
	sourcePkg, err := opc.Open(sourceFixture)
	require.NoError(t, err)
	_, err = mutate.RenameLayout(&mutate.RenameLayoutRequest{
		Package:       sourcePkg,
		LayoutPartURI: "/ppt/slideLayouts/slideLayout1.xml",
		NewName:       "Imported Title Slide",
	})
	require.NoError(t, err)
	require.NoError(t, sourcePkg.SaveAs(sourcePath))
	require.NoError(t, sourcePkg.Close())

	outPath := filepath.Join(t.TempDir(), "layout-imported.pptx")
	cmd := GetRootCmd()
	cmd.SetArgs([]string{"pptx", "layouts", "import", targetPath, "--source", sourcePath, "--layout", "1", "--theme-policy", "import", "--out", outPath})
	require.NoError(t, cmd.Execute())

	pkg, err := opc.Open(outPath)
	require.NoError(t, err)
	defer pkg.Close()

	layouts, err := ParsePresentationLayouts(pkg)
	require.NoError(t, err)
	assert.Len(t, layouts, 12)
	masters, err := ParsePresentationMasters(pkg)
	require.NoError(t, err)
	assert.Len(t, masters, 2)

	var found bool
	for _, layout := range layouts {
		if layout.Name == "Imported Title Slide" {
			found = true
			break
		}
	}
	assert.True(t, found, "expected imported layout name to be discoverable")
}
