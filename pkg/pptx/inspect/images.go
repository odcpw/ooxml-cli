package inspect

import (
	"fmt"
	"path/filepath"

	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/namespaces"
)

// EnumerateImageRelationships extracts all image relationships from a slide or layout XML part.
// sourcePartURI is the URI of the slide/layout (e.g., /ppt/slides/slide1.xml)
// session is the OPC package session for resolving relationships
// spTree is the shape tree element containing pictures
func EnumerateImageRelationships(sourcePartURI string, session opc.PackageSession, spTree *etree.Element) []model.ExtractedImageInfo {
	var images []model.ExtractedImageInfo

	if spTree == nil {
		return images
	}

	// Get all relationships for this part
	relationships := session.ListRelationships(sourcePartURI)
	relMap := make(map[string]opc.RelationshipInfo)
	for _, rel := range relationships {
		relMap[rel.ID] = rel
	}

	// Enumerate all pictures in the shape tree
	pics := spTree.FindElements("pic")
	if len(pics) == 0 {
		pics = spTree.FindElements("{" + namespaces.NsP + "}pic")
	}
	for _, pic := range pics {
		imgInfo := extractImageInfo(sourcePartURI, pic, relMap, session)
		if imgInfo != nil {
			images = append(images, *imgInfo)
		}
	}

	return images
}

// extractImageInfo extracts image information from a p:pic element
func extractImageInfo(sourcePartURI string, pic *etree.Element, relMap map[string]opc.RelationshipInfo, session opc.PackageSession) *model.ExtractedImageInfo {
	info := &model.ExtractedImageInfo{
		SourcePartURI: sourcePartURI,
	}

	// Get picture ID and name from p:nvPicPr/p:cNvPr
	nvPicPr := pic.FindElement("nvPicPr")
	if nvPicPr == nil {
		nvPicPr = pic.FindElement("{" + namespaces.NsP + "}nvPicPr")
	}
	if nvPicPr != nil {
		cNvPr := nvPicPr.FindElement("cNvPr")
		if cNvPr == nil {
			cNvPr = nvPicPr.FindElement("{" + namespaces.NsP + "}cNvPr")
		}
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			if idStr != "" {
				if id, err := stringToInt(idStr); err == nil {
					info.ShapeID = id
				}
			}
			info.ShapeName = cNvPr.SelectAttrValue("name", "")
		}
	}

	// Get image reference from p:blipFill/a:blip@r:embed
	blipFill := pic.FindElement("blipFill")
	if blipFill == nil {
		blipFill = pic.FindElement("{" + namespaces.NsP + "}blipFill")
	}
	if blipFill == nil {
		return nil
	}

	blip := blipFill.FindElement("blip")
	if blip == nil {
		blip = blipFill.FindElement("{" + namespaces.NsA + "}blip")
	}
	if blip == nil {
		return nil
	}

	// Extract the r:embed attribute
	relID := ""
	for _, attr := range blip.Attr {
		if attr.Key == "embed" && attr.Space == "r" {
			relID = attr.Value
			break
		}
	}

	if relID == "" {
		relID = blip.SelectAttrValue("embed", "")
	}

	if relID == "" {
		return nil
	}

	info.RelationshipID = relID

	// Resolve the relationship
	rel, exists := relMap[relID]
	if !exists {
		return nil
	}

	// Resolve the target URI
	targetURI := opc.ResolveRelationshipTarget(sourcePartURI, rel.Target)
	info.TargetURI = targetURI

	// Get content type
	contentType := session.GetContentType(targetURI)
	info.ContentType = contentType

	// Extract filename from target URI
	filename := filepath.Base(targetURI)
	info.FilePath = filename

	// Get file size
	data, err := session.ReadRawPart(targetURI)
	if err != nil {
		return nil
	}
	info.FileSize = int64(len(data))

	// Extract geometry from p:spPr/a:xfrm and a:srcRect
	spPr := pic.FindElement("spPr")
	if spPr != nil {
		xfrm := spPr.FindElement("xfrm")
		if xfrm != nil {
			info.Geometry = extractImageGeometry(xfrm, blipFill)
		}
	}

	return info
}

// stringToInt is a helper function to convert string to int
func stringToInt(s string) (int, error) {
	var result int
	_, err := fmt.Sscanf(s, "%d", &result)
	return result, err
}

// extractImageGeometry extracts geometry information from a:xfrm and a:srcRect
func extractImageGeometry(xfrm *etree.Element, blipFill *etree.Element) *model.Geometry {
	if xfrm == nil {
		return nil
	}

	geom := &model.Geometry{}

	// Parse rotation from a:xfrm@rot attribute (in 60,000ths of a degree)
	if rotStr := xfrm.SelectAttrValue("rot", ""); rotStr != "" {
		if rot, err := stringToInt(rotStr); err == nil && rot != 0 {
			geom.Rotation = rot
		}
	}

	// Parse flip attributes from a:xfrm
	if flipHStr := xfrm.SelectAttrValue("flipH", ""); flipHStr == "1" {
		geom.FlipH = true
	}
	if flipVStr := xfrm.SelectAttrValue("flipV", ""); flipVStr == "1" {
		geom.FlipV = true
	}

	// Parse crop from a:srcRect if available
	if blipFill != nil {
		srcRect := blipFill.FindElement("srcRect")
		if srcRect == nil {
			srcRect = blipFill.FindElement("{http://schemas.openxmlformats.org/drawingml/2006/main}srcRect")
		}
		if srcRect != nil {
			crop := &model.CropInfo{}
			if lStr := srcRect.SelectAttrValue("l", ""); lStr != "" {
				if l, err := stringToInt(lStr); err == nil {
					crop.Left = l
				}
			}
			if tStr := srcRect.SelectAttrValue("t", ""); tStr != "" {
				if t, err := stringToInt(tStr); err == nil {
					crop.Top = t
				}
			}
			if rStr := srcRect.SelectAttrValue("r", ""); rStr != "" {
				if r, err := stringToInt(rStr); err == nil {
					crop.Right = r
				}
			}
			if bStr := srcRect.SelectAttrValue("b", ""); bStr != "" {
				if b, err := stringToInt(bStr); err == nil {
					crop.Bottom = b
				}
			}
			geom.Crop = crop
		}
	}

	// Only return geometry if it has actual values
	if geom.Rotation == 0 && !geom.FlipH && !geom.FlipV && geom.Crop == nil {
		return nil
	}

	return geom
}
