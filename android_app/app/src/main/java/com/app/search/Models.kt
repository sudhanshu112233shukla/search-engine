package com.app.search

data class Answer(
    val text: String,
    val confidence: Float,
    val source: String
)

data class ResultItem(
    val id: String,
    val text: String,
    val score: Float
)

data class SearchResponse(
    val answer: Answer?,
    val answers: List<Answer>,
    val results: List<ResultItem>
)
