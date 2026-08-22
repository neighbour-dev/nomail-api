package push

import (
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/util"
	"testing"
	"time"
)

func TestRegisterDeviceRequestValidation(t *testing.T) {
	request := registerDeviceRequest{
		DeviceID: "device-a", Provider: "apns", Environment: "production", PushToken: "token",
	}
	if err := util.Validate(&request); err != nil {
		t.Fatalf("Validate() error = %v", err)
	}

	request.Provider = "unsupported"
	if err := util.Validate(&request); err == nil {
		t.Fatal("Validate() expected unsupported provider error")
	}
}

func TestBuildEndpointResponseExcludesToken(t *testing.T) {
	now := time.Now()
	response := buildEndpointResponse("device-a", tables.PushEndpoint{
		Provider: "fcm", Environment: "production", Enabled: true, RegisteredAt: now,
		EncryptedToken: "encrypted", TokenHash: "hash",
	})

	if response.DeviceID != "device-a" || response.Provider != "fcm" || !response.Enabled {
		t.Fatalf("unexpected response: %+v", response)
	}
}
