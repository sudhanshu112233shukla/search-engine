package com.app.search

import android.content.Context
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

object SecureDownloadManager {
    fun downloadEncryptedPack(
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
        var doneBytes = 0L

        val packRoot = File(context.filesDir, "packs_download/$language/$profile")
        packRoot.mkdirs()

        for (shard in shards) {
            val packUrl = "${base.trimEnd('/')}/${language}/${shard.name}.spack"
            val sigUrl = "${base.trimEnd('/')}/${language}/${shard.name}.spack.sig"
            val outDir = File(packRoot, shard.name)
            val tmpOutDir = File(packRoot, "${shard.name}.tmp")
            val tmpFile = File(packRoot, "${shard.name}.spack.tmp")

            onProgress(doneBytes.toFloat() / totalBytes, "Downloading ${shard.name}")
            if (!downloadToFile(packUrl, tmpFile)) return false
            val sigB64 = downloadToString(sigUrl) ?: return false

            onProgress(doneBytes.toFloat() / totalBytes, "Verifying ${shard.name}")
            if (!SecurePackCrypto.verifyTrustedSignature(tmpFile, sigB64)) {
                tmpFile.delete()
                return false
            }

            onProgress(doneBytes.toFloat() / totalBytes, "Decrypting ${shard.name}")
            val ok = SecurePackCrypto.decryptAndUnzipToDir(context, tmpFile, tmpOutDir)
            tmpFile.delete()
            if (!ok) return false

            if (outDir.exists()) outDir.deleteRecursively()
            if (!tmpOutDir.renameTo(outDir)) {
                tmpOutDir.deleteRecursively()
                return false
            }

            doneBytes += shard.bytes
            onProgress(doneBytes.toFloat() / totalBytes, "Ready ${shard.name}")
        }

        onProgress(1f, "Download complete")
        return true
    }

    private fun downloadToFile(urlStr: String, outFile: File): Boolean {
        return try {
            val url = URL(urlStr)
            val conn = url.openConnection() as HttpURLConnection
            conn.connectTimeout = 15000
            conn.readTimeout = 60000
            conn.requestMethod = "GET"
            conn.connect()
            if (conn.responseCode !in 200..299) {
                conn.disconnect()
                return false
            }
            outFile.parentFile?.mkdirs()
            outFile.outputStream().use { output ->
                conn.inputStream.use { input ->
                    input.copyTo(output)
                }
            }
            conn.disconnect()
            true
        } catch (_: Exception) {
            false
        }
    }

    private fun downloadToString(urlStr: String): String? {
        return try {
            val url = URL(urlStr)
            val conn = url.openConnection() as HttpURLConnection
            conn.connectTimeout = 15000
            conn.readTimeout = 30000
            conn.requestMethod = "GET"
            conn.connect()
            if (conn.responseCode !in 200..299) {
                conn.disconnect()
                return null
            }
            val text = conn.inputStream.bufferedReader().use { it.readText() }
            conn.disconnect()
            text
        } catch (_: Exception) {
            null
        }
    }
}
