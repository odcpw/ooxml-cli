package extract

import (
	"fmt"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/core/xmlx"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/inspect"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// NotesReport represents the extracted notes for a slide
type NotesReport struct {
	ID      string               `json:"id"`
	Slide   int                  `json:"slide"`
	PartURI string               `json:"partUri,omitempty"`
	Notes   *model.TextBlockInfo `json:"notes"`
}

// ExtractNotesForSlide extracts notes from a single slide
// Returns nil notes (with empty content) if the slide has no notes rather than an error
func ExtractNotesForSlide(session opc.PackageSession, slideRef inspect.SlideRef) (*NotesReport, error) {
	report := &NotesReport{
		ID:    fmt.Sprintf("slide%d-notes", slideRef.SlideNumber),
		Slide: slideRef.SlideNumber,
	}

	// If no notes part URI, return report with empty notes
	if slideRef.NotesPartURI == "" {
		report.Notes = &model.TextBlockInfo{
			Paragraphs: []model.Paragraph{},
			PlainText:  "",
		}
		return report, nil
	}

	report.PartURI = slideRef.NotesPartURI

	// Read the notes slide XML
	notesDoc, err := session.ReadXMLPart(slideRef.NotesPartURI)
	if err != nil {
		// If we can't read the notes, return empty notes instead of error
		report.Notes = &model.TextBlockInfo{
			Paragraphs: []model.Paragraph{},
			PlainText:  "",
		}
		return report, nil
	}

	// Find the text body in the notes slide
	// The notes slide structure is p:notes -> p:cSld -> p:spTree -> p:sp -> p:txBody
	// We need to find the shape that contains the notes text (type="body")
	notes := extractNotesFromDocument(notesDoc)
	report.Notes = notes

	return report, nil
}

// extractNotesFromDocument extracts text content from a notes slide document
func extractNotesFromDocument(doc *etree.Document) *model.TextBlockInfo {
	if doc == nil || doc.Root() == nil {
		return &model.TextBlockInfo{
			Paragraphs: []model.Paragraph{},
			PlainText:  "",
		}
	}

	root := doc.Root()

	// Find p:cSld element using xmlx helpers
	cSld := xmlx.FindChild(root, namespaces.NsP, "cSld")
	if cSld == nil {
		return &model.TextBlockInfo{
			Paragraphs: []model.Paragraph{},
			PlainText:  "",
		}
	}

	// Find p:spTree element
	spTree := xmlx.FindChild(cSld, namespaces.NsP, "spTree")
	if spTree == nil {
		return &model.TextBlockInfo{
			Paragraphs: []model.Paragraph{},
			PlainText:  "",
		}
	}

	// Iterate through shapes to find the notes body (type="body")
	shapes := xmlx.FindChildren(spTree, namespaces.NsP, "sp")
	for _, sp := range shapes {
		// Check if this is the notes body placeholder
		nvSpPr := xmlx.FindChild(sp, namespaces.NsP, "nvSpPr")
		if nvSpPr == nil {
			continue
		}

		nvPr := xmlx.FindChild(nvSpPr, namespaces.NsP, "nvPr")
		if nvPr == nil {
			continue
		}

		ph := xmlx.FindChild(nvPr, namespaces.NsP, "ph")
		if ph == nil {
			continue
		}

		// Check if this placeholder is of type "body"
		phType := ph.SelectAttrValue("type", "")
		if phType != "body" {
			continue
		}

		// Found the notes placeholder, extract text from p:txBody
		txBody := xmlx.FindChild(sp, namespaces.NsP, "txBody")
		if txBody != nil {
			return inspect.ExtractTextBody(txBody)
		}
	}

	// No notes found, return empty
	return &model.TextBlockInfo{
		Paragraphs: []model.Paragraph{},
		PlainText:  "",
	}
}
