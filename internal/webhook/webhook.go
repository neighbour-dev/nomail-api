package webhook

import (
	"nomail-api/internal/platform/response"
	"nomail-api/internal/webhook/model"

	"github.com/gofiber/fiber/v3"
)

type Handler struct {
	registry *Registry
}

func NewHandler(registry Registry) *Handler {
	return &Handler{
		registry: &registry,
	}
}

func (h *Handler) Handle(c fiber.Ctx) error {
	provider := c.Params("provider")

	parser, ok := h.registry.Get(provider)

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

	return response.Success(c, model.Message{})
}
