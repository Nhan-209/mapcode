// tests/fixtures/sample_workspace/go_app/main.go

package main

import (
	"net/http"
	"github.com/gin-gonic/gin"
)

func pingHandler(c *gin.Context) {
	c.JSON(http.StatusOK, gin.H{
		"message": "pong",
	})
}

func main() {
	r := gin.Default()
	r.GET("/ping", pingHandler)
	r.Run(":8080")
}
