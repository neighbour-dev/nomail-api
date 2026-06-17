package util

import (
	"crypto/rand"
	"encoding/hex"
)

func RandomToken(byteCount int) string {
	b := make([]byte, byteCount)
	if _, err := rand.Read(b); err != nil {
		panic(err)
	}
	return hex.EncodeToString(b)
}

func RandomUserToken() string {
	return RandomToken(32)
}
