package util

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/base64"
	"errors"
	"fmt"
	"io"
)

func ValidateAESGCMKey(base64Key string) error {
	if _, err := newAESGCM(base64Key); err != nil {
		return fmt.Errorf("invalid AES-GCM encryption key: %w", err)
	}
	return nil
}

func EncryptAESGCMString(plaintext string, base64Key string) (string, error) {
	gcm, err := newAESGCM(base64Key)
	if err != nil {
		return "", err
	}

	nonce := make([]byte, gcm.NonceSize())
	if _, err := io.ReadFull(rand.Reader, nonce); err != nil {
		return "", err
	}

	ciphertext := gcm.Seal(nil, nonce, []byte(plaintext), nil)
	data := append(nonce, ciphertext...)

	return base64.StdEncoding.EncodeToString(data), nil
}

func DecryptAESGCMString(encoded string, base64Key string) (string, error) {
	gcm, err := newAESGCM(base64Key)
	if err != nil {
		return "", err
	}

	data, err := base64.StdEncoding.DecodeString(encoded)
	if err != nil {
		return "", err
	}

	nonceSize := gcm.NonceSize()
	if len(data) < nonceSize {
		return "", errors.New("invalid ciphertext")
	}

	nonce := data[:nonceSize]
	ciphertext := data[nonceSize:]

	plaintext, err := gcm.Open(nil, nonce, ciphertext, nil)
	if err != nil {
		return "", err
	}

	return string(plaintext), nil
}

func newAESGCM(base64Key string) (cipher.AEAD, error) {
	key, err := base64.StdEncoding.DecodeString(base64Key)
	if err != nil {
		return nil, err
	}

	if len(key) != 32 {
		return nil, errors.New("encryption key must be 32 bytes")
	}

	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, err
	}

	return cipher.NewGCM(block)
}
