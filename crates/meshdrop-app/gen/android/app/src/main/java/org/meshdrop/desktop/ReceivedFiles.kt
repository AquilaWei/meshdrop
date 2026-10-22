package org.meshdrop.desktop

import java.io.File

/** Sharing is restricted to complete files in the one private receive directory. */
object ReceivedFiles {
    fun completed(root: File, name: String): File {
        val parent = root.canonicalFile
        val file = File(parent, name).canonicalFile
        require(file.parentFile == parent && file.isFile && !partial(file.name)) { "Only completed receive files can be shared" }
        return file
    }
    fun partial(name: String) = name.startsWith(".meshdrop-") && name.endsWith(".part")
}
