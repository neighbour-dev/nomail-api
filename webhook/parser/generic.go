package parser

import (
	"nomail-api/models"

	"github.com/gofiber/fiber/v3"
)

type GenericParser struct{}

func (p *GenericParser) Provider() string { return "generic" }

func (p *GenericParser) Verify(r fiber.Ctx, body []byte) error {
	return nil
}

func (p *GenericParser) Parse(r fiber.Ctx, body []byte) (models.WebHookMessage, error) {
	return models.WebHookMessage{}, nil
}
