package com.app.search

object NativeSearchEngine {
    init {
        System.loadLibrary("search_engine_rust")
    }

    external fun init(path: String)
    external fun update(path: String)
    external fun search(query: String): String
}
