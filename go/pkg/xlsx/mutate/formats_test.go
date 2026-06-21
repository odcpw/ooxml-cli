package mutate

import "testing"

func TestResolveNumberFormatPresets(t *testing.T) {
	tests := []struct {
		name        string
		opts        NumberFormatOptions
		wantCode    string
		wantID      int
		wantBuiltin bool
	}{
		{
			name:        "number builtin two decimals",
			opts:        NumberFormatOptions{Preset: "number", Decimals: 2},
			wantCode:    "#,##0.00",
			wantID:      4,
			wantBuiltin: true,
		},
		{
			name:        "percent custom one decimal",
			opts:        NumberFormatOptions{Preset: "percent", Decimals: 1},
			wantCode:    "0.0%",
			wantBuiltin: false,
		},
		{
			name:        "currency custom",
			opts:        NumberFormatOptions{Preset: "currency", Decimals: 0, CurrencySymbol: "CHF"},
			wantCode:    `"CHF"#,##0`,
			wantBuiltin: false,
		},
		{
			name:        "date custom",
			opts:        NumberFormatOptions{Preset: "date", Decimals: 2},
			wantCode:    "yyyy-mm-dd",
			wantBuiltin: false,
		},
		{
			name:        "text builtin",
			opts:        NumberFormatOptions{Preset: "text", Decimals: 2},
			wantCode:    "@",
			wantID:      49,
			wantBuiltin: true,
		},
		{
			name:        "custom code",
			opts:        NumberFormatOptions{FormatCode: `0 "days"`},
			wantCode:    `0 "days"`,
			wantBuiltin: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ResolveNumberFormat(tt.opts)
			if err != nil {
				t.Fatalf("ResolveNumberFormat returned error: %v", err)
			}
			if got.FormatCode != tt.wantCode || got.NumFmtID != tt.wantID || got.Builtin != tt.wantBuiltin {
				t.Fatalf("ResolveNumberFormat() = %+v, want code %q id %d builtin %t", got, tt.wantCode, tt.wantID, tt.wantBuiltin)
			}
		})
	}
}

func TestResolveNumberFormatRejectsAmbiguousInput(t *testing.T) {
	for _, opts := range []NumberFormatOptions{
		{},
		{Preset: "number", FormatCode: "0.0"},
		{Preset: "number", Decimals: -1},
		{Preset: "number", Decimals: 11},
		{Preset: "word-art", Decimals: 2},
	} {
		if _, err := ResolveNumberFormat(opts); err == nil {
			t.Fatalf("ResolveNumberFormat(%+v) expected error", opts)
		}
	}
}
