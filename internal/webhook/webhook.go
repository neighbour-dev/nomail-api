package webhook

import (
	"errors"
	"nomail-api/config"
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/response"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

type Handler struct {
	registry  *Registry
	configs   *repo.WebhookConfigRepo
	appConfig config.AppConfig
}

func NewHandler(registry Registry, db *gorm.DB, appConfig config.AppConfig) *Handler {
	return &Handler{
		registry:  &registry,
		configs:   repo.NewWebhookConfigRepo(db),
		appConfig: appConfig,
	}
}

func (h *Handler) Handle(c fiber.Ctx) error {
	key := c.Params("key")

	webhookConfig, err := h.configs.GetByKey(key)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeNotFound, "webhook config not found")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get webhook config")
	}

	parser, ok := h.registry.Get(webhookConfig.Provider)

	if !ok {
		return response.ErrorCodeWithMsg(c, response.CodeNotFound, "provider not found")
	}

	body := c.Body()

	if err := parser.Verify(c, body); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid webhook signature")
	}

	msg, err := parser.Parse(c, body)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, "invalid webhook signature")
	}

	_ = msg

	return c.SendString("ok")
}
