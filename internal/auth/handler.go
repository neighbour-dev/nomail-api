package auth

import (
	"nomail-api/config"
	"nomail-api/internal/platform/db/repo"

	"gorm.io/gorm"
)

type Handler struct {
	users         *repo.UserRepo
	refreshTokens *repo.RefreshTokenRepo
	appConfig     config.AppConfig
}

type userResponse struct {
	ID       uint   `json:"id"`
	Username string `json:"username"`
}

func NewHandler(db *gorm.DB, appConfig config.AppConfig) *Handler {
	return &Handler{
		users:         repo.NewUserRepo(db),
		refreshTokens: repo.NewRefreshTokenRepo(db),
		appConfig:     appConfig,
	}
}
