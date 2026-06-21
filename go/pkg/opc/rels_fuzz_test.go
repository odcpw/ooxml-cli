package opc

import (
	"strings"
	"testing"
)

func FuzzResolveRelationshipTargetNormalization(f *testing.F) {
	seeds := []struct {
		sourceURI string
		target    string
	}{
		{"/ppt/slides/slide1.xml", "../slideLayouts/slideLayout1.xml"},
		{"/ppt/slides/slide1.xml", "media/../embeddings/oleObject1.bin"},
		{"/ppt/slides/slide1.xml", "./media/image 1.png"},
		{"/ppt/slides/slide1.xml", "../media/image%201.png"},
		{"/ppt/slides/slide1.xml", "../media/%2E%2E/image.png"},
		{"/ppt/slides/slide1.xml", "..\\media\\image1.png"},
		{"/ppt/slides/slide1.xml", "/ppt/media/image1.png"},
		{"/xl/worksheets/sheet1.xml", "../drawings/../sharedStrings.xml"},
		{"/ppt/presentation.xml", "theme/theme 1.xml"},
		{"/ppt/presentation.xml", "http://example.com/a/../b"},
		{"/ppt/presentation.xml", "https://example.com/path%2Fname"},
		{"/ppt/presentation.xml", "mailto:owner@example.com"},
		{"/ppt/presentation.xml", "file:///C:/Users/owner/report.xlsx"},
		{"/ppt/presentation.xml", "urn:uuid:1234"},
		{"/", "docProps/core.xml"},
		{"", ""},
	}

	for _, seed := range seeds {
		f.Add(seed.sourceURI, seed.target)
	}

	f.Fuzz(func(t *testing.T, sourceURI, target string) {
		if len(sourceURI) > 256 || len(target) > 256 {
			return
		}

		got := ResolveRelationshipTarget(sourceURI, target)
		want := referenceResolveRelationshipTarget(sourceURI, target)
		if got != want {
			t.Fatalf("ResolveRelationshipTarget(%q, %q) = %q, want %q", sourceURI, target, got, want)
		}
		if got != "" && !IsExternalRelationshipTarget(target) && !strings.HasPrefix(got, "/") {
			t.Fatalf("ResolveRelationshipTarget(%q, %q) = %q, want internal targets to start with /", sourceURI, target, got)
		}
		if again := ResolveRelationshipTarget(sourceURI, target); again != got {
			t.Fatalf("ResolveRelationshipTarget(%q, %q) was not deterministic: %q then %q", sourceURI, target, got, again)
		}
	})
}

func referenceResolveRelationshipTarget(sourceURI, target string) string {
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
	return NormalizeURI(sourceDir + "/" + target)
}
