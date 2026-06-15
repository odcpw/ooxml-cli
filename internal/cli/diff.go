package cli

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxdiff "github.com/ooxml-cli/ooxml-cli/pkg/pptx/diff"
	pkgrender "github.com/ooxml-cli/ooxml-cli/pkg/render"
)

var (
	diffRender    bool
	diffThreshold float64
)

var semanticDiffFn = pptxdiff.SemanticDiff
var visualDiffFn = pkgrender.VisualDiff

// DiffResult is the combined semantic/visual diff payload.
type DiffResult struct {
	Semantic *pptxdiff.Report `json:"semantic"`
	Visual   VisualResult     `json:"visual"`
}

// VisualResult is the optional visual diff summary.
type VisualResult struct {
	Enabled   bool              `json:"enabled"`
	Status    string            `json:"status"`
	Threshold float64           `json:"threshold,omitempty"`
	Pass      bool              `json:"pass,omitempty"`
	Slides    []VisualSlideDiff `json:"slides,omitempty"`
}

// VisualSlideDiff reports one slide image comparison.
type VisualSlideDiff struct {
	Slide      int     `json:"slide"`
	Difference float64 `json:"difference"`
	Pass       bool    `json:"pass"`
	DiffImage  string  `json:"diffImage,omitempty"`
}

var diffCmd = &cobra.Command{
	Use:   "diff <baseline> <candidate>",
	Short: "Compare two PPTX presentations",
	Args:  cobra.ExactArgs(2),
	RunE: func(cmd *cobra.Command, args []string) error {
		baselinePath, candidatePath := args[0], args[1]
		if _, err := os.Stat(baselinePath); err != nil {
			return FileNotFoundError(baselinePath)
		}
		if _, err := os.Stat(candidatePath); err != nil {
			return FileNotFoundError(candidatePath)
		}

		baseline, err := openPackageExpectType(baselinePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer baseline.Close()
		candidate, err := openPackageExpectType(candidatePath, opc.PackageTypePPTX)
		if err != nil {
			return err
		}
		defer candidate.Close()

		semantic, err := semanticDiffFn(baseline, candidate)
		if err != nil {
			return NewCLIErrorf(ExitUnexpected, "failed to compute semantic diff: %v", err)
		}

		result := &DiffResult{
			Semantic: semantic,
			Visual:   VisualResult{Enabled: diffRender, Status: "disabled"},
		}
		var deferredErr error

		if diffRender {
			visual, err := runVisualDiff(baselinePath, candidatePath, cmd)
			if err != nil {
				var missing *pkgrender.MissingDependencyError
				var toolFailure *pkgrender.ToolFailureError
				if errors.As(err, &missing) || errors.As(err, &toolFailure) {
					result.Visual = VisualResult{Enabled: true, Status: "unavailable", Threshold: diffThreshold}
					if GetGlobalConfig(cmd).Strict {
						return mapRenderError(err)
					}
					deferredErr = NewCLIError(ExitPartialSuccess, err.Error())
				} else {
					return NewCLIErrorf(ExitUnexpected, "failed to compute visual diff: %v", err)
				}
			} else {
				result.Visual = *visual
				if !visual.Pass {
					deferredErr = DiffThresholdError(fmt.Sprintf("visual difference exceeded threshold %.4f", diffThreshold))
				}
			}
		}

		config := GetGlobalConfig(cmd)
		if config.Format == "json" {
			if err := outputDiffJSON(cmd, result); err != nil {
				return err
			}
			if cliErr, ok := AsCLIError(deferredErr); ok {
				cliErr.Reported = true
			}
		} else {
			if err := outputDiffText(cmd, result); err != nil {
				return err
			}
		}
		return deferredErr
	},
}

func runVisualDiff(baselinePath, candidatePath string, cmd *cobra.Command) (*VisualResult, error) {
	outDir, err := cmd.Flags().GetString("out")
	if err != nil {
		return nil, err
	}
	cleanupDir := false
	if outDir == "" {
		outDir, err = os.MkdirTemp("", "ooxml-diff-*")
		if err != nil {
			return nil, err
		}
		cleanupDir = !GetGlobalConfig(cmd).KeepTemp
	}
	if cleanupDir {
		defer os.RemoveAll(outDir)
	}
	if err := os.MkdirAll(outDir, 0o755); err != nil {
		return nil, err
	}

	baseDir := filepath.Join(outDir, "baseline")
	candDir := filepath.Join(outDir, "candidate")
	diffDir := filepath.Join(outDir, "diff")
	if err := os.MkdirAll(diffDir, 0o755); err != nil {
		return nil, err
	}

	basePDF, err := renderToPDFFn(baselinePath, baseDir)
	if err != nil {
		return nil, err
	}
	candPDF, err := renderToPDFFn(candidatePath, candDir)
	if err != nil {
		return nil, err
	}
	baseImages, err := rasterizeFn(basePDF, baseDir, pkgrender.RasterizeOptions{Format: pkgrender.ImageFormatPNG, DPI: 144, Prefix: "slide"})
	if err != nil {
		return nil, err
	}
	candImages, err := rasterizeFn(candPDF, candDir, pkgrender.RasterizeOptions{Format: pkgrender.ImageFormatPNG, DPI: 144, Prefix: "slide"})
	if err != nil {
		return nil, err
	}

	maxSlides := len(baseImages)
	if len(candImages) > maxSlides {
		maxSlides = len(candImages)
	}
	visual := &VisualResult{Enabled: true, Status: "ok", Threshold: diffThreshold, Pass: true, Slides: make([]VisualSlideDiff, 0, maxSlides)}
	for i := 0; i < maxSlides; i++ {
		slide := i + 1
		entry := VisualSlideDiff{Slide: slide, Pass: false}
		if i >= len(baseImages) || i >= len(candImages) {
			entry.Difference = 1.0
			entry.Pass = false
			visual.Pass = false
			visual.Slides = append(visual.Slides, entry)
			continue
		}
		diffImage := filepath.Join(diffDir, fmt.Sprintf("slide-%d-diff.png", slide))
		difference, err := visualDiffFn(baseImages[i], candImages[i], diffImage)
		if err != nil {
			return nil, err
		}
		entry.Difference = difference
		entry.Pass = difference <= diffThreshold
		if _, err := os.Stat(diffImage); err == nil {
			entry.DiffImage = diffImage
		}
		if !entry.Pass {
			visual.Pass = false
		}
		visual.Slides = append(visual.Slides, entry)
	}
	return visual, nil
}

func outputDiffJSON(cmd *cobra.Command, result *DiffResult) error {
	config := GetGlobalConfig(cmd)
	var (
		data []byte
		err  error
	)
	if config.Pretty {
		data, err = json.MarshalIndent(result, "", "  ")
	} else {
		data, err = json.Marshal(result)
	}
	if err != nil {
		return NewCLIErrorf(ExitUnexpected, "failed to marshal diff JSON: %v", err)
	}
	return writeCLIOutput(cmd, data)
}

func outputDiffText(cmd *cobra.Command, result *DiffResult) error {
	text := fmt.Sprintf("Changed slides: %v\nText diffs: %d\nImage diffs: %d\n", result.Semantic.ChangedSlides, len(result.Semantic.TextDiffs), len(result.Semantic.ImageDiffs))
	if result.Visual.Enabled {
		text += fmt.Sprintf("Visual status: %s\n", result.Visual.Status)
	}
	return writeCLIOutput(cmd, []byte(text))
}

func init() {
	diffCmd.Flags().BoolVar(&diffRender, "render", false, "enable visual diff via rendered slide images")
	diffCmd.Flags().Float64Var(&diffThreshold, "threshold", 0.01, "visual diff threshold")
	diffCmd.Flags().String("out", "", "output directory for visual diff artifacts")
	pptxCmd.AddCommand(diffCmd)
}
