// Package validate provides layered validation for OOXML packages.
// It validates package integrity, relationships, type-specific semantics, and XML well-formedness.
package validate

import (
	"github.com/ooxml-cli/ooxml-cli/pkg/core/result"
	"github.com/ooxml-cli/ooxml-cli/pkg/opc"
)

// ValidatePackage validates an OPC package through 5 stages:
// 1. Package integrity (zip structure, required parts)
// 2. Relationship integrity (no dangling references)
// 3. Cross-family package feature validation (VBA package wiring)
// 4. Type-specific semantic validation (PPTX, XLSX, etc.)
// 5. XML well-formedness of modified parts
//
// Returns a slice of diagnostics. Empty slice means valid.
// Diagnostics are ordered by stage and include code, severity, and message.
func ValidatePackage(session opc.PackageSession) ([]result.Diagnostic, error) {
	var allDiags []result.Diagnostic

	// Stage 1: Package integrity
	pkgDiags, err := validatePackageIntegrity(session)
	if err != nil {
		return nil, err
	}
	allDiags = append(allDiags, pkgDiags...)

	// Stage 2: Relationship integrity
	relDiags, err := validateRelationshipIntegrity(session)
	if err != nil {
		return nil, err
	}
	allDiags = append(allDiags, relDiags...)

	// Stage 3: cross-family package feature validation
	vbaDiags, err := validateVBAPackageConsistency(session)
	if err != nil {
		return nil, err
	}
	allDiags = append(allDiags, vbaDiags...)

	// Stage 4: type-specific semantic validation
	switch opc.DetectType(session) {
	case opc.PackageTypePPTX:
		pptxDiags, err := validatePPTXSemantics(session)
		if err != nil {
			return nil, err
		}
		allDiags = append(allDiags, pptxDiags...)
	case opc.PackageTypeXLSX:
		xlsxDiags, err := validateXLSXSemantics(session)
		if err != nil {
			return nil, err
		}
		allDiags = append(allDiags, xlsxDiags...)
	case opc.PackageTypeDOCX:
		docxDiags, err := validateDOCXSemantics(session)
		if err != nil {
			return nil, err
		}
		allDiags = append(allDiags, docxDiags...)
	}

	// Stage 5: XML well-formedness of modified parts
	xmlDiags, err := validateModifiedXML(session)
	if err != nil {
		return nil, err
	}
	allDiags = append(allDiags, xmlDiags...)

	return allDiags, nil
}
