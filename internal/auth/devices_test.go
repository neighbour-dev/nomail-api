package auth

import (
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/util"
	"strings"
	"testing"
	"time"
)

func TestDeleteDeviceParamsValidation(t *testing.T) {
	tests := []struct {
		name     string
		deviceID string
		wantID   string
		wantErr  bool
	}{
		{name: "accepts device id", deviceID: "device-a", wantID: "device-a"},
		{name: "trims whitespace", deviceID: " device-a ", wantID: "device-a"},
		{name: "rejects empty", wantErr: true},
		{name: "rejects whitespace", deviceID: "   ", wantErr: true},
		{name: "accepts max length", deviceID: strings.Repeat("a", 128), wantID: strings.Repeat("a", 128)},
		{name: "rejects over max length", deviceID: strings.Repeat("a", 129), wantErr: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			params := deleteDeviceParams{DeviceID: tt.deviceID}
			err := util.Validate(&params)
			if (err != nil) != tt.wantErr {
				t.Fatalf("Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
			if !tt.wantErr && params.DeviceID != tt.wantID {
				t.Fatalf("DeviceID = %q, want %q", params.DeviceID, tt.wantID)
			}
		})
	}
}

func TestBuildDeviceResponses(t *testing.T) {
	now := time.Now()
	responses := buildDeviceResponses([]repo.LoginDevice{{
		DeviceID: "device-a", DeviceName: "Phone", Platform: "ios", DeviceType: "phone",
		Manufacturer: "Apple", Model: "iPhone 15 Pro", OSVersion: "18.6",
		AppVersion: "1.2.0", AppBuild: "102", Locale: "zh-CN", TimeZone: "Asia/Shanghai",
		IPAddress: "127.0.0.1", UserAgent: "Nomail", LastSeenAt: &now,
		LoggedIn: true, PushEnabled: true, ExpiresAt: &now, LoggedInAt: &now,
		CreatedAt: now, UpdatedAt: now,
	}})

	if len(responses) != 1 {
		t.Fatalf("len(responses) = %d, want 1", len(responses))
	}
	if responses[0].DeviceID != "device-a" || responses[0].DeviceName != "Phone" {
		t.Fatalf("unexpected response: %+v", responses[0])
	}
	if responses[0].Platform != "ios" || responses[0].Model != "iPhone 15 Pro" || !responses[0].PushEnabled {
		t.Fatalf("device metadata not mapped: %+v", responses[0])
	}
	if empty := buildDeviceResponses(nil); empty == nil || len(empty) != 0 {
		t.Fatalf("empty response = %#v, want non-nil empty slice", empty)
	}
}
