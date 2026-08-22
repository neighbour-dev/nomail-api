package tables

import (
	"time"

	"gorm.io/gorm"
)

type User struct {
	gorm.Model
	Username     string `gorm:"uniqueIndex;not null;size:64"`
	PasswordHash string `gorm:"not null;size:255"`

	Devices []Device `gorm:"foreignKey:UserID"`
}

type Device struct {
	gorm.Model

	UserID uint `gorm:"not null;index;uniqueIndex:idx_user_device"`
	User   User `gorm:"constraint:OnDelete:CASCADE;"`

	Identifier string `gorm:"column:device_id;not null;size:128;uniqueIndex:idx_user_device"`
	Name       string `gorm:"size:128"`
	IPAddress  string `gorm:"size:64"`
	UserAgent  string `gorm:"size:255"`
}

type Session struct {
	gorm.Model

	DeviceID uint   `gorm:"not null;uniqueIndex"`
	Device   Device `gorm:"constraint:OnDelete:CASCADE;"`

	// 只存 refresh token 的 hash，不存明文 token。
	RefreshTokenHash string `gorm:"uniqueIndex;not null;size:64"`

	ExpiresAt  time.Time `gorm:"not null;index"`
	LoggedInAt time.Time `gorm:"not null;index"`
	RevokedAt  *time.Time
}
