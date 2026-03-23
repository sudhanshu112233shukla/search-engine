package com.app.search

import android.content.Context
import androidx.compose.runtime.staticCompositionLocalOf

val LocalAppContext = staticCompositionLocalOf<Context> {
    error("No context provided")
}
