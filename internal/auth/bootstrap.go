package auth

import (
	"fmt"
	"nomail-api/config"
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/db/tables"
	"nomail-api/internal/platform/util"

	"gorm.io/gorm"
)

type initialUserParams struct {
	Username string `json:"initialUsername" validate:"required,max=64"`
	Password string `json:"initialPassword" validate:"required"`
}

func EnsureInitialUser(db *gorm.DB, appConfig config.AppConfig) error {
	users := repo.NewUserRepo(db)
	count, err := users.Count()
	if err != nil {
		return fmt.Errorf("failed to check initial user: %w", err)
	}
	if count > 0 {
		return nil
	}

	params := initialUserParams{
		Username: appConfig.InitialUsername,
		Password: appConfig.InitialPassword,
	}
	if err := util.Validate(&params); err != nil {
		return fmt.Errorf("invalid initial account configuration: %w", err)
	}
	if err := util.ValidatePassword(params.Password); err != nil {
		return fmt.Errorf("invalid initial account configuration: %w", err)
	}

	passwordHash, err := util.HashPassword(params.Password)
	if err != nil {
		return fmt.Errorf("failed to hash initial password: %w", err)
	}
	if err := users.Create(&tables.User{Username: params.Username, PasswordHash: passwordHash}); err != nil {
		return fmt.Errorf("failed to create initial user: %w", err)
	}

	return nil
}
