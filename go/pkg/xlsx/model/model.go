package model

const (
	// SheetStateVisible is the effective state when workbook.xml omits sheet@state.
	SheetStateVisible = "visible"
)

type Workbook struct {
	PartURI          string     `json:"partUri"`
	Sheets           []SheetRef `json:"sheets"`
	SharedStringsURI string     `json:"sharedStringsPartUri,omitempty"`
	StylesURI        string     `json:"stylesPartUri,omitempty"`
	ThemeURI         string     `json:"themePartUri,omitempty"`
}

type SheetRef struct {
	Number           int      `json:"number"`
	Position         int      `json:"position,omitempty"`
	Name             string   `json:"name"`
	SheetID          string   `json:"sheetId"`
	State            string   `json:"state,omitempty"`
	RelationshipID   string   `json:"relationshipId"`
	PartURI          string   `json:"partUri,omitempty"`
	RelationshipType string   `json:"relationshipType,omitempty"`
	PrimarySelector  string   `json:"primarySelector,omitempty"`
	Selectors        []string `json:"selectors,omitempty"`
}

type DefinedName struct {
	Number          int      `json:"number"`
	Name            string   `json:"name"`
	Scope           string   `json:"scope"`
	LocalSheetID    *int     `json:"localSheetId,omitempty"`
	SheetNumber     int      `json:"sheetNumber,omitempty"`
	SheetName       string   `json:"sheetName,omitempty"`
	Ref             string   `json:"ref"`
	Hidden          bool     `json:"hidden,omitempty"`
	Comment         string   `json:"comment,omitempty"`
	Description     string   `json:"description,omitempty"`
	PrimarySelector string   `json:"primarySelector,omitempty"`
	Selectors       []string `json:"selectors,omitempty"`
}

type WorkbookSummary struct {
	Type              string `json:"type"`
	WorkbookPartURI   string `json:"workbookPartUri,omitempty"`
	SheetCount        int    `json:"sheets"`
	WorksheetCount    int    `json:"worksheets"`
	SharedStrings     bool   `json:"sharedStrings"`
	SharedStringCount int    `json:"sharedStringCount,omitempty"`
	Styles            bool   `json:"styles"`
	Themes            int    `json:"themes"`
	Tables            int    `json:"tables"`
	Pivots            int    `json:"pivots"`
	PivotCaches       int    `json:"pivotCaches"`
	Charts            int    `json:"charts"`
	MediaAssets       int    `json:"mediaAssets"`
	CustomXMLParts    int    `json:"customXmlParts"`
}

type CellDataType string

const (
	CellTypeEmpty   CellDataType = "empty"
	CellTypeString  CellDataType = "string"
	CellTypeNumber  CellDataType = "number"
	CellTypeBoolean CellDataType = "boolean"
	CellTypeDate    CellDataType = "date"
	CellTypeError   CellDataType = "error"
	CellTypeUnknown CellDataType = "unknown"
)

type UsedRange struct {
	Ref    string `json:"ref,omitempty"`
	MinRow int    `json:"minRow,omitempty"`
	MaxRow int    `json:"maxRow,omitempty"`
	MinCol int    `json:"minCol,omitempty"`
	MaxCol int    `json:"maxCol,omitempty"`
	Rows   int    `json:"rows"`
	Cols   int    `json:"cols"`
	Empty  bool   `json:"empty"`
}

type Cell struct {
	Ref string `json:"ref"`
	// Handle is the stable, paste-safe cell handle (H:xlsx/ws:<sheetId>/cell:a:<A1>),
	// surfaced by read commands that have a sheetId context. It is omitted when no
	// handle can be minted (no sheetId, or a duplicated sheetId). A cell handle
	// survives sheet reorder/rename but NOT a row/column insert that shifts the
	// A1 address (see pkg/xlsx/handle).
	Handle           string       `json:"handle,omitempty"`
	PrimarySelector  string       `json:"primarySelector,omitempty"`
	Selectors        []string     `json:"selectors,omitempty"`
	Row              int          `json:"row"`
	Col              int          `json:"col"`
	Column           string       `json:"column"`
	Type             CellDataType `json:"type"`
	Value            string       `json:"value,omitempty"`
	RawValue         string       `json:"rawValue,omitempty"`
	Formula          string       `json:"formula,omitempty"`
	StyleIndex       int          `json:"styleIndex,omitempty"`
	NumberFormatID   int          `json:"numberFormatId,omitempty"`
	NumberFormatCode string       `json:"numberFormatCode,omitempty"`
	DateStyle        bool         `json:"dateStyle,omitempty"`
}

type Row struct {
	Number int    `json:"number"`
	Cells  []Cell `json:"cells"`
}

type SheetReport struct {
	Number            int       `json:"number"`
	Name              string    `json:"name"`
	SheetID           string    `json:"sheetId"`
	State             string    `json:"state,omitempty"`
	PartURI           string    `json:"partUri,omitempty"`
	PrimarySelector   string    `json:"primarySelector,omitempty"`
	Selectors         []string  `json:"selectors,omitempty"`
	DimensionDeclared string    `json:"dimensionDeclared,omitempty"`
	UsedRange         UsedRange `json:"usedRange"`
	RowCount          int       `json:"rowCount"`
	CellCount         int       `json:"cellCount"`
	MergedCellCount   int       `json:"mergedCellCount"`
	Rows              []Row     `json:"rows,omitempty"`
	Truncated         bool      `json:"truncated,omitempty"`
}

type TableColumn struct {
	ID   int    `json:"id"`
	Name string `json:"name"`
}

type TableRef struct {
	Number          int           `json:"number"`
	Sheet           string        `json:"sheet"`
	SheetNumber     int           `json:"sheetNumber"`
	SheetPartURI    string        `json:"sheetPartUri"`
	RelationshipID  string        `json:"relationshipId"`
	PartURI         string        `json:"partUri"`
	ID              int           `json:"id"`
	Name            string        `json:"name,omitempty"`
	DisplayName     string        `json:"displayName"`
	PrimarySelector string        `json:"primarySelector,omitempty"`
	Selectors       []string      `json:"selectors,omitempty"`
	Range           string        `json:"range"`
	Rows            int           `json:"rows"`
	Cols            int           `json:"cols"`
	HeaderRowCount  int           `json:"headerRowCount"`
	DataRowCount    int           `json:"dataRowCount"`
	TotalsRowCount  int           `json:"totalsRowCount"`
	StyleName       string        `json:"styleName,omitempty"`
	Columns         []TableColumn `json:"columns,omitempty"`
}

type ChartMarkerRef struct {
	Column       int `json:"column"`
	ColumnOffset int `json:"columnOffset,omitempty"`
	Row          int `json:"row"`
	RowOffset    int `json:"rowOffset,omitempty"`
}

type ChartAnchorRef struct {
	Type string          `json:"type"`
	From *ChartMarkerRef `json:"from,omitempty"`
	To   *ChartMarkerRef `json:"to,omitempty"`
}

type ChartDataSourceRef struct {
	Formula      string   `json:"formula,omitempty"`
	Sheet        string   `json:"sheet,omitempty"`
	Range        string   `json:"range,omitempty"`
	RefKind      string   `json:"refKind,omitempty"`
	CacheType    string   `json:"cacheType,omitempty"`
	PointCount   int      `json:"pointCount,omitempty"`
	CachePreview []string `json:"cachePreview,omitempty"`
}

type ChartSeriesRef struct {
	Number     int                 `json:"number"`
	Index      int                 `json:"index,omitempty"`
	Order      int                 `json:"order,omitempty"`
	Name       *ChartDataSourceRef `json:"name,omitempty"`
	Categories *ChartDataSourceRef `json:"categories,omitempty"`
	Values     *ChartDataSourceRef `json:"values,omitempty"`
	XValues    *ChartDataSourceRef `json:"xValues,omitempty"`
	YValues    *ChartDataSourceRef `json:"yValues,omitempty"`
	BubbleSize *ChartDataSourceRef `json:"bubbleSize,omitempty"`
}

type ChartRef struct {
	Number                int              `json:"number"`
	Sheet                 string           `json:"sheet"`
	SheetNumber           int              `json:"sheetNumber"`
	SheetPartURI          string           `json:"sheetPartUri"`
	DrawingRelationshipID string           `json:"drawingRelationshipId"`
	DrawingPartURI        string           `json:"drawingPartUri"`
	RelationshipID        string           `json:"relationshipId"`
	PartURI               string           `json:"partUri"`
	Name                  string           `json:"name,omitempty"`
	Title                 string           `json:"title,omitempty"`
	Types                 []string         `json:"types,omitempty"`
	Anchor                *ChartAnchorRef  `json:"anchor,omitempty"`
	PrimarySelector       string           `json:"primarySelector,omitempty"`
	Selectors             []string         `json:"selectors,omitempty"`
	Series                []ChartSeriesRef `json:"series,omitempty"`
}

type PivotFieldRef struct {
	Index    int    `json:"index"`
	Name     string `json:"name,omitempty"`
	Axis     string `json:"axis,omitempty"`
	Subtotal string `json:"subtotal,omitempty"`
	Caption  string `json:"caption,omitempty"`
}

type PivotSourceRef struct {
	Type  string `json:"type,omitempty"`
	Sheet string `json:"sheet,omitempty"`
	Range string `json:"range,omitempty"`
	Name  string `json:"name,omitempty"`
}

type PivotCacheField struct {
	Index int    `json:"index"`
	Name  string `json:"name,omitempty"`
}

type PivotCacheRef struct {
	CacheID          int               `json:"cacheId,omitempty"`
	PartURI          string            `json:"partUri,omitempty"`
	RelationshipID   string            `json:"relationshipId,omitempty"`
	RecordsPartURI   string            `json:"recordsPartUri,omitempty"`
	RecordCount      int               `json:"recordCount,omitempty"`
	CreatedVersion   string            `json:"createdVersion,omitempty"`
	RefreshedVersion string            `json:"refreshedVersion,omitempty"`
	RefreshOnLoad    bool              `json:"refreshOnLoad,omitempty"`
	SaveData         *bool             `json:"saveData,omitempty"`
	Source           PivotSourceRef    `json:"source,omitempty"`
	Fields           []PivotCacheField `json:"fields,omitempty"`
}

type PivotRef struct {
	Number          int             `json:"number"`
	Sheet           string          `json:"sheet"`
	SheetNumber     int             `json:"sheetNumber"`
	SheetPartURI    string          `json:"sheetPartUri"`
	RelationshipID  string          `json:"relationshipId"`
	PartURI         string          `json:"partUri"`
	Name            string          `json:"name,omitempty"`
	CacheID         int             `json:"cacheId,omitempty"`
	Location        string          `json:"location,omitempty"`
	Rows            int             `json:"rows,omitempty"`
	Cols            int             `json:"cols,omitempty"`
	PrimarySelector string          `json:"primarySelector,omitempty"`
	Selectors       []string        `json:"selectors,omitempty"`
	Cache           *PivotCacheRef  `json:"cache,omitempty"`
	RowFields       []PivotFieldRef `json:"rowFields,omitempty"`
	ColumnFields    []PivotFieldRef `json:"columnFields,omitempty"`
	DataFields      []PivotFieldRef `json:"dataFields,omitempty"`
	FilterFields    []PivotFieldRef `json:"filterFields,omitempty"`
	Fields          []PivotFieldRef `json:"fields,omitempty"`
}
