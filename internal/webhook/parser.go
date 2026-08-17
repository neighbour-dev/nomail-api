package webhook

import (
	"nomail-api/internal/webhook/model"
)

type Parser interface {
	Provider() string
	Verify(headers map[string][]string, body []byte, secret string) error
	Parse(headers map[string][]string, body []byte) (model.Message, error)
}
