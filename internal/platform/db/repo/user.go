package repo

import (
	"nomail-api/internal/platform/db/generated"
	"nomail-api/internal/platform/db/tables"
	"time"

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

func (r *UserRepo) GetByID(id uint) (tables.User, error) {
	var user tables.User
	err := r.db.First(&user, id).Error
	return user, err
}

func (r *UserRepo) First() (tables.User, error) {
	var user tables.User
	err := r.db.Order("id ASC").First(&user).Error
	return user, err
}

func (r *UserRepo) Count() (int64, error) {
	var count int64
	err := r.db.Model(&tables.User{}).Count(&count).Error
	return count, err
}

func (r *UserRepo) Create(user *tables.User) error {
	return r.db.Create(user).Error
}

func (r *UserRepo) UpdateAccount(userID uint, username string, passwordHash *string) error {
	return r.db.Transaction(func(tx *gorm.DB) error {
		values := map[string]any{"username": username}
		if passwordHash != nil {
			values["password_hash"] = *passwordHash
		}
		if err := tx.Model(&tables.User{}).Where("id = ?", userID).Updates(values).Error; err != nil {
			return err
		}
		if passwordHash == nil {
			return nil
		}

		now := time.Now()
		return tx.Model(&tables.Session{}).
			Where("device_id IN (?) AND revoked_at IS NULL", tx.Model(&tables.Device{}).Select("id").Where("user_id = ?", userID)).
			Update("revoked_at", now).Error
	})
}
