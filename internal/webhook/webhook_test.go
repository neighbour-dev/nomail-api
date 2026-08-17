package webhook

import (
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/webhook/model"
	"strings"
	"testing"
	"time"

	"gorm.io/gorm"
)

func TestBuildNotifyMessage(t *testing.T) {
	now := time.Date(2026, time.August, 17, 12, 0, 0, 0, time.UTC)
	message := buildNotifyMessage(model.Message{
		Provider:   "generic",
		ExternalID: "event-1",
		Title:      "Disk alert",
		Content:    "Usage is 92%",
		Level:      "alert",
		URL:        "https://example.com/alerts/1",
	}, tables.WebhookConfig{Model: gorm.Model{ID: 1}, Source: "server-monitor"}, []byte(`{"id":"event-1"}`), "application/json", now)

	if message.ExternalID == nil || *message.ExternalID != "event-1" {
		t.Fatalf("ExternalID = %#v", message.ExternalID)
	}
	if message.WebhookConfigID != 1 || message.Source != "server-monitor" || message.Level != "critical" {
		t.Fatalf("buildNotifyMessage() = %#v", message)
	}
	if message.ExpireAt == nil || !message.ExpireAt.Equal(now.Add(30*24*time.Hour)) {
		t.Fatalf("ExpireAt = %v", message.ExpireAt)
	}
}

func TestNormalizeLevelAndRetention(t *testing.T) {
	tests := []struct {
		input string
		level string
		days  int
	}{
		{input: "failure", level: "critical", days: 30},
		{input: "warn", level: "warning", days: 7},
		{input: "pass", level: "success", days: 3},
		{input: "info", level: "normal", days: 3},
	}
	for _, tt := range tests {
		level := normalizeLevel(tt.input)
		if level != tt.level {
			t.Fatalf("normalizeLevel(%q) = %q, want %q", tt.input, level, tt.level)
		}
		if got := retentionForLevel(level); got != time.Duration(tt.days)*24*time.Hour {
			t.Fatalf("retentionForLevel(%q) = %v", level, got)
		}
	}
}

func TestTruncateRunes(t *testing.T) {
	got := truncateRunes(strings.Repeat("消", maxTitleRunes+1), maxTitleRunes)
	if len([]rune(got)) != maxTitleRunes {
		t.Fatalf("truncateRunes() returned %d runes", len([]rune(got)))
	}
}
