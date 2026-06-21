package rangeio

import "testing"

func TestDecodeRecordsAndMapToRows(t *testing.T) {
	set, err := DecodeRecords([]byte(`[{"Region":"West","Amount":42,"Check":{"formula":"SUM(B2:B2)"}}]`))
	if err != nil {
		t.Fatalf("DecodeRecords returned error: %v", err)
	}
	rows, err := RecordsToRows(set.Records, []string{"Region", "Amount", "Check"}, MissingReject, false)
	if err != nil {
		t.Fatalf("RecordsToRows returned error: %v", err)
	}
	if len(rows) != 1 || len(rows[0]) != 3 {
		t.Fatalf("rows shape = %+v", rows)
	}
	if rows[0][0].Value != "West" || rows[0][1].Type != "number" || rows[0][1].Value != "42" || rows[0][2].Formula != "SUM(B2:B2)" {
		t.Fatalf("unexpected row values: %+v", rows[0])
	}
}

func TestDecodeRecordsWrappedObject(t *testing.T) {
	set, err := DecodeRecords([]byte(`{"records":[{"Region":"West","Amount":42}]}`))
	if err != nil {
		t.Fatalf("DecodeRecords returned error: %v", err)
	}
	if len(set.Records) != 1 || set.Records[0]["Region"].Value != "West" {
		t.Fatalf("unexpected records: %+v", set.Records)
	}
}

func TestRecordsToRowsMissingAndExtraPolicies(t *testing.T) {
	set, err := DecodeRecords([]byte(`[{"Region":"West","Extra":"ignored"}]`))
	if err != nil {
		t.Fatalf("DecodeRecords returned error: %v", err)
	}
	if _, err := RecordsToRows(set.Records, []string{"Region", "Amount"}, MissingReject, true); err == nil {
		t.Fatal("expected missing field error")
	}
	rows, err := RecordsToRows(set.Records, []string{"Region", "Amount"}, MissingSkip, true)
	if err != nil {
		t.Fatalf("RecordsToRows missing skip returned error: %v", err)
	}
	if !rows[0][1].Null {
		t.Fatalf("missing skip cell = %+v, want null", rows[0][1])
	}
	rows, err = RecordsToRows(set.Records, []string{"Region", "Amount"}, MissingEmptyString, true)
	if err != nil {
		t.Fatalf("RecordsToRows missing empty-string returned error: %v", err)
	}
	if rows[0][1].Null || rows[0][1].Value != "" {
		t.Fatalf("missing empty-string cell = %+v", rows[0][1])
	}
	if _, err := RecordsToRows(set.Records, []string{"Region"}, MissingReject, false); err == nil {
		t.Fatal("expected extra field error")
	}
}

func TestRecordsToRowsRejectsBadColumns(t *testing.T) {
	set, err := DecodeRecords([]byte(`[{"Region":"West"}]`))
	if err != nil {
		t.Fatalf("DecodeRecords returned error: %v", err)
	}
	if _, err := RecordsToRows(set.Records, []string{"Region", "Region"}, MissingReject, false); err == nil {
		t.Fatal("expected duplicate column error")
	}
	if _, err := RecordsToRows(set.Records, []string{" "}, MissingReject, false); err == nil {
		t.Fatal("expected blank column error")
	}
}
