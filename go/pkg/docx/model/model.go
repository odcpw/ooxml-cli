package model

type Document struct {
	PartURI      string `json:"partUri"`
	StylesURI    string `json:"stylesPartUri,omitempty"`
	NumberingURI string `json:"numberingPartUri,omitempty"`
}

type DocumentSummary struct {
	Type            string `json:"type"`
	DocumentPartURI string `json:"documentPartUri,omitempty"`
	Paragraphs      int    `json:"paragraphs"`
	Tables          int    `json:"tables"`
	Hyperlinks      int    `json:"hyperlinks"`
	Headers         int    `json:"headers"`
	Footers         int    `json:"footers"`
	Footnotes       bool   `json:"footnotes"`
	Endnotes        bool   `json:"endnotes"`
	Comments        bool   `json:"comments"`
	Sections        int    `json:"sections"`
	Styles          bool   `json:"styles"`
	Numbering       bool   `json:"numbering"`
	MediaAssets     int    `json:"mediaAssets"`
	CustomXMLParts  int    `json:"customXmlParts"`
}

type BlockKind string

const (
	BlockKindParagraph BlockKind = "paragraph"
	BlockKindTable     BlockKind = "table"
)

type Block struct {
	Index int       `json:"index"`
	Kind  BlockKind `json:"kind"`
	Style string    `json:"style,omitempty"`
	Text  string    `json:"text"`
	// ParaID is the paragraph's w14:paraId marker when physically present
	// (read-only; never injected by inspect). Empty for unmarked paragraphs and
	// tables.
	ParaID string `json:"paraId,omitempty"`
	// Handle is the stable paragraph handle when a unique w14:paraId marker is
	// present; empty otherwise (marker-less, or a non-unique/ambiguous marker).
	Handle string `json:"handle,omitempty"`
	Table  *Table `json:"table,omitempty"`
}

type Table struct {
	Rows []TableRow `json:"rows"`
}

type TableRow struct {
	Cells []string `json:"cells"`
}
