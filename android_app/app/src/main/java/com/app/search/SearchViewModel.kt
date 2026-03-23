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
    val answer: Answer? = null,
    val answers: List<Answer> = emptyList(),
    val results: List<ResultItem> = emptyList(),
    val history: List<String> = emptyList(),
    val selected: ResultItem? = null,
    val showSettings: Boolean = false,
    val error: String? = null,
    val slowQuery: Boolean = false
)

class SearchViewModel(private val appContext: Context) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state

    private val initialized = AtomicBoolean(false)
    private var searchJob: Job? = null
    private var lastQuery: String = ""
    private val historyStore = SearchHistoryStore(appContext)

    fun initIfNeeded(datasetPath: String) {
        if (initialized.compareAndSet(false, true)) {
            NativeSearchEngine.init(datasetPath)
            val history = historyStore.load()
            _state.update { it.copy(history = history) }
        }
    }

    fun onQueryChanged(query: String) {
        _state.update { it.copy(query = query, error = null, slowQuery = false) }

        searchJob?.cancel()
        if (query.isBlank()) {
            _state.update { it.copy(loading = false, answer = null, answers = emptyList(), results = emptyList()) }
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
}

class SearchViewModelFactory(private val context: Context) : ViewModelProvider.Factory {
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        return SearchViewModel(context.applicationContext) as T
    }
}
