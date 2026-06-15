package cli

import "github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"

func layoutNameByURI(graph *inspect.PresentationGraph, layoutURI string) string {
	if graph == nil {
		return ""
	}
	for _, layout := range graph.Layouts {
		if layout.PartURI == layoutURI {
			return layout.Name
		}
	}
	return ""
}

func layoutNameExists(graph *inspect.PresentationGraph, name string, exceptURI string) bool {
	if graph == nil || name == "" {
		return false
	}
	for _, layout := range graph.Layouts {
		if layout.Name == name && layout.PartURI != exceptURI {
			return true
		}
	}
	return false
}
