package org.meshdrop.desktop

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.os.Build
import android.provider.MediaStore
import java.io.File

/** Only verified receive files enter here. A new pending row never opens an old file. */
class Downloads(private val context: Context) {
    private val saved = context.getSharedPreferences("downloads", Context.MODE_PRIVATE)
    fun uri(name: String): Uri? = saved.getString(name, null)?.let(Uri::parse)

    fun publish(source: File): String {
        if (Build.VERSION.SDK_INT < 29) return source.absolutePath
        val resolver = context.contentResolver
        val collection = MediaStore.Downloads.EXTERNAL_CONTENT_URI
        var index = 0
        var name: String
        do {
            check(index < 10000) { "Download filename collision limit exhausted" }
            name = DownloadNames.candidate(source.name, index++)
            val exists = resolver.query(collection, arrayOf(MediaStore.Downloads._ID),
                "${MediaStore.Downloads.DISPLAY_NAME} = ? AND ${MediaStore.Downloads.RELATIVE_PATH} = ?",
                arrayOf(name, DownloadNames.relativePath), null)?.use { it.moveToFirst() } ?: false
        } while (exists)
        val values = ContentValues().apply {
            put(MediaStore.Downloads.DISPLAY_NAME, name)
            put(MediaStore.Downloads.MIME_TYPE, "application/octet-stream")
            put(MediaStore.Downloads.RELATIVE_PATH, DownloadNames.relativePath)
            put(MediaStore.Downloads.IS_PENDING, 1)
        }
        val destination = requireNotNull(resolver.insert(collection, values)) { "Cannot create Download entry" }
        try {
            source.inputStream().use { input ->
                requireNotNull(resolver.openOutputStream(destination, "w")) { "Cannot write Download entry" }.use { output ->
                    input.copyTo(output, 65536)
                }
            }
            check(resolver.update(destination, ContentValues().apply { put(MediaStore.Downloads.IS_PENDING, 0) }, null, null) == 1) { "Cannot publish Download entry" }
            // MediaProvider may choose another unique name for an inaccessible collision.
            val actual = resolver.query(destination, arrayOf(MediaStore.Downloads.DISPLAY_NAME), null, null, null)?.use {
                check(it.moveToFirst()) { "Published entry disappeared" }; it.getString(0)
            } ?: error("Cannot read published name")
            check(saved.edit().putString(source.name, destination.toString()).commit()) { "Cannot record Download entry" }
            return "Download/$actual ($destination)"
        } catch (error: Exception) {
            try { resolver.delete(destination, null, null) } catch (cleanup: Exception) { error.addSuppressed(cleanup) }
            throw error
        }
    }
}
