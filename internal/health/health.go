package health

import (
	"nomail-api/internal/platform/response"

	"github.com/gofiber/fiber/v3"
	"gorm.io/gorm"
)

type Handler struct {
	db *gorm.DB
}

type Status struct {
	Status string `json:"status"`
	DB     string `json:"db"`
	Redis  string `json:"redis"`
}

func NewHandler(db *gorm.DB) *Handler {
	return &Handler{db: db}
}

func (h *Handler) Handle(c fiber.Ctx) error {
	status := Status{
		Status: "ok",
		DB:     "ok",
		Redis:  "unknown",
	}

	sqlDB, err := h.db.DB()
	if err != nil {
		status.Status = "error"
		status.DB = "error"
		return response.Success(c, status)
	}

	if err := sqlDB.Ping(); err != nil {
		status.Status = "error"
		status.DB = "error"
		return response.Success(c, status)
	}

	return response.Success(c, status)
}
