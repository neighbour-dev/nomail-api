package main

import (
	"log"
	"nomail-api/config"
	"nomail-api/handle/auth"
	"nomail-api/handle/health"
	webhookHandler "nomail-api/handle/webhook"
	"nomail-api/infra/db"
	"nomail-api/middleware"
	"nomail-api/models/response"
	"nomail-api/webhook"

	"github.com/gofiber/fiber/v3"
	"github.com/joho/godotenv"
	"gorm.io/gorm"
)

func main() {

	err := godotenv.Load()
	if err != nil {
		log.Fatal("Error loading .env file", err.Error(), "\n")
	}

	appConfig := config.FromEnv()
	dbClient := db.Init(appConfig)

	app := fiber.New()
	registerRoutes(app, dbClient, appConfig)

	log.Fatal(app.Listen(":3000"))
}

func registerRoutes(app *fiber.App, dbClient *gorm.DB, appConfig config.AppConfig) {

	authHandler := auth.NewHandler(dbClient, appConfig)

	app.Route("/api", func(api fiber.Router) {
		api.Post("/webhook/:provider", webhookHandler.NewWebHookHandler(new(webhook.Init())).Handle)

		api.Get("/health", health.NewHandler(dbClient).Handle)

		api.Post("/test", func(c fiber.Ctx) error {
			return response.Success(c, nil)
		})

		api.Route("/auth", func(authRouter fiber.Router) {
			authRouter.Post("/register", authHandler.Register)
			authRouter.Post("/login", authHandler.Login)
			authRouter.Post("/refresh-token", authHandler.RefreshToken)
			authRouter.Post("/logout", authHandler.Logout)

			authRouter.Group("", middleware.Auth(appConfig))
			authRouter.Get("/devices", authHandler.Devices)

		})

		api.Use(middleware.Auth(appConfig))

	})
}
