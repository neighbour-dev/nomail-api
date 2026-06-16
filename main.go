package main

import (
	"log"
	"nomail-api/handles"
	"nomail-api/webhook"

	"github.com/gofiber/fiber/v3"
)

func main() {
	app := fiber.New()

	webhookHandler := handles.NewWebHookHandler(new(webhook.Init()))

	app.Get("/webhook/:provider", webhookHandler.Handle)

	log.Fatal(app.Listen(":3000"))
}
