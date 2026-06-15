package extract

import (
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

type ExtractBlocksRequest struct {
	Session     opc.PackageSession
	DocumentURI string
	Block       int
	IncludeRuns bool
}

type ExtractedBlocks struct {
	File            string        `json:"file,omitempty"`
	DocumentPartURI string        `json:"documentPartUri"`
	Blocks          []BlockReport `json:"blocks"`
}

type BlockReport struct {
	ID              string          `json:"id"`
	Index           int             `json:"index"`
	Kind            model.BlockKind `json:"kind"`
	Text            string          `json:"text"`
	PrimarySelector string          `json:"primarySelector,omitempty"`
	Selectors       []string        `json:"selectors,omitempty"`
	// ParaID is the paragraph's w14:paraId marker when one is PHYSICALLY present
	// (read-only). It is the basis for a stable paragraph handle. It is empty for
	// paragraphs that carry no marker yet (inspect/find never inject one) and for
	// table blocks.
	ParaID string `json:"paraId,omitempty"`
	// Handle is the stable paragraph handle (H:docx/pt:doc/para:m:<paraId>) when
	// a w14:paraId marker is physically present; empty otherwise (inspect/find
	// never inject — a mutate does). It is the same string the mutate side
	// accepts via --handle.
	Handle      string         `json:"handle,omitempty"`
	ContentHash string         `json:"contentHash"`
	Paragraph   *ParagraphInfo `json:"paragraph,omitempty"`
	Table       *TableInfo     `json:"table,omitempty"`
}

type ParagraphInfo struct {
	Style string    `json:"style,omitempty"`
	Runs  []RunInfo `json:"runs,omitempty"`
}

type RunInfo struct {
	Text      string `json:"text"`
	Bold      bool   `json:"bold,omitempty"`
	Italic    bool   `json:"italic,omitempty"`
	Underline string `json:"underline,omitempty"`
	Color     string `json:"color,omitempty"`
	Size      string `json:"size,omitempty"`
}

type TableInfo struct {
	Rows []TableRowInfo `json:"rows"`
}

type TableRowInfo struct {
	Cells []TableCellInfo `json:"cells"`
}

type TableCellInfo struct {
	Text string `json:"text"`
}

func ExtractBlocks(req *ExtractBlocksRequest) (*ExtractedBlocks, error) {
	if req == nil {
		return nil, fmt.Errorf("extract blocks request is nil")
	}
	if req.Session == nil {
		return nil, fmt.Errorf("package session is nil")
	}
	if req.DocumentURI == "" {
		return nil, fmt.Errorf("document URI is required")
	}
	if req.Block < 0 {
		return nil, fmt.Errorf("block must be >= 0")
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

	result := &ExtractedBlocks{
		DocumentPartURI: req.DocumentURI,
		Blocks:          make([]BlockReport, 0),
	}
	// Count paraId occurrences across the WHOLE body so a non-unique marker never
	// advertises a handle that would mis-resolve (the AMBIGUITY surface contract:
	// omit the handle for duplicate markers, never resolve them positionally).
	paraIDCounts := docxParaIDCounts(bodyElem)

	for _, block := range docxbody.Blocks(bodyElem) {
		if req.Block > 0 && block.Index != req.Block {
			continue
		}
		report := ReportBlock(block, req.IncludeRuns)
		if report.ParaID != "" && paraIDCounts[docxhandle.NormalizeParaID(report.ParaID)] > 1 {
			report.Handle = ""
		}
		result.Blocks = append(result.Blocks, report)
	}
	if req.Block > 0 && len(result.Blocks) == 0 {
		return nil, fmt.Errorf("block %d not found", req.Block)
	}
	return result, nil
}

// docxParaIDCounts counts how many body paragraphs carry each w14:paraId
// (normalized), so a duplicate marker can be omitted from the handle surface.
func docxParaIDCounts(bodyElem *etree.Element) map[string]int {
	counts := make(map[string]int)
	for _, block := range docxbody.Blocks(bodyElem) {
		if block.Kind != model.BlockKindParagraph {
			continue
		}
		if id := docxhandle.ReadParaID(block.Element); id != "" {
			counts[docxhandle.NormalizeParaID(id)]++
		}
	}
	return counts
}

func ReportBlock(block docxbody.BodyBlock, includeRuns bool) BlockReport {
	report := BlockReport{
		ID:              fmt.Sprintf("body.b%d", block.Index),
		Index:           block.Index,
		Kind:            block.Kind,
		PrimarySelector: fmt.Sprintf("%d", block.Index),
		Selectors:       []string{fmt.Sprintf("%d", block.Index)},
	}

	switch block.Kind {
	case model.BlockKindParagraph:
		text := docxbody.ParagraphText(block.Element)
		style := docxbody.ParagraphStyle(block.Element)
		report.Text = text
		// Pure-read surface of the paragraph's existing w14:paraId marker; never
		// inject. A handle is surfaced only when this is non-empty.
		report.ParaID = docxhandle.ReadParaID(block.Element)
		if report.ParaID != "" {
			report.Handle = docxhandle.FormatParagraph(report.ParaID)
		}
		report.ContentHash = BlockContentHash(block.Kind, style, text)
		report.Paragraph = &ParagraphInfo{Style: style}
		if includeRuns {
			report.Paragraph.Runs = paragraphRuns(block.Element)
		}
	case model.BlockKindTable:
		table := tableInfo(block.Element)
		report.Table = table
		report.Text = tableInfoText(table)
		report.ContentHash = BlockContentHash(block.Kind, "", report.Text)
	default:
		report.Text = docxbody.ParagraphText(block.Element)
		report.ContentHash = BlockContentHash(block.Kind, "", report.Text)
	}
	return report
}

func paragraphRuns(paragraph *etree.Element) []RunInfo {
	runs := make([]RunInfo, 0)
	for _, run := range namespaces.FindChildren(paragraph, namespaces.NsW, "r") {
		text := docxbody.ParagraphText(run)
		info := RunInfo{Text: text}
		if rPr := namespaces.FindChild(run, namespaces.NsW, "rPr"); rPr != nil {
			info.Bold = wordToggleEnabled(namespaces.FindChild(rPr, namespaces.NsW, "b"))
			info.Italic = wordToggleEnabled(namespaces.FindChild(rPr, namespaces.NsW, "i"))
			if underline := namespaces.FindChild(rPr, namespaces.NsW, "u"); underline != nil {
				value, _ := namespaces.Attr(underline, namespaces.NsW, "val")
				if value == "" {
					value = "single"
				}
				if value != "none" && value != "0" {
					info.Underline = value
				}
			}
			if color := namespaces.FindChild(rPr, namespaces.NsW, "color"); color != nil {
				info.Color, _ = namespaces.Attr(color, namespaces.NsW, "val")
			}
			if size := namespaces.FindChild(rPr, namespaces.NsW, "sz"); size != nil {
				info.Size, _ = namespaces.Attr(size, namespaces.NsW, "val")
			}
		}
		if text != "" || info.Bold || info.Italic || info.Underline != "" || info.Color != "" || info.Size != "" {
			runs = append(runs, info)
		}
	}
	return runs
}

func wordToggleEnabled(elem *etree.Element) bool {
	if elem == nil {
		return false
	}
	value, ok := namespaces.Attr(elem, namespaces.NsW, "val")
	if !ok || value == "" {
		return true
	}
	switch strings.ToLower(value) {
	case "0", "false", "off":
		return false
	default:
		return true
	}
}

func tableInfo(tbl *etree.Element) *TableInfo {
	table := &TableInfo{Rows: make([]TableRowInfo, 0)}
	for _, tr := range namespaces.FindChildren(tbl, namespaces.NsW, "tr") {
		row := TableRowInfo{Cells: make([]TableCellInfo, 0)}
		for _, tc := range namespaces.FindChildren(tr, namespaces.NsW, "tc") {
			var paragraphs []string
			for _, p := range namespaces.FindChildren(tc, namespaces.NsW, "p") {
				paragraphs = append(paragraphs, docxbody.ParagraphText(p))
			}
			row.Cells = append(row.Cells, TableCellInfo{Text: strings.Join(paragraphs, "\n")})
		}
		table.Rows = append(table.Rows, row)
	}
	return table
}

func tableInfoText(table *TableInfo) string {
	if table == nil {
		return ""
	}
	lines := make([]string, 0, len(table.Rows))
	for _, row := range table.Rows {
		cells := make([]string, 0, len(row.Cells))
		for _, cell := range row.Cells {
			cells = append(cells, cell.Text)
		}
		lines = append(lines, strings.Join(cells, "\t"))
	}
	return strings.Join(lines, "\n")
}

func BlockContentHash(kind model.BlockKind, style string, text string) string {
	hash := sha256.New()
	hash.Write([]byte(string(kind)))
	hash.Write([]byte{0})
	hash.Write([]byte(style))
	hash.Write([]byte{0})
	hash.Write([]byte(text))
	return "sha256:" + hex.EncodeToString(hash.Sum(nil))
}
