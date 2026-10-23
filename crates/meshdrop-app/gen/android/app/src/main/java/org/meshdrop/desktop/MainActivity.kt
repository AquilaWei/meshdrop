package org.meshdrop.desktop

import android.os.Bundle
import android.content.Intent
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private val shares = ArrayDeque<Intent>()
  @Synchronized private fun capture(intent: Intent?) {
    if (intent?.action == Intent.ACTION_SEND || intent?.action == Intent.ACTION_SEND_MULTIPLE) {
      shares.addLast(Intent(intent))
    }
  }
  @Synchronized fun takeShare(): Intent? = if (shares.isEmpty()) null else shares.removeFirst()
  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    capture(intent)
  }
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    capture(intent)
  }
}
