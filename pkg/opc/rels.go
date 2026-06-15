package opc

import (
	"encoding/xml"
	"fmt"
	"path/filepath"
	"strconv"
	"strings"
)

// RelationshipInfo is defined in types.go

const ContentTypeRelationships = "application/vnd.openxmlformats-package.relationships+xml"

// relRoot is the XML structure of a .rels file.
type relRoot struct {
	XMLName       xml.Name `xml:"http://schemas.openxmlformats.org/package/2006/relationships Relationships"`
	Relationships []rel    `xml:"Relationship"`
}

type rel struct {
	XMLName    xml.Name `xml:"http://schemas.openxmlformats.org/package/2006/relationships Relationship"`
	ID         string   `xml:"Id,attr"`
	Type       string   `xml:"Type,attr"`
	Target     string   `xml:"Target,attr"`
	TargetMode string   `xml:"TargetMode,attr,omitempty"`
}

// ParseRelationships parses a .rels file and returns the relationships.
func ParseRelationships(sourceURI string, data []byte) ([]RelationshipInfo, error) {
	var root relRoot
	if err := xml.Unmarshal(data, &root); err != nil {
		return nil, fmt.Errorf("failed to parse relationships for %s: %w", sourceURI, err)
	}

	rels := make([]RelationshipInfo, 0, len(root.Relationships))
	for _, r := range root.Relationships {
		rels = append(rels, RelationshipInfo{
			SourceURI:  sourceURI,
			ID:         r.ID,
			Type:       r.Type,
			Target:     r.Target,
			TargetMode: r.TargetMode,
		})
	}
	return rels, nil
}

// ResolveRelationshipTarget resolves a relative relationship target against the source URI.
// For example:
// - source: "/ppt/slides/slide1.xml"
// - target: "../slideLayouts/slideLayout1.xml"
// - result: "/ppt/slideLayouts/slideLayout1.xml"
func ResolveRelationshipTarget(sourceURI, target string) string {
	if target == "" {
		return ""
	}
	sourceURI = NormalizeURI(sourceURI)
	target = strings.ReplaceAll(target, "\\", "/")

	if IsExternalRelationshipTarget(target) {
		return target
	}

	if strings.HasPrefix(target, "/") {
		return NormalizeURI(target)
	}

	sourceDir := "/"
	if sourceURI != "/" {
		lastSlash := strings.LastIndex(sourceURI, "/")
		if lastSlash > 0 {
			sourceDir = sourceURI[:lastSlash]
		}
	}
	return JoinPaths(sourceDir, target)
}

// IsExternalRelationshipTarget reports whether a relationship target is a URI
// that should not be normalized as an internal package part path.
func IsExternalRelationshipTarget(target string) bool {
	lowered := strings.ToLower(strings.TrimSpace(target))
	return strings.Contains(lowered, "://") ||
		strings.HasPrefix(lowered, "mailto:") ||
		strings.HasPrefix(lowered, "file:") ||
		strings.HasPrefix(lowered, "urn:")
}

// RelsURIForPart returns the package URI for a source part's relationship part.
// The package-level source "/" maps to "/_rels/.rels".
func RelsURIForPart(sourceURI string) string {
	sourceURI = NormalizeURI(sourceURI)
	if sourceURI == "/" {
		return "/_rels/.rels"
	}

	dir := GetDirectory(sourceURI)
	fileName := GetFileName(sourceURI)
	relsName := "_rels/" + fileName + ".rels"
	if dir == "/" {
		return NormalizeURI("/" + relsName)
	}
	return JoinPaths(dir, relsName)
}

// RelationshipTarget returns a relative relationship target from sourceURI to targetURI.
func RelationshipTarget(sourceURI, targetURI string) string {
	targetURI = NormalizeURI(targetURI)
	targetPath := strings.TrimPrefix(targetURI, "/")

	sourceDir := "."
	sourceURI = NormalizeURI(sourceURI)
	if sourceURI != "/" {
		sourceDir = strings.TrimPrefix(GetDirectory(sourceURI), "/")
		if sourceDir == "" {
			sourceDir = "."
		}
	}

	relPath, err := filepath.Rel(sourceDir, targetPath)
	if err != nil || relPath == "." {
		return targetPath
	}
	return filepath.ToSlash(relPath)
}

// AllocateRelationshipID returns the next unused rIdN identifier.
func AllocateRelationshipID(rels []RelationshipInfo) string {
	used := make(map[string]bool, len(rels))
	maxN := 0
	for _, rel := range rels {
		used[rel.ID] = true
		if !strings.HasPrefix(rel.ID, "rId") {
			continue
		}
		n, err := strconv.Atoi(strings.TrimPrefix(rel.ID, "rId"))
		if err == nil && n > maxN {
			maxN = n
		}
	}

	for i := maxN + 1; ; i++ {
		id := fmt.Sprintf("rId%d", i)
		if !used[id] {
			return id
		}
	}
}

// BuildRelationshipsXML serializes relationship metadata into an OPC .rels part.
func BuildRelationshipsXML(rels []RelationshipInfo) ([]byte, error) {
	root := relRoot{
		XMLName:       xml.Name{Space: "http://schemas.openxmlformats.org/package/2006/relationships", Local: "Relationships"},
		Relationships: make([]rel, 0, len(rels)),
	}
	for _, item := range rels {
		root.Relationships = append(root.Relationships, rel{
			XMLName:    xml.Name{Space: "http://schemas.openxmlformats.org/package/2006/relationships", Local: "Relationship"},
			ID:         item.ID,
			Type:       item.Type,
			Target:     item.Target,
			TargetMode: item.TargetMode,
		})
	}

	data, err := xml.MarshalIndent(root, "", "  ")
	if err != nil {
		return nil, err
	}
	return append([]byte(xml.Header), data...), nil
}

// WriteRelationships replaces or creates the relationship part for sourceURI.
func WriteRelationships(session PackageSession, sourceURI string, rels []RelationshipInfo) error {
	data, err := BuildRelationshipsXML(rels)
	if err != nil {
		return err
	}
	return session.ReplaceRawPart(RelsURIForPart(sourceURI), data, ContentTypeRelationships)
}
