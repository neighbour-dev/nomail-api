package parser

import (
	"nomail-api/models"

	"github.com/gofiber/fiber/v3"
)

type FeishuParser struct{}

func (p *FeishuParser) Provider() string {
	return TypeFeiShu
}

func (p *FeishuParser) Verify(r fiber.Ctx, body []byte) error {
	return nil
}

func (p *FeishuParser) Parse(r fiber.Ctx, body []byte) (models.WebHookMessage, error) {
	return models.WebHookMessage{}, nil
}
