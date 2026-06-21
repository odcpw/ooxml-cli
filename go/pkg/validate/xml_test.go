package validate

import "testing"

func TestIsXMLPartRecognizesOpenXMLCorePropertiesContentType(t *testing.T) {
	const corePropsContentType = "application/vnd.openxmlformats-package.core-properties+xml"
	if !isXMLPart("/docProps/core.bin", corePropsContentType) {
		t.Fatalf("expected standard core-properties content type to be treated as XML")
	}
}

func TestIsXMLPartDoesNotBlessLegacyCorePropertiesContentType(t *testing.T) {
	const legacyCorePropsContentType = "application/vnd.openxmlformats-officedocument.core-properties+xml"
	if isXMLPart("/docProps/core.bin", legacyCorePropsContentType) {
		t.Fatalf("legacy core-properties content type should not be recognized by content type alone")
	}
}
