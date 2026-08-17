package message

import (
	"strings"
	"testing"
	"time"
)

func TestUpdateValues(t *testing.T) {
	now := time.Date(2026, time.August, 17, 12, 0, 0, 0, time.UTC)
	read := "read"
	values, err := updateValues(updateRequest{Status: &read}, now)
	if err != nil {
		t.Fatalf("updateValues() error = %v", err)
	}
	if values["status"] != "read" || values["read_at"] != now {
		t.Fatalf("updateValues() = %#v", values)
	}

	unread := "unread"
	values, err = updateValues(updateRequest{Status: &unread}, now)
	if err != nil {
		t.Fatalf("updateValues() error = %v", err)
	}
	if values["status"] != "unread" || values["read_at"] != nil {
		t.Fatalf("updateValues() = %#v", values)
	}
}

func TestUpdateValuesRequiresSupportedChange(t *testing.T) {
	invalid := "archived"
	tests := []updateRequest{
		{},
		{Status: &invalid},
	}
	for _, request := range tests {
		if _, err := updateValues(request, time.Now()); err == nil {
			t.Fatalf("updateValues(%#v) expected error", request)
		}
	}
}

func TestBatchUpdateValues(t *testing.T) {
	now := time.Date(2026, time.August, 17, 12, 0, 0, 0, time.UTC)
	values, err := batchUpdateValues("mark_read", now)
	if err != nil {
		t.Fatalf("batchUpdateValues() error = %v", err)
	}
	if values["status"] != "read" || values["read_at"] != now {
		t.Fatalf("batchUpdateValues() = %#v", values)
	}

	values, err = batchUpdateValues("mark_unread", now)
	if err != nil {
		t.Fatalf("batchUpdateValues() error = %v", err)
	}
	if values["status"] != "unread" || values["read_at"] != nil {
		t.Fatalf("batchUpdateValues() = %#v", values)
	}
}

func TestParseUUIDsDeduplicates(t *testing.T) {
	const id = "0198c000-0000-7000-8000-000000000001"
	ids, err := parseUUIDs([]string{id, id})
	if err != nil {
		t.Fatalf("parseUUIDs() error = %v", err)
	}
	if len(ids) != 1 {
		t.Fatalf("parseUUIDs() returned %d IDs, want 1", len(ids))
	}

	if _, err := parseUUIDs([]string{"invalid"}); err == nil {
		t.Fatal("parseUUIDs() expected error")
	}
}

func TestSummarize(t *testing.T) {
	if got := summarize(" first\n\nsecond\tthird "); got != "first second third" {
		t.Fatalf("summarize() = %q", got)
	}

	got := summarize(strings.Repeat("消", maxSummaryRunes+1))
	if len([]rune(got)) != maxSummaryRunes {
		t.Fatalf("summarize() returned %d runes, want %d", len([]rune(got)), maxSummaryRunes)
	}
}
