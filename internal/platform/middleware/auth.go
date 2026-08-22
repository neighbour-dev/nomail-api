package middleware

import (
	"nomail-api/config"
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"
	"strings"
	"time"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

const (
	userIDLocalKey            = "user_id"
	sessionIDLocalKey         = "session_id"
	accessTokenExpiresAtLocal = "access_token_expires_at"
	deviceTouchInterval       = 5 * time.Minute
)

func Auth(db *gorm.DB, appConfig config.AppConfig) fiber.Handler {
	sessions := repo.NewSessionRepo(db)
	return func(c fiber.Ctx) error {
		authHeader := c.Get("Authorization")
		if authHeader == "" {
			return response.ErrorCode(c, response.CodeUnauthorized)
		}

		tokenString, ok := strings.CutPrefix(authHeader, "Bearer ")
		if !ok || tokenString == "" {
			return response.ErrorCode(c, response.CodeUnauthorized)
		}

		claims, err := util.ParseAccessToken(tokenString, appConfig.JwtSecret, appConfig.JwtIssuer)
		if err != nil {
			return response.ErrorCode(c, response.CodeUnauthorized)
		}
		active, err := sessions.IsActive(claims.SessionID, claims.UserID)
		if err != nil {
			return response.ErrorCode(c, response.CodeInternalError)
		}
		if !active {
			return response.ErrorCode(c, response.CodeUnauthorized)
		}
		if err := sessions.TouchDevice(claims.SessionID, deviceTouchInterval); err != nil {
			return response.ErrorCode(c, response.CodeInternalError)
		}

		c.Locals(userIDLocalKey, claims.UserID)
		c.Locals(sessionIDLocalKey, claims.SessionID)
		if claims.ExpiresAt != nil {
			c.Locals(accessTokenExpiresAtLocal, claims.ExpiresAt.Time)
		}
		return c.Next()
	}
}

func CurrentUserID(c fiber.Ctx) (uint, bool) {
	userID, ok := c.Locals(userIDLocalKey).(uint)
	return userID, ok
}

func CurrentAccessTokenExpiresAt(c fiber.Ctx) (time.Time, bool) {
	expiresAt, ok := c.Locals(accessTokenExpiresAtLocal).(time.Time)
	return expiresAt, ok
}
