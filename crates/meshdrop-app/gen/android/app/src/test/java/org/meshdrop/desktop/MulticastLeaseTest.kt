package org.meshdrop.desktop

import java.util.concurrent.atomic.AtomicInteger
import org.junit.Assert.assertEquals
import org.junit.Test

class MulticastLeaseTest {
    @Test fun repeatedServiceStartAcquiresOnce() {
        val acquired = AtomicInteger()
        val lease = MulticastLease({ acquired.incrementAndGet(); Unit }, {})
        lease.start()
        lease.start()
        assertEquals(1, acquired.get())
    }
    @Test fun repeatedServiceStopReleasesOnce() {
        val released = AtomicInteger()
        val lease = MulticastLease({}, { released.incrementAndGet(); Unit })
        lease.start()
        lease.stop()
        lease.stop()
        assertEquals(1, released.get())
    }
    @Test fun stopWithoutStartDoesNotRelease() {
        val released = AtomicInteger()
        val lease = MulticastLease({}, { released.incrementAndGet(); Unit })
        lease.stop()
        assertEquals(0, released.get())
    }
}
