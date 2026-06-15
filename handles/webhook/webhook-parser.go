package webhook

import "nomail-api/models"

type WebHookBody interface {
	parse() models.WebHookMessage
}
