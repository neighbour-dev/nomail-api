package message

import (
	"nomail-api/internal/platform/db/repo"

	"gorm.io/gorm"
)

type Handler struct {
	messages *repo.NotifyMessageRepo
	configs  *repo.WebhookConfigRepo
}

func NewHandler(db *gorm.DB) *Handler {
	return &Handler{
		messages: repo.NewNotifyMessageRepo(db),
		configs:  repo.NewWebhookConfigRepo(db),
	}
}
