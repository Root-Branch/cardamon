package main

import (
	"encoding/json"
	"log"
	"net/http"

	"github.com/go-redis/redis/v8"
	"github.com/gorilla/mux"
)

var redisClient *redis.Client

type Note struct {
	ID   string `json:"id"`
	Text string `json:"text"`
}

func main() {
	// Initialize Redis client
	redisClient = redis.NewClient(&redis.Options{
		Addr: "redis:6379", // Using the service name from docker-compose
		DB:   0,
	})

	// Initialize router
	r := mux.NewRouter()

	// Define routes
	r.HandleFunc("/notes", getNotes).Methods("GET")
	r.HandleFunc("/notes/{id}", getNote).Methods("GET")
	r.HandleFunc("/notes", setNote).Methods("POST")
	r.HandleFunc("/notes/{id}", deleteNote).Methods("DELETE")

	// Start server
	log.Println("Server is running on port 8080")
	log.Fatal(http.ListenAndServe(":8080", r))
}

func getNotes(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	keys, err := redisClient.Keys(ctx, "*").Result()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	notes := []Note{}
	for _, key := range keys {
		val, err := redisClient.Get(ctx, key).Result()
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		notes = append(notes, Note{ID: key, Text: val})
	}

	json.NewEncoder(w).Encode(notes)
}

func getNote(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	vars := mux.Vars(r)
	id := vars["id"]

	val, err := redisClient.Get(ctx, id).Result()
	if err == redis.Nil {
		http.Error(w, "Note not found", http.StatusNotFound)
		return
	} else if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	json.NewEncoder(w).Encode(Note{ID: id, Text: val})
}

func setNote(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	var note Note
	err := json.NewDecoder(r.Body).Decode(&note)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}

	err = redisClient.Set(ctx, note.ID, note.Text, 0).Err()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	w.WriteHeader(http.StatusCreated)
	json.NewEncoder(w).Encode(note)
}

func deleteNote(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	vars := mux.Vars(r)
	id := vars["id"]

	_, err := redisClient.Del(ctx, id).Result()
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	w.WriteHeader(http.StatusNoContent)
}
