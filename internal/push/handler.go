package push

import (
	"nomail-api/config"
	"nomail-api/internal/platform/db/repo"

	"gorm.io/gorm"
)

type Handler struct {
	endpoints *repo.PushEndpointRepo
	appConfig config.AppConfig
}

func NewHandler(db *gorm.DB, appConfig config.AppConfig) *Handler {
	return &Handler{
		endpoints: repo.NewPushEndpointRepo(db),
		appConfig: appConfig,
	}
}
