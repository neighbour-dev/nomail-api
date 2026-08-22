package auth

import (
	"errors"
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

	oldTokenHash := util.HashToken(req.RefreshToken)
	session, err := h.sessions.GetValidByHash(oldTokenHash)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid refresh token")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get login session")
	}

	refreshToken := util.RandomUserToken()
	refreshTokenExpiresAt := time.Now().Add(refreshTokenTTL)
	accessToken, accessTokenExpiresAt, err := util.GenerateAccessToken(
		session.Device.UserID,
		session.ID,
		h.appConfig.JwtSecret,
		h.appConfig.JwtIssuer,
		accessTokenTTL,
	)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to generate access token")
	}

	if err := h.sessions.Rotate(
		session.ID,
		oldTokenHash,
		util.HashToken(refreshToken),
		refreshTokenExpiresAt,
		c.IP(),
		c.Get("User-Agent"),
	); err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid refresh token")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to rotate refresh token")
	}

	return response.Success(c, tokenResponse{
		AccessToken:           accessToken,
		AccessTokenExpiresAt:  accessTokenExpiresAt,
		RefreshToken:          refreshToken,
		RefreshTokenExpiresAt: refreshTokenExpiresAt,
		User: userResponse{
			ID:       session.Device.User.ID,
			Username: session.Device.User.Username,
		},
	})
}
