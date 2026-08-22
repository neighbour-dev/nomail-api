package message

import (
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/realtime"

	"gorm.io/gorm"
)

type Handler struct {
	messages *repo.NotifyMessageRepo
	configs  *repo.WebhookConfigRepo
	realtime *realtime.Hub
}

func NewHandler(db *gorm.DB, realtimeHub *realtime.Hub) *Handler {
	return &Handler{
		messages: repo.NewNotifyMessageRepo(db),
		configs:  repo.NewWebhookConfigRepo(db),
		realtime: realtimeHub,
	}
}
