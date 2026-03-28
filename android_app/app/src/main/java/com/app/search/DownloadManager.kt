package com.app.search

import android.content.Context
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.util.zip.ZipInputStream

object DownloadManager {
    fun downloadPack(
        context: Context,
        manifest: BundleManifest,
        language: String,
        profile: String,
        onProgress: (Float, String) -> Unit
    ): Boolean {
        val base = manifest.downloadBase ?: return false
        val shards = BundleManager.selectShards(manifest, language, profile)
        if (shards.isEmpty()) return false

        val totalBytes = shards.sumOf { it.bytes }.coerceAtLeast(1L)
        var downloaded = 0L
        val packRoot = File(context.filesDir, "packs_download/$language/$profile")
        packRoot.mkdirs()

        for (shard in shards) {
            val url = "${base.trimEnd('/')}/${language}/${shard.name}.zip"
            onProgress(downloaded.toFloat() / totalBytes, "Downloading ${shard.name}")
            val ok = downloadAndUnzip(url, File(packRoot, shard.name))
            if (!ok) return false
            downloaded += shard.bytes
            onProgress(downloaded.toFloat() / totalBytes, "Downloaded ${shard.name}")
        }
        onProgress(1f, "Download complete")
        return true
    }

    private fun downloadAndUnzip(urlStr: String, outDir: File): Boolean {
        return try {
            val url = URL(urlStr)
            val conn = url.openConnection() as HttpURLConnection
            conn.connectTimeout = 15000
            conn.readTimeout = 30000
            conn.requestMethod = "GET"
            conn.connect()
            if (conn.responseCode !in 200..299) {
                conn.disconnect()
                return false
            }
            outDir.mkdirs()
            val zipStream = ZipInputStream(conn.inputStream)
            var entry = zipStream.nextEntry
            while (entry != null) {
                val outFile = File(outDir, entry.name)
                if (entry.isDirectory) {
                    outFile.mkdirs()
                } else {
                    outFile.parentFile?.mkdirs()
                    outFile.outputStream().use { output ->
                        zipStream.copyTo(output)
                    }
                }
                zipStream.closeEntry()
                entry = zipStream.nextEntry
            }
            zipStream.close()
            conn.disconnect()
            true
        } catch (_: Exception) {
            false
        }
    }
}
