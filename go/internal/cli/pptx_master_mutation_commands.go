package cli

import "fmt"

type PPTXMasterMutationReadbackCommands struct {
	ReadbackCommand            string `json:"readbackCommand,omitempty"`
	MastersListCommand         string `json:"mastersListCommand,omitempty"`
	ValidateCommand            string `json:"validateCommand,omitempty"`
	RenderCommand              string `json:"renderCommand,omitempty"`
	ReadbackCommandTemplate    string `json:"readbackCommandTemplate,omitempty"`
	MastersListCommandTemplate string `json:"mastersListCommandTemplate,omitempty"`
	ValidateCommandTemplate    string `json:"validateCommandTemplate,omitempty"`
	RenderCommandTemplate      string `json:"renderCommandTemplate,omitempty"`
}

func pptxMasterMutationReadbackCommands(destinationFile string, masterIndex int) PPTXMasterMutationReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return PPTXMasterMutationReadbackCommands{
			ReadbackCommandTemplate:    pptxMasterReadbackCommand(placeholder, masterIndex),
			MastersListCommandTemplate: pptxMastersListCommand(placeholder),
			ValidateCommandTemplate:    pptxValidateCommand(placeholder),
			RenderCommandTemplate:      pptxRenderCommand(placeholder),
		}
	}
	return PPTXMasterMutationReadbackCommands{
		ReadbackCommand:    pptxMasterReadbackCommand(destinationFile, masterIndex),
		MastersListCommand: pptxMastersListCommand(destinationFile),
		ValidateCommand:    pptxValidateCommand(destinationFile),
		RenderCommand:      pptxRenderCommand(destinationFile),
	}
}

func pptxMasterReadbackCommand(filePath string, masterIndex int) string {
	return fmt.Sprintf("ooxml --json pptx masters show %s --master %d", pptxXLSXCommandArg(filePath), masterIndex)
}

func pptxMastersListCommand(filePath string) string {
	return fmt.Sprintf("ooxml --json pptx masters list %s", pptxXLSXCommandArg(filePath))
}
