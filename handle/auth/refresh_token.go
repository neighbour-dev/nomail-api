package auth

import (
	"errors"
	"nomail-api/infra/db/tables"
	"nomail-api/models/response"
	"nomail-api/util"
	"time"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

type refreshTokenRequest struct {
	RefreshToken string `json:"refreshToken" validate:"required"`
}

func (h *Handler) RefreshToken(c fiber.Ctx) error {
	req, err := util.BindAndValidate[refreshTokenRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	oldRefreshToken, err := h.refreshTokens.GetValidByHash(util.HashToken(req.RefreshToken))
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid refresh token")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get refresh token")
	}

	if err := h.refreshTokens.Revoke(oldRefreshToken.ID); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to revoke refresh token")
	}

	accessToken := util.RandomUserToken()
	refreshToken := util.RandomUserToken()
	accessTokenExpiresAt := time.Now().Add(accessTokenTTL)
	refreshTokenExpiresAt := time.Now().Add(refreshTokenTTL)

	if err := h.refreshTokens.Create(&tables.RefreshToken{
		UserID:     oldRefreshToken.UserID,
		TokenHash:  util.HashToken(refreshToken),
		ExpiresAt:  refreshTokenExpiresAt,
		DeviceID:   oldRefreshToken.DeviceID,
		DeviceName: oldRefreshToken.DeviceName,
		IPAddress:  c.IP(),
		UserAgent:  c.Get("User-Agent"),
	}); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to create refresh token")
	}

	return response.Success(c, tokenResponse{
		AccessToken:           accessToken,
		AccessTokenExpiresAt:  accessTokenExpiresAt,
		RefreshToken:          refreshToken,
		RefreshTokenExpiresAt: refreshTokenExpiresAt,
		User: userResponse{
			ID:       oldRefreshToken.User.ID,
			Username: oldRefreshToken.User.Username,
		},
	})

}
