package extract

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// ExtractItem represents an item to extract
type ExtractItem struct {
	Type    string // "slide", "layout", or "master"
	Number  int    // 1-indexed
	PartURI string // /ppt/slides/slide1.xml, /ppt/slideLayouts/slideLayout1.xml, etc.
}

// ExtractXMLResult represents the result of extraction
type ExtractXMLResult struct {
	File           string   `json:"file"`
	OutputDir      string   `json:"output_dir"`
	ExtractedItems []string `json:"extracted_items"`
}

// ExtractXML extracts XML and rels from a package item and writes them to an output directory
func ExtractXML(pkg *opc.Package, item *ExtractItem, outputDir string) error {
	// Create subdirectory for this item
	itemDir := filepath.Join(outputDir, fmt.Sprintf("%s-%d", item.Type, item.Number))
	if err := os.MkdirAll(itemDir, 0755); err != nil {
		return fmt.Errorf("failed to create item directory: %w", err)
	}

	// Extract the main XML file
	xmlData, err := pkg.ReadRawPart(item.PartURI)
	if err != nil {
		return fmt.Errorf("failed to read XML part: %w", err)
	}

	// Write the XML file
	xmlFileName := filepath.Base(item.PartURI)
	xmlPath := filepath.Join(itemDir, xmlFileName)
	if err := os.WriteFile(xmlPath, xmlData, 0644); err != nil {
		return fmt.Errorf("failed to write XML file: %w", err)
	}

	// Extract relationships (.rels file)
	relsURI := item.PartURI + ".rels"
	relsData, err := pkg.ReadRawPart(relsURI)
	if err == nil {
		// Write the .rels file if it exists
		relsFileName := xmlFileName + ".rels"
		relsPath := filepath.Join(itemDir, relsFileName)
		if err := os.WriteFile(relsPath, relsData, 0644); err != nil {
			return fmt.Errorf("failed to write rels file: %w", err)
		}
	}
	// Note: Some items might not have rels files, so we don't error if they don't exist

	// Write a summary file with metadata
	summary := generateSummary(item)
	summaryPath := filepath.Join(itemDir, "EXTRACTION_SUMMARY.txt")
	if err := os.WriteFile(summaryPath, []byte(summary), 0644); err != nil {
		return fmt.Errorf("failed to write summary file: %w", err)
	}

	return nil
}

// generateSummary generates a summary file describing the extracted content
func generateSummary(item *ExtractItem) string {
	var sb strings.Builder
	sb.WriteString("=== XML Extraction Summary ===\n\n")
	sb.WriteString(fmt.Sprintf("Type: %s\n", item.Type))
	sb.WriteString(fmt.Sprintf("Number: %d\n", item.Number))
	sb.WriteString(fmt.Sprintf("Part URI: %s\n", item.PartURI))
	sb.WriteString("\nFiles:\n")
	sb.WriteString(fmt.Sprintf("  - %s (main XML content)\n", filepath.Base(item.PartURI)))
	sb.WriteString(fmt.Sprintf("  - %s.rels (relationships, if present)\n", filepath.Base(item.PartURI)))
	sb.WriteString("\nNote: These are raw extracts from the OPC package.\n")
	sb.WriteString("Original package bytes are preserved without reserialization.\n")
	return sb.String()
}
