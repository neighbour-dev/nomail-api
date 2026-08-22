package auth

import (
	"nomail-api/config"
	"nomail-api/internal/platform/util"
	"testing"
)

func TestRequiresAccountSetup(t *testing.T) {
	handler := &Handler{appConfig: config.AppConfig{InitialUsername: "admin", InitialPassword: "change-me-now"}}

	if !handler.requiresAccountSetup("admin", "change-me-now") {
		t.Fatal("expected initial credentials to require account setup")
	}
	if handler.requiresAccountSetup("admin", "changed-password") {
		t.Fatal("expected changed password to skip account setup")
	}
	if handler.requiresAccountSetup("changed-user", "change-me-now") {
		t.Fatal("expected changed username to skip account setup")
	}
}

func TestLoginRequestDeviceValidation(t *testing.T) {
	valid := loginRequest{
		Username: "admin", Password: "password123", DeviceID: "device-a",
		Platform: "ios", DeviceType: "phone",
	}
	if err := util.Validate(&valid); err != nil {
		t.Fatalf("Validate() error = %v", err)
	}

	invalidPlatform := valid
	invalidPlatform.Platform = "symbian"
	if err := util.Validate(&invalidPlatform); err == nil {
		t.Fatal("Validate() expected unsupported platform error")
	}

	missingType := valid
	missingType.DeviceType = ""
	if err := util.Validate(&missingType); err == nil {
		t.Fatal("Validate() expected missing device type error")
	}
}
