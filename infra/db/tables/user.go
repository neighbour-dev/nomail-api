package tables

import (
	"time"

	"gorm.io/gorm"
)

type User struct {
	gorm.Model
	Username     string `gorm:"uniqueIndex;not null;size:64"`
	PasswordHash string `gorm:"not null;size:255"`

	RefreshTokens []RefreshToken `gorm:"foreignKey:UserID"`

	Nonce string `gorm:"uniqueIndex;not null;size:64"`
}

type RefreshToken struct {
	gorm.Model

	UserID uint `gorm:"not null;index"`
	User   User `gorm:"constraint:OnDelete:CASCADE;"`

	// 只存 refresh token 的 hash，不存明文 token
	TokenHash string `gorm:"uniqueIndex;not null;size:64"`

	// 过期时间
	ExpiresAt time.Time `gorm:"not null;index"`

	// 主动登出、踢设备、token 轮换时设置
	RevokedAt *time.Time `gorm:"default:null"`

	// 可选：设备信息，方便多端管理
	DeviceID   string `gorm:"size:128;index"`
	DeviceName string `gorm:"size:128"`
	IPAddress  string `gorm:"size:64"`
	UserAgent  string `gorm:"size:255"`
}
