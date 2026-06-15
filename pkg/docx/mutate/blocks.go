package mutate

import (
	"errors"
	"fmt"

	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/extract"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

var (
	ErrBlockHashMismatch = errors.New("block hash mismatch")
	ErrDeleteLastBlock   = errors.New("cannot delete the last body block")
	ErrBlockHasSectionPr = errors.New("block contains section properties")
)

type ReplaceBlockWithParagraphRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	Index        int
	ExpectedHash string
	Text         string
	Style        string
}

type ReplaceBlockWithParagraphResult struct {
	Index        int             `json:"index"`
	ContentHash  string          `json:"contentHash"`
	PreviousKind model.BlockKind `json:"previousKind"`
	PreviousHash string          `json:"previousHash"`
	PreviousText string          `json:"previousText"`
	Style        string          `json:"style,omitempty"`
	Text         string          `json:"text"`
}

type DeleteBlockRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	Index        int
	ExpectedHash string
}

type DeleteBlockResult struct {
	Index        int             `json:"index"`
	PreviousKind model.BlockKind `json:"previousKind"`
	PreviousHash string          `json:"previousHash"`
	PreviousText string          `json:"previousText"`
}

type InsertParagraphAfterBlockRequest struct {
	Package      opc.PackageSession
	DocumentURI  string
	AfterIndex   int
	ExpectedHash string
	Text         string
	Style        string
}

type InsertParagraphAfterBlockResult struct {
	Index       int    `json:"index"`
	ContentHash string `json:"contentHash"`
	InsertAfter int    `json:"insertAfter"`
	AnchorHash  string `json:"anchorHash,omitempty"`
	Style       string `json:"style,omitempty"`
	Text        string `json:"text"`
}

func ReplaceBlockWithParagraph(req *ReplaceBlockWithParagraphRequest) (*ReplaceBlockWithParagraphResult, error) {
	if req == nil {
		return nil, fmt.Errorf("replace block request is nil")
	}
	doc, bodyElem, block, prefix, err := locateBlock(req.Package, req.DocumentURI, req.Index)
	if err != nil {
		return nil, err
	}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}
	if blockHasSectionProperties(block.Element) {
		return nil, fmt.Errorf("%w: block %d", ErrBlockHasSectionPr, req.Index)
	}

	style := req.Style
	if style == "" && report.Kind == model.BlockKindParagraph && report.Paragraph != nil {
		style = report.Paragraph.Style
	}
	replacement := buildParagraph(doc.Root(), prefix, req.Text, style)
	replaceBodyBlock(bodyElem, block.Element, replacement)
	newReport := extract.ReportBlock(docxbody.BodyBlock{
		Index:   req.Index,
		Kind:    model.BlockKindParagraph,
		Element: replacement,
	}, false)

	ensureDocumentTableScaffolds(doc.Root(), prefix)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &ReplaceBlockWithParagraphResult{
		Index:        req.Index,
		ContentHash:  newReport.ContentHash,
		PreviousKind: report.Kind,
		PreviousHash: report.ContentHash,
		PreviousText: report.Text,
		Style:        style,
		Text:         req.Text,
	}, nil
}

func DeleteBlock(req *DeleteBlockRequest) (*DeleteBlockResult, error) {
	if req == nil {
		return nil, fmt.Errorf("delete block request is nil")
	}
	doc, bodyElem, block, _, err := locateBlock(req.Package, req.DocumentURI, req.Index)
	if err != nil {
		return nil, err
	}
	if len(docxbody.Blocks(bodyElem)) <= 1 {
		return nil, ErrDeleteLastBlock
	}
	if blockHasSectionProperties(block.Element) {
		return nil, fmt.Errorf("%w: block %d", ErrBlockHasSectionPr, req.Index)
	}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}
	bodyElem.RemoveChild(block.Element)

	ensureDocumentTableScaffolds(doc.Root(), doc.Root().Space)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &DeleteBlockResult{
		Index:        req.Index,
		PreviousKind: report.Kind,
		PreviousHash: report.ContentHash,
		PreviousText: report.Text,
	}, nil
}

func InsertParagraphAfterBlock(req *InsertParagraphAfterBlockRequest) (*InsertParagraphAfterBlockResult, error) {
	if req == nil {
		return nil, fmt.Errorf("insert paragraph after block request is nil")
	}
	if req.AfterIndex < 0 {
		return nil, fmt.Errorf("insert-after index must be >= 0")
	}

	if req.AfterIndex == 0 {
		doc, bodyElem, prefix, err := locateBody(req.Package, req.DocumentURI)
		if err != nil {
			return nil, err
		}
		paragraph := buildParagraph(doc.Root(), prefix, req.Text, req.Style)
		if firstBlock := firstBodyBlock(bodyElem); firstBlock != nil {
			bodyElem.InsertChildAt(firstBlock.Index(), paragraph)
		} else {
			appendBodyBlock(bodyElem, paragraph)
		}
		ensureDocumentTableScaffolds(doc.Root(), prefix)
		if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
			return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
		}
		return &InsertParagraphAfterBlockResult{
			Index:       1,
			ContentHash: extract.ReportBlock(docxbody.BodyBlock{Index: 1, Kind: model.BlockKindParagraph, Element: paragraph}, false).ContentHash,
			InsertAfter: 0,
			Style:       req.Style,
			Text:        req.Text,
		}, nil
	}

	doc, bodyElem, block, prefix, err := locateBlock(req.Package, req.DocumentURI, req.AfterIndex)
	if err != nil {
		return nil, err
	}
	report, err := verifyExpectedBlockHash(block, req.ExpectedHash)
	if err != nil {
		return nil, err
	}
	paragraph := buildParagraph(doc.Root(), prefix, req.Text, req.Style)
	bodyElem.InsertChildAt(block.Element.Index()+1, paragraph)
	newReport := extract.ReportBlock(docxbody.BodyBlock{
		Index:   req.AfterIndex + 1,
		Kind:    model.BlockKindParagraph,
		Element: paragraph,
	}, false)

	ensureDocumentTableScaffolds(doc.Root(), prefix)
	if err := req.Package.ReplaceXMLPart(req.DocumentURI, doc); err != nil {
		return nil, fmt.Errorf("failed to replace document part %s: %w", req.DocumentURI, err)
	}
	return &InsertParagraphAfterBlockResult{
		Index:       req.AfterIndex + 1,
		ContentHash: newReport.ContentHash,
		InsertAfter: req.AfterIndex,
		AnchorHash:  report.ContentHash,
		Style:       req.Style,
		Text:        req.Text,
	}, nil
}

func blockHasSectionProperties(block *etree.Element) bool {
	return len(namespaces.FindDescendants(block, namespaces.NsW, "sectPr")) > 0
}

func verifyExpectedBlockHash(block docxbody.BodyBlock, expectedHash string) (extract.BlockReport, error) {
	report := extract.ReportBlock(block, false)
	if expectedHash != "" && expectedHash != report.ContentHash {
		return report, fmt.Errorf("%w: block %d expected %s but found %s", ErrBlockHashMismatch, block.Index, expectedHash, report.ContentHash)
	}
	return report, nil
}

func replaceBodyBlock(bodyElem *etree.Element, oldBlock, newBlock *etree.Element) {
	index := oldBlock.Index()
	bodyElem.RemoveChild(oldBlock)
	bodyElem.InsertChildAt(index, newBlock)
}
