package repo

import (
	"nomail-api/internal/platform/db/tables"
	"time"

	"github.com/google/uuid"
	"gorm.io/gorm"
)

type NotifyMessageRepo struct {
	db *gorm.DB
}

type NotifyMessageFilter struct {
	Query           string
	Status          string
	Level           string
	WebhookConfigID uint
	Pinned          *bool
	From            *time.Time
	To              *time.Time
}

func NewNotifyMessageRepo(db *gorm.DB) *NotifyMessageRepo {
	return &NotifyMessageRepo{db: db}
}

func (r *NotifyMessageRepo) ListByUserID(userID uint, filter NotifyMessageFilter, offset, limit int) ([]tables.NotifyMessage, int64, error) {
	var messages []tables.NotifyMessage
	var total int64

	query := r.ownedByUser(userID)
	if filter.Query != "" {
		pattern := "%" + filter.Query + "%"
		query = query.Where("notify_messages.title ILIKE ? OR notify_messages.content ILIKE ?", pattern, pattern)
	}
	if filter.Status != "" {
		query = query.Where("notify_messages.status = ?", filter.Status)
	}
	if filter.Level != "" {
		query = query.Where("notify_messages.level = ?", filter.Level)
	}
	if filter.WebhookConfigID != 0 {
		query = query.Where("notify_messages.webhook_config_id = ?", filter.WebhookConfigID)
	}
	if filter.Pinned != nil {
		query = query.Where("notify_messages.pinned = ?", *filter.Pinned)
	}
	if filter.From != nil {
		query = query.Where("notify_messages.created_at >= ?", *filter.From)
	}
	if filter.To != nil {
		query = query.Where("notify_messages.created_at <= ?", *filter.To)
	}

	if err := query.Count(&total).Error; err != nil {
		return nil, 0, err
	}

	err := query.Order("notify_messages.created_at DESC").Offset(offset).Limit(limit).Find(&messages).Error
	return messages, total, err
}

func (r *NotifyMessageRepo) CountUnreadByUserID(userID uint) (int64, error) {
	var count int64
	err := r.ownedByUser(userID).Where("notify_messages.status = ?", "unread").Count(&count).Error
	return count, err
}

func (r *NotifyMessageRepo) GetByIDAndUserID(id uuid.UUID, userID uint) (tables.NotifyMessage, error) {
	var message tables.NotifyMessage
	err := r.ownedByUser(userID).Where("notify_messages.id = ?", id).First(&message).Error
	return message, err
}

func (r *NotifyMessageRepo) Update(message *tables.NotifyMessage, values map[string]any) error {
	return r.db.Model(message).Updates(values).Error
}

func (r *NotifyMessageRepo) Delete(message *tables.NotifyMessage) error {
	return r.db.Delete(message).Error
}

func (r *NotifyMessageRepo) BatchUpdateByUserID(userID uint, ids []uuid.UUID, values map[string]any) (int64, error) {
	ownedIDs, err := r.listOwnedIDs(userID, ids)
	if err != nil || len(ownedIDs) == 0 {
		return 0, err
	}

	result := r.db.Model(&tables.NotifyMessage{}).Where("id IN ?", ownedIDs).Updates(values)
	return result.RowsAffected, result.Error
}

func (r *NotifyMessageRepo) BatchDeleteByUserID(userID uint, ids []uuid.UUID) (int64, error) {
	ownedIDs, err := r.listOwnedIDs(userID, ids)
	if err != nil || len(ownedIDs) == 0 {
		return 0, err
	}

	result := r.db.Where("id IN ?", ownedIDs).Delete(&tables.NotifyMessage{})
	return result.RowsAffected, result.Error
}

func (r *NotifyMessageRepo) listOwnedIDs(userID uint, ids []uuid.UUID) ([]uuid.UUID, error) {
	var ownedIDs []uuid.UUID
	err := r.ownedByUser(userID).Where("notify_messages.id IN ?", ids).Pluck("notify_messages.id", &ownedIDs).Error
	return ownedIDs, err
}

func (r *NotifyMessageRepo) ownedByUser(userID uint) *gorm.DB {
	return r.db.Model(&tables.NotifyMessage{}).
		Joins("JOIN webhook_configs ON webhook_configs.id = notify_messages.webhook_config_id AND webhook_configs.deleted_at IS NULL").
		Where("webhook_configs.user_id = ?", userID)
}
