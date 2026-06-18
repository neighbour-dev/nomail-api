package auth

import (
	"nomail-api/config"
	"nomail-api/infra/db/repo"

	"gorm.io/gorm"
)

type Handler struct {
	users         *repo.UserRepo
	refreshTokens *repo.RefreshTokenRepo
	appConfig     config.AppConfig
}

func NewHandler(db *gorm.DB, appConfig config.AppConfig) *Handler {
	return &Handler{
		users:         repo.NewUserRepo(db),
		refreshTokens: repo.NewRefreshTokenRepo(db),
		appConfig:     appConfig,
	}
}
