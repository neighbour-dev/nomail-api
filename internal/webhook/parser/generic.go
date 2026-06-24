package parser

import (
	"nomail-api/internal/webhook/model"

	"github.com/gofiber/fiber/v3"
)

type GenericParser struct{}

func (p *GenericParser) Provider() string { return TypeGeneric }

func (p *GenericParser) Verify(r fiber.Ctx, body []byte) error {
	return nil
}

func (p *GenericParser) Parse(r fiber.Ctx, body []byte) (model.Message, error) {
	return model.Message{}, nil
}
