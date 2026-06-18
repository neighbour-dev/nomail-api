package main

import (
	"log"
	"nomail-api/config"
	"nomail-api/handle/auth"
	"nomail-api/handle/health"
	webhookHandler "nomail-api/handle/webhook"
	"nomail-api/infra/db"
	"nomail-api/webhook"

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

	app.Post("/webhook/:provider", webhookHandler.NewWebHookHandler(new(webhook.Init())).Handle)
	app.Get("/health", health.Handler)

	api := app.Group("/api")
	authGroup := api.Group("/auth")

	authHandler := auth.NewHandler(dbClient, appConfig)

	authGroup.Post("/register", authHandler.Register)
	authGroup.Post("/login", authHandler.Login)
	authGroup.Post("/refresh-token", authHandler.RefreshToken)

	log.Fatal(app.Listen(":3000"))
}
