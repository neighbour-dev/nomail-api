package parser

import (
	"nomail-api/models"

	"github.com/gofiber/fiber/v3"
)

type GithubParser struct{}

func (p *GithubParser) Provider() string {
	return TypeGeneric
}

func (p *GithubParser) Verify(r fiber.Ctx, body []byte) error {
	return nil
}

func (p *GithubParser) Parse(r fiber.Ctx, body []byte) (models.WebHookMessage, error) {
	return models.WebHookMessage{}, nil
}
