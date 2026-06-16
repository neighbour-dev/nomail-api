package handles

import (
	"nomail-api/webhook"

	"github.com/gofiber/fiber/v3"
)

type WebHookHandler struct {
	registry *webhook.Registry
}

func NewWebHookHandler(registry *webhook.Registry) *WebHookHandler {
	return &WebHookHandler{
		registry: registry,
	}
}

func (h *WebHookHandler) Handle(c fiber.Ctx) error {
	provider := c.Params("provider")

	parser, ok := h.registry.Get(provider)

	if !ok {
		return c.Status(fiber.StatusNotFound).JSON(fiber.Map{
			"error": "provider not found",
		})
	}

	body := c.Body()

	if err := parser.Verify(c, body); err != nil {
		return c.Status(fiber.StatusUnauthorized).JSON(fiber.Map{
			"error": "invalid webhook signature",
		})
	}

	msg, err := parser.Parse(c, body)
	if err != nil {
		return c.Status(fiber.StatusBadRequest).JSON(fiber.Map{
			"error": "invalid webhook payload",
		})
	}

	_ = msg

	return c.SendStatus(fiber.StatusOK)
}
