package repo

import (
	"nomail-api/internal/platform/db/generated"
	"nomail-api/internal/platform/db/tables"

	"gorm.io/gorm"
)

type WebhookConfigRepo struct {
	db *gorm.DB
}

func NewWebhookConfigRepo(db *gorm.DB) *WebhookConfigRepo {
	return &WebhookConfigRepo{db: db}
}

func (r *WebhookConfigRepo) Create(config *tables.WebhookConfig) error {
	return r.db.Create(config).Error
}

func (r *WebhookConfigRepo) GetByKey(key string) (tables.WebhookConfig, error) {
	var config tables.WebhookConfig
	err := r.db.Where(
		generated.WebhookConfig.Key.Eq(key),
		generated.WebhookConfig.Enabled.Eq(true),
	).First(&config).Error
	return config, err
}

func (r *WebhookConfigRepo) GetByIDAndUserID(id uint, userID uint) (tables.WebhookConfig, error) {
	var config tables.WebhookConfig
	err := r.db.Where(
		generated.WebhookConfig.ID.Eq(id),
		generated.WebhookConfig.UserID.Eq(userID),
	).First(&config).Error
	return config, err
}

func (r *WebhookConfigRepo) UpdateEnabled(config *tables.WebhookConfig, enabled bool) error {
	return r.db.Model(config).Update(generated.WebhookConfig.Enabled.Column().Name, enabled).Error
}
