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
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AssistChip
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ElevatedCard
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.IconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedCard
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
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.compose.ui.text.style.LineHeightStyle

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
        SettingsScreen(
            state,
            onBack = { viewModel.onBack() },
            onClearHistory = { viewModel.clearHistory() },
            onProfile = { viewModel.setProfile(it) },
            onLanguage = { viewModel.setLanguage(it) },
            onDownload = { viewModel.downloadSelectedPack() }
        )
        return
    }

    if (state.selected != null) {
        DetailScreen(item = state.selected, onBack = { viewModel.onBack() }, query = state.query)
        return
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 16.dp, vertical = 12.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            androidx.compose.foundation.Image(
                painter = painterResource(id = R.drawable.app_logo),
                contentDescription = "App logo",
                modifier = Modifier.size(42.dp),
                contentScale = ContentScale.Crop
            )
            Spacer(modifier = Modifier.size(12.dp))
            Column {
                Text(
                    "Offline Search",
                    style = MaterialTheme.typography.headlineSmall,
                    fontWeight = FontWeight.Bold
                )
                Text(
                    "Search the downloaded pack entirely on-device.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Color.Gray
                )
            }
        }
        Spacer(modifier = Modifier.height(12.dp))

        if (!state.packInstalled && state.bundleAvailable) {
            DemoBanner(
                title = "Full demo pack not installed",
                body = "The app can run from the built-in sample right now, but the full offline pack is available for download."
            )
            Spacer(modifier = Modifier.height(12.dp))
        } else if (state.packInstalled) {
            DemoBanner(
                title = "Offline pack ready",
                body = "The full pack is installed locally and the engine can search offline."
            )
            Spacer(modifier = Modifier.height(12.dp))
        }

        TopAppBar(
            title = {
                Column {
                    Text("Offline Search")
                    Text(
                        "Fast, offline wiki-style search",
                        style = MaterialTheme.typography.labelSmall,
                        color = Color.Gray
                    )
                }
            },
            actions = {
                IconButton(onClick = { viewModel.openSettings() }) {
                    Text("Settings")
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
        } else if (!state.packInstalled && state.bundleAvailable) {
            IndexingBanner("Sample mode active. Download the full pack for complete offline coverage.", 0f)
        } else {
            StatusRow(state)
        }

        if (state.query.isBlank()) {
            SuggestionList(state.history, onClick = { viewModel.onSuggestion(it) })
        }

        if (state.loading) {
            ShimmerList()
            Spacer(modifier = Modifier.height(12.dp))
        }

        val visibleAnswer = state.answer ?: state.results.firstOrNull()?.let {
            Answer(
                text = it.text,
                confidence = 0.30f,
                source = it.id
            )
        }

        AnimatedVisibility(visible = visibleAnswer != null && !state.loading, enter = fadeIn(), exit = fadeOut()) {
            visibleAnswer?.let { AnswerCard(it, state.answers) }
        }

        if (state.slowQuery) {
            Text("Searching...", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
        }

        if (!state.loading && state.results.isEmpty() && state.answer == null && state.query.isNotBlank()) {
            EmptyState()
        }

        if (state.results.isNotEmpty()) {
            Text(
                "Supporting results",
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.padding(top = 8.dp, bottom = 4.dp)
            )
            LazyColumn(verticalArrangement = Arrangement.spacedBy(10.dp), contentPadding = PaddingValues(bottom = 16.dp)) {
                items(state.results) { item ->
                    ResultRow(item, state.query) { viewModel.onResultClick(item) }
                }
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
            Text(text, style = MaterialTheme.typography.bodyMedium, fontWeight = FontWeight.SemiBold)
            if (progress > 0f) {
                Spacer(modifier = Modifier.height(6.dp))
                LinearProgressIndicator(progress = { progress }, modifier = Modifier.fillMaxWidth())
            }
        }
    }
    Spacer(modifier = Modifier.height(8.dp))
}

@Composable
fun DemoBanner(title: String, body: String) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.tertiaryContainer)
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Spacer(modifier = Modifier.height(4.dp))
            Text(body, style = MaterialTheme.typography.bodySmall, color = Color.Black.copy(alpha = 0.78f))
        }
    }
}

@Composable
fun AnswerCard(answer: Answer, answers: List<Answer>) {
    ElevatedCard(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(20.dp),
        colors = CardDefaults.elevatedCardColors(containerColor = MaterialTheme.colorScheme.primaryContainer)
    ) {
        Column(modifier = Modifier.padding(18.dp)) {
            Text(
                "Best answer",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
                fontWeight = FontWeight.SemiBold
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                answer.text.ifBlank { "No concise answer extracted." },
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                lineHeight = MaterialTheme.typography.titleMedium.lineHeight
            )
            Spacer(modifier = Modifier.height(8.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                AssistChip(onClick = {}, label = { Text("Confidence ${"%.2f".format(answer.confidence)}") })
                AssistChip(onClick = {}, label = { Text("Source") })
            }
            Spacer(modifier = Modifier.height(6.dp))
            Text("Source: ${answer.source}", style = MaterialTheme.typography.bodySmall, color = Color.Gray)

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
    OutlinedCard(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onClick() },
        shape = RoundedCornerShape(16.dp)
    ) {
        Column(modifier = Modifier.padding(14.dp)) {
            Text(
                highlightText(item.text, query),
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis
            )
            Spacer(modifier = Modifier.height(4.dp))
            Text(item.id, style = MaterialTheme.typography.bodySmall, color = Color.Gray)
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
        Text("Recent searches", style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.SemiBold)
        Spacer(modifier = Modifier.height(8.dp))
        history.forEach { q ->
            OutlinedCard(
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { onClick(q) },
                shape = RoundedCornerShape(12.dp)
            ) {
                Text(
                    q,
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp),
                    style = MaterialTheme.typography.bodyMedium
                )
            }
            Spacer(modifier = Modifier.height(6.dp))
        }
    }
}

@Composable
fun EmptyState() {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 20.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text("No results found", style = MaterialTheme.typography.bodyLarge, fontWeight = FontWeight.SemiBold)
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

    Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
        repeat(4) {
            OutlinedCard(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(80.dp)
                    .alpha(alpha),
                shape = RoundedCornerShape(16.dp)
            ) {}
        }
    }
}

@Composable
fun DetailScreen(item: ResultItem?, onBack: () -> Unit, query: String) {
    if (item == null) return
    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Text("Back", modifier = Modifier.clickable { onBack() }, color = MaterialTheme.colorScheme.primary)
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
    onProfile: (String) -> Unit,
    onLanguage: (String) -> Unit,
    onDownload: () -> Unit
) {
    Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
        Text("Back", modifier = Modifier.clickable { onBack() }, color = MaterialTheme.colorScheme.primary)
        Spacer(modifier = Modifier.height(12.dp))
        Text("Settings", style = MaterialTheme.typography.titleMedium)
        Spacer(modifier = Modifier.height(12.dp))
        Text("Clear search history", modifier = Modifier.clickable { onClearHistory() })
        Spacer(modifier = Modifier.height(16.dp))
        Text("Device activation", style = MaterialTheme.typography.titleSmall)
        Text("Device public key (share with pack builder):", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
        Text(state.devicePublicKey, style = MaterialTheme.typography.bodySmall)
        Spacer(modifier = Modifier.height(16.dp))
        Text("Offline bundle", style = MaterialTheme.typography.titleSmall)
        if (state.bundleAvailable) {
            Text("Language: ${state.bundleLanguage}")
            if (state.availableLanguages.isNotEmpty()) {
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    state.availableLanguages.forEach { lang ->
                        Text(lang, modifier = Modifier
                            .clickable { onLanguage(lang) }
                            .padding(vertical = 6.dp))
                    }
                }
            }
            Spacer(modifier = Modifier.height(8.dp))
            Text("Profile: ${state.bundleProfile}")
            RowItem("default (1GB)") { onProfile("default") }
            RowItem("power (full demo)") { onProfile("power") }
            Spacer(modifier = Modifier.height(8.dp))
            RowItem(if (state.downloading) "Downloading..." else "Download pack") { onDownload() }
            if (state.downloading) {
                Spacer(modifier = Modifier.height(6.dp))
                LinearProgressIndicator(progress = { state.downloadProgress }, modifier = Modifier.fillMaxWidth())
                Text(state.downloadMessage, style = MaterialTheme.typography.bodySmall)
            } else if (state.packInstalled) {
                Text("Pack installed locally", style = MaterialTheme.typography.bodySmall)
                Text("Restart after downloading to switch to the pack.", style = MaterialTheme.typography.bodySmall, color = Color.Gray)
            }
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
fun StatusRow(state: SearchUiState) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        AssistChip(onClick = {}, label = { Text(if (state.packInstalled) "Pack ready" else "Sample mode") })
        AssistChip(onClick = {}, label = { Text(if (state.engineReady) "Engine ready" else "Loading") })
    }
    Spacer(modifier = Modifier.height(8.dp))
}

@Composable
fun RowItem(text: String, onClick: () -> Unit) {
    Text(text, modifier = Modifier
        .fillMaxWidth()
        .clickable { onClick() }
        .padding(vertical = 6.dp))
}
