package mutate

import (
	"github.com/beevik/etree"
	docxbody "github.com/ooxml-cli/ooxml-cli/pkg/docx/body"
	docxhandle "github.com/ooxml-cli/ooxml-cli/pkg/docx/handle"
)

// ensureParagraphMarker injects (or reuses) a durable w14:paraId marker on the
// target paragraph so it becomes durably addressable by a handle, and returns
// that marker. This is the LAZY-UPGRADE step of PR-HANDLES-1 phase 3: any
// mutation that targets a paragraph (by ANY selector — block index or handle)
// calls this so the mutated paragraph gains a stable handle, returned to the
// caller in its readback.
//
// It is IDEMPOTENT: a paragraph already carrying a w14:paraId keeps it (no
// second marker is added and the id never churns), and the minted id is
// guaranteed unique within the part. Only inspect/find must NOT call this — they
// stay pure-read (see pkg/docx/handle.ReadParaID).
//
// root is the document root (for declaring xmlns:w14); body is the body element
// used to collect existing paraIds for collision-free minting.
func ensureParagraphMarker(root, body, paragraph *etree.Element) string {
	if paragraph == nil {
		return ""
	}
	existing := docxhandle.CollectParaIDs(body)
	paraID, _ := docxhandle.EnsureParaID(root, paragraph, existing)
	return paraID
}

// stampMarkerForParagraph injects/reuses a durable marker on the given
// paragraph, locating the body via the document root to collect existing
// paraIds for collision-free minting. It returns "" when root or paragraph is
// nil or the body cannot be found.
func stampMarkerForParagraph(root, paragraph *etree.Element) string {
	if root == nil || paragraph == nil {
		return ""
	}
	bodyElem, err := docxbody.FindBody(root)
	if err != nil {
		return ""
	}
	return ensureParagraphMarker(root, bodyElem, paragraph)
}
