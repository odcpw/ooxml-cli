package sst

import "testing"

func TestParseSharedStrings(t *testing.T) {
	xml := []byte(`<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="5" uniqueCount="4">
  <si><t>Hello</t></si>
  <si><t xml:space="preserve"> leading and trailing </t></si>
  <si><r><rPr/><t>Rich</t></r><r><t xml:space="preserve"> Text</t></r></si>
  <si><t>Tom &amp; Jerry</t></si>
</sst>`)

	table, err := ParseBytes(xml)
	if err != nil {
		t.Fatalf("ParseBytes returned error: %v", err)
	}
	if table.Count != 5 {
		t.Fatalf("Count = %d, want 5", table.Count)
	}
	if table.UniqueCount != 4 {
		t.Fatalf("UniqueCount = %d, want 4", table.UniqueCount)
	}
	if table.Len() != 4 {
		t.Fatalf("Len = %d, want 4", table.Len())
	}

	assertText(t, table, 0, "Hello")
	assertText(t, table, 1, " leading and trailing ")
	assertText(t, table, 2, "Rich Text")
	assertText(t, table, 3, "Tom & Jerry")

	if len(table.Items[2].Runs) != 2 {
		t.Fatalf("rich item runs = %d, want 2", len(table.Items[2].Runs))
	}
	if table.Items[2].Runs[1].Text != " Text" {
		t.Fatalf("second run text = %q, want %q", table.Items[2].Runs[1].Text, " Text")
	}
}

func TestParseSharedStringsEmptyItem(t *testing.T) {
	table, err := ParseBytes([]byte(`<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si/></sst>`))
	if err != nil {
		t.Fatalf("ParseBytes returned error: %v", err)
	}
	assertText(t, table, 0, "")
}

func TestTextOutOfRange(t *testing.T) {
	table, err := ParseBytes([]byte(`<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>A</t></si></sst>`))
	if err != nil {
		t.Fatalf("ParseBytes returned error: %v", err)
	}
	if _, ok := table.Text(-1); ok {
		t.Fatal("Text(-1) ok = true, want false")
	}
	if _, ok := table.Text(1); ok {
		t.Fatal("Text(1) ok = true, want false")
	}
}

func TestParseSharedStringsInvalidRoot(t *testing.T) {
	if _, err := ParseBytes([]byte(`<worksheet/>`)); err == nil {
		t.Fatal("ParseBytes expected error for invalid root")
	}
}

func assertText(t *testing.T, table *Table, index int, want string) {
	t.Helper()
	got, ok := table.Text(index)
	if !ok {
		t.Fatalf("Text(%d) ok = false, want true", index)
	}
	if got != want {
		t.Fatalf("Text(%d) = %q, want %q", index, got, want)
	}
}
