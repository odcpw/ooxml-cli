package extract

import (
	"fmt"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

type ExtractTextRequest struct {
	Session     opc.PackageSession
	DocumentURI string
}

type ExtractedDocument struct {
	File   string        `json:"file,omitempty"`
	Blocks []model.Block `json:"blocks"`
}

func ExtractText(req *ExtractTextRequest) (*ExtractedDocument, error) {
	if req == nil {
		return nil, fmt.Errorf("extract text request is nil")
	}
	if req.Session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.DocumentURI == "" {
		return nil, fmt.Errorf("document URI is required")
	}

	doc, err := req.Session.ReadXMLPart(req.DocumentURI)
	if err != nil {
		return nil, fmt.Errorf("failed to read document part %s: %w", req.DocumentURI, err)
	}
	root := doc.Root()
	if root == nil || !namespaces.IsElement(root, namespaces.NsW, "document") {
		return nil, fmt.Errorf("document root element not found")
	}
	bodyElem, err := docxbody.FindBody(root)
	if err != nil {
		return nil, err
	}

	paraIDCounts := docxParaIDCounts(bodyElem)
	result := &ExtractedDocument{Blocks: make([]model.Block, 0)}
	for _, block := range docxbody.Blocks(bodyElem) {
		switch block.Kind {
		case model.BlockKindParagraph:
			b := model.Block{
				Index: block.Index,
				Kind:  block.Kind,
				Style: docxbody.ParagraphStyle(block.Element),
				Text:  docxbody.ParagraphText(block.Element),
			}
			// Pure-read marker surface; omit the handle for a non-unique marker.
			b.ParaID = docxhandle.ReadParaID(block.Element)
			if b.ParaID != "" && paraIDCounts[docxhandle.NormalizeParaID(b.ParaID)] == 1 {
				b.Handle = docxhandle.FormatParagraph(b.ParaID)
			}
			result.Blocks = append(result.Blocks, b)
		case model.BlockKindTable:
			table := extractTable(block.Element)
			result.Blocks = append(result.Blocks, model.Block{
				Index: block.Index,
				Kind:  block.Kind,
				Text:  tableText(table),
				Table: table,
			})
		}
	}

	return result, nil
}

func extractTable(tbl *etree.Element) *model.Table {
	table := &model.Table{Rows: make([]model.TableRow, 0)}
	for _, tr := range namespaces.FindChildren(tbl, namespaces.NsW, "tr") {
		row := model.TableRow{Cells: make([]string, 0)}
		for _, tc := range namespaces.FindChildren(tr, namespaces.NsW, "tc") {
			var paragraphs []string
			for _, p := range namespaces.FindChildren(tc, namespaces.NsW, "p") {
				paragraphs = append(paragraphs, docxbody.ParagraphText(p))
			}
			row.Cells = append(row.Cells, strings.Join(paragraphs, "\n"))
		}
		table.Rows = append(table.Rows, row)
	}
	return table
}

func tableText(table *model.Table) string {
	if table == nil {
		return ""
	}
	lines := make([]string, 0, len(table.Rows))
	for _, row := range table.Rows {
		lines = append(lines, strings.Join(row.Cells, "\t"))
	}
	return strings.Join(lines, "\n")
}
