package inspect

import (
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/xlsx/namespaces"
)

func TestConventionalCommentsPartURI(t *testing.T) {
	tests := map[string]string{
		"/xl/worksheets/sheet1.xml":  "/xl/comments1.xml",
		"/xl/worksheets/sheet12.xml": "/xl/comments12.xml",
		"xl/worksheets/sheet3.xml":   "/xl/comments3.xml",
		"/xl/worksheets/custom.xml":  "/xl/comments1.xml",
	}
	for in, want := range tests {
		if got := ConventionalCommentsPartURI(in); got != want {
			t.Fatalf("ConventionalCommentsPartURI(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestFindCommentsPartRelAndFallback(t *testing.T) {
	worksheet := "/xl/worksheets/sheet1.xml"

	// No part present: fall back to conventional URI, exists=false.
	empty := &commentsMockSession{}
	uri, exists := FindCommentsPart(empty, worksheet)
	if uri != "/xl/comments1.xml" || exists {
		t.Fatalf("empty: uri=%q exists=%v", uri, exists)
	}

	// Relationship-resolved path with the part present.
	withRel := &commentsMockSession{
		parts: []opc.PartInfo{{URI: "/xl/comments9.xml"}},
		rels: map[string][]opc.RelationshipInfo{
			worksheet: {{ID: "rId1", Type: namespaces.RelComments, Target: "../comments9.xml"}},
		},
	}
	uri, exists = FindCommentsPart(withRel, worksheet)
	if uri != "/xl/comments9.xml" || !exists {
		t.Fatalf("withRel: uri=%q exists=%v", uri, exists)
	}
}

func TestListCommentsParsesAuthorsAndAnchors(t *testing.T) {
	worksheet := "/xl/worksheets/sheet1.xml"
	commentsXML := `<?xml version="1.0" encoding="UTF-8"?>
<comments xmlns="` + namespaces.NsSpreadsheetML + `">
  <authors><author>Ann</author><author>Bob</author></authors>
  <commentList>
    <comment ref="A1" authorId="0" date="2024-01-01T00:00:00Z" initials="A"><text><t>first</t></text></comment>
    <comment ref="B2" authorId="1"><text><r><t>rich </t></r><r><t>note</t></r></text></comment>
  </commentList>
</comments>`
	session := &commentsMockSession{
		parts: []opc.PartInfo{{URI: "/xl/comments1.xml"}},
		xml:   map[string]string{"/xl/comments1.xml": commentsXML},
	}
	listing, err := ListComments(session, model.SheetRef{PartURI: worksheet})
	if err != nil {
		t.Fatalf("ListComments: %v", err)
	}
	if listing.CommentsPart != "/xl/comments1.xml" || len(listing.Comments) != 2 {
		t.Fatalf("unexpected listing: %+v", listing)
	}
	first := listing.Comments[0]
	if first.ID != 0 || first.Author != "Ann" || first.Text != "first" || first.AnchoredToCell != "A1" {
		t.Fatalf("unexpected first comment: %+v", first)
	}
	second := listing.Comments[1]
	if second.ID != 1 || second.Author != "Bob" || second.Text != "rich note" || second.AnchoredToCell != "B2" || second.AnchoredToCellColumn != 2 || second.AnchoredToCellRow != 2 {
		t.Fatalf("unexpected second comment: %+v", second)
	}
}

func TestCommentContentHashStableAndDistinct(t *testing.T) {
	a := CommentContentHash("Ann", "text")
	b := CommentContentHash("Ann", "text")
	c := CommentContentHash("Bob", "text")
	if a != b {
		t.Fatalf("hash not stable: %q != %q", a, b)
	}
	if a == c {
		t.Fatalf("hash should differ for different author")
	}
}

// commentsMockSession is a minimal read-only session for comments inspection.
type commentsMockSession struct {
	opc.PackageSession
	parts []opc.PartInfo
	rels  map[string][]opc.RelationshipInfo
	xml   map[string]string
}

func (m *commentsMockSession) ListParts() []opc.PartInfo { return m.parts }

func (m *commentsMockSession) ListRelationships(sourceURI string) []opc.RelationshipInfo {
	return m.rels[sourceURI]
}

func (m *commentsMockSession) ReadXMLPart(uri string) (*etree.Document, error) {
	doc := etree.NewDocument()
	if err := doc.ReadFromString(m.xml[uri]); err != nil {
		return nil, err
	}
	return doc, nil
}
