package com.app.search

import android.content.Context
import org.json.JSONObject


data class BundleProfile(val name: String, val maxBytes: Long)

data class ShardInfo(val name: String, val path: String, val docs: Int, val bytes: Long)

data class LanguagePack(val code: String, val profiles: List<BundleProfile>, val shards: List<ShardInfo>)

data class BundleManifest(
    val version: Int,
    val languages: List<LanguagePack>
)

object BundleManager {
    fun loadManifest(context: Context): BundleManifest? {
        return try {
            val json = context.assets.open("packs/manifest.json").bufferedReader().use { it.readText() }
            val obj = JSONObject(json)
            val langs = obj.optJSONArray("languages")
            val languages = mutableListOf<LanguagePack>()
            if (langs != null) {
                for (i in 0 until langs.length()) {
                    val l = langs.getJSONObject(i)
                    languages.add(parseLanguage(l))
                }
            } else {
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
                languages.add(LanguagePack(code = "en", profiles = profiles, shards = shards))
            }
            BundleManifest(
                version = obj.optInt("version", 1),
                languages = languages
            )
        } catch (_: Exception) {
            null
        }
    }

    private fun parseLanguage(obj: JSONObject): LanguagePack {
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
        return LanguagePack(
            code = obj.optString("code", "en"),
            profiles = profiles,
            shards = shards
        )
    }

    fun languages(manifest: BundleManifest): List<String> {
        return manifest.languages.map { it.code }
    }

    fun selectShards(manifest: BundleManifest, language: String, profileName: String): List<ShardInfo> {
        val lang = manifest.languages.firstOrNull { it.code == language } ?: manifest.languages.firstOrNull()
        if (lang == null) return emptyList()
        val profile = lang.profiles.firstOrNull { it.name == profileName } ?: lang.profiles.firstOrNull()
        val maxBytes = profile?.maxBytes ?: Long.MAX_VALUE
        val selected = mutableListOf<ShardInfo>()
        var total = 0L
        for (shard in lang.shards) {
            if (total + shard.bytes > maxBytes) break
            selected.add(shard)
            total += shard.bytes
        }
        return selected
    }
}
