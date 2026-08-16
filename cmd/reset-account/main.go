package main

import (
	"flag"
	"log"
	"nomail-api/config"
	"nomail-api/internal/platform/db"
	"nomail-api/internal/platform/db/repo"
	"nomail-api/internal/platform/util"

	"github.com/joho/godotenv"
)

type resetAccountParams struct {
	Username string `json:"username" validate:"omitempty,max=64"`
	Password string `json:"password" validate:"required,min=8"`
}

func main() {
	username := flag.String("username", "", "new username; keep the current username when omitted")
	password := flag.String("password", "", "new password (minimum 8 characters)")
	flag.Parse()

	params := resetAccountParams{
		Username: *username,
		Password: *password,
	}
	if err := util.Validate(&params); err != nil {
		log.Fatal("invalid account parameters: ", err)
	}

	if err := godotenv.Load(); err != nil {
		log.Fatal("Error loading .env file: ", err)
	}

	dbClient := db.Init(config.FromEnv())
	users := repo.NewUserRepo(dbClient)
	user, err := users.First()
	if err != nil {
		log.Fatal("failed to get account: ", err)
	}
	if params.Username == "" {
		params.Username = user.Username
	}

	passwordHash, err := util.HashPassword(params.Password)
	if err != nil {
		log.Fatal("failed to hash password: ", err)
	}
	if err := users.UpdateAccount(user.ID, params.Username, &passwordHash); err != nil {
		log.Fatal("failed to reset account: ", err)
	}

	log.Printf("account reset successfully; username=%s", params.Username)
}
