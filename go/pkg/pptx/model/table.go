package model

// TableInfo represents a table in a presentation
type TableInfo struct {
	Rows       int          `json:"rows"`
	Cols       int          `json:"cols"`
	Cells      [][]string   `json:"cells"`                // Backward-compatible: plain text per cell
	RowDefs    []*TableRow  `json:"rowDefs,omitempty"`    // Row definitions with height
	ColumnDefs []*TableCol  `json:"columnDefs,omitempty"` // Column definitions with width
	CellDefs   [][]CellInfo `json:"cellDefs,omitempty"`   // Cell-level details: merges, fills, borders, styles
}

// TableRow represents a table row with height information
type TableRow struct {
	Height int64       `json:"height,omitempty"` // Row height in EMUs
	Cells  []*CellInfo `json:"cells,omitempty"`  // Cells in this row
}

// TableCol represents a table column with width information
type TableCol struct {
	Width int64 `json:"width,omitempty"` // Column width in EMUs
}

// CellInfo contains detailed information about a single cell
type CellInfo struct {
	Text      string      `json:"text,omitempty"`      // Plain text content
	GridSpan  int         `json:"gridSpan,omitempty"`  // Horizontal merge span (>1 means merged)
	RowSpan   int         `json:"rowSpan,omitempty"`   // Vertical merge span (>1 means merged)
	Fill      *CellFill   `json:"fill,omitempty"`      // Cell background fill
	Border    *CellBorder `json:"border,omitempty"`    // Cell borders
	TextAlign string      `json:"textAlign,omitempty"` // Text alignment (l, r, ctr)
	VertAlign string      `json:"vertAlign,omitempty"` // Vertical alignment (t, b, ctr)
	Bold      bool        `json:"bold,omitempty"`      // Text bold
	Italic    bool        `json:"italic,omitempty"`    // Text italic
	FontSize  int         `json:"fontSize,omitempty"`  // Font size in points
	FontColor string      `json:"fontColor,omitempty"` // Font color (hex or theme index)
}

// CellFill represents fill information for a cell
type CellFill struct {
	Type  string `json:"type,omitempty"`  // "solid", "none", etc.
	Color string `json:"color,omitempty"` // RGB color in hex or theme index
}

// CellBorder represents border information for a cell
type CellBorder struct {
	Left   *BorderLine `json:"left,omitempty"`
	Right  *BorderLine `json:"right,omitempty"`
	Top    *BorderLine `json:"top,omitempty"`
	Bottom *BorderLine `json:"bottom,omitempty"`
}

// BorderLine represents a single border line
type BorderLine struct {
	Width int64  `json:"width,omitempty"` // Border width in EMUs
	Color string `json:"color,omitempty"` // Border color (hex or theme index)
	Style string `json:"style,omitempty"` // Line style ("solid", "dash", etc.)
}
