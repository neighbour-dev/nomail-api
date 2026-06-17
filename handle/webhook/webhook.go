package webhook

import (
	"nomail-api/models"
	"nomail-api/models/response"
	"nomail-api/webhook"

	"github.com/gofiber/fiber/v3"
)

type Handler struct {
	registry *webhook.Registry
}

func NewWebHookHandler(registry *webhook.Registry) *Handler {
	return &Handler{
		registry: registry,
	}
}

func (h *Handler) Handle(c fiber.Ctx) error {
	provider := c.Params("provider")

	parser, ok := h.registry.Get(provider)

	if !ok {
		return response.ErrorWithStatus(c, fiber.StatusNotFound, 3, "provider not found")
	}

	body := c.Body()

	if err := parser.Verify(c, body); err != nil {
		return response.ErrorWithStatus(c, fiber.StatusUnauthorized, 2, "invalid webhook signature")
	}

	msg, err := parser.Parse(c, body)
	if err != nil {
		return response.ErrorWithStatus(c, fiber.StatusBadRequest, 3, "invalid webhook signature")
	}

	_ = msg

	return response.Success(c, models.WebHookMessage{})
}
