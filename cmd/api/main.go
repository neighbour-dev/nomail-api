package main

import (
	"log"
	"nomail-api/config"
	appModule "nomail-api/internal/app"
	"nomail-api/internal/platform/db"

	"github.com/gofiber/fiber/v3"
	"github.com/joho/godotenv"
)

func main() {

	err := godotenv.Load()
	if err != nil {
		log.Fatal("Error loading .env file", err.Error(), "\n")
	}

	appConfig := config.FromEnv()
	dbClient := db.Init(appConfig)

	app := fiber.New()
	appModule.RegisterRoutes(app, dbClient, appConfig)

	log.Fatal(app.Listen(":3000"))
}
