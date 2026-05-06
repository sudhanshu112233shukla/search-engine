package com.app.search

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean


data class SearchUiState(
    val query: String = "",
    val loading: Boolean = false,
    val engineLoading: Boolean = false,
    val engineReady: Boolean = false,
    val indexingProgress: Float = 0f,
    val answer: Answer? = null,
    val answers: List<Answer> = emptyList(),
    val results: List<ResultItem> = emptyList(),
    val history: List<String> = emptyList(),
    val selected: ResultItem? = null,
    val showSettings: Boolean = false,
    val error: String? = null,
    val slowQuery: Boolean = false,
    val datasetSizeMb: String = "",
    val textStoreSizeMb: String = "",
    val lastInit: String = "",
    val bundleProfile: String = "default",
    val bundleLanguage: String = "en",
    val bundleAvailable: Boolean = false,
    val packInstalled: Boolean = false,
    val availableLanguages: List<String> = emptyList(),
    val downloading: Boolean = false,
    val downloadProgress: Float = 0f,
    val downloadMessage: String = "",
    val devicePublicKey: String = "",
    val demoMode: Boolean = false,
    val showSupporting: Boolean = false,
    val selfTestPassed: Boolean = false,
    val selfTestMessage: String = ""
)

class SearchViewModel(private val appContext: Context) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state

    private val initialized = AtomicBoolean(false)
    private var searchJob: Job? = null
    private var progressJob: Job? = null
    private var lastQuery: String = ""
    private val historyStore = SearchHistoryStore(appContext)
    private val bundlePrefs = BundlePrefs(appContext)
    private val noisyFallbackPhrases = listOf(
        "bm25 is a ranking function",
        "stopwords are common words",
        "exact match and phrase match"
    )

    private fun hasUsablePack(language: String, profile: String): Boolean {
        val root = File(appContext.filesDir, "packs_download/$language/$profile")
        if (!root.exists()) return false
        val shardDirs = root.listFiles()?.filter { it.isDirectory && it.name.startsWith("shard_") } ?: emptyList()
        if (shardDirs.isEmpty()) return false
        return shardDirs.any { shard ->
            val names = shard.listFiles()?.map { it.name } ?: emptyList()
            names.contains("meta.bin") && names.contains("chunks.bin") && names.contains("textstore.bin")
        }
    }

    private fun isPackSource(source: String?): Boolean {
        val s = source?.trim().orEmpty()
        if (s.isBlank()) return false
        if (s.matches(Regex("^\\d+$"))) return false
        return s.contains(":")
    }

    private fun cleanAnswerText(text: String?): String? {
        val normalized = text
            ?.trim()
            ?.replace(Regex("\\s+"), " ")
            ?.replace(Regex("\\[[^\\]]*]"), "")
            ?.replace(Regex("\\(\\s*\\)"), "")
            ?.trim()
        val firstSentence = normalized
            ?.split(Regex("(?<=[.!?])\\s+"))
            ?.firstOrNull()
            ?.trim()
        return (firstSentence ?: normalized)?.takeIf { it.isNotBlank() }?.take(220)
    }

    private fun queryLooksValid(query: String, response: SearchResponse): Boolean {
        val answerText = response.answer?.text.orEmpty().lowercase()
        val topText = response.results.firstOrNull()?.text.orEmpty().lowercase()
        val source = response.answer?.source ?: response.results.firstOrNull()?.id.orEmpty()
        if (!isPackSource(source)) return false
        return when (query.lowercase()) {
            "what is earth" -> answerText.contains("earth") || topText.contains("earth")
            "what is google" -> answerText.contains("google") || topText.contains("google")
            "what is web browser" -> answerText.contains("browser") || topText.contains("browser")
            else -> response.results.isNotEmpty()
        }
    }

    private fun runPackSelfTest(): Pair<Boolean, String> {
        val checks = listOf("what is earth", "what is google", "what is web browser")
        for (query in checks) {
            val parsed = runCatching {
                SearchParser.parse(NativeSearchEngine.search(query))
            }.getOrNull() ?: return false to "Self-test failed: query error ($query)"
            if (!queryLooksValid(query, parsed)) {
                return false to "Self-test failed: weak result ($query)"
            }
        }
        return true to "Self-test passed"
    }

    fun initIfNeeded(datasetPath: String) {
        if (initialized.compareAndSet(false, true)) {
            SecurePackCrypto.ensureDeviceWrapKeyPair()
            val deviceKey = SecurePackCrypto.devicePublicKeyBase64()
            val history = historyStore.load()
            val profile = bundlePrefs.getProfile()
            val language = bundlePrefs.getLanguage()
            val demoMode = false
            val bundleManifest = BundleManager.loadManifest(appContext)
            val bundleAvailable = bundleManifest != null
            val langs = bundleManifest?.let { BundleManager.languages(it) } ?: emptyList()
            val packInstalled = hasUsablePack(language, profile)
            val datasetSize = FileSize.formatMb(datasetPath)
            val textStorePath = datasetPath.replace(Regex("\\.[^.]+"), ".textstore")
            val textStoreSize = FileSize.formatMb(textStorePath)
            _state.update {
                it.copy(
                    history = history,
                    engineLoading = true,
                    engineReady = false,
                    indexingProgress = 0.05f,
                    datasetSizeMb = datasetSize,
                    textStoreSizeMb = textStoreSize,
                    lastInit = TimeFormat.now(),
                    bundleProfile = profile,
                    bundleLanguage = language,
                    bundleAvailable = bundleAvailable,
                    packInstalled = packInstalled,
                    availableLanguages = langs,
                    devicePublicKey = deviceKey,
                    demoMode = demoMode
                )
            }

            progressJob = viewModelScope.launch {
                while (true) {
                    delay(200)
                    _state.update { state ->
                        val next = (state.indexingProgress + 0.02f).coerceAtMost(0.9f)
                        state.copy(indexingProgress = next)
                    }
                }
            }

            viewModelScope.launch {
                val initOk = withContext(Dispatchers.IO) {
                    runCatching {
                        val packDir = DatasetLoader.prepareIndexPack(appContext, language, profile)
                        if (packDir != null) NativeSearchEngine.initIndex(packDir) else false
                    }.getOrDefault(false)
                }
                val selfTest = if (initOk) {
                    withContext(Dispatchers.Default) { runPackSelfTest() }
                } else {
                    false to "Pack index not ready"
                }
                val ready = initOk && selfTest.first
                progressJob?.cancel()
                _state.update {
                    it.copy(
                        engineLoading = false,
                        engineReady = ready,
                        indexingProgress = if (ready) 1f else it.indexingProgress,
                        selfTestPassed = selfTest.first,
                        selfTestMessage = selfTest.second,
                        error = if (!ready) "Pack not usable yet. ${selfTest.second}" else null
                    )
                }
            }
        }
    }

    fun onQueryChanged(query: String) {
        _state.update { it.copy(query = query, error = null, slowQuery = false) }

        searchJob?.cancel()
        if (query.isBlank()) {
            _state.update { it.copy(loading = false, answer = null, answers = emptyList(), results = emptyList()) }
            return
        }
        if (query.trim().length < 2) {
            _state.update { it.copy(loading = false, error = "Type at least 2 characters") }
            return
        }
        if (!_state.value.packInstalled) {
            _state.update { it.copy(loading = false, error = "No pack installed. Open Settings and download default pack.") }
            return
        }
        if (!_state.value.engineReady || !_state.value.selfTestPassed) {
            _state.update { it.copy(loading = false, error = "Indexing data... please wait") }
            return
        }

        searchJob = viewModelScope.launch {
            delay(300)
            if (query == lastQuery) return@launch
            lastQuery = query

            _state.update { it.copy(loading = true, showSupporting = false) }
            val start = System.currentTimeMillis()
            val response = withContext(Dispatchers.Default) {
                withTimeoutOrNull(8000L) {
                    val json = NativeSearchEngine.search(query)
                    SearchParser.parse(json)
                }
            }
            val elapsed = System.currentTimeMillis() - start

            if (response == null) {
                _state.update {
                    it.copy(
                        loading = false,
                        answer = null,
                        answers = emptyList(),
                        results = emptyList(),
                        error = "Search timed out. Try a shorter query."
                    )
                }
                return@launch
            }

            val filteredResults = response.results.filter { result ->
                isPackSource(result.id) && noisyFallbackPhrases.none { phrase ->
                    result.text.lowercase().contains(phrase)
                }
            }
            val filteredAnswers = response.answers.filter { answer ->
                isPackSource(answer.source) && noisyFallbackPhrases.none { phrase ->
                    answer.text.lowercase().contains(phrase)
                }
            }
            val primaryAnswer = response.answer?.takeIf { answer ->
                isPackSource(answer.source) && noisyFallbackPhrases.none { phrase ->
                    answer.text.lowercase().contains(phrase)
                }
            }

            if (filteredResults.isNotEmpty()) {
                historyStore.add(query)
            }
            val history = historyStore.load()
            val bestResult = filteredResults.firstOrNull()
            val fallbackText = cleanAnswerText(primaryAnswer?.text)
                ?: cleanAnswerText(bestResult?.text)

            _state.update {
                val fallbackAnswer = fallbackText?.let {
                    Answer(
                        text = it,
                        confidence = primaryAnswer?.confidence ?: 0.35f,
                        source = primaryAnswer?.source ?: bestResult?.id.orEmpty()
                    )
                }
                it.copy(
                    loading = false,
                    answer = fallbackAnswer,
                    answers = filteredAnswers,
                    results = filteredResults,
                    history = history,
                    error = if (filteredResults.isEmpty() && fallbackAnswer == null) "No relevant result found in installed pack" else null,
                    slowQuery = elapsed > 200
                )
            }
        }
    }

    fun setDemoMode(enabled: Boolean) {
        _state.update { it.copy(demoMode = enabled, showSupporting = false) }
    }

    fun setShowSupporting(enabled: Boolean) {
        _state.update { it.copy(showSupporting = enabled) }
    }

    fun onSuggestion(query: String) {
        onQueryChanged(query)
    }

    fun onResultClick(item: ResultItem) {
        _state.update { it.copy(selected = item) }
    }

    fun onBack() {
        _state.update { it.copy(selected = null, showSettings = false) }
    }

    fun openSettings() {
        _state.update { it.copy(showSettings = true) }
    }

    fun clearHistory() {
        historyStore.clear()
        _state.update { it.copy(history = emptyList()) }
    }

    fun setProfile(profile: String) {
        bundlePrefs.setProfile(profile)
        _state.update { it.copy(bundleProfile = profile) }
    }

    fun setLanguage(language: String) {
        bundlePrefs.setLanguage(language)
        _state.update { it.copy(bundleLanguage = language) }
    }

    fun downloadSelectedPack() {
        val manifest = BundleManager.loadManifest(appContext) ?: return
        val language = _state.value.bundleLanguage
        val profile = _state.value.bundleProfile
        if (manifest.downloadBase.isNullOrBlank()) {
            _state.update { it.copy(error = "No download base configured") }
            return
        }
        viewModelScope.launch(Dispatchers.IO) {
            _state.update { it.copy(downloading = true, downloadProgress = 0f, downloadMessage = "Starting download") }
            val ok = if (manifest.version >= 2) {
                SecureDownloadManager.downloadEncryptedPack(appContext, manifest, language, profile) { progress, msg -> 
                    _state.update { it.copy(downloadProgress = progress, downloadMessage = msg) }
                }
            } else {
                DownloadManager.downloadPack(appContext, manifest, language, profile) { progress, msg ->
                    _state.update { it.copy(downloadProgress = progress, downloadMessage = msg) }
                }
            }
            _state.update {
                it.copy(
                    downloading = false,
                    downloadProgress = if (ok) 1f else it.downloadProgress,
                    downloadMessage = if (ok) "Download complete. Restart app to use the pack." else "Download failed",
                    packInstalled = if (ok) hasUsablePack(language, profile) else it.packInstalled,
                    error = if (ok) null else "Pack download failed"
                )
            }
        }
    }
}

class SearchViewModelFactory(private val context: Context) : ViewModelProvider.Factory {
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        return SearchViewModel(context.applicationContext) as T
    }
}

object FileSize {
    fun formatMb(path: String): String {
        return runCatching {
            val file = java.io.File(path)
            if (!file.exists()) "0 MB" else String.format("%.2f MB", file.length() / (1024.0 * 1024.0))
        }.getOrDefault("0 MB")
    }
}

object TimeFormat {
    fun now(): String {
        val ms = System.currentTimeMillis()
        val seconds = ms / 1000
        return java.text.SimpleDateFormat("yyyy-MM-dd HH:mm:ss").format(java.util.Date(seconds * 1000))
    }
}
