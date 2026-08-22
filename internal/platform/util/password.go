package util

import (
	"errors"
	"unicode/utf8"
)

var ErrWeakPassword = errors.New("password must be at least 8 characters and contain letters and numbers")

func ValidatePassword(password string) error {
	if utf8.RuneCountInString(password) < 8 {
		return ErrWeakPassword
	}

	hasLetter := false
	hasDigit := false
	for _, character := range password {
		switch {
		case character >= 'a' && character <= 'z', character >= 'A' && character <= 'Z':
			hasLetter = true
		case character >= '0' && character <= '9':
			hasDigit = true
		}
	}
	if !hasLetter || !hasDigit {
		return ErrWeakPassword
	}
	return nil
}
