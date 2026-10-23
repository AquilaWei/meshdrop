package org.meshdrop.desktop

/** Flat, safe names; each collision gets a distinct name before creating a new row. */
object DownloadNames {
    const val relativePath = "Download/"
    fun safe(display: String): String = display.substringAfterLast('/').substringAfterLast('\\')
        .replace(Regex("[\\p{Cntrl}<>:\"|?*]"), "_").take(180)
        .takeIf { it.isNotBlank() && it != "." && it != ".." } ?: "document"

    fun candidate(display: String, index: Int): String {
        require(index >= 0)
        val name = safe(display)
        if (index == 0) return name
        val dot = name.lastIndexOf('.').takeIf { it > 0 } ?: name.length
        return name.substring(0, dot) + " ($index)" + name.substring(dot)
    }
}
