package inspect

import (
	"github.com/beevik/etree"
	"github.com/ooxml-cli/ooxml-cli/pkg/pptx/model"
	"strconv"
)

// EnumerateShapes traverses the shape tree and returns a list of shapes
// spTree is the p:spTree element from a slide
func EnumerateShapes(spTree *etree.Element) []model.ShapeInfo {
	var shapes []model.ShapeInfo

	if spTree == nil {
		return shapes
	}

	// Process p:sp (regular shapes)
	for _, sp := range spTree.FindElements("sp") {
		if shape := parseShape(sp); shape != nil {
			shapes = append(shapes, *shape)
		}
	}

	// Process p:pic (pictures)
	for _, pic := range spTree.FindElements("pic") {
		if shape := parsePicture(pic); shape != nil {
			shapes = append(shapes, *shape)
		}
	}

	// Process p:graphicFrame (tables, charts, etc.)
	for _, gf := range spTree.FindElements("graphicFrame") {
		if shape := parseGraphicFrame(gf); shape != nil {
			shapes = append(shapes, *shape)
		}
	}

	// Process p:grpSp (group shapes)
	for _, grpSp := range spTree.FindElements("grpSp") {
		if shape := parseGroup(grpSp); shape != nil {
			shapes = append(shapes, *shape)
		}
	}

	return shapes
}

// parseShape parses a p:sp element and returns shape information
func parseShape(sp *etree.Element) *model.ShapeInfo {
	info := &model.ShapeInfo{
		Type: model.ShapeTypeSP,
	}

	// Get shape ID and name from p:nvSpPr/p:cNvPr
	nvSpPr := sp.FindElement("nvSpPr")
	if nvSpPr != nil {
		cNvPr := nvSpPr.FindElement("cNvPr")
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			if idStr != "" {
				if id, err := strconv.Atoi(idStr); err == nil {
					info.ID = id
				}
			}
			info.Name = cNvPr.SelectAttrValue("name", "")
		}

		// Check for placeholder
		nvPr := nvSpPr.FindElement("nvPr")
		if nvPr != nil {
			ph := nvPr.FindElement("ph")
			if ph != nil {
				info.IsPlaceholder = true
			}
		}
	}

	// Get bounds from p:spPr/a:xfrm
	spPr := sp.FindElement("spPr")
	if spPr != nil {
		xfrm := spPr.FindElement("xfrm")
		if xfrm != nil {
			info.Bounds = parseBounds(xfrm)
		}
	}

	// Extract text content from p:txBody
	txBody := sp.FindElement("txBody")
	if txBody != nil {
		// Don't embed text in shape info for now - it's optional per acceptance criteria
		_ = txBody // could call ExtractTextBody(txBody) here if needed
	}

	return info
}

// parsePicture parses a p:pic element and returns shape information
func parsePicture(pic *etree.Element) *model.ShapeInfo {
	info := &model.ShapeInfo{
		Type: model.ShapeTypePic,
	}

	// Get picture ID and name from p:nvPicPr/p:cNvPr
	nvPicPr := pic.FindElement("nvPicPr")
	if nvPicPr != nil {
		cNvPr := nvPicPr.FindElement("cNvPr")
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			if idStr != "" {
				if id, err := strconv.Atoi(idStr); err == nil {
					info.ID = id
				}
			}
			info.Name = cNvPr.SelectAttrValue("name", "")
		}
	}

	// Get image reference from p:blipFill/a:blip@r:embed
	blipFill := pic.FindElement("blipFill")
	if blipFill != nil {
		blip := blipFill.FindElement("blip")
		if blip != nil {
			// The r:embed attribute contains the relationship ID
			// etree uses the namespace "r" as stored in the element
			relID := ""

			// Try to get the embed attribute with r: prefix
			for _, attr := range blip.Attr {
				if attr.Key == "embed" && attr.Space == "r" {
					relID = attr.Value
					break
				}
			}

			// If not found, try SelectAttrValue
			if relID == "" {
				relID = blip.SelectAttrValue("embed", "")
			}

			if relID != "" {
				info.ImageRef = &model.ImageRef{
					RelID: relID,
				}
			}
		}
	}

	// Get bounds and geometry from p:spPr/a:xfrm
	spPr := pic.FindElement("spPr")
	if spPr != nil {
		xfrm := spPr.FindElement("xfrm")
		if xfrm != nil {
			info.Bounds = parseBounds(xfrm)
			info.Geometry = parseGeometry(xfrm, spPr)
		}
	}

	return info
}

// parseGraphicFrame parses a p:graphicFrame element and returns shape information
func parseGraphicFrame(gf *etree.Element) *model.ShapeInfo {
	info := &model.ShapeInfo{
		Type: model.ShapeTypeGraphicFrame,
	}

	// Get frame ID and name from p:nvGraphicFramePr/p:cNvPr
	nvGraphicFramePr := gf.FindElement("nvGraphicFramePr")
	if nvGraphicFramePr != nil {
		cNvPr := nvGraphicFramePr.FindElement("cNvPr")
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			if idStr != "" {
				if id, err := strconv.Atoi(idStr); err == nil {
					info.ID = id
				}
			}
			info.Name = cNvPr.SelectAttrValue("name", "")
		}
	}

	// Get bounds from p:xfrm
	xfrm := gf.FindElement("xfrm")
	if xfrm != nil {
		info.Bounds = parseBounds(xfrm)
	}

	// Check if this is a table via a:graphic/a:graphicData/a:tbl
	graphic := gf.FindElement("graphic")
	if graphic != nil {
		graphicData := graphic.FindElement("graphicData")
		if graphicData != nil {
			tbl := graphicData.FindElement("tbl")
			if tbl != nil {
				info.TableInfo = ParseTable(tbl)
			}
		}
	}

	return info
}

// parseGroup parses a p:grpSp element and returns shape information
func parseGroup(grpSp *etree.Element) *model.ShapeInfo {
	info := &model.ShapeInfo{
		Type: model.ShapeTypeGroup,
	}

	// Get group ID and name from p:nvGrpSpPr/p:cNvPr
	nvGrpSpPr := grpSp.FindElement("nvGrpSpPr")
	if nvGrpSpPr != nil {
		cNvPr := nvGrpSpPr.FindElement("cNvPr")
		if cNvPr != nil {
			idStr := cNvPr.SelectAttrValue("id", "")
			if idStr != "" {
				if id, err := strconv.Atoi(idStr); err == nil {
					info.ID = id
				}
			}
			info.Name = cNvPr.SelectAttrValue("name", "")
		}
	}

	return info
}

// parseBounds extracts bounds information from an a:xfrm element
func parseBounds(xfrm *etree.Element) *model.Bounds {
	bounds := &model.Bounds{}

	// Get position from a:off
	off := xfrm.FindElement("off")
	if off != nil {
		if x, err := strconv.ParseInt(off.SelectAttrValue("x", "0"), 10, 64); err == nil {
			bounds.X = x
		}
		if y, err := strconv.ParseInt(off.SelectAttrValue("y", "0"), 10, 64); err == nil {
			bounds.Y = y
		}
	}

	// Get size from a:ext
	ext := xfrm.FindElement("ext")
	if ext != nil {
		if cx, err := strconv.ParseInt(ext.SelectAttrValue("cx", "0"), 10, 64); err == nil {
			bounds.CX = cx
		}
		if cy, err := strconv.ParseInt(ext.SelectAttrValue("cy", "0"), 10, 64); err == nil {
			bounds.CY = cy
		}
	}

	return bounds
}

// parseGeometry extracts geometry information (rotation, flip, crop) from a:xfrm and parent elements
func parseGeometry(xfrm *etree.Element, spPr *etree.Element) *model.Geometry {
	if xfrm == nil {
		return nil
	}

	geom := &model.Geometry{}

	// Parse rotation from a:xfrm@rot attribute (in 1/60000 of a degree)
	if rotStr := xfrm.SelectAttrValue("rot", ""); rotStr != "" {
		if rot, err := strconv.Atoi(rotStr); err == nil && rot != 0 {
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

	// Parse crop from parent blipFill/a:srcRect if available
	if spPr != nil {
		// Get parent to find blipFill (if this is a picture)
		parent := spPr.Parent()
		if parent != nil {
			blipFill := parent.FindElement("blipFill")
			if blipFill != nil {
				srcRect := blipFill.FindElement("srcRect")
				if srcRect != nil {
					crop := &model.CropInfo{}
					if lStr := srcRect.SelectAttrValue("l", ""); lStr != "" {
						if l, err := strconv.Atoi(lStr); err == nil {
							crop.Left = l
						}
					}
					if tStr := srcRect.SelectAttrValue("t", ""); tStr != "" {
						if t, err := strconv.Atoi(tStr); err == nil {
							crop.Top = t
						}
					}
					if rStr := srcRect.SelectAttrValue("r", ""); rStr != "" {
						if r, err := strconv.Atoi(rStr); err == nil {
							crop.Right = r
						}
					}
					if bStr := srcRect.SelectAttrValue("b", ""); bStr != "" {
						if b, err := strconv.Atoi(bStr); err == nil {
							crop.Bottom = b
						}
					}
					geom.Crop = crop
				}
			}
		}
	}

	// Only return geometry if it has actual values
	if geom.Rotation == 0 && !geom.FlipH && !geom.FlipV && geom.Crop == nil {
		return nil
	}

	return geom
}
