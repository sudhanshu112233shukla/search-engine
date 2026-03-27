package com.app.search

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardActions
import androidx.compose.ui.text.input.KeyboardOptions
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SearchScreen(datasetPath: String, viewModel: SearchViewModel = viewModel(factory = SearchViewModelFactory(LocalAppContext.current))) {
    val state by viewModel.state.collectAsState()
    val focusRequester = remember { FocusRequester() }
    val focusManager = LocalFocusManager.current

    LaunchedEffect(datasetPath) {
        viewModel.initIfNeeded(datasetPath)
        focusRequester.requestFocus()
    }

    if (state.showSettings) {
        SettingsScreen(state, onBack = { viewModel.onBack() }, onClearHistory = { viewModel.clearHistory() }, onProfile = { viewModel.setProfile(it) })
        return
    }

    if (state.selected != null) {
        DetailScreen(item = state.selected, onBack = { viewModel.onBack() }, query = state.query)
        return
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
    ) {
        TopAppBar(
            title = { Text("Offline Search") },
            actions = {
                IconButton(onClick = { viewModel.openSettings() }) {
                    Text("?")
                }
            }
        )

        OutlinedTextField(
            value = state.query,
            onValueChange = { viewModel.onQueryChanged(it) },
            placeholder = { Text("Search anything...") },
            modifier = Modifier
                .fillMaxWidth()
                .focusRequester(focusRequester),
            singleLine = true,
            trailingIcon = {
                if (state.query.isNotBlank()) {
                    Text(
                        text = "Clear",
                        modifier = Modifier.clickable { viewModel.onQueryChanged("") }
                    )
                }
            },
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            keyboardActions = KeyboardActions(onSearch = {
                focusManager.clearFocus()
                viewModel.onQueryChanged(state.query)
            }),
            colors = TextFieldDefaults.outlinedTextFieldColors()
        )

        Spacer(modifier = Modifier.height(12.dp))

        if (state.engineLoading) {
            IndexingBanner("Building local index…", state.indexingProgress)
        } else if (!state.engineReady) {
            IndexingBanner("Search engine not ready", 0f)
        }

        if (state.query.isBlank()) {
            SuggestionList(state.history, onClick = { viewModel.onSuggestion(it) })
        }

        if (state.loading) {
            ShimmerList()
            Spacer(modifier = Modifier.height(12.dp))
        }

        AnimatedVisibility(visible = state.answer != null && !state.loading, enter = fadeIn(), exit = fadeOut()) {
            state.answer?.let { AnswerCard(it, state.answers) }
        }

        if (state.slowQuery) {
            Text("Searching...", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
        }

        if (!state.loading && state.results.isEmpty() && state.query.isNotBlank()) {
            EmptyState()
        }

        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(state.results) { item ->
                ResultRow(item, state.query) { viewModel.onResultClick(item) }
            }
        }
    }
}

@Composable
fun IndexingBanner(text: String, progress: Float) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer)
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Text(text, style = MaterialTheme.typography.bodySmall)
            if (progress > 0f) {
                Spacer(modifier = Modifier.height(6.dp))
                LinearProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth())
            }
        }
    }
    Spacer(modifier = Modifier.height(8.dp))
}

@Composable
fun AnswerCard(answer: Answer, answers: List<Answer>) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.primaryContainer)
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(answer.text, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.Bold)
            Spacer(modifier = Modifier.height(8.dp))
            Text("Confidence: ${"%.2f".format(answer.confidence)}")
            Text("Source: ${answer.source}")

            if (answers.size > 1) {
                Spacer(modifier = Modifier.height(8.dp))
                Text("Other possible answers", style = MaterialTheme.typography.bodySmall)
                answers.drop(1).forEach { a ->
                    Text("• ${a.text}", style = MaterialTheme.typography.bodySmall)
                }
            }
        }
    }
}

@Composable
fun ResultRow(item: ResultItem, query: String, onClick: () -> Unit) {
    Card(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() },
        shape = RoundedCornerShape(12.dp)
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Text(
                highlightText(item.text, query),
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 3,
                overflow = TextOverflow.Ellipsis
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text("Score: ${"%.3f".format(item.score)}", style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
fun highlightText(text: String, query: String) = buildAnnotatedString {
    val tokens = query.lowercase().split(" ").filter { it.isNotBlank() }
    val words = text.split(" ")
    for (w in words) {
        val clean = w.lowercase().replace(Regex("[^a-z0-9]"), "")
        if (tokens.contains(clean)) {
            withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(w) }
        } else {
            append(w)
        }
        append(" ")
    }
}

@Composable
fun SuggestionList(history: List<String>, onClick: (String) -> Unit) {
    if (history.isEmpty()) return
    Column {
        Text("Recent searches", style = MaterialTheme.typography.bodyMedium)
        Spacer(modifier = Modifier.height(8.dp))
        history.forEach { q ->
            Text(q, modifier = Modifier
                .fillMaxWidth()
                .clickable { onClick(q) }
                .padding(vertical = 6.dp))
        }
    }
}

@Composable
fun EmptyState() {
    Column(modifier = Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
        Text("No results found", style = MaterialTheme.typography.bodyMedium)
        Text("Try a different query", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
    }
}

@Composable
fun ShimmerList() {
    val transition = rememberInfiniteTransition(label = "shimmer")
    val alpha by transition.animateFloat(
        initialValue = 0.3f,
        targetValue = 0.8f,
        animationSpec = infiniteRepeatable(
            animation = tween(800),
            repeatMode = RepeatMode.Reverse
        ),
        label = "alpha"
    )

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        repeat(4) {
            Card(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(72.dp)
                    .alpha(alpha),
                shape = RoundedCornerShape(12.dp)
            ) {}
        }
    }
}

@Composable
fun DetailScreen(item: ResultItem?, onBack: () -> Unit, query: String) {
    if (item == null) return
    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Text("Back", modifier = Modifier.clickable { onBack() })
        Spacer(modifier = Modifier.height(12.dp))
        Text("Document: ${item.id}", style = MaterialTheme.typography.titleMedium)
        Spacer(modifier = Modifier.height(12.dp))
        Text(highlightText(item.text, query), style = MaterialTheme.typography.bodyMedium)
    }
}

@Composable
fun SettingsScreen(
    state: SearchUiState,
    onBack: () -> Unit,
    onClearHistory: () -> Unit,
    onProfile: (String) -> Unit
) {
    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Text("Back", modifier = Modifier.clickable { onBack() })
        Spacer(modifier = Modifier.height(12.dp))
        Text("Settings", style = MaterialTheme.typography.titleMedium)
        Spacer(modifier = Modifier.height(12.dp))
        Text("Clear search history", modifier = Modifier.clickable { onClearHistory() })
        Spacer(modifier = Modifier.height(16.dp))
        Text("Offline bundle", style = MaterialTheme.typography.titleSmall)
        if (state.bundleAvailable) {
            Text("Current: ${state.bundleProfile}")
            Spacer(modifier = Modifier.height(8.dp))
            RowItem("default (1GB)") { onProfile("default") }
            RowItem("power (5GB)") { onProfile("power") }
        } else {
            Text("No bundle manifest found in assets")
        }
        Spacer(modifier = Modifier.height(16.dp))
        Text("Index health", style = MaterialTheme.typography.titleSmall)
        Text("Dataset size: ${state.datasetSizeMb}")
        Text("Text store size: ${state.textStoreSizeMb}")
        Text("Last init: ${state.lastInit}")
    }
}

@Composable
fun RowItem(text: String, onClick: () -> Unit) {
    Text(text, modifier = Modifier
        .fillMaxWidth()
        .clickable { onClick() }
        .padding(vertical = 6.dp))
}
