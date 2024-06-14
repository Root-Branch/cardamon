package main

import (
	"log"
	"net/http"

	"github.com/go-redis/redis"
	"github.com/labstack/echo"
)

var redisClient *redis.Client

func main() {
	// Initialize Redis client with connection pooling
	redisClient = redis.NewClient(&redis.Options{
		Addr:         "redis:6379",
		PoolSize:     10, // Adjust the pool size as needed
		MinIdleConns: 5,  // Adjust the minimum idle connections as needed
	})
	defer redisClient.Close()

	log.Println("Created redis client")

	// Create Echo instance
	e := echo.New()

	// Login endpoint
	e.POST("/login", loginHandler)
	log.Println("Created login endpoint")

	// Start server
	log.Println("Starting server")
	e.Logger.Fatal(e.Start(":5800"))
}

func loginHandler(c echo.Context) error {
	username := c.FormValue("username")
	password := c.FormValue("password")

	// Check if username and password exist in Redis
	exists, err := redisClient.HExists("users", username).Result()
	if err != nil {
		log.Println("error", err)
		return c.JSON(http.StatusInternalServerError, "Error checking username")
	}

	if exists {
		storedPassword, err := redisClient.HGet("users", username).Result()
		if err != nil {
			log.Println("error", err)
			return c.JSON(http.StatusInternalServerError, "Error retrieving password")
		}
		if storedPassword == password {
			return c.JSON(http.StatusOK, "Login successful")
		}
	}

	return c.JSON(http.StatusUnauthorized, "Invalid username or password")
}

