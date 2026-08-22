package util

import (
	"testing"
	"time"
)

func TestAccessTokenIncludesSession(t *testing.T) {
	token, _, err := GenerateAccessToken(7, 11, "test-secret", "nomail-test", time.Minute)
	if err != nil {
		t.Fatalf("GenerateAccessToken() error = %v", err)
	}

	claims, err := ParseAccessToken(token, "test-secret", "nomail-test")
	if err != nil {
		t.Fatalf("ParseAccessToken() error = %v", err)
	}
	if claims.UserID != 7 || claims.SessionID != 11 {
		t.Fatalf("claims = %+v, want user 7 and session 11", claims)
	}
}

func TestAccessTokenRejectsMissingSession(t *testing.T) {
	token, _, err := GenerateAccessToken(7, 0, "test-secret", "nomail-test", time.Minute)
	if err != nil {
		t.Fatalf("GenerateAccessToken() error = %v", err)
	}
	if _, err := ParseAccessToken(token, "test-secret", "nomail-test"); err == nil {
		t.Fatal("ParseAccessToken() expected missing session error")
	}
}
