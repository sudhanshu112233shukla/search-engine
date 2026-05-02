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
    val devicePublicKey: String = ""
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

    fun initIfNeeded(datasetPath: String) {
        if (initialized.compareAndSet(false, true)) {
            SecurePackCrypto.ensureDeviceWrapKeyPair()
            val deviceKey = SecurePackCrypto.devicePublicKeyBase64()
            val history = historyStore.load()
            val profile = bundlePrefs.getProfile()
            val language = bundlePrefs.getLanguage()
            val bundleManifest = BundleManager.loadManifest(appContext)
            val bundleAvailable = bundleManifest != null
            val langs = bundleManifest?.let { BundleManager.languages(it) } ?: emptyList()
            val packInstalled = File(appContext.filesDir, "packs_download/$language/$profile").exists()
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
                    devicePublicKey = deviceKey
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
                val ok = withContext(Dispatchers.IO) {
                    runCatching {
                        val packDir = DatasetLoader.prepareIndexPack(appContext, language, profile)
                        if (packDir != null) {
                            NativeSearchEngine.initIndex(packDir)
                        } else {
                            NativeSearchEngine.init(datasetPath)
                        }
                    }.getOrDefault(false)
                }
                progressJob?.cancel()
                _state.update {
                    it.copy(
                        engineLoading = false,
                        engineReady = ok,
                        indexingProgress = if (ok) 1f else it.indexingProgress,
                        error = if (!ok) "Failed to initialize search engine" else null
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
        if (!_state.value.engineReady) {
            _state.update { it.copy(loading = false, error = "Indexing data... please wait") }
            return
        }

        searchJob = viewModelScope.launch {
            delay(300)
            if (query == lastQuery) return@launch
            lastQuery = query

            _state.update { it.copy(loading = true) }
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

            if (response.results.isNotEmpty()) {
                historyStore.add(query)
            }
            val history = historyStore.load()
            val bestResult = response.results.firstOrNull()
            val fallbackText = response.answer?.text
                ?.trim()
                ?.takeIf { it.isNotEmpty() }
                ?: bestResult?.text
                    ?.trim()
                    ?.replace(Regex("\\s+"), " ")
                    ?.take(220)

            _state.update {
                val fallbackAnswer = fallbackText?.let {
                    Answer(
                        text = it,
                        confidence = response.answer?.confidence ?: 0.35f,
                        source = response.answer?.source ?: bestResult?.id.orEmpty()
                    )
                }
                it.copy(
                    loading = false,
                    answer = fallbackAnswer,
                    answers = response.answers,
                    results = response.results,
                    history = history,
                    error = if (response.results.isEmpty() && fallbackAnswer == null) "No results found" else null,
                    slowQuery = elapsed > 200
                )
            }
        }
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
                    packInstalled = if (ok) true else it.packInstalled
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
