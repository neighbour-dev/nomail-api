package auth

import (
	"errors"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
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

	refreshToken := util.RandomUserToken()
	accessToken, accessTokenExpiresAt, err := util.GenerateAccessToken(oldRefreshToken.UserID, h.appConfig.JwtSecret, h.appConfig.JwtIssuer, accessTokenTTL)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to generate access token")
	}
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
