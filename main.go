package main

import (
	"log"
	"nomail-api/handle/auth"
	"nomail-api/handle/health"
	webhookHandler "nomail-api/handle/webhook"
	"nomail-api/webhook"

	"github.com/gofiber/fiber/v3"
)

func main() {
	app := fiber.New()

	app.Post("/webhook/:provider", webhookHandler.NewWebHookHandler(new(webhook.Init())).Handle)
	app.Get("/health", health.Handler)

	api := app.Group("/api")
	authGroup := api.Group("/auth")

	authGroup.Post("/register", auth.RegisterHandler)
	authGroup.Post("/login", auth.LoginHandler)
	authGroup.Post("/refresh-token", auth.GetRefreshTokenHandler)

	log.Fatal(app.Listen(":3000"))
}
