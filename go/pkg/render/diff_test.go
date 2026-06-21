package render

import (
	"context"
	"errors"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestVisualDiff_MissingTool(t *testing.T) {
	tools := &Tools{Runner: &fakeRunner{lookups: map[string]error{
		"compare": errors.New("missing"),
		"magick":  errors.New("missing"),
	}}}

	_, err := tools.VisualDiff("a.png", "b.png", t.TempDir()+"/diff.png")
	require.Error(t, err)
	var missing *MissingDependencyError
	assert.ErrorAs(t, err, &missing)
}

func TestVisualDiff_ParsesMetricOutput(t *testing.T) {
	tools := &Tools{Runner: &fakeRunner{runFn: func(ctx context.Context, name string, args []string) (*RunResult, error) {
		return &RunResult{Stderr: "12.34 (0.0042)"}, errors.New("exit status 1")
	}}}

	difference, err := tools.VisualDiff("a.png", "b.png", t.TempDir()+"/diff.png")
	require.NoError(t, err)
	assert.Equal(t, 0.0042, difference)
}
