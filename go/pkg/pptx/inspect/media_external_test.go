package inspect

import (
	"testing"

	"github.com/beevik/etree"
	ns "github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// Finding 3: external-linked media (TargetMode=External) must be reported with
// the raw target verbatim, IsExternal=true, and NOT flagged stale/missing-part.
// resolveRel must branch on TargetMode BEFORE path-joining the target.

func TestResolveRel_ExternalTargets(t *testing.T) {
	const partURI = "/ppt/slides/slide1.xml"
	cases := []struct {
		name   string
		target string
	}{
		{"fileURL", "file:///C:/Videos/butti.mp4"},
		{"driveLetter", "C:\\Videos\\butti.mp4"},
		{"unc", "\\\\server\\share\\media\\butti.mp4"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			relMap := map[string]opc.RelationshipInfo{
				"rId5": {ID: "rId5", Type: relTypeVideoForTest, Target: tc.target, TargetMode: "External"},
			}
			// partSet deliberately does NOT contain the external target; an external
			// rel must never consult partSet nor be flagged missing-part.
			uri, reason, external := resolveRel(partURI, "rId5", relMap, map[string]bool{})
			if !external {
				t.Errorf("external=false; expected true for TargetMode=External")
			}
			if uri != tc.target {
				t.Errorf("uri = %q, want raw target %q (unmunged)", uri, tc.target)
			}
			if reason != "" {
				t.Errorf("reason = %q; external target must not be flagged stale", reason)
			}
		})
	}
}

// TestResolveRel_ExternalCaseInsensitive confirms the TargetMode check is
// case-insensitive (PowerPoint emits "External"; tolerate any casing).
func TestResolveRel_ExternalCaseInsensitive(t *testing.T) {
	relMap := map[string]opc.RelationshipInfo{
		"rId1": {ID: "rId1", Target: "file:///D:/clip.mp4", TargetMode: "external"},
	}
	uri, reason, external := resolveRel("/ppt/slides/slide1.xml", "rId1", relMap, map[string]bool{})
	if !external || uri != "file:///D:/clip.mp4" || reason != "" {
		t.Fatalf("case-insensitive external check failed: external=%v uri=%q reason=%q", external, uri, reason)
	}
}

// TestResolveRel_InternalUnaffected confirms internal rels keep their existing
// behavior (resolve + missing-part / dangling-rel checks).
func TestResolveRel_InternalUnaffected(t *testing.T) {
	relMap := map[string]opc.RelationshipInfo{
		"rId2": {ID: "rId2", Target: "../media/media1.mp4"}, // internal, no TargetMode
	}
	partSet := map[string]bool{"/ppt/media/media1.mp4": true}
	uri, reason, external := resolveRel("/ppt/slides/slide1.xml", "rId2", relMap, partSet)
	if external {
		t.Error("internal rel marked external")
	}
	if uri != "/ppt/media/media1.mp4" || reason != "" {
		t.Errorf("internal rel resolution wrong: uri=%q reason=%q", uri, reason)
	}

	// Missing internal part -> missing-part, not external.
	_, reason2, ext2 := resolveRel("/ppt/slides/slide1.xml", "rId2", relMap, map[string]bool{})
	if ext2 || reason2 == "" {
		t.Errorf("missing internal part should be stale missing-part, got external=%v reason=%q", ext2, reason2)
	}
}

// TestMediaFromPic_ExternalVideo asserts the full mediaFromPic path on a pic that
// links an external video via a:videoFile r:link: IsExternal=true, the
// MediaPartURI is the raw target (unmunged), and the clip is NOT stale.
func TestMediaFromPic_ExternalVideo(t *testing.T) {
	const partURI = "/ppt/slides/slide1.xml"
	const externalTarget = "file:///C:/Videos/butti.mp4"

	pic := parsePicXML(t, `<p:pic xmlns:p="`+ns.NsP+`" xmlns:a="`+ns.NsA+`" xmlns:r="`+ns.NsR+`">
  <p:nvPicPr>
    <p:cNvPr id="7" name="Butti Video"/>
    <p:cNvPicPr/>
    <p:nvPr>
      <a:videoFile r:link="rId5"/>
    </p:nvPr>
  </p:nvPicPr>
  <p:blipFill>
    <a:blip r:embed="rId6"/>
  </p:blipFill>
  <p:spPr/>
</p:pic>`)

	relMap := map[string]opc.RelationshipInfo{
		"rId5": {ID: "rId5", Type: relTypeVideoForTest, Target: externalTarget, TargetMode: "External"},
		"rId6": {ID: "rId6", Type: relTypeImageForTest, Target: "../media/image1.png"},
	}
	partSet := map[string]bool{"/ppt/media/image1.png": true}

	mi, ok := mediaFromPic(nil, partURI, pic, relMap, partSet)
	if !ok {
		t.Fatal("mediaFromPic returned ok=false for a media pic")
	}
	if !mi.IsExternal {
		t.Error("IsExternal=false; expected true for external video link")
	}
	if mi.MediaPartURI != externalTarget {
		t.Errorf("MediaPartURI = %q, want raw external target %q", mi.MediaPartURI, externalTarget)
	}
	if mi.Stale {
		t.Errorf("external media flagged stale: %s", mi.StaleReason)
	}
	if mi.Kind != "video" {
		t.Errorf("kind = %q, want video", mi.Kind)
	}
}

// relTypeVideoForTest / relTypeImageForTest mirror the OOXML rel type strings used
// for media; declared here so the inspect-package test is self-contained.
const (
	relTypeVideoForTest = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video"
	relTypeImageForTest = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
)

func parsePicXML(t *testing.T, xml string) *etree.Element {
	t.Helper()
	doc := etree.NewDocument()
	if err := doc.ReadFromString(xml); err != nil {
		t.Fatalf("parse pic xml: %v", err)
	}
	return doc.Root()
}
