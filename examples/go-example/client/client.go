package main

import (
	"log"
	"net/http"
	"strings"
	"sync"
	"time"
)

func main() {
	var wg sync.WaitGroup
	concurrency := 100
	iterations := 1000

	for i := 0; i < concurrency; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < iterations; j++ {
				resp, err := http.Post("http://127.0.0.1:5800/login", "application/x-www-form-urlencoded", strings.NewReader("username=user1&password=pass1"))
				if err != nil {
					log.Printf("Error: %v\n", err)
				} else {
					resp.Body.Close()
					log.Println("Hit endpoint :) ")
				}
				time.Sleep(1 * time.Second)
			}
		}()
	}

	wg.Wait()
	log.Println("Stress test completed")
}
