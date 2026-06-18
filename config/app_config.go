package config

import "os"

type AppConfig struct {
	// db
	//DbHost     string
	//DbPort     string
	//DbUsername string
	//DbPassword string
	//DbDatabase string
	//// redis
	//RedisHost   string
	//RedisPort   string
	//RedisUser   string
	//RedisPass   string
	//RedisDb     string
	//RedisPrefix string
	DbPath string `json:"db_path"`
}

func FromEnv() AppConfig {

	return AppConfig{
		//DbHost:      os.Getenv("DB_HOST"),
		//DbPort:      os.Getenv("DB_PORT"),
		//DbUsername:  os.Getenv("DB_USERNAME"),
		//DbPassword:  os.Getenv("DB_PASSWORD"),
		//DbDatabase:  os.Getenv("DB_DATABASE"),
		//RedisHost:   os.Getenv("REDIS_HOST"),
		//RedisPort:   os.Getenv("REDIS_PORT"),
		//RedisUser:   os.Getenv("REDIS_USERNAME"),
		//RedisPass:   os.Getenv("REDIS_PASSWORD"),
		//RedisDb:     os.Getenv("REDIS_DB"),
		//RedisPrefix: os.Getenv("REDIS_PREFIX"),
		DbPath: os.Getenv("DB_PATH"),
	}
}
