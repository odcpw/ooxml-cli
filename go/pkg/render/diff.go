package render

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
)

var metricPattern = regexp.MustCompile(`\(([-+0-9.eE]+)\)`)

// VisualDiff compares two rendered slide images and writes a diff image.
func VisualDiff(imgA, imgB, outDiff string) (float64, error) {
	return NewTools().VisualDiff(imgA, imgB, outDiff)
}

// VisualDiff compares two rendered slide images and writes a diff image.
func (t *Tools) VisualDiff(imgA, imgB, outDiff string) (float64, error) {
	if t == nil {
		t = NewTools()
	}
	if t.Runner == nil {
		t.Runner = ExecRunner{}
	}
	if t.Timeout <= 0 {
		t.Timeout = defaultTimeout
	}
	if imgA == "" || imgB == "" || outDiff == "" {
		return 0, fmt.Errorf("visual diff requires two input images and an output path")
	}
	if err := os.MkdirAll(filepath.Dir(outDiff), 0o755); err != nil {
		return 0, fmt.Errorf("failed to create visual diff directory: %w", err)
	}

	binary, args, err := t.visualDiffCommand(imgA, imgB, outDiff)
	if err != nil {
		return 0, err
	}

	ctx, cancel := context.WithTimeout(context.Background(), t.Timeout)
	defer cancel()
	result, runErr := t.Runner.Run(ctx, binary, args)
	metric := extractMetric(result)
	if metric != nil {
		return *metric, nil
	}
	if runErr != nil {
		return 0, &ToolFailureError{Tool: binary, Args: args, Cause: runErr, Stderr: stderrFromResult(result)}
	}
	return 0, fmt.Errorf("could not parse visual diff metric output")
}

func (t *Tools) visualDiffCommand(imgA, imgB, outDiff string) (string, []string, error) {
	if _, err := t.Runner.LookPath("compare"); err == nil {
		return "compare", []string{"-metric", "RMSE", imgA, imgB, outDiff}, nil
	}
	if _, err := t.Runner.LookPath("magick"); err == nil {
		return "magick", []string{"compare", "-metric", "RMSE", imgA, imgB, outDiff}, nil
	}
	return "", nil, &MissingDependencyError{Tool: "compare"}
}

func extractMetric(result *RunResult) *float64 {
	if result == nil {
		return nil
	}
	for _, candidate := range []string{result.Stderr, result.Stdout} {
		candidate = strings.TrimSpace(candidate)
		if candidate == "" {
			continue
		}
		if matches := metricPattern.FindStringSubmatch(candidate); len(matches) == 2 {
			if value, err := strconv.ParseFloat(matches[1], 64); err == nil {
				return &value
			}
		}
		fields := strings.Fields(candidate)
		if len(fields) > 0 {
			if value, err := strconv.ParseFloat(fields[0], 64); err == nil {
				return &value
			}
		}
	}
	return nil
}

func stderrFromResult(result *RunResult) string {
	if result == nil {
		return ""
	}
	return result.Stderr
}
