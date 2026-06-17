package main

import (
	"log"
	"nomail-api/handle/auth"
	"nomail-api/handle/health"
	"nomail-api/webhook"

	"github.com/gofiber/fiber/v3"
)

func main() {
	app := fiber.New()

	webhookHandler := handles.NewWebHookHandler(new(webhook.Init()))
	app.Get("/health", health.Handler)

	app.Get("/webhook/:provider", webhookHandler.Handle)
	api := app.Group("/api")
	authGroup := api.Group("/auth")

	authGroup.Post("/register", auth.RegisterHandler)
	authGroup.Post("/login", auth.LoginHandler)
	authGroup.Post("/refresh-token", auth.GetRefreshTokenHandler)

	log.Fatal(app.Listen(":3000"))
}
