package main

import (
	"crypto/rand"
	"encoding/base64"
	"fmt"
	"log"
)

func main() {
	key := make([]byte, 32)
	if _, err := rand.Read(key); err != nil {
		log.Fatal(err)
	}

	fmt.Printf("SECRET_ENCRYPTION_KEY=%s\n", base64.StdEncoding.EncodeToString(key))
}
