package model

// ThemeInfo represents parsed theme information
type ThemeInfo struct {
	Name        string       `json:"name,omitempty"`
	ColorScheme *ColorScheme `json:"colorScheme,omitempty"`
	FontScheme  *FontScheme  `json:"fontScheme,omitempty"`
}

// ColorScheme represents the color scheme of a theme
type ColorScheme struct {
	Name string `json:"name,omitempty"`
	// Primary colors
	Dark1  string `json:"dark1,omitempty"`  // dk1
	Light1 string `json:"light1,omitempty"` // lt1
	Dark2  string `json:"dark2,omitempty"`  // dk2
	Light2 string `json:"light2,omitempty"` // lt2
	// Accent colors
	Accent1 string `json:"accent1,omitempty"`
	Accent2 string `json:"accent2,omitempty"`
	Accent3 string `json:"accent3,omitempty"`
	Accent4 string `json:"accent4,omitempty"`
	Accent5 string `json:"accent5,omitempty"`
	Accent6 string `json:"accent6,omitempty"`
	// Hyperlink colors
	HypLink string `json:"hypLink,omitempty"` // hlink
	FolLink string `json:"folLink,omitempty"` // folhlink
}

// FontScheme represents the font scheme of a theme
type FontScheme struct {
	Name                   string `json:"name,omitempty"`
	MajorFont              string `json:"majorFont,omitempty"`              // Latin font for titles
	MinorFont              string `json:"minorFont,omitempty"`              // Latin font for body
	EastAsianMajorFont     string `json:"eastAsianMajorFont,omitempty"`     // East Asian font for titles
	EastAsianMinorFont     string `json:"eastAsianMinorFont,omitempty"`     // East Asian font for body
	ComplexScriptMajorFont string `json:"complexScriptMajorFont,omitempty"` // Complex script font for titles
	ComplexScriptMinorFont string `json:"complexScriptMinorFont,omitempty"` // Complex script font for body
}

// DefaultTextStyleInfo represents default text style information for a master/layout
type DefaultTextStyleInfo struct {
	ThemeName    string   `json:"themeName,omitempty"`
	MajorFont    string   `json:"majorFont,omitempty"`
	MinorFont    string   `json:"minorFont,omitempty"`
	AccentColors []string `json:"accentColors,omitempty"`
}
