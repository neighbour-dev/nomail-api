package auth

import (
	"nomail-api/internal/platform/util"
	"testing"
)

func TestUpdateAccountRequestValidation(t *testing.T) {
	tests := []struct {
		name    string
		request updateAccountRequest
		wantErr bool
	}{
		{
			name:    "requires an update",
			request: updateAccountRequest{CurrentPassword: "current-password"},
			wantErr: true,
		},
		{
			name:    "accepts username",
			request: updateAccountRequest{CurrentPassword: "current-password", Username: "admin"},
		},
		{
			name:    "accepts new password",
			request: updateAccountRequest{CurrentPassword: "current-password", NewPassword: "new-password"},
		},
		{
			name:    "defers password strength to policy validation",
			request: updateAccountRequest{CurrentPassword: "current-password", NewPassword: "short"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := util.Validate(&tt.request)
			if (err != nil) != tt.wantErr {
				t.Fatalf("Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}
