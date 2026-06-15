// Package sst parses XLSX sharedStrings.xml parts.
package sst

import (
	"bytes"
	"encoding/xml"
	"fmt"
	"io"
	"strings"
)

// Table is a parsed shared string table.
type Table struct {
	Count       int
	UniqueCount int
	Items       []Item
}

// Item is one shared string entry.
type Item struct {
	Text string
	Runs []Run
}

// Run is one rich-text run within a shared string entry.
type Run struct {
	Text string
}

type sstXML struct {
	XMLName     xml.Name `xml:"sst"`
	Count       int      `xml:"count,attr"`
	UniqueCount int      `xml:"uniqueCount,attr"`
	Items       []siXML  `xml:"si"`
}

type siXML struct {
	Text string `xml:"t"`
	Runs []rXML `xml:"r"`
}

type rXML struct {
	Text string `xml:"t"`
}

// Parse reads and decodes a sharedStrings.xml document.
func Parse(r io.Reader) (*Table, error) {
	if r == nil {
		return nil, fmt.Errorf("shared string reader cannot be nil")
	}
	var raw sstXML
	decoder := xml.NewDecoder(r)
	if err := decoder.Decode(&raw); err != nil {
		return nil, fmt.Errorf("failed to parse shared strings: %w", err)
	}
	if raw.XMLName.Local != "sst" {
		return nil, fmt.Errorf("expected shared string root <sst>, got <%s>", raw.XMLName.Local)
	}

	table := &Table{
		Count:       raw.Count,
		UniqueCount: raw.UniqueCount,
		Items:       make([]Item, 0, len(raw.Items)),
	}
	for _, rawItem := range raw.Items {
		item := Item{Runs: make([]Run, 0, len(rawItem.Runs))}
		if len(rawItem.Runs) == 0 {
			item.Text = rawItem.Text
		} else {
			var b strings.Builder
			for _, rawRun := range rawItem.Runs {
				run := Run{Text: rawRun.Text}
				item.Runs = append(item.Runs, run)
				b.WriteString(run.Text)
			}
			item.Text = b.String()
		}
		table.Items = append(table.Items, item)
	}

	return table, nil
}

// ParseBytes decodes a sharedStrings.xml document from bytes.
func ParseBytes(data []byte) (*Table, error) {
	return Parse(bytes.NewReader(data))
}

// Len returns the number of unique shared string items parsed.
func (t *Table) Len() int {
	if t == nil {
		return 0
	}
	return len(t.Items)
}

// Text returns the string at index and whether it exists.
func (t *Table) Text(index int) (string, bool) {
	if t == nil || index < 0 || index >= len(t.Items) {
		return "", false
	}
	return t.Items[index].Text, true
}
