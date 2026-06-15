// Package styles parses the XLSX styles.xml parts needed by sheet readers.
package styles

import (
	"bytes"
	"encoding/xml"
	"fmt"
	"io"
	"strconv"
	"strings"
	"unicode"
)

// Styles is a parsed subset of an XLSX styles.xml document.
type Styles struct {
	NumFmts map[int]string
	CellXfs []CellFormat
}

// CellFormat is one <xf> entry in <cellXfs>.
type CellFormat struct {
	NumFmtID          int
	ApplyNumberFormat bool
}

type styleSheetXML struct {
	XMLName xml.Name    `xml:"styleSheet"`
	NumFmts []numFmtXML `xml:"numFmts>numFmt"`
	CellXfs []xfXML     `xml:"cellXfs>xf"`
}

type numFmtXML struct {
	ID         int    `xml:"numFmtId,attr"`
	FormatCode string `xml:"formatCode,attr"`
}

type xfXML struct {
	NumFmtID          int    `xml:"numFmtId,attr"`
	ApplyNumberFormat string `xml:"applyNumberFormat,attr"`
}

var builtinDateNumFmts = map[int]struct{}{
	14: {}, 15: {}, 16: {}, 17: {},
	18: {}, 19: {}, 20: {}, 21: {}, 22: {},
	27: {}, 28: {}, 29: {}, 30: {}, 31: {}, 32: {}, 33: {}, 34: {}, 35: {}, 36: {},
	45: {}, 46: {}, 47: {},
	50: {}, 51: {}, 52: {}, 53: {}, 54: {}, 55: {}, 56: {}, 57: {}, 58: {},
}

var builtinNumFmtCodes = map[int]string{
	0:  "General",
	1:  "0",
	2:  "0.00",
	3:  "#,##0",
	4:  "#,##0.00",
	9:  "0%",
	10: "0.00%",
	11: "0.00E+00",
	12: "# ?/?",
	13: "# ??/??",
	14: "m/d/yy",
	15: "d-mmm-yy",
	16: "d-mmm",
	17: "mmm-yy",
	18: "h:mm AM/PM",
	19: "h:mm:ss AM/PM",
	20: "h:mm",
	21: "h:mm:ss",
	22: "m/d/yy h:mm",
	37: "#,##0 ;(#,##0)",
	38: "#,##0 ;[Red](#,##0)",
	39: "#,##0.00;(#,##0.00)",
	40: "#,##0.00;[Red](#,##0.00)",
	45: "mm:ss",
	46: "[h]:mm:ss",
	47: "mmss.0",
	48: "##0.0E+0",
	49: "@",
}

// Parse reads and decodes a styles.xml document.
func Parse(r io.Reader) (*Styles, error) {
	if r == nil {
		return nil, fmt.Errorf("styles reader cannot be nil")
	}
	var raw styleSheetXML
	decoder := xml.NewDecoder(r)
	if err := decoder.Decode(&raw); err != nil {
		return nil, fmt.Errorf("failed to parse styles: %w", err)
	}
	if raw.XMLName.Local != "styleSheet" {
		return nil, fmt.Errorf("expected styles root <styleSheet>, got <%s>", raw.XMLName.Local)
	}

	result := &Styles{
		NumFmts: make(map[int]string, len(raw.NumFmts)),
		CellXfs: make([]CellFormat, 0, len(raw.CellXfs)),
	}
	for _, fmtDef := range raw.NumFmts {
		result.NumFmts[fmtDef.ID] = fmtDef.FormatCode
	}
	for _, xf := range raw.CellXfs {
		result.CellXfs = append(result.CellXfs, CellFormat{
			NumFmtID:          xf.NumFmtID,
			ApplyNumberFormat: parseBoolAttr(xf.ApplyNumberFormat),
		})
	}

	return result, nil
}

// BuiltinNumberFormatCode returns the format code for a common built-in number format id.
func BuiltinNumberFormatCode(id int) (string, bool) {
	code, ok := builtinNumFmtCodes[id]
	return code, ok
}

// ParseBytes decodes a styles.xml document from bytes.
func ParseBytes(data []byte) (*Styles, error) {
	return Parse(bytes.NewReader(data))
}

// NumberFormat returns the number format id/code used by a cell style index.
func (s *Styles) NumberFormat(styleIndex int) (int, string, bool) {
	if s == nil || styleIndex < 0 || styleIndex >= len(s.CellXfs) {
		return 0, "", false
	}
	formatID := s.CellXfs[styleIndex].NumFmtID
	if code, ok := s.NumFmts[formatID]; ok {
		return formatID, code, true
	}
	if code, ok := BuiltinNumberFormatCode(formatID); ok {
		return formatID, code, true
	}
	return formatID, "", true
}

// IsDateStyle reports whether a cell style index uses a date-like number format.
func (s *Styles) IsDateStyle(styleIndex int) bool {
	if s == nil || styleIndex < 0 || styleIndex >= len(s.CellXfs) {
		return false
	}
	formatID := s.CellXfs[styleIndex].NumFmtID
	if _, ok := builtinDateNumFmts[formatID]; ok {
		return true
	}
	formatCode, ok := s.NumFmts[formatID]
	if !ok {
		return false
	}
	return IsDateLikeFormat(formatCode)
}

// IsDateLikeFormat reports whether a custom number format contains date/time tokens.
func IsDateLikeFormat(formatCode string) bool {
	formatCode = strings.TrimSpace(formatCode)
	if formatCode == "" {
		return false
	}
	code := stripFormatLiterals(formatCode)
	code = strings.ToLower(code)

	if strings.Contains(code, "am/pm") || strings.Contains(code, "a/p") {
		return true
	}

	hasMonthOrMinute := false
	for _, r := range code {
		switch r {
		case 'y', 'd', 'h', 's':
			return true
		case 'm':
			hasMonthOrMinute = true
		}
	}
	return hasMonthOrMinute
}

func parseBoolAttr(value string) bool {
	value = strings.TrimSpace(strings.ToLower(value))
	if value == "" {
		return false
	}
	parsed, err := strconv.ParseBool(value)
	if err == nil {
		return parsed
	}
	return value == "1"
}

func stripFormatLiterals(formatCode string) string {
	var b strings.Builder
	runes := []rune(formatCode)
	for i := 0; i < len(runes); i++ {
		r := runes[i]
		switch r {
		case '"':
			for i++; i < len(runes) && runes[i] != '"'; i++ {
			}
		case '\\':
			i++
		case '_', '*':
			i++
		case '[':
			end := i + 1
			for end < len(runes) && runes[end] != ']' {
				end++
			}
			if end < len(runes) {
				content := strings.ToLower(string(runes[i+1 : end]))
				if isElapsedTimeToken(content) {
					b.WriteString(content)
				}
				i = end
			} else {
				b.WriteRune(r)
			}
		default:
			if unicode.IsLetter(r) || r == '/' {
				b.WriteRune(r)
			}
		}
	}
	return b.String()
}

func isElapsedTimeToken(value string) bool {
	switch value {
	case "h", "hh", "m", "mm", "s", "ss":
		return true
	default:
		return false
	}
}
