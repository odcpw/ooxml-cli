package cli

import (
	"strings"
	"testing"
)

func TestBuildSelectorCandidates(t *testing.T) {
	items := []SelectorCandidate{
		{Primary: "sheet:1", Selectors: []string{"sheet:1", "name:Summary", "Summary"}},
		{Primary: "sheet:2", Selectors: []string{"sheet:2", "name:Data", "Data"}},
		{Primary: "sheet:3", Selectors: []string{"sheet:3", "name:Archive", "Archive"}},
		{Primary: "sheet:4", Selectors: []string{"sheet:4", "name:Summary2", "Summary2"}},
	}

	tests := []struct {
		name     string
		items    []SelectorCandidate
		selector string
		maxCount int
		want     []string
	}{
		{
			name:     "substring match is case-insensitive and ordered",
			items:    items,
			selector: "summ",
			maxCount: 3,
			want:     []string{"sheet:1", "sheet:4"},
		},
		{
			name:     "no match falls back to first N in catalog order",
			items:    items,
			selector: "zzz",
			maxCount: 3,
			want:     []string{"sheet:1", "sheet:2", "sheet:3"},
		},
		{
			name:     "cap honored",
			items:    items,
			selector: "sheet",
			maxCount: 2,
			want:     []string{"sheet:1", "sheet:2"},
		},
		{
			name:     "empty selector falls back to first N",
			items:    items,
			selector: "",
			maxCount: 1,
			want:     []string{"sheet:1"},
		},
		{
			name:     "empty catalog yields no candidates",
			items:    nil,
			selector: "x",
			maxCount: 3,
			want:     []string{},
		},
		{
			name:     "non-positive maxCount defaults to maxSelectorCandidates",
			items:    items,
			selector: "zzz",
			maxCount: 0,
			want:     []string{"sheet:1", "sheet:2", "sheet:3"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := BuildSelectorCandidates(tt.items, tt.selector, tt.maxCount)
			if len(got) != len(tt.want) {
				t.Fatalf("got %v, want %v", got, tt.want)
			}
			for i := range got {
				if got[i] != tt.want[i] {
					t.Fatalf("got %v, want %v", got, tt.want)
				}
			}
		})
	}
}

func TestBuildSelectorCandidatesDedupesEmptyPrimaries(t *testing.T) {
	items := []SelectorCandidate{
		{Primary: "", Selectors: []string{"orphan"}},
		{Primary: "chart:1", Selectors: []string{"chart:1"}},
		{Primary: "chart:1", Selectors: []string{"chart:1"}},
	}
	got := BuildSelectorCandidates(items, "x", 3)
	if len(got) != 1 || got[0] != "chart:1" {
		t.Fatalf("expected only [chart:1], got %v", got)
	}
}

func TestSelectorNotFoundError(t *testing.T) {
	tests := []struct {
		name         string
		entity       string
		selector     string
		candidates   []string
		discoveryCmd string
		wantParts    []string
		wantAbsent   []string
	}{
		{
			name:         "candidates and discovery both present",
			entity:       "sheet",
			selector:     "Foo",
			candidates:   []string{"sheet:1", "sheet:2"},
			discoveryCmd: "ooxml --json xlsx sheets list <file>",
			wantParts: []string{
				"sheet not found: Foo",
				"did you mean: sheet:1, sheet:2",
				"discover with `ooxml --json xlsx sheets list <file>`",
			},
		},
		{
			name:         "discovery only when no candidates",
			entity:       "chart",
			selector:     "bar",
			candidates:   nil,
			discoveryCmd: "ooxml --json xlsx charts list <file>",
			wantParts:    []string{"chart not found: bar", "discover with"},
			wantAbsent:   []string{"did you mean"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := SelectorNotFoundError(tt.entity, tt.selector, tt.candidates, tt.discoveryCmd)
			if err.ExitCode != ExitTargetNotFound {
				t.Fatalf("exit code = %d, want %d", err.ExitCode, ExitTargetNotFound)
			}
			for _, part := range tt.wantParts {
				if !strings.Contains(err.Message, part) {
					t.Fatalf("message %q missing %q", err.Message, part)
				}
			}
			for _, part := range tt.wantAbsent {
				if strings.Contains(err.Message, part) {
					t.Fatalf("message %q unexpectedly contains %q", err.Message, part)
				}
			}
		})
	}
}
