package config

import "os"

type AppConfig struct {
	// db
	DbHost     string
	DbPort     string
	DbUsername string
	DbPassword string
	DbDatabase string
	// redis
	RedisHost   string
	RedisPort   string
	RedisUser   string
	RedisPass   string
	RedisDb     string
	RedisPrefix string
	// jwt
	JwtSecret string
	JwtIssuer string
	// secret encryption
	SecretEncryptionKey string
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
		RedisHost:           os.Getenv("REDIS_HOST"),
		RedisPort:           os.Getenv("REDIS_PORT"),
		RedisUser:           os.Getenv("REDIS_USERNAME"),
		RedisPass:           os.Getenv("REDIS_PASSWORD"),
		RedisDb:             os.Getenv("REDIS_DB"),
		RedisPrefix:         os.Getenv("REDIS_PREFIX"),
		SecretEncryptionKey: os.Getenv("SECRET_ENCRYPTION_KEY"),
	}
}
