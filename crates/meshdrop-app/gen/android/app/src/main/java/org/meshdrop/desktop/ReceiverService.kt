package org.meshdrop.desktop

import android.app.*
import android.content.*
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.*
import androidx.core.app.NotificationCompat

/** Keeps the existing Rust receiver process important; never accepts invitations. */
class ReceiverService : Service() {
    private var multicast: WifiManager.MulticastLock? = null
    private val lease = MulticastLease({
        val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        multicast = wifi.createMulticastLock("meshdrop-discovery").apply { setReferenceCounted(false); acquire() }
    }, {
        multicast?.let { if (it.isHeld) it.release() }; multicast = null
    })
    override fun onBind(intent: Intent?) = null
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == "STOP") { stopSelf(); return START_NOT_STICKY }
        try {
            val manager = getSystemService(NotificationManager::class.java)
            if (Build.VERSION.SDK_INT >= 26) manager.createNotificationChannel(NotificationChannel("receiver", "Background reception", NotificationManager.IMPORTANCE_LOW))
            val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            val stop = PendingIntent.getService(this, 1, Intent(this, ReceiverService::class.java).setAction("STOP"), PendingIntent.FLAG_IMMUTABLE)
            val notification = NotificationCompat.Builder(this, "receiver")
                .setSmallIcon(android.R.drawable.stat_sys_download).setContentTitle("meshdrop receiving")
                .setContentText("Invitations require your acceptance in the app")
                .setContentIntent(open).setOngoing(true).addAction(0, "Stop receiving", stop).build()
            if (Build.VERSION.SDK_INT >= 29) startForeground(41, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
            else startForeground(41, notification)
            lease.start()
        } catch (error: Exception) {
            sendBroadcast(Intent("$packageName.RECEIVER_STOPPED").setPackage(packageName).putExtra("error", error.toString()))
            stopSelf()
        }
        return START_NOT_STICKY
    }
    // Android 15 caps dataSync foreground services; stop cleanly rather than restart-looping.
    override fun onTimeout(startId: Int, fgsType: Int) { stopSelf() }
    override fun onDestroy() {
        lease.stop()
        stopForeground(STOP_FOREGROUND_REMOVE)
        sendBroadcast(Intent("$packageName.RECEIVER_STOPPED").setPackage(packageName))
        super.onDestroy()
    }
}
