package main

import (
	"log"
	"nomail-api/config"
	appModule "nomail-api/internal/app"
	"nomail-api/internal/auth"
	"nomail-api/internal/platform/db"
	"nomail-api/internal/platform/util"

	"github.com/gofiber/fiber/v3"
	"github.com/joho/godotenv"
)

func main() {

	err := godotenv.Load()
	if err != nil {
		log.Fatal("Error loading .env file", err.Error(), "\n")
	}

	appConfig := config.FromEnv()
	if err := util.ValidateAESGCMKey(appConfig.SecretEncryptionKey); err != nil {
		log.Fatal(err)
	}

	dbClient := db.Init(appConfig)
	if err := auth.EnsureInitialUser(dbClient, appConfig); err != nil {
		log.Fatal(err)
	}

	app := fiber.New()
	appModule.RegisterRoutes(app, dbClient, appConfig)

	log.Fatal(app.Listen(":" + appConfig.Port))
}
