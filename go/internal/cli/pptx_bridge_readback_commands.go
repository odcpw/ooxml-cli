package cli

import "fmt"

type PPTXBridgeReadbackCommands struct {
	ReadbackCommand              string `json:"readbackCommand,omitempty"`
	SlideReadbackCommand         string `json:"slideReadbackCommand,omitempty"`
	ValidateCommand              string `json:"validateCommand,omitempty"`
	RenderCommand                string `json:"renderCommand,omitempty"`
	ReadbackCommandTemplate      string `json:"readbackCommandTemplate,omitempty"`
	SlideReadbackCommandTemplate string `json:"slideReadbackCommandTemplate,omitempty"`
	ValidateCommandTemplate      string `json:"validateCommandTemplate,omitempty"`
	RenderCommandTemplate        string `json:"renderCommandTemplate,omitempty"`
}

func pptxBridgeReadbackCommands(destinationFile string, slide int, objectReadback func(string) string) PPTXBridgeReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return PPTXBridgeReadbackCommands{
			ReadbackCommandTemplate:      objectReadback(placeholder),
			SlideReadbackCommandTemplate: pptxSlideReadbackCommand(placeholder, slide),
			ValidateCommandTemplate:      pptxValidateCommand(placeholder),
			RenderCommandTemplate:        pptxRenderCommand(placeholder),
		}
	}
	return PPTXBridgeReadbackCommands{
		ReadbackCommand:      objectReadback(destinationFile),
		SlideReadbackCommand: pptxSlideReadbackCommand(destinationFile, slide),
		ValidateCommand:      pptxValidateCommand(destinationFile),
		RenderCommand:        pptxRenderCommand(destinationFile),
	}
}

func pptxBridgeOutputVerificationCommands(destinationFile string) PPTXBridgeReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return PPTXBridgeReadbackCommands{
			ValidateCommandTemplate: pptxValidateCommand(placeholder),
			RenderCommandTemplate:   pptxRenderCommand(placeholder),
		}
	}
	return PPTXBridgeReadbackCommands{
		ValidateCommand: pptxValidateCommand(destinationFile),
		RenderCommand:   pptxRenderCommand(destinationFile),
	}
}

func pptxTableMutationReadbackCommands(destination *PPTXTableSummary) PPTXBridgeReadbackCommands {
	if destination == nil {
		return PPTXBridgeReadbackCommands{}
	}
	return pptxBridgeReadbackCommands(destination.File, destination.Slide, func(path string) string {
		return pptxTableReadbackCommand(path, destination.Slide, destination.PrimarySelector)
	})
}

func pptxShapeMutationReadbackCommands(destination *PPTXShapeDestination, includeText, includeBounds bool) PPTXBridgeReadbackCommands {
	if destination == nil {
		return PPTXBridgeReadbackCommands{}
	}
	return pptxBridgeReadbackCommands(destination.File, destination.Slide, func(path string) string {
		return pptxShapeReadbackCommand(path, destination.Slide, destination.PrimarySelector, includeText, includeBounds)
	})
}

func pptxValidateCommand(filePath string) string {
	return fmt.Sprintf("ooxml validate --strict %s", pptxXLSXCommandArg(filePath))
}

func pptxRenderCommand(filePath string) string {
	return fmt.Sprintf("ooxml pptx render %s --out render-check", pptxXLSXCommandArg(filePath))
}

func pptxTableReadbackCommand(filePath string, slide int, target string) string {
	return fmt.Sprintf("ooxml --json pptx tables show %s --slide %d --target %s", pptxXLSXCommandArg(filePath), slide, pptxXLSXCommandArg(target))
}

func pptxShapeReadbackCommand(filePath string, slide int, target string, includeText, includeBounds bool) string {
	command := fmt.Sprintf("ooxml --json pptx shapes get %s --slide %d --target %s", pptxXLSXCommandArg(filePath), slide, pptxXLSXCommandArg(target))
	if includeText {
		command += " --include-text"
	}
	if includeBounds {
		command += " --include-bounds"
	}
	return command
}

func pptxShapeTextReadbackCommand(filePath string, slide int, target string) string {
	return pptxShapeReadbackCommand(filePath, slide, target, true, false)
}

func pptxNotesMutationReadbackCommands(destinationFile string, slide int) PPTXBridgeReadbackCommands {
	return pptxBridgeReadbackCommands(destinationFile, slide, func(path string) string {
		return pptxNotesReadbackCommand(path, slide)
	})
}

func pptxNotesReadbackCommand(filePath string, slide int) string {
	return fmt.Sprintf("ooxml --json pptx notes show %s --slide %d", pptxXLSXCommandArg(filePath), slide)
}
