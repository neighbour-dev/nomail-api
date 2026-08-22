package auth

import (
	"errors"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"time"

	"github.com/gofiber/fiber/v3"
	"golang.org/x/crypto/bcrypt"
	"gorm.io/gorm"
)

const (
	accessTokenTTL   = 2 * time.Hour
	refreshTokenTTL  = 7 * 24 * time.Hour
	maxActiveDevices = 3
)

type loginRequest struct {
	Username     string `json:"username" validate:"required,max=64"`
	Password     string `json:"password" validate:"required"`
	DeviceID     string `json:"deviceId" validate:"required,max=128"`
	DeviceName   string `json:"deviceName" validate:"omitempty,max=128"`
	Platform     string `json:"platform" validate:"required,oneof=ios android macos windows linux web unknown"`
	DeviceType   string `json:"deviceType" validate:"required,oneof=phone tablet desktop web unknown"`
	Manufacturer string `json:"manufacturer" validate:"omitempty,max=128"`
	Model        string `json:"model" validate:"omitempty,max=128"`
	OSVersion    string `json:"osVersion" validate:"omitempty,max=64"`
	AppVersion   string `json:"appVersion" validate:"omitempty,max=64"`
	AppBuild     string `json:"appBuild" validate:"omitempty,max=64"`
	Locale       string `json:"locale" validate:"omitempty,max=32"`
	TimeZone     string `json:"timeZone" validate:"omitempty,max=64"`
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
	refreshTokenExpiresAt := time.Now().Add(refreshTokenTTL)
	session := tables.Session{
		RefreshTokenHash: util.HashToken(refreshToken),
		ExpiresAt:        refreshTokenExpiresAt,
	}

	if err := h.sessions.CreateForLogin(&tables.Device{
		UserID:        user.ID,
		Identifier:    req.DeviceID,
		Name:          req.DeviceName,
		Platform:      req.Platform,
		DeviceType:    req.DeviceType,
		Manufacturer:  req.Manufacturer,
		HardwareModel: req.Model,
		OSVersion:     req.OSVersion,
		AppVersion:    req.AppVersion,
		AppBuild:      req.AppBuild,
		Locale:        req.Locale,
		TimeZone:      req.TimeZone,
		IPAddress:     c.IP(),
		UserAgent:     c.Get("User-Agent"),
	}, &session, maxActiveDevices); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to create login session")
	}

	accessToken, accessTokenExpiresAt, err := util.GenerateAccessToken(user.ID, session.ID, h.appConfig.JwtSecret, h.appConfig.JwtIssuer, accessTokenTTL)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to generate access token")
	}

	return response.Success(c, tokenResponse{
		AccessToken:           accessToken,
		AccessTokenExpiresAt:  accessTokenExpiresAt,
		RefreshToken:          refreshToken,
		RefreshTokenExpiresAt: refreshTokenExpiresAt,
		User: userResponse{
			ID:                   user.ID,
			Username:             user.Username,
			RequiresAccountSetup: h.requiresAccountSetup(req.Username, req.Password),
		},
	})
}

func (h *Handler) requiresAccountSetup(username string, password string) bool {
	return username == h.appConfig.InitialUsername && password == h.appConfig.InitialPassword
}
