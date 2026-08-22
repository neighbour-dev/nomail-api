package app

import (
	"nomail-api/config"
	"nomail-api/internal/auth"
	"nomail-api/internal/health"
	"nomail-api/internal/message"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/realtime"
	"nomail-api/internal/webhook"
	"time"

	"github.com/gofiber/fiber/v3"
	"github.com/gofiber/fiber/v3/middleware/limiter"
	"gorm.io/gorm"
)

func RegisterRoutes(app *fiber.App, dbClient *gorm.DB, appConfig config.AppConfig) {
	realtimeHub := realtime.NewHub()
	authHandler := auth.NewHandler(dbClient, appConfig)
	webhookHandler := webhook.NewHandler(webhook.Init(), dbClient, appConfig, realtimeHub)
	healthHandler := health.NewHandler(dbClient)
	messageHandler := message.NewHandler(dbClient, realtimeHub)

	app.Route("/api", func(api fiber.Router) {
		api.Route("/webhook/configs", func(webhookRouter fiber.Router) {
			webhookRouter.Use(middleware.Auth(dbClient, appConfig))
			webhookRouter.Get("/", webhookHandler.ListConfigs)
			webhookRouter.Post("/", webhookHandler.Create)
			webhookRouter.Get("/:id", webhookHandler.GetConfig)
			webhookRouter.Patch("/:id", webhookHandler.UpdateConfig)
			webhookRouter.Patch("/:id/enabled", webhookHandler.UpdateEnabled)
			webhookRouter.Delete("/:id", webhookHandler.DeleteConfig)
		})

		api.Post("/webhook/:key", webhookHandler.Handle)

		api.Get("/health", limiter.New(limiter.Config{
			Max:        1,
			Expiration: 2 * time.Second,
		}), healthHandler.Handle)

		api.Route("/messages", func(messageRouter fiber.Router) {
			messageRouter.Use(middleware.Auth(dbClient, appConfig))
			messageRouter.Get("/", messageHandler.List)
			messageRouter.Get("/unread-count", messageHandler.UnreadCount)
			messageRouter.Get("/stream", messageHandler.Stream)
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

			authRouter = authRouter.Use(middleware.Auth(dbClient, appConfig))
			authRouter.Get("/devices", authHandler.Devices)
			authRouter.Delete("/devices/:deviceId/session", authHandler.KickDevice)
			authRouter.Delete("/devices/:deviceId", authHandler.DeleteDevice)
			authRouter.Get("/account", authHandler.GetAccount)
			authRouter.Patch("/account", authHandler.UpdateAccount)
		})
	})
}
