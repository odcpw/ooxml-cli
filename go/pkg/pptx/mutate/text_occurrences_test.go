package mutate

import "testing"

func TestReplaceTextOccurrencesInString(t *testing.T) {
	tests := []struct {
		name        string
		text        string
		match       string
		replacement string
		ignoreCase  bool
		wantText    string
		wantCount   int
	}{
		{
			name:        "exact multiple",
			text:        "Old Client and Old Client",
			match:       "Old Client",
			replacement: "New Client",
			wantText:    "New Client and New Client",
			wantCount:   2,
		},
		{
			name:        "ignore case",
			text:        "FY25 fy25 Fy25",
			match:       "fy25",
			replacement: "FY26",
			ignoreCase:  true,
			wantText:    "FY26 FY26 FY26",
			wantCount:   3,
		},
		{
			name:        "not found",
			text:        "unchanged",
			match:       "missing",
			replacement: "x",
			wantText:    "unchanged",
			wantCount:   0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			gotText, gotCount := replaceTextOccurrencesInString(tt.text, tt.match, tt.replacement, tt.ignoreCase)
			if gotText != tt.wantText || gotCount != tt.wantCount {
				t.Fatalf("replaceTextOccurrencesInString() = (%q, %d), want (%q, %d)", gotText, gotCount, tt.wantText, tt.wantCount)
			}
		})
	}
}
