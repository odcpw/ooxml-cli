package mutate

import (
	"strconv"
	"testing"

	"github.com/beevik/etree"
)

func colElem(min, max int, attrs map[string]string) *etree.Element {
	el := etree.NewElement("col")
	el.CreateAttr("min", strconv.Itoa(min))
	el.CreateAttr("max", strconv.Itoa(max))
	for k, v := range attrs {
		el.CreateAttr(k, v)
	}
	return el
}

func TestTargetWidthSegmentsPreservesCoveringSpan(t *testing.T) {
	existing := []*etree.Element{colElem(1, 5, map[string]string{"hidden": "1", "style": "3"})}
	segs := targetWidthSegments(existing, 2, 4)
	if len(segs) != 1 {
		t.Fatalf("expected 1 segment, got %d: %+v", len(segs), segs)
	}
	if segs[0].min != 2 || segs[0].max != 4 {
		t.Fatalf("unexpected segment bounds: %+v", segs[0])
	}
	if segs[0].base == nil {
		t.Fatalf("expected segment to carry covering span as base")
	}
	if segs[0].base.SelectAttrValue("hidden", "") != "1" || segs[0].base.SelectAttrValue("style", "") != "3" {
		t.Fatalf("expected hidden/style preserved on base: %v", segs[0].base.Attr)
	}
}

func TestTargetWidthSegmentsFillsGaps(t *testing.T) {
	existing := []*etree.Element{colElem(2, 2, map[string]string{"style": "7"})}
	segs := targetWidthSegments(existing, 1, 3)
	if len(segs) != 3 {
		t.Fatalf("expected 3 segments, got %d: %+v", len(segs), segs)
	}
	if segs[0].base != nil || segs[0].min != 1 || segs[0].max != 1 {
		t.Fatalf("unexpected gap segment[0]: %+v", segs[0])
	}
	if segs[1].base == nil || segs[1].min != 2 || segs[1].max != 2 {
		t.Fatalf("unexpected covered segment[1]: %+v", segs[1])
	}
	if segs[2].base != nil || segs[2].min != 3 || segs[2].max != 3 {
		t.Fatalf("unexpected gap segment[2]: %+v", segs[2])
	}
}

func TestTargetWidthSegmentsNoExisting(t *testing.T) {
	segs := targetWidthSegments(nil, 1, 3)
	if len(segs) != 1 || segs[0].base != nil || segs[0].min != 1 || segs[0].max != 3 {
		t.Fatalf("expected single base-less segment: %+v", segs)
	}
}
