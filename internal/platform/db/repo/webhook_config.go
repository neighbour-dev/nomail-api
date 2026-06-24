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

func (r *WebhookConfigRepo) ListByUserID(userID uint, offset int, limit int) ([]tables.WebhookConfig, int64, error) {
	var configs []tables.WebhookConfig
	var total int64

	query := r.db.Model(&tables.WebhookConfig{}).Where(generated.WebhookConfig.UserID.Eq(userID))
	if err := query.Count(&total).Error; err != nil {
		return nil, 0, err
	}

	err := query.Order("created_at DESC").Offset(offset).Limit(limit).Find(&configs).Error
	return configs, total, err
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

func (r *WebhookConfigRepo) Update(config *tables.WebhookConfig, values map[string]any) error {
	return r.db.Model(config).Updates(values).Error
}

func (r *WebhookConfigRepo) Delete(config *tables.WebhookConfig) error {
	return r.db.Delete(config).Error
}
