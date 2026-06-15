package cli

type PPTXSlidesMutationReadbackCommands struct {
	SlidesListCommand         string `json:"slidesListCommand,omitempty"`
	ValidateCommand           string `json:"validateCommand,omitempty"`
	RenderCommand             string `json:"renderCommand,omitempty"`
	SlidesListCommandTemplate string `json:"slidesListCommandTemplate,omitempty"`
	ValidateCommandTemplate   string `json:"validateCommandTemplate,omitempty"`
	RenderCommandTemplate     string `json:"renderCommandTemplate,omitempty"`
}

func pptxSlidesMutationReadbackCommands(destinationFile string) PPTXSlidesMutationReadbackCommands {
	if destinationFile == "" {
		placeholder := outputPlaceholder()
		return PPTXSlidesMutationReadbackCommands{
			SlidesListCommandTemplate: pptxSlidesListCommand(placeholder),
			ValidateCommandTemplate:   pptxValidateCommand(placeholder),
			RenderCommandTemplate:     pptxRenderCommand(placeholder),
		}
	}
	return PPTXSlidesMutationReadbackCommands{
		SlidesListCommand: pptxSlidesListCommand(destinationFile),
		ValidateCommand:   pptxValidateCommand(destinationFile),
		RenderCommand:     pptxRenderCommand(destinationFile),
	}
}
