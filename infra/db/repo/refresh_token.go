package repo

import (
	"nomail-api/infra/db/generated"
	"nomail-api/infra/db/tables"
	"time"

	"gorm.io/gorm"
)

type RefreshTokenRepo struct {
	db *gorm.DB
}

// NewRefreshTokenRepo 创建 refresh token 持久化仓储。
func NewRefreshTokenRepo(db *gorm.DB) *RefreshTokenRepo {
	return &RefreshTokenRepo{db: db}
}

// Create 保存 refresh token 记录。
// 调用前应先对 token 明文做 hash，避免明文 token 入库。
func (r *RefreshTokenRepo) Create(refreshToken *tables.RefreshToken) error {
	return r.db.Create(refreshToken).Error
}

// CreateForLogin 保存登录会话，并限制同一用户同时活跃的设备数量。
// 同一个 deviceID 再次登录会替换旧 refresh token；新设备超限时踢掉最老设备。
func (r *RefreshTokenRepo) CreateForLogin(refreshToken *tables.RefreshToken, maxActiveDevices int) error {
	return r.db.Transaction(func(tx *gorm.DB) error {
		now := time.Now()

		if err := revokeActiveByDevice(tx, refreshToken.UserID, refreshToken.DeviceID, now); err != nil {
			return err
		}

		activeDeviceCount, err := countActiveDevices(tx, refreshToken.UserID, now)
		if err != nil {
			return err
		}

		if activeDeviceCount >= int64(maxActiveDevices) {
			oldestDeviceID, err := getOldestActiveDeviceID(tx, refreshToken.UserID, now)
			if err != nil {
				return err
			}
			if oldestDeviceID != "" {
				if err := revokeActiveByDevice(tx, refreshToken.UserID, oldestDeviceID, now); err != nil {
					return err
				}
			}
		}

		return tx.Create(refreshToken).Error
	})
}

// GetValidByHash 根据 token hash 查询仍可使用的 refresh token。
// 可使用的 token 必须存在、未过期且未被撤销。
func (r *RefreshTokenRepo) GetValidByHash(tokenHash string) (tables.RefreshToken, error) {
	var refreshToken tables.RefreshToken
	err := r.db.Preload("User").Where(
		generated.RefreshToken.TokenHash.Eq(tokenHash),
		generated.RefreshToken.ExpiresAt.Gt(time.Now()),
		generated.RefreshToken.RevokedAt.IsNull(),
	).First(&refreshToken).Error
	return refreshToken, err
}

// Revoke 将 refresh token 标记为已撤销，防止再次使用。
// 通常用于 refresh token 轮换和退出登录流程。
func (r *RefreshTokenRepo) Revoke(id uint) error {
	return r.db.Model(&tables.RefreshToken{}).Where(generated.RefreshToken.ID.Eq(id)).Updates(map[string]any{
		"revoked_at": new(time.Now()),
	}).Error
}

func revokeActiveByDevice(tx *gorm.DB, userID uint, deviceID string, revokedAt time.Time) error {
	return tx.Model(&tables.RefreshToken{}).Where(
		generated.RefreshToken.UserID.Eq(userID),
		generated.RefreshToken.DeviceID.Eq(deviceID),
		generated.RefreshToken.ExpiresAt.Gt(time.Now()),
		generated.RefreshToken.RevokedAt.IsNull(),
	).Update("revoked_at", revokedAt).Error
}

func countActiveDevices(tx *gorm.DB, userID uint, now time.Time) (int64, error) {
	var count int64
	err := tx.Model(&tables.RefreshToken{}).Where(
		generated.RefreshToken.UserID.Eq(userID),
		generated.RefreshToken.ExpiresAt.Gt(now),
		generated.RefreshToken.RevokedAt.IsNull(),
	).Distinct("device_id").Count(&count).Error
	return count, err
}

func getOldestActiveDeviceID(tx *gorm.DB, userID uint, now time.Time) (string, error) {
	var refreshToken tables.RefreshToken
	err := tx.Select("device_id").Where(
		generated.RefreshToken.UserID.Eq(userID),
		generated.RefreshToken.ExpiresAt.Gt(now),
		generated.RefreshToken.RevokedAt.IsNull(),
	).Order("created_at ASC").First(&refreshToken).Error
	if err != nil {
		return "", err
	}
	return refreshToken.DeviceID, nil
}
