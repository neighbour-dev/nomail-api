package repo

import (
	"nomail-api/internal/platform/db/generated"
	"nomail-api/internal/platform/db/tables"

	"gorm.io/gorm"
)

type UserRepo struct {
	db *gorm.DB
}

func NewUserRepo(db *gorm.DB) *UserRepo {
	return &UserRepo{db: db}
}

func (r *UserRepo) GetByUsername(username string) (tables.User, error) {
	var user tables.User
	err := r.db.Where(generated.User.Username.Eq(username)).First(&user).Error
	return user, err
}

func (r *UserRepo) Create(user *tables.User) error {
	return r.db.Create(user).Error
}
