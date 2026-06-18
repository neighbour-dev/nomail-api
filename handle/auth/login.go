package auth

import (
	"errors"
	"nomail-api/infra/db/tables"
	"nomail-api/models/response"
	"nomail-api/util"
	"time"

	"github.com/gofiber/fiber/v3"
	"golang.org/x/crypto/bcrypt"
	"gorm.io/gorm"
)

const (
	accessTokenTTL  = 2 * time.Hour
	refreshTokenTTL = 7 * 24 * time.Hour
)

type loginRequest struct {
	Username   string `json:"username" validate:"required,max=64"`
	Password   string `json:"password" validate:"required"`
	DeviceID   string `json:"deviceId" validate:"omitempty,max=128"`
	DeviceName string `json:"deviceName" validate:"omitempty,max=128"`
}

type tokenResponse struct {
	AccessToken           string       `json:"accessToken"`
	AccessTokenExpiresAt  time.Time    `json:"accessTokenExpiresAt"`
	RefreshToken          string       `json:"refreshToken"`
	RefreshTokenExpiresAt time.Time    `json:"refreshTokenExpiresAt"`
	User                  userResponse `json:"user"`
}

func (h *Handler) Login(c fiber.Ctx) error {
	req, err := util.BindAndValidate[loginRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	user, err := h.users.GetByUsername(req.Username)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid username or password")
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get user")
	}

	if err := bcrypt.CompareHashAndPassword([]byte(user.PasswordHash), []byte(req.Password)); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid username or password")
	}

	refreshToken := util.RandomUserToken()
	accessToken, accessTokenExpiresAt, err := util.GenerateAccessToken(user.ID, h.appConfig.JwtSecret, h.appConfig.JwtIssuer, accessTokenTTL)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to generate access token")
	}
	refreshTokenExpiresAt := time.Now().Add(refreshTokenTTL)

	if err := h.refreshTokens.Create(&tables.RefreshToken{
		UserID:     user.ID,
		TokenHash:  util.HashToken(refreshToken),
		ExpiresAt:  refreshTokenExpiresAt,
		DeviceID:   req.DeviceID,
		DeviceName: req.DeviceName,
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
			ID:       user.ID,
			Username: user.Username,
		},
	})
}
