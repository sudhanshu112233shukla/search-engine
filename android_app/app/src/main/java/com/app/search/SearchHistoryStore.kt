package com.app.search

import android.content.Context
import android.content.SharedPreferences

class SearchHistoryStore(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("search_history", Context.MODE_PRIVATE)

    fun load(): List<String> {
        val raw = prefs.getString("queries", "") ?: ""
        if (raw.isBlank()) return emptyList()
        return raw.split("||").filter { it.isNotBlank() }
    }

    private fun save(queries: List<String>) {
        val joined = queries.joinToString("||")
        prefs.edit().putString("queries", joined).apply()
    }

    fun add(query: String) {
        val existing = load().toMutableList()
        existing.remove(query)
        existing.add(0, query)
        val trimmed = existing.take(10)
        save(trimmed)
    }

    fun clear() {
        prefs.edit().remove("queries").apply()
    }
}
