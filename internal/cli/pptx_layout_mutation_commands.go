package cli

import "fmt"

type PPTXLayoutMutationReadbackCommands struct {
	ReadbackCommand            string `json:"readbackCommand,omitempty"`
	LayoutsListCommand         string `json:"layoutsListCommand,omitempty"`
	ValidateCommand            string `json:"validateCommand,omitempty"`
	RenderCommand              string `json:"renderCommand,omitempty"`
	ReadbackCommandTemplate    string `json:"readbackCommandTemplate,omitempty"`
	LayoutsListCommandTemplate string `json:"layoutsListCommandTemplate,omitempty"`
	ValidateCommandTemplate    string `json:"validateCommandTemplate,omitempty"`
	RenderCommandTemplate      string `json:"renderCommandTemplate,omitempty"`
}

func pptxLayoutMutationReadbackCommands(destinationFile string, layoutSelector string) PPTXLayoutMutationReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return PPTXLayoutMutationReadbackCommands{
			ReadbackCommandTemplate:    pptxLayoutReadbackCommand(placeholder, layoutSelector),
			LayoutsListCommandTemplate: pptxLayoutsListCommand(placeholder),
			ValidateCommandTemplate:    pptxValidateCommand(placeholder),
			RenderCommandTemplate:      pptxRenderCommand(placeholder),
		}
	}
	return PPTXLayoutMutationReadbackCommands{
		ReadbackCommand:    pptxLayoutReadbackCommand(destinationFile, layoutSelector),
		LayoutsListCommand: pptxLayoutsListCommand(destinationFile),
		ValidateCommand:    pptxValidateCommand(destinationFile),
		RenderCommand:      pptxRenderCommand(destinationFile),
	}
}

func pptxLayoutReadbackCommand(filePath string, layoutSelector string) string {
	return fmt.Sprintf("ooxml --json pptx layouts show %s --layout %s", pptxXLSXCommandArg(filePath), pptxXLSXCommandArg(layoutSelector))
}

func pptxLayoutsListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json pptx layouts list %s", pptxXLSXCommandArg(filePath))
}
