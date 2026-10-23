package org.meshdrop.desktop

import android.app.Activity
import android.content.*
import android.os.Build
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import app.tauri.annotation.*
import app.tauri.plugin.*
import java.io.File
import java.util.UUID

@InvokeArg
class WatchArgs { lateinit var channel: Channel }
@InvokeArg
class FileArgs { lateinit var name: String }

/** Android-only process lifetime and content URI bridge; transport stays in Rust. */
@TauriPlugin
class ReceiverPlugin(private val activity: Activity) : Plugin(activity) {
    private var receiver: BroadcastReceiver? = null
    @Command
    fun start(invoke: Invoke) {
        val args = invoke.parseArgs(WatchArgs::class.java)
        if (receiver == null) {
            receiver = object : BroadcastReceiver() {
                override fun onReceive(context: Context, intent: Intent) {
                    args.channel.send(JSObject().put("event", "stopped").put("error", intent.getStringExtra("error")))
                }
            }
            ContextCompat.registerReceiver(activity, receiver, IntentFilter("${activity.packageName}.RECEIVER_STOPPED"), ContextCompat.RECEIVER_NOT_EXPORTED)
        }
        try {
            ContextCompat.startForegroundService(activity, Intent(activity, ReceiverService::class.java))
            invoke.resolve()
        } catch (error: Exception) { invoke.reject("Cannot start background reception: $error") }
    }
    @Command
    fun stop(invoke: Invoke) { activity.stopService(Intent(activity, ReceiverService::class.java)); invoke.resolve() }
    private fun received(): File = File(activity.filesDir, "meshdrop/received")
    @Command
    fun files(invoke: Invoke) {
        val names = received().listFiles()?.filter { it.isFile && !ReceivedFiles.partial(it.name) }?.map { it.name } ?: emptyList()
        invoke.resolve(JSObject().put("files", JSArray(names)))
    }
    @Command
    fun share(invoke: Invoke) {
        try {
            val name = invoke.parseArgs(FileArgs::class.java).name
            val file = ReceivedFiles.completed(received(), name)
            val uri = Downloads(activity).uri(name) ?: FileProvider.getUriForFile(activity, "${activity.packageName}.fileprovider", file)
            val send = Intent(Intent.ACTION_SEND).setType("application/octet-stream")
                .putExtra(Intent.EXTRA_STREAM, uri).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            send.clipData = ClipData.newRawUri(file.name, uri)
            activity.startActivity(Intent.createChooser(send, "Share received file"))
            invoke.resolve()
        } catch (error: Exception) { invoke.reject("Cannot share received file: $error") }
    }
    @Command
    fun publish(invoke: Invoke) {
        Thread {
            try {
                val source = ReceivedFiles.completed(received(), invoke.parseArgs(FileArgs::class.java).name)
                invoke.resolve(JSObject().put("location", Downloads(activity).publish(source)))
            } catch (error: Exception) { invoke.reject("Cannot publish verified receive: $error") }
        }.start()
    }
    @Command
    fun shared(invoke: Invoke) {
        val intent = (activity as MainActivity).takeShare()
        if (intent == null) { invoke.resolve(JSObject().put("paths", JSArray())); return }
        importDocuments(invoke, intent, true)
    }
    @Command
    fun pick(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).setType("*/*").addCategory(Intent.CATEGORY_OPENABLE)
            .putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
        startActivityForResult(invoke, intent, "picked")
    }
    @ActivityCallback
    fun picked(invoke: Invoke, result: ActivityResult) {
        if (result.resultCode != Activity.RESULT_OK) { invoke.resolve(JSObject().put("paths", JSArray())); return }
        importDocuments(invoke, result.data, false)
    }
    @Suppress("DEPRECATION")
    private fun importDocuments(invoke: Invoke, data: Intent?, shared: Boolean) {
        // Copy on an IO thread in bounded buffers: SAF URIs are not filesystem paths.
        Thread {
            val imported = mutableListOf<File>()
            try {
                requireNotNull(data) { "No document returned" }
                val uris = if (shared && data.action == Intent.ACTION_SEND_MULTIPLE) {
                    data.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)?.toList() ?: emptyList()
                } else if (shared) {
                    listOfNotNull(data.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))
                } else {
                    data.clipData?.let { clip -> (0 until clip.itemCount).map { clip.getItemAt(it).uri } } ?: listOfNotNull(data.data)
                }
                require(uris.size <= 128) { "Choose at most 128 documents at once" }
                require(uris.all { it.scheme == "content" }) { "Only content URIs can be imported" }
                val names = mutableSetOf<String>()
                val paths = uris.map { uri ->
                    val directory = File(activity.cacheDir, "send-${UUID.randomUUID()}").apply { mkdirs() }
                    val display = activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null } ?: "document"
                    var index = 0
                    var name: String
                    do { name = DownloadNames.candidate(display, index++) } while (!names.add(name))
                    val destination = File(directory, name); imported.add(destination)
                    activity.contentResolver.openInputStream(uri).use { input ->
                        requireNotNull(input) { "Cannot read selected document" }
                        destination.outputStream().use { output -> input.copyTo(output, 65536) }
                    }
                    destination.absolutePath
                }
                invoke.resolve(JSObject().put("paths", JSArray(paths)))
            } catch (error: Exception) {
                imported.forEach { it.delete(); it.parentFile?.delete() }
                invoke.reject("Cannot import selected documents: $error")
            }
        }.start()
    }
}
