package parser

import (
	"nomail-api/internal/webhook/model"

	"github.com/gofiber/fiber/v3"
)

type FeishuParser struct{}

func (p *FeishuParser) Provider() string {
	return TypeFeiShu
}

func (p *FeishuParser) Verify(r fiber.Ctx, body []byte) error {
	return nil
}

func (p *FeishuParser) Parse(r fiber.Ctx, body []byte) (model.Message, error) {
	return model.Message{}, nil
}
