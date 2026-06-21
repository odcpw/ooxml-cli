package capabilities

import (
	"reflect"
	"sort"
	"strconv"
	"strings"
	"testing"
)

func TestObjectKindsSortedAndUnique(t *testing.T) {
	if !sort.StringsAreSorted(ObjectKinds) {
		t.Fatalf("ObjectKinds is not sorted: %v", ObjectKinds)
	}
	seen := map[string]bool{}
	for _, k := range ObjectKinds {
		if seen[k] {
			t.Fatalf("duplicate object kind %q", k)
		}
		seen[k] = true
	}
}

func TestIsObjectKind(t *testing.T) {
	for _, k := range []string{"shape", "sheet", "chart", "package"} {
		if !IsObjectKind(k) {
			t.Fatalf("IsObjectKind(%q) = false, want true", k)
		}
	}
	for _, k := range []string{"", "bogus", "Shape", "widget"} {
		if IsObjectKind(k) {
			t.Fatalf("IsObjectKind(%q) = true, want false", k)
		}
	}
}

func TestMetadataIsWellFormed(t *testing.T) {
	for path, meta := range commandMetadata {
		if !strings.HasPrefix(path, "ooxml ") {
			t.Fatalf("metadata key %q should be a full command path starting with 'ooxml '", path)
		}
		if len(meta.Examples) == 0 {
			t.Fatalf("%q has no examples; metadata is meant for high-use commands", path)
		}
		for i, ex := range meta.Examples {
			if !strings.HasPrefix(ex.Command, "ooxml ") {
				t.Fatalf("%q example %d command %q must start with 'ooxml '", path, i, ex.Command)
			}
			if containsAnglePlaceholder(ex.Command) {
				t.Fatalf("%q example %d uses shell-active angle placeholder: %q", path, i, ex.Command)
			}
			if strings.TrimSpace(ex.Description) == "" {
				t.Fatalf("%q example %d has empty description", path, i)
			}
		}
		for i, ce := range meta.CommonErrors {
			if strings.TrimSpace(ce.Pattern) == "" || strings.TrimSpace(ce.Solution) == "" {
				t.Fatalf("%q commonError %d has empty pattern or solution", path, i)
			}
		}
		for _, kind := range meta.TargetObjectKinds {
			if !IsObjectKind(kind) {
				t.Fatalf("%q targets unknown object kind %q (add it to ObjectKinds or fix the metadata)", path, kind)
			}
		}
	}
}

func containsAnglePlaceholder(command string) bool {
	start := strings.Index(command, "<")
	if start == -1 {
		return false
	}
	return strings.Contains(command[start+1:], ">")
}

func TestCommandPathsSorted(t *testing.T) {
	paths := CommandPaths()
	if !sort.StringsAreSorted(paths) {
		t.Fatalf("CommandPaths not sorted: %v", paths)
	}
	if len(paths) != len(commandMetadata) {
		t.Fatalf("CommandPaths len = %d, want %d", len(paths), len(commandMetadata))
	}
}

func TestVBAAddModuleExampleUsesParseableModuleCount(t *testing.T) {
	meta, ok := MetadataFor("ooxml vba add-module")
	if !ok {
		t.Fatal("missing metadata for ooxml vba add-module")
	}
	for _, ex := range meta.Examples {
		words := strings.Fields(ex.Command)
		for i, word := range words {
			if word != "--expect-module-count" {
				continue
			}
			if i+1 >= len(words) {
				t.Fatalf("example missing --expect-module-count value: %s", ex.Command)
			}
			if _, err := strconv.Atoi(words[i+1]); err != nil {
				t.Fatalf("--expect-module-count example value must parse as int, got %q in %s", words[i+1], ex.Command)
			}
		}
	}
}

func TestBuildObjectKindIndexDeterministic(t *testing.T) {
	a := BuildObjectKindIndex()
	b := BuildObjectKindIndex()
	if !reflect.DeepEqual(a, b) {
		t.Fatalf("index not deterministic:\n%v\n%v", a, b)
	}
	for kind, paths := range a {
		if !IsObjectKind(kind) {
			t.Fatalf("index contains non-taxonomy kind %q", kind)
		}
		if !sort.StringsAreSorted(paths) {
			t.Fatalf("paths for %q not sorted: %v", kind, paths)
		}
		seen := map[string]bool{}
		for _, p := range paths {
			if seen[p] {
				t.Fatalf("duplicate path %q for kind %q", p, kind)
			}
			seen[p] = true
		}
	}
}

func TestCommandsForKind(t *testing.T) {
	shape := CommandsForKind("shape")
	if len(shape) == 0 {
		t.Fatalf("expected shape-targeting commands")
	}
	if !contains(shape, "ooxml pptx shapes show") {
		t.Fatalf("shape commands missing 'ooxml pptx shapes show': %v", shape)
	}

	sheet := CommandsForKind("sheet")
	if !contains(sheet, "ooxml xlsx sheets list") {
		t.Fatalf("sheet commands missing 'ooxml xlsx sheets list': %v", sheet)
	}

	// Unknown kind: empty, non-nil, no panic.
	unknown := CommandsForKind("definitely-not-a-kind")
	if unknown == nil {
		t.Fatalf("CommandsForKind(unknown) returned nil, want empty slice")
	}
	if len(unknown) != 0 {
		t.Fatalf("CommandsForKind(unknown) = %v, want empty", unknown)
	}
}

func TestMetadataForMiss(t *testing.T) {
	if _, ok := MetadataFor("ooxml not-a-command"); ok {
		t.Fatalf("MetadataFor returned ok for a missing path")
	}
	if _, ok := MetadataFor("ooxml pptx shapes show"); !ok {
		t.Fatalf("MetadataFor missing a known path")
	}
}

func TestConditionalFormatsAddMetadataIncludesColorScaleExample(t *testing.T) {
	meta, ok := MetadataFor("ooxml xlsx conditional-formats add")
	if !ok {
		t.Fatal("missing metadata for conditional-format add")
	}
	found := false
	for _, ex := range meta.Examples {
		if strings.Contains(ex.Command, "--type color-scale") &&
			strings.Contains(ex.Command, "--cfvo") &&
			strings.Contains(ex.Command, "--color") {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("conditional-format add metadata missing color-scale example: %+v", meta.Examples)
	}
	if !contains(meta.TargetObjectKinds, "conditional-format") || !contains(meta.TargetObjectKinds, "range") {
		t.Fatalf("conditional-format add metadata targets wrong object kinds: %+v", meta.TargetObjectKinds)
	}
}

func contains(s []string, want string) bool {
	for _, v := range s {
		if v == want {
			return true
		}
	}
	return false
}
