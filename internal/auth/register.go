package auth

import (
	"errors"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/response"
	"nomail-api/internal/platform/util"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

type registerRequest struct {
	Username string `json:"username" validate:"required,max=64"`
	Password string `json:"password" validate:"required,min=8"`
}

type userResponse struct {
	ID       uint   `json:"id"`
	Username string `json:"username"`
}

func (h *Handler) Register(c fiber.Ctx) error {
	req, err := util.BindAndValidate[registerRequest](c)

	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInvalidRequest, err.Error())
	}

	if _, err := h.users.GetByUsername(req.Username); err == nil {
		return response.ErrorCodeWithMsg(c, response.CodeConflict, "username already exists")
	} else if !errors.Is(err, gorm.ErrRecordNotFound) {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to check username")
	}

	passwordHash, err := util.HashPassword(req.Password)
	if err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to hash password")
	}

	user := tables.User{
		Username:     req.Username,
		PasswordHash: passwordHash,
		Nonce:        util.RandomToken(32),
	}
	if err := h.users.Create(&user); err != nil {
		return response.ErrorCodeWithMsg(c, response.CodeInternalError, "failed to create user")
	}

	return response.Success(c, userResponse{
		ID:       user.ID,
		Username: user.Username,
	})
}
