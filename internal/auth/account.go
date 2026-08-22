package auth

import (
	"errors"
	"nomail-api/internal/platform/middleware"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"

	"github.com/gofiber/fiber/v3"
	"golang.org/x/crypto/bcrypt"
	"gorm.io/gorm"
)

type updateAccountRequest struct {
	CurrentPassword string `json:"currentPassword" validate:"required"`
	Username        string `json:"username" validate:"required_without=NewPassword,omitempty,max=64"`
	NewPassword     string `json:"newPassword" validate:"required_without=Username,omitempty"`
}

func (h *Handler) GetAccount(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	user, err := h.users.GetByID(userID)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCode(c, response.CodeUnauthorized)
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get user")
	}

	return response.Success(c, userResponse{ID: user.ID, Username: user.Username})
}

func (h *Handler) UpdateAccount(c fiber.Ctx) error {
	userID, ok := middleware.CurrentUserID(c)
	if !ok {
		return response.ErrorCode(c, response.CodeUnauthorized)
	}

	req, err := util.BindAndValidate[updateAccountRequest](c)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	user, err := h.users.GetByID(userID)
	if err != nil {
		if errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCode(c, response.CodeUnauthorized)
		}
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to get user")
	}
	if err := bcrypt.CompareHashAndPassword([]byte(user.PasswordHash), []byte(req.CurrentPassword)); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeUnauthorized, "invalid current password")
	}

	username := user.Username
	if req.Username != "" && req.Username != user.Username {
		if _, err := h.users.GetByUsername(req.Username); err == nil {
			return response.ErrorCodeWithMsg(c, response.CodeConflict, "username already exists")
		} else if !errors.Is(err, gorm.ErrRecordNotFound) {
			return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to check username")
		}
		username = req.Username
	}

	var passwordHash *string
	if req.NewPassword != "" {
		if err := util.ValidatePassword(req.NewPassword); err != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
		}
		hash, err := util.HashPassword(req.NewPassword)
		if err != nil {
			return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to hash password")
		}
		passwordHash = &hash
	}

	if err := h.users.UpdateAccount(user.ID, username, passwordHash); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to update account")
	}

	return response.Success(c, userResponse{ID: user.ID, Username: username})
}
