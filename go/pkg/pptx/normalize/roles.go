package normalize

// CanonicalRole maps PPTX placeholder type values to canonical role names.
// This is the authoritative mapping table defined in docs/placeholder-key-rules.md.
//
// If phType is not in the mapping table, it is returned as-is (preserve literally
// for forward compatibility).
func CanonicalRole(phType string) string {
	switch phType {
	case "title":
		return "title"
	case "ctrTitle":
		return "title" // Center title maps to title
	case "subTitle":
		return "subtitle"
	case "body":
		return "body"
	case "pic":
		return "pic"
	case "tbl":
		return "table"
	case "chart":
		return "chart"
	case "obj":
		return "object"
	case "dt":
		return "date"
	case "ftr":
		return "footer"
	case "sldNum":
		return "slideNumber"
	default:
		// Unknown types are preserved literally for forward compatibility
		return phType
	}
}
