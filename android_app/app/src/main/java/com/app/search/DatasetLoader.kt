package com.app.search

import android.content.Context
import java.io.File

object DatasetLoader {
    private const val DATASET_NAME = "dataset.json"

    fun prepareDataset(context: Context): String {
        val outFile = File(context.filesDir, DATASET_NAME)
        if (!outFile.exists()) {
            context.assets.open(DATASET_NAME).use { input ->
                outFile.outputStream().use { output ->
                    input.copyTo(output)
                }
            }
        }
        return outFile.absolutePath
    }
}
