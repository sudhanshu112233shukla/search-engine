package com.app.search

import android.content.Context
import org.json.JSONObject


data class BundleProfile(val name: String, val maxBytes: Long)

data class ShardInfo(val name: String, val path: String, val docs: Int, val bytes: Long)

data class BundleManifest(
    val version: Int,
    val language: String,
    val profiles: List<BundleProfile>,
    val shards: List<ShardInfo>
)

object BundleManager {
    fun loadManifest(context: Context): BundleManifest? {
        return try {
            val json = context.assets.open("packs/manifest.json").bufferedReader().use { it.readText() }
            val obj = JSONObject(json)
            val profiles = obj.getJSONArray("profiles").let { arr ->
                (0 until arr.length()).map { i ->
                    val p = arr.getJSONObject(i)
                    BundleProfile(
                        name = p.optString("name"),
                        maxBytes = p.optLong("max_bytes")
                    )
                }
            }
            val shards = obj.getJSONArray("shards").let { arr ->
                (0 until arr.length()).map { i ->
                    val s = arr.getJSONObject(i)
                    ShardInfo(
                        name = s.optString("name"),
                        path = s.optString("path"),
                        docs = s.optInt("docs"),
                        bytes = s.optLong("bytes")
                    )
                }
            }
            BundleManifest(
                version = obj.optInt("version", 1),
                language = obj.optString("language", "en"),
                profiles = profiles,
                shards = shards
            )
        } catch (_: Exception) {
            null
        }
    }

    fun selectShards(manifest: BundleManifest, profileName: String): List<ShardInfo> {
        val profile = manifest.profiles.firstOrNull { it.name == profileName }
            ?: manifest.profiles.firstOrNull()
        val maxBytes = profile?.maxBytes ?: Long.MAX_VALUE
        val selected = mutableListOf<ShardInfo>()
        var total = 0L
        for (shard in manifest.shards) {
            if (total + shard.bytes > maxBytes) break
            selected.add(shard)
            total += shard.bytes
        }
        return selected
    }
}
