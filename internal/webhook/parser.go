package webhook

import (
	"nomail-api/internal/webhook/model"

	"github.com/gofiber/fiber/v3"
)

type Parser interface {
	Provider() string
	Verify(r fiber.Ctx, body []byte) error
	Parse(r fiber.Ctx, body []byte) (model.Message, error)
}
