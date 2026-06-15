package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	pptxinspect "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/spf13/cobra"
)

// PPTXCommentsListResult is the JSON shape of pptx comments list.
type PPTXCommentsListResult struct {
	File   string                      `json:"file"`
	Slides []pptxinspect.SlideComments `json:"slides"`
}

func newPPTXCommentsListCmd() *cobra.Command {
	var (
		slide     int
		commentID int
	)
	cmd := &cobra.Command{
		Use:   "list <file>",
		Short: "List comments for a slide or the whole presentation",
		Long: `List slide comments (id, author, date, text, content hash).

By default lists comments across every slide. Use --slide N to restrict to one
slide and --comment-id ID to show a single comment on that slide.`,
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			filePath := args[0]
			if _, err := os.Stat(filePath); err != nil {
				return FileNotFoundError(filePath)
			}
			slideSet := cmd.Flags().Changed("slide")
			commentSet := cmd.Flags().Changed("comment-id")
			if slideSet && slide < 1 {
				return InvalidArgsError("--slide must be >= 1")
			}
			if commentSet && !slideSet {
				return InvalidArgsError("--comment-id requires --slide")
			}

			pkg, err := openPackageExpectType(filePath, opc.PackageTypePPTX)
			if err != nil {
				return err
			}
			defer pkg.Close()

			graph, err := pptxinspect.ParsePresentation(pkg)
			if err != nil {
				return NewCLIErrorf(ExitUnexpected, "failed to parse presentation: %v", err)
			}
			total := len(graph.Slides)
			if slideSet && slide > total {
				return InvalidArgsError(fmt.Sprintf("--slide %d out of range (presentation has %d slides)", slide, total))
			}

			result := &PPTXCommentsListResult{File: filePath, Slides: []pptxinspect.SlideComments{}}
			for i, sr := range graph.Slides {
				number := i + 1
				if slideSet && number != slide {
					continue
				}
				listing, err := pptxinspect.ListSlideComments(pkg, sr.PartURI, number)
				if err != nil {
					return NewCLIErrorf(ExitUnexpected, "failed to list comments: %v", err)
				}
				annotatePPTXCommentSelectors(listing, sr.SlideID)
				if commentSet {
					filtered := make([]pptxinspect.SlideComment, 0, 1)
					for _, c := range listing.Comments {
						if c.ID == commentID {
							filtered = append(filtered, c)
						}
					}
					if len(filtered) == 0 {
						return pptxCommentNotFoundError(listing, commentID)
					}
					listing.Comments = filtered
				}
				result.Slides = append(result.Slides, *listing)
			}

			if GetGlobalConfig(cmd).Format == "json" {
				data, err := marshalWithConfig(GetGlobalConfig(cmd), result)
				if err != nil {
					return NewCLIErrorf(ExitUnexpected, "failed to marshal comments list JSON: %v", err)
				}
				return writeCLIOutput(cmd, data)
			}
			return writeCLIOutput(cmd, []byte(formatPPTXCommentsListText(result)))
		},
	}
	cmd.Flags().IntVar(&slide, "slide", 0, "restrict to a 1-based slide number")
	cmd.Flags().IntVar(&commentID, "comment-id", 0, "show only this comment id (requires --slide)")
	return cmd
}

func formatPPTXCommentsListText(result *PPTXCommentsListResult) string {
	var b strings.Builder
	any := false
	for _, s := range result.Slides {
		for _, c := range s.Comments {
			any = true
			b.WriteString(fmt.Sprintf("slide %d: comment %d by %s", s.Slide, c.ID, c.Author))
			if c.Date != "" {
				b.WriteString(" @ " + c.Date)
			}
			b.WriteString(fmt.Sprintf(" — %q\n", c.Text))
		}
	}
	if !any {
		return "no comments"
	}
	return strings.TrimRight(b.String(), "\n")
}

func init() {
	pptxCommentsCmd.AddCommand(newPPTXCommentsListCmd())
}
