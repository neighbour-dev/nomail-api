package main

import (
	"log"
	"nomail-api/handles"
	"nomail-api/handle/health"
	"nomail-api/webhook"

	"github.com/gofiber/fiber/v3"
)

func main() {
	app := fiber.New()

	webhookHandler := handles.NewWebHookHandler(new(webhook.Init()))
	app.Get("/health", health.Handler)

	app.Get("/webhook/:provider", webhookHandler.Handle)

	log.Fatal(app.Listen(":3000"))
}
