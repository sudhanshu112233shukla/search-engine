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
    val bundleAvailable: Boolean = false
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
            val history = historyStore.load()
            val profile = bundlePrefs.getProfile()
            val bundleManifest = BundleManager.loadManifest(appContext)
            val bundleAvailable = bundleManifest != null
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
                    bundleAvailable = bundleAvailable
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
                        val packDir = DatasetLoader.prepareIndexPack(appContext, profile)
                        if (packDir != null) {
                            NativeSearchEngine.initIndex(packDir)
                        } else {
                            NativeSearchEngine.init(datasetPath)
                        }
                        true
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
                val json = NativeSearchEngine.search(query)
                SearchParser.parse(json)
            }
            val elapsed = System.currentTimeMillis() - start

            if (response.results.isNotEmpty()) {
                historyStore.add(query)
            }
            val history = historyStore.load()

            _state.update {
                it.copy(
                    loading = false,
                    answer = response.answer,
                    answers = response.answers,
                    results = response.results,
                    history = history,
                    error = if (response.results.isEmpty()) "No results found" else null,
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
