package org.meshdrop.desktop

/** Serial service callbacks acquire once and release once; acquisition failures propagate. */
class MulticastLease(private val acquire: () -> Unit, private val release: () -> Unit) {
    private var held = false
    fun start() { if (!held) { acquire(); held = true } }
    fun stop() { if (held) { release(); held = false } }
}
