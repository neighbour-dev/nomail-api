package auth

import (
	"nomail-api/infra/db/repo"

	"gorm.io/gorm"
)

type Handler struct {
	users         *repo.UserRepo
	refreshTokens *repo.RefreshTokenRepo
}

func NewHandler(db *gorm.DB) *Handler {
	return &Handler{
		users:         repo.NewUserRepo(db),
		refreshTokens: repo.NewRefreshTokenRepo(db),
	}
}
