package mutate

import (
	"fmt"
	"sort"
	"testing"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

type mockPackageSession struct {
	xmlParts     map[string][]byte
	rawParts     map[string][]byte
	contentTypes map[string]string
	rels         map[string][]opc.RelationshipInfo
	zipMeta      map[string]*opc.ZipEntryMeta
	removedParts map[string]bool
}

func newMockPackageSession() *mockPackageSession {
	return &mockPackageSession{
		xmlParts:     map[string][]byte{},
		rawParts:     map[string][]byte{},
		contentTypes: map[string]string{},
		rels:         map[string][]opc.RelationshipInfo{},
		zipMeta:      map[string]*opc.ZipEntryMeta{},
		removedParts: map[string]bool{},
	}
}

func (m *mockPackageSession) ListParts() []opc.PartInfo {
	uris := make([]string, 0, len(m.contentTypes))
	for uri := range m.contentTypes {
		if m.removedParts[uri] {
			continue
		}
		uris = append(uris, uri)
	}
	sort.Strings(uris)

	parts := make([]opc.PartInfo, 0, len(uris))
	for _, uri := range uris {
		_, isXML := m.xmlParts[uri]
		size := int64(len(m.rawParts[uri]))
		if data, ok := m.xmlParts[uri]; ok {
			size = int64(len(data))
		}
		parts = append(parts, opc.PartInfo{URI: uri, ContentType: m.contentTypes[uri], SizeBytes: size, IsXML: isXML})
	}
	return parts
}

func (m *mockPackageSession) ListRelationships(sourceURI string) []opc.RelationshipInfo {
	rels := m.rels[sourceURI]
	copied := make([]opc.RelationshipInfo, len(rels))
	copy(copied, rels)
	return copied
}

func (m *mockPackageSession) ReadRawPart(uri string) ([]byte, error) {
	data, ok := m.rawParts[uri]
	if !ok {
		return nil, fmt.Errorf("raw part not found: %s", uri)
	}
	copied := make([]byte, len(data))
	copy(copied, data)
	return copied, nil
}

func (m *mockPackageSession) ReadXMLPart(uri string) (*etree.Document, error) {
	data, ok := m.xmlParts[uri]
	if !ok {
		return nil, fmt.Errorf("xml part not found: %s", uri)
	}
	doc := etree.NewDocument()
	requireNoError(doc.ReadFromBytes(data))
	return doc, nil
}

func (m *mockPackageSession) GetContentType(uri string) string {
	return m.contentTypes[uri]
}

func (m *mockPackageSession) GetZipMeta(uri string) *opc.ZipEntryMeta {
	return m.zipMeta[uri]
}

func (m *mockPackageSession) ReplaceRawPart(uri string, data []byte, contentType string) error {
	copied := make([]byte, len(data))
	copy(copied, data)
	m.rawParts[uri] = copied
	m.contentTypes[uri] = contentType
	return nil
}

func (m *mockPackageSession) ReplaceXMLPart(uri string, doc *etree.Document) error {
	data, err := doc.WriteToBytes()
	if err != nil {
		return err
	}
	m.xmlParts[uri] = data
	if _, ok := m.contentTypes[uri]; !ok {
		m.contentTypes[uri] = "application/xml"
	}
	return nil
}

func (m *mockPackageSession) AddPart(uri string, data []byte, contentType string, meta *opc.ZipEntryMeta) error {
	copied := make([]byte, len(data))
	copy(copied, data)
	m.rawParts[uri] = copied
	m.contentTypes[uri] = contentType
	m.zipMeta[uri] = meta
	delete(m.removedParts, uri)
	return nil
}

func (m *mockPackageSession) RemovePart(uri string) error {
	m.removedParts[uri] = true
	delete(m.xmlParts, uri)
	delete(m.rawParts, uri)
	delete(m.contentTypes, uri)
	return nil
}

func (m *mockPackageSession) SaveAs(path string) error { return nil }
func (m *mockPackageSession) Close() error             { return nil }
func (m *mockPackageSession) IsDirty() bool            { return true }
func (m *mockPackageSession) Warnings() []string       { return nil }

func TestAllocateSlidePartName(t *testing.T) {
	session := newMockPackageSession()
	session.contentTypes["/ppt/slides/slide1.xml"] = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"
	session.contentTypes["/ppt/slides/slide5.xml"] = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"
	session.contentTypes["/ppt/presentation.xml"] = "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"

	uri, err := AllocateSlidePartName(session)
	require.NoError(t, err)
	assert.Equal(t, "/ppt/slides/slide6.xml", uri)
}

func TestAllocateSlideID(t *testing.T) {
	doc := mustReadDoc(t, `
		<p:presentation xmlns:p="`+namespaces.NsP+`" xmlns:r="`+namespaces.NsR+`">
		  <p:sldIdLst>
		    <p:sldId id="256" r:id="rId3"/>
		    <p:sldId id="400" r:id="rId7"/>
		  </p:sldIdLst>
		</p:presentation>`)

	next, err := AllocateSlideID(doc)
	require.NoError(t, err)
	assert.Equal(t, uint32(401), next)
}

func TestAllocateRelationshipID(t *testing.T) {
	rels := []opc.RelationshipInfo{{ID: "rId1"}, {ID: "rId9"}, {ID: "custom"}}
	assert.Equal(t, "rId10", AllocateRelationshipID(rels))
}

func TestBuildDefaultRelationshipDecisions(t *testing.T) {
	rels := []opc.RelationshipInfo{
		{ID: "rId1", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"},
		{ID: "rId2", Type: notesRelationshipType},
		{ID: "rId3", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"},
	}

	decisions := BuildDefaultRelationshipDecisions(rels)
	assert.Equal(t, RelDecisionShared, decisions["rId1"])
	assert.Equal(t, RelDecisionDuplicated, decisions["rId2"])
	assert.Equal(t, RelDecisionShared, decisions["rId3"])
}

func TestRemapRelationships(t *testing.T) {
	rels := []opc.RelationshipInfo{
		{ID: "rId1", Type: slideRelationshipType, Target: "slides/slide1.xml"},
		{ID: "rId2", Type: notesRelationshipType, Target: "notesSlides/notesSlide1.xml"},
	}

	mapped, err := RemapRelationships(&RemapRelationshipsRequest{
		SourceRelationships: rels,
		Decisions: map[string]RelationshipDecision{
			"rId1": RelDecisionShared,
			"rId2": RelDecisionDuplicated,
		},
		TargetMapper: func(oldTarget string, relType string) (string, error) {
			return oldTarget + ".copy", nil
		},
	})
	require.NoError(t, err)
	require.Len(t, mapped, 2)

	assert.Equal(t, "rId1", mapped[0].NewID)
	assert.Equal(t, "slides/slide1.xml", mapped[0].NewTarget)

	assert.NotEqual(t, "rId2", mapped[1].NewID)
	assert.Equal(t, "notesSlides/notesSlide1.xml.copy", mapped[1].NewTarget)
	assert.Equal(t, RelDecisionDuplicated, mapped[1].Decision)
}

func TestUpdateContentTypes(t *testing.T) {
	session := newMockPackageSession()
	session.contentTypes["/ppt/slides/slide3.xml"] = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"

	require.NoError(t, UpdateContentTypes(session, "/ppt/slides/slide3.xml", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"))
	require.NoError(t, UpdateContentTypes(session, "/ppt/slides/slide4.xml", "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"))
	assert.Error(t, UpdateContentTypes(session, "/ppt/slides/slide3.xml", "application/xml"))
}

func TestInsertSlideReference(t *testing.T) {
	doc := mustReadDoc(t, `
		<p:presentation xmlns:p="`+namespaces.NsP+`" xmlns:r="`+namespaces.NsR+`">
		  <p:sldIdLst>
		    <p:sldId id="256" r:id="rId3"/>
		    <p:sldId id="257" r:id="rId7"/>
		  </p:sldIdLst>
		</p:presentation>`)

	result, err := InsertSlideReference(&InsertSlideReferenceRequest{
		PresentationDoc: doc,
		PresentationRelationships: []opc.RelationshipInfo{
			{ID: "rId3"},
			{ID: "rId7"},
		},
		SlidePartURI: "/ppt/slides/slide9.xml",
		SlideID:      300,
		Position:     1,
	})
	require.NoError(t, err)
	assert.Equal(t, "rId8", result.RelationshipID)
	assert.Equal(t, slideRelationshipType, result.Relationship.Type)
	assert.Equal(t, "slides/slide9.xml", result.Relationship.Target)

	root := doc.Root()
	sldIDList := namespaces.FindChild(root, namespaces.NsP, "sldIdLst")
	require.NotNil(t, sldIDList)
	sldIDs := namespaces.FindChildren(sldIDList, namespaces.NsP, "sldId")
	require.Len(t, sldIDs, 3)
	assert.Equal(t, "256", sldIDs[0].SelectAttrValue("id", ""))
	assert.Equal(t, "300", sldIDs[1].SelectAttrValue("id", ""))
	assert.Equal(t, "257", sldIDs[2].SelectAttrValue("id", ""))
	assert.Contains(t, mustWriteDoc(t, doc), `r:id="rId8"`)
}

func TestBuildRelationshipsXML(t *testing.T) {
	data, err := BuildRelationshipsXML([]opc.RelationshipInfo{
		{ID: "rId1", Type: slideRelationshipType, Target: "slides/slide9.xml"},
		{ID: "rId2", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", Target: "https://example.com", TargetMode: "External"},
	})
	require.NoError(t, err)
	assert.Contains(t, string(data), `<?xml version="1.0" encoding="UTF-8"?>`)

	rels, err := opc.ParseRelationships("/ppt/_rels/presentation.xml.rels", data)
	require.NoError(t, err)
	require.Len(t, rels, 2)
	assert.Equal(t, "External", rels[1].TargetMode)
	assert.Equal(t, "https://example.com", rels[1].Target)
}

func TestCloneHelperContract_MixedRelationships(t *testing.T) {
	rels := []opc.RelationshipInfo{
		{ID: "rId1", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout", Target: "../slideLayouts/slideLayout1.xml"},
		{ID: "rId2", Type: notesRelationshipType, Target: "../notesSlides/notesSlide1.xml"},
		{ID: "rId3", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image", Target: "../media/image1.png"},
		{ID: "rId4", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", Target: "https://example.com", TargetMode: "External"},
	}

	decisions := BuildDefaultRelationshipDecisions(rels)
	assert.True(t, IsLayoutOrMasterRelationship(rels[0]))
	assert.True(t, IsNoteSlideRelationship(rels[1]))
	assert.True(t, IsMediaRelationship(rels[2]))
	assert.Equal(t, RelDecisionShared, decisions["rId1"])
	assert.Equal(t, RelDecisionDuplicated, decisions["rId2"])
	assert.Equal(t, RelDecisionShared, decisions["rId3"])
	assert.Equal(t, RelDecisionShared, decisions["rId4"])

	mapped, err := RemapRelationships(&RemapRelationshipsRequest{
		SourceRelationships: rels,
		Decisions:           decisions,
		TargetMapper: func(oldTarget string, relType string) (string, error) {
			if relType == notesRelationshipType {
				return "../notesSlides/notesSlide2.xml", nil
			}
			return oldTarget, nil
		},
	})
	require.NoError(t, err)
	require.Len(t, mapped, 4)
	assert.Equal(t, "rId1", mapped[0].NewID)
	assert.NotEqual(t, "rId2", mapped[1].NewID)
	assert.Equal(t, "../notesSlides/notesSlide2.xml", mapped[1].NewTarget)
	assert.Equal(t, "External", mapped[3].TargetMode)
}

func TestCloneNotes(t *testing.T) {
	session := newMockPackageSession()
	session.xmlParts["/ppt/notesSlides/notesSlide1.xml"] = []byte(`
		<p:notes xmlns:p="` + namespaces.NsP + `">
		  <p:cSld/>
		</p:notes>`)
	session.contentTypes["/ppt/notesSlides/notesSlide1.xml"] = notesContentType
	session.contentTypes["/ppt/slides/slide2.xml"] = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml"
	session.rels["/ppt/slides/slide2.xml"] = []opc.RelationshipInfo{{ID: "rId4"}}

	result, err := CloneNotes(&CloneNotesRequest{
		Session:                  session,
		SourceNotesURI:           "/ppt/notesSlides/notesSlide1.xml",
		DestinationSlideURI:      "/ppt/slides/slide2.xml",
		DestinationRelationships: session.ListRelationships("/ppt/slides/slide2.xml"),
		Policy:                   NotesClone,
	})
	require.NoError(t, err)
	assert.Equal(t, "/ppt/notesSlides/notesSlide2.xml", result.NewNotesURI)
	assert.Equal(t, "rId5", result.NotesRelationship.ID)
	assert.Equal(t, "../notesSlides/notesSlide2.xml", result.NotesRelationship.Target)
	assert.Equal(t, notesContentType, session.GetContentType(result.NewNotesURI))

	dropped, err := CloneNotes(&CloneNotesRequest{Policy: NotesDrop})
	require.NoError(t, err)
	assert.Empty(t, dropped.NewNotesURI)
}

func TestCloneNotesRetargetsSlideBacklinkToDestinationSlide(t *testing.T) {
	session := newMockPackageSession()
	session.xmlParts["/ppt/notesSlides/notesSlide1.xml"] = []byte(`
		<p:notes xmlns:p="` + namespaces.NsP + `">
		  <p:cSld/>
		</p:notes>`)
	session.contentTypes["/ppt/notesSlides/notesSlide1.xml"] = notesContentType
	session.contentTypes["/ppt/slides/slide1.xml"] = slideContentType
	session.contentTypes["/ppt/slides/slide2.xml"] = slideContentType
	session.contentTypes["/ppt/notesMasters/notesMaster1.xml"] = notesMasterContentType
	session.rels["/ppt/notesSlides/notesSlide1.xml"] = []opc.RelationshipInfo{
		{SourceURI: "/ppt/notesSlides/notesSlide1.xml", ID: "rId1", Type: slideRelationshipType, Target: "../slides/slide1.xml"},
		{SourceURI: "/ppt/notesSlides/notesSlide1.xml", ID: "rId2", Type: notesMasterRelationshipType, Target: "../notesMasters/notesMaster1.xml"},
		{SourceURI: "/ppt/notesSlides/notesSlide1.xml", ID: "rId3", Type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink", Target: "https://example.com", TargetMode: "External"},
	}

	result, err := CloneNotes(&CloneNotesRequest{
		Session:                  session,
		SourceNotesURI:           "/ppt/notesSlides/notesSlide1.xml",
		DestinationSlideURI:      "/ppt/slides/slide2.xml",
		DestinationRelationships: nil,
		Policy:                   NotesClone,
	})
	require.NoError(t, err)
	require.Equal(t, "/ppt/notesSlides/notesSlide2.xml", result.NewNotesURI)

	relsURI := "/ppt/notesSlides/_rels/notesSlide2.xml.rels"
	relsXML, ok := session.rawParts[relsURI]
	require.True(t, ok, "expected cloned notes relationships part")
	rels, err := opc.ParseRelationships(relsURI, relsXML)
	require.NoError(t, err)
	require.Len(t, rels, 3)

	assert.Equal(t, slideRelationshipType, rels[0].Type)
	assert.Equal(t, "../slides/slide2.xml", rels[0].Target)
	assert.Equal(t, "/ppt/slides/slide2.xml", opc.ResolveRelationshipTarget(result.NewNotesURI, rels[0].Target))
	assert.Equal(t, notesMasterRelationshipType, rels[1].Type)
	assert.Equal(t, "../notesMasters/notesMaster1.xml", rels[1].Target)
	assert.Equal(t, "External", rels[2].TargetMode)
	assert.Equal(t, "https://example.com", rels[2].Target)
}

func mustReadDoc(t *testing.T, xmlText string) *etree.Document {
	t.Helper()
	doc := etree.NewDocument()
	require.NoError(t, doc.ReadFromString(xmlText))
	return doc
}

func mustWriteDoc(t *testing.T, doc *etree.Document) string {
	t.Helper()
	text, err := doc.WriteToString()
	require.NoError(t, err)
	return text
}

func requireNoError(err error) {
	if err != nil {
		panic(err)
	}
}
