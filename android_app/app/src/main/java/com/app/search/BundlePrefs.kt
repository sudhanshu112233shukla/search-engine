package com.app.search

import android.content.Context
import android.content.SharedPreferences

class BundlePrefs(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("bundle_prefs", Context.MODE_PRIVATE)

    fun getProfile(): String = prefs.getString("profile", "default") ?: "default"

    fun setProfile(profile: String) {
        prefs.edit().putString("profile", profile).apply()
    }
}
