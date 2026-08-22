package auth

import (
	"nomail-api/config"
	"nomail-api/internal/platform/db/repo"

	"gorm.io/gorm"
)

type Handler struct {
	users     *repo.UserRepo
	sessions  *repo.SessionRepo
	appConfig config.AppConfig
}

type userResponse struct {
	ID                   uint   `json:"id"`
	Username             string `json:"username"`
	RequiresAccountSetup bool   `json:"requiresAccountSetup"`
}

func NewHandler(db *gorm.DB, appConfig config.AppConfig) *Handler {
	return &Handler{
		users:     repo.NewUserRepo(db),
		sessions:  repo.NewSessionRepo(db),
		appConfig: appConfig,
	}
}
