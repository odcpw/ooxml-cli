package cli

import (
	"bytes"
	"encoding/json"
	"path/filepath"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestSlidesSelectorsJSON(t *testing.T) {
	fixturePath, err := filepath.Abs("../../testdata/pptx/title-content/presentation.pptx")
	require.NoError(t, err)

	rootCmd := newTestRootCmd(t)
	rootCmd.SetArgs([]string{
		"pptx", "slides", "selectors",
		fixturePath,
		"--slide", "2",
		"--format", "json",
		"--pretty",
	})

	var output bytes.Buffer
	rootCmd.SetOut(&output)
	require.NoError(t, rootCmd.Execute())

	var result SlideSelectorsOutput
	require.NoError(t, json.Unmarshal(output.Bytes(), &result))
	assert.Equal(t, 2, result.Slide)
	assert.NotEmpty(t, result.Targets)

	var foundBody bool
	for _, target := range result.Targets {
		if target.Placeholder == nil {
			continue
		}
		for _, selector := range target.Selectors {
			if selector == "body" || selector == "body:1" {
				foundBody = true
			}
		}
	}
	assert.True(t, foundBody, "expected body selector metadata in slide selector catalog")
}
