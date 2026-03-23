package com.app.search

import org.json.JSONArray
import org.json.JSONObject

object SearchParser {
    fun parse(json: String): SearchResponse {
        return try {
            val obj = JSONObject(json)
            val answerObj = obj.optJSONObject("answer")
            val answer = if (answerObj != null && answerObj.has("text")) {
                Answer(
                    text = answerObj.optString("text"),
                    confidence = answerObj.optDouble("confidence", 0.0).toFloat(),
                    source = answerObj.optString("source")
                )
            } else {
                null
            }

            val answersArr = obj.optJSONArray("answers") ?: JSONArray()
            val answers = mutableListOf<Answer>()
            for (i in 0 until answersArr.length()) {
                val a = answersArr.getJSONObject(i)
                answers.add(
                    Answer(
                        text = a.optString("text"),
                        confidence = a.optDouble("confidence", 0.0).toFloat(),
                        source = a.optString("source")
                    )
                )
            }

            val resultsArr = obj.optJSONArray("results") ?: JSONArray()
            val results = mutableListOf<ResultItem>()
            for (i in 0 until resultsArr.length()) {
                val r = resultsArr.getJSONObject(i)
                results.add(
                    ResultItem(
                        id = r.optString("id"),
                        text = r.optString("text"),
                        score = r.optDouble("score", 0.0).toFloat()
                    )
                )
            }

            SearchResponse(answer = answer, answers = answers, results = results)
        } catch (_: Exception) {
            SearchResponse(answer = null, answers = emptyList(), results = emptyList())
        }
    }
}
