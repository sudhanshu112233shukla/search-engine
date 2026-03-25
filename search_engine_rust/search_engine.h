#ifndef SEARCH_ENGINE_H
#define SEARCH_ENGINE_H

#ifdef __cplusplus
extern "C" {
#endif

// Initialize engine from a JSON file path.
void init_engine_from_file(const char* path);

// Initialize engine from a JSON string.
void init_engine_from_json(const char* json);

// Returns a heap-allocated JSON string. Caller must free with free_string.
char* search_query(const char* query);

// Incrementally update engine from a JSON file path.
void update_engine_from_file(const char* path);

// Incrementally update engine from a JSON string.
void update_engine_from_json(const char* json);

// Frees a string allocated by search_query.
void free_string(char* ptr);

#ifdef __cplusplus
}
#endif

#endif // SEARCH_ENGINE_H
