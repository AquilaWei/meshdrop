package org.meshdrop.desktop

import org.junit.Assert.assertEquals
import org.junit.Test

class DownloadNamesTest {
    @Test fun publicCollectionUsesDownloadRoot() { assertEquals("Download/", DownloadNames.relativePath) }
    @Test fun firstNameIsUnchanged() { assertEquals("photo.jpg", DownloadNames.candidate("photo.jpg", 0)) }
    @Test fun collisionPreservesExtension() { assertEquals("photo (1).jpg", DownloadNames.candidate("photo.jpg", 1)) }
    @Test fun repeatedCollisionIsDistinct() { assertEquals("photo (2).jpg", DownloadNames.candidate("photo.jpg", 2)) }
    @Test fun extensionlessCollisionIsDistinct() { assertEquals("document (1)", DownloadNames.candidate("document", 1)) }
    @Test fun traversalIsFlattened() { assertEquals("photo.jpg", DownloadNames.safe("../../photo.jpg")) }
    @Test fun controlCharactersAreRemoved() { assertEquals("hello_.txt", DownloadNames.safe("hello\n.txt")) }
    @Test fun dotNameHasSafeFallback() { assertEquals("document", DownloadNames.safe("..")) }
}
