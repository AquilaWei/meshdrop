package org.meshdrop.desktop

import java.io.File
import java.nio.file.Files
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ReceivedFilesTest {
    @get:Rule val temporary = TemporaryFolder()
    @Test fun completedFileCanBeShared() {
        val root = temporary.newFolder("received")
        val file = File(root, "document.txt").apply { writeText("verified content") }
        assertEquals(file.canonicalFile, ReceivedFiles.completed(root, "document.txt"))
    }
    @Test(expected = IllegalArgumentException::class) fun traversalCannotSharePrivateIdentity() {
        val root = temporary.newFolder("received")
        File(temporary.root, "identity.json").writeText("fixture")
        ReceivedFiles.completed(root, "../identity.json")
    }
    @Test(expected = IllegalArgumentException::class) fun temporarySinkCannotBeShared() {
        val root = temporary.newFolder("received")
        File(root, ".meshdrop-fixture.part").writeText("unfinished")
        ReceivedFiles.completed(root, ".meshdrop-fixture.part")
    }
    @Test fun ordinaryPartExtensionCanBeShared() {
        val root = temporary.newFolder("received")
        val file = File(root, "document.part").apply { writeText("complete") }
        assertEquals(file.canonicalFile, ReceivedFiles.completed(root, "document.part"))
    }
    @Test(expected = IllegalArgumentException::class) fun symlinkCannotShareOutsideReceiveDirectory() {
        val root = temporary.newFolder("received")
        val outside = File(temporary.root, "outside.txt").apply { writeText("fixture") }
        Files.createSymbolicLink(File(root, "link.txt").toPath(), outside.toPath())
        ReceivedFiles.completed(root, "link.txt")
    }
}
