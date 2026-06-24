package app

import (
	"nomail-api/config"
	"nomail-api/internal/auth"
	"nomail-api/internal/health"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
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

	app.Route("/api", func(api fiber.Router) {
		webhookProtected := api.Group("/webhook", middleware.Auth(appConfig))
		webhookProtected.Post("/configs", webhookHandler.Create)
		api.Post("/webhook/:key", webhookHandler.Handle)

		api.Get("/health", limiter.New(limiter.Config{
			Max:        1,
			Expiration: 2 * time.Second,
		}), healthHandler.Handle)

		api.Post("/test", func(c fiber.Ctx) error {
			return response.Success(c, nil)
		})

		api.Route("/auth", func(authRouter fiber.Router) {
			authRouter.Post("/register", authHandler.Register)
			authRouter.Post("/login", authHandler.Login)
			authRouter.Post("/refresh-token", authHandler.RefreshToken)
			authRouter.Post("/logout", authHandler.Logout)

			authProtected := authRouter.Group("", middleware.Auth(appConfig))
			authProtected.Get("/devices", authHandler.Devices)
		})
	})
}
