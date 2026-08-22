package auth

import (
	"nomail-api/config"
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
