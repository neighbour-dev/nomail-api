package middleware

import (
	"nomail-api/config"
	"nomail-api/models/response"
	"nomail-api/util"
	"strings"

	"github.com/gofiber/fiber/v3"
)

const userIDLocalKey = "user_id"

func Auth(appConfig config.AppConfig) fiber.Handler {
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

		c.Locals(userIDLocalKey, claims.UserID)
		return c.Next()
	}
}

func CurrentUserID(c fiber.Ctx) (uint, bool) {
	userID, ok := c.Locals(userIDLocalKey).(uint)
	return userID, ok
}
