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
