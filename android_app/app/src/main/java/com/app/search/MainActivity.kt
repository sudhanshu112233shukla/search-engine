package com.app.search

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.CompositionLocalProvider

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val datasetPath = DatasetLoader.prepareDataset(this)

        setContent {
            CompositionLocalProvider(LocalAppContext provides this) {
                SearchScreen(datasetPath = datasetPath)
            }
        }
    }
}
