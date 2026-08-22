package config

import "os"

type AppConfig struct {
	// db
	DbHost     string
	DbPort     string
	DbUsername string
	DbPassword string
	DbDatabase string
	// jwt
	JwtSecret string
	JwtIssuer string
	// secret encryption
	SecretEncryptionKey string
	// server
	Port string
	// initial single-user account
	InitialUsername string
	InitialPassword string
}

func FromEnv() AppConfig {

	return AppConfig{
		DbHost:              os.Getenv("DB_HOST"),
		DbPort:              os.Getenv("DB_PORT"),
		DbUsername:          os.Getenv("DB_USERNAME"),
		DbPassword:          os.Getenv("DB_PASSWORD"),
		DbDatabase:          os.Getenv("DB_DATABASE"),
		JwtSecret:           os.Getenv("JWT_SECRET"),
		JwtIssuer:           os.Getenv("JWT_ISSUER"),
		SecretEncryptionKey: os.Getenv("SECRET_ENCRYPTION_KEY"),
		Port:                os.Getenv("PORT"),
		InitialUsername:     os.Getenv("INITIAL_USERNAME"),
		InitialPassword:     os.Getenv("INITIAL_PASSWORD"),
	}
}
