package util

import (
	"errors"
	"strconv"
	"time"

	"github.com/golang-jwt/jwt/v5"
)

type AccessTokenClaims struct {
	UserID uint `json:"user_id"`
	jwt.RegisteredClaims
}

func GenerateAccessToken(userID uint, secret string, issuer string, ttl time.Duration) (string, time.Time, error) {
	if secret == "" {
		return "", time.Time{}, errors.New("jwt secret is required")
	}

	now := time.Now()
	expiresAt := now.Add(ttl)
	claims := AccessTokenClaims{
		UserID: userID,
		RegisteredClaims: jwt.RegisteredClaims{
			Subject:   strconv.FormatUint(uint64(userID), 10),
			Issuer:    issuer,
			IssuedAt:  jwt.NewNumericDate(now),
			ExpiresAt: jwt.NewNumericDate(expiresAt),
		},
	}

	token := jwt.NewWithClaims(jwt.SigningMethodHS256, claims)
	tokenString, err := token.SignedString([]byte(secret))
	if err != nil {
		return "", time.Time{}, err
	}

	return tokenString, expiresAt, nil
}

func ParseAccessToken(tokenString string, secret string, issuer string) (*AccessTokenClaims, error) {
	if secret == "" {
		return nil, errors.New("jwt secret is required")
	}

	claims := &AccessTokenClaims{}
	token, err := jwt.ParseWithClaims(tokenString, claims, func(token *jwt.Token) (interface{}, error) {
		if token.Method != jwt.SigningMethodHS256 {
			return nil, errors.New("unexpected jwt signing method")
		}
		return []byte(secret), nil
	}, jwt.WithIssuer(issuer))
	if err != nil {
		return nil, err
	}
	if !token.Valid {
		return nil, errors.New("invalid access token")
	}
	if claims.UserID == 0 {
		return nil, errors.New("missing user id")
	}

	return claims, nil
}
