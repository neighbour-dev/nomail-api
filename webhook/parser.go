package webhook

import (
	"nomail-api/models"

	"github.com/gofiber/fiber/v3"
)

type Parser interface {
	Provider() string
	Verify(r fiber.Ctx, body []byte) error
	Parse(r fiber.Ctx, body []byte) (models.WebHookMessage, error)
}
