package util

import "testing"

func TestValidatePassword(t *testing.T) {
	tests := []struct {
		password string
		valid    bool
	}{
		{password: "", valid: false},
		{password: "12345678", valid: false},
		{password: "abcdefgh", valid: false},
		{password: "abc123", valid: false},
		{password: "abc12345", valid: true},
		{password: "Abc12345!", valid: true},
	}

	for _, test := range tests {
		err := ValidatePassword(test.password)
		if (err == nil) != test.valid {
			t.Fatalf("ValidatePassword(%q) error = %v, valid = %v", test.password, err, test.valid)
		}
	}
}
