package com.app.search

import android.content.Context
import android.content.SharedPreferences

class BundlePrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("bundle_prefs", Context.MODE_PRIVATE)

    fun getProfile(): String = prefs.getString("profile", "power") ?: "power"

    fun setProfile(profile: String) {
        prefs.edit().putString("profile", profile).apply()
    }

    fun getLanguage(): String = prefs.getString("language", "en") ?: "en"

    fun setLanguage(language: String) {
        prefs.edit().putString("language", language).apply()
    }
}
