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

    fun prepareIndexPack(context: Context, language: String, profile: String): String? {
        val downloaded = File(context.filesDir, "packs_download/$language/$profile")
        if (downloaded.exists() && downloaded.listFiles()?.isNotEmpty() == true) {
            return downloaded.absolutePath
        }

        val manifest = BundleManager.loadManifest(context) ?: return null
        val shards = BundleManager.selectShards(manifest, language, profile)
        if (shards.isEmpty()) return null

        val packRoot = File(context.filesDir, "packs/$language/$profile")
        if (packRoot.exists() && packRoot.listFiles()?.isNotEmpty() == true) {
            return packRoot.absolutePath
        }
        packRoot.mkdirs()

        for (shard in shards) {
            val shardRoot = File(packRoot, shard.name)
            copyAssetDir(context, shard.path, shardRoot)
        }
        return packRoot.absolutePath
    }

    private fun copyAssetDir(context: Context, assetPath: String, outDir: File) {
        val files = context.assets.list(assetPath) ?: return
        if (files.isEmpty()) {
            outDir.parentFile?.mkdirs()
            context.assets.open(assetPath).use { input ->
                outDir.outputStream().use { output ->
                    input.copyTo(output)
                }
            }
            return
        }
        outDir.mkdirs()
        for (name in files) {
            val childAsset = if (assetPath.isEmpty()) name else "$assetPath/$name"
            val childFile = File(outDir, name)
            copyAssetDir(context, childAsset, childFile)
        }
    }
}
