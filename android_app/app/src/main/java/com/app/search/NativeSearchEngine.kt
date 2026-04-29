package com.app.search

object NativeSearchEngine {
    init {
        System.loadLibrary("search_engine_rust")
    }

    external fun init(path: String): Boolean
    external fun initIndex(path: String): Boolean
    external fun update(path: String)
    external fun search(query: String): String
}
