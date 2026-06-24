package parser

import (
	"nomail-api/internal/webhook/model"

	"github.com/gofiber/fiber/v3"
)

type GithubParser struct{}

func (p *GithubParser) Provider() string {
	return TypeGithub
}

func (p *GithubParser) Verify(r fiber.Ctx, body []byte) error {
	return nil
}

func (p *GithubParser) Parse(r fiber.Ctx, body []byte) (model.Message, error) {
	return model.Message{}, nil
}
