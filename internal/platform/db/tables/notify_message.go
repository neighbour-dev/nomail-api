package tables

import (
	"time"

	"github.com/google/uuid"
	"gorm.io/gorm"
)

type NotifyMessage struct {
	ID        uuid.UUID      `gorm:"type:uuid;primaryKey"`
	CreatedAt time.Time      `gorm:"not null"`
	UpdatedAt time.Time      `gorm:"not null"`
	DeletedAt gorm.DeletedAt `gorm:"index"`

	WebhookConfigID uint `gorm:"not null;index"`

	Source string `gorm:"size:100;index"`

	Title   string `gorm:"not null;size:255"`
	Content string `gorm:"type:text"`

	Level  string `gorm:"not null;default:normal;size:30;index"`
	Status string `gorm:"not null;default:unread;size:30;index"`

	URL string `gorm:"type:text"`

	RawPayload  string `gorm:"type:text"`
	ContentType string `gorm:"size:100"`

	Pinned bool `gorm:"not null;default:false;index"`

	ExpireAt *time.Time `gorm:"index"`
	ReadAt   *time.Time `gorm:"index"`
}

func (m *NotifyMessage) BeforeCreate(tx *gorm.DB) error {
	if m.ID == uuid.Nil {
		id, err := uuid.NewV7()
		if err != nil {
			return err
		}
		m.ID = id
	}
	return nil
}
