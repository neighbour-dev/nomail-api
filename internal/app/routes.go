package app

import (
	"nomail-api/config"
	"nomail-api/internal/auth"
	"nomail-api/internal/health"
	"nomail-api/internal/message"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/webhook"
	"time"

	"github.com/gofiber/fiber/v3"
	"github.com/gofiber/fiber/v3/middleware/limiter"
	"gorm.io/gorm"
)

func RegisterRoutes(app *fiber.App, dbClient *gorm.DB, appConfig config.AppConfig) {
	authHandler := auth.NewHandler(dbClient, appConfig)
	webhookHandler := webhook.NewHandler(webhook.Init(), dbClient, appConfig)
	healthHandler := health.NewHandler(dbClient)
	messageHandler := message.NewHandler(dbClient)

	app.Route("/api", func(api fiber.Router) {
		api.Route("/webhook", func(webhookRouter fiber.Router) {
			webhookRouter.Use(middleware.Auth(appConfig))
			webhookRouter.Get("/configs", webhookHandler.ListConfigs)
			webhookRouter.Post("/configs", webhookHandler.Create)
			webhookRouter.Get("/configs/:id", webhookHandler.GetConfig)
			webhookRouter.Patch("/configs/:id", webhookHandler.UpdateConfig)
			webhookRouter.Patch("/configs/:id/enabled", webhookHandler.UpdateEnabled)
			webhookRouter.Delete("/configs/:id", webhookHandler.DeleteConfig)
		})

		api.Post("/webhook/:key", webhookHandler.Handle)

		api.Get("/health", limiter.New(limiter.Config{
			Max:        1,
			Expiration: 2 * time.Second,
		}), healthHandler.Handle)

		api.Route("/messages", func(messageRouter fiber.Router) {
			messageRouter.Use(middleware.Auth(appConfig))
			messageRouter.Get("/", messageHandler.List)
			messageRouter.Get("/unread-count", messageHandler.UnreadCount)
			messageRouter.Post("/batch", messageHandler.Batch)
			messageRouter.Get("/:id/raw", messageHandler.Raw)
			messageRouter.Get("/:id", messageHandler.Get)
			messageRouter.Patch("/:id", messageHandler.Update)
			messageRouter.Delete("/:id", messageHandler.Delete)
		})

		api.Route("/auth", func(authRouter fiber.Router) {
			authRouter.Post("/login", authHandler.Login)
			authRouter.Post("/refresh-token", authHandler.RefreshToken)
			authRouter.Post("/logout", authHandler.Logout)

			authRouter = authRouter.Use(middleware.Auth(appConfig))
			authRouter.Get("/devices", authHandler.Devices)
			authRouter.Get("/account", authHandler.GetAccount)
			authRouter.Patch("/account", authHandler.UpdateAccount)
		})
	})
}
