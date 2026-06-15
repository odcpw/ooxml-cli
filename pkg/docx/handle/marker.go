package handle

import (
	"crypto/rand"
	"encoding/binary"
	"fmt"
	"strings"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/docx/namespaces"
)

// paraIDAttrKey is the literal attribute key Word uses for the durable
// paragraph id. The w14 prefix is declared on the document root (see
// EnsureW14Namespace). The repo's namespace helper only knows the w/r/xml
// prefixes, so paraId is read and written through this literal key.
const paraIDAttrKey = "w14:paraId"

// w14NamespaceAttrKey is the xmlns declaration key for the w14 extension
// namespace on the document root.
const w14NamespaceAttrKey = "xmlns:w14"

// ReadParaID returns the w14:paraId marker physically present on a w:p element,
// or "" when none is set. It is a PURE READ: it never injects. The read path
// (inspect, find) uses this directly so it can surface a paragraph handle ONLY
// when a marker already exists, never causing a write.
func ReadParaID(paragraph *etree.Element) string {
	if paragraph == nil {
		return ""
	}
	if attr := paragraph.SelectAttr(paraIDAttrKey); attr != nil {
		return strings.TrimSpace(attr.Value)
	}
	// Fall back to a Space/Key match in case the document used a different prefix
	// bound to the w14 namespace.
	for _, attr := range paragraph.Attr {
		if attr.Key == "paraId" && (attr.Space == "w14" || attr.Space == namespaces.NsW14) {
			return strings.TrimSpace(attr.Value)
		}
	}
	return ""
}

// NormalizeParaID canonicalizes a w14:paraId for case-insensitive comparison
// (hex values are case-insensitive). It is the same normalization resolution
// uses, so duplicate-marker surface-omission and ambiguous-resolution agree.
func NormalizeParaID(paraID string) string {
	return strings.ToUpper(strings.TrimSpace(paraID))
}

// EnsureW14Namespace idempotently declares xmlns:w14 on the document root so an
// injected w14:paraId is namespace-bound and the part stays well-formed and
// passes validate --strict. It does nothing if the declaration already exists.
func EnsureW14Namespace(root *etree.Element) {
	if root == nil {
		return
	}
	if attr := root.SelectAttr(w14NamespaceAttrKey); attr != nil {
		return
	}
	root.CreateAttr(w14NamespaceAttrKey, namespaces.NsW14)
}

// EnsureParaID returns the durable w14:paraId marker for a paragraph, injecting
// one when the paragraph carries none. It is IDEMPOTENT and INJECTION-SAFE:
//
//   - If the paragraph already has a w14:paraId (whether authored by Word or
//     injected by a previous run), that value is returned and NO write happens
//     (injected reports false). Re-running a mutate therefore never churns the id
//     or adds a second marker.
//   - Otherwise a fresh 8-hex-digit paraId is minted that does NOT collide with
//     any existing paraId in the part (the caller passes the set of existing
//     ids), the w14 namespace is declared on root, and the marker is written
//     (injected reports true).
//
// The returned value is the marker the caller should encode into the handle.
func EnsureParaID(root, paragraph *etree.Element, existing map[string]bool) (paraID string, injected bool) {
	if paragraph == nil {
		return "", false
	}
	if current := ReadParaID(paragraph); current != "" {
		return current, false
	}
	EnsureW14Namespace(root)
	paraID = mintParaID(existing)
	paragraph.CreateAttr(paraIDAttrKey, paraID)
	if existing != nil {
		existing[strings.ToUpper(paraID)] = true
	}
	return paraID, true
}

// CollectParaIDs scans all w:p descendants of a body element and returns a set
// (keyed by upper-cased value) of every w14:paraId already in use, so injection
// can mint a non-colliding marker. paraIds are case-insensitive hex values, so
// the set is keyed upper-case for collision safety.
func CollectParaIDs(body *etree.Element) map[string]bool {
	ids := make(map[string]bool)
	if body == nil {
		return ids
	}
	for _, p := range namespaces.FindDescendants(body, namespaces.NsW, "p") {
		if id := ReadParaID(p); id != "" {
			ids[strings.ToUpper(id)] = true
		}
	}
	return ids
}

// mintParaID generates a fresh 8-hex-digit (uppercase) paraId not present in the
// supplied set. The Open XML SDK validates w14:paraId as a signed long-hex
// extension value, so keep the high bit clear (< 0x80000000).
func mintParaID(existing map[string]bool) string {
	for {
		var buf [4]byte
		if _, err := rand.Read(buf[:]); err != nil {
			// rand.Read on a healthy system does not fail; if it ever does, fall
			// back to a deterministic-but-still-unique value derived from the set
			// size so the loop terminates rather than panicking.
			n := uint32(len(existing) + 1)
			binary.BigEndian.PutUint32(buf[:], n)
		}
		id := fmt.Sprintf("%08X", binary.BigEndian.Uint32(buf[:])&0x7fffffff)
		if existing == nil || !existing[id] {
			return id
		}
	}
}
