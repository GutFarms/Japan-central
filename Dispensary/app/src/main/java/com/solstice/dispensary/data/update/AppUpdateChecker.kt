package com.solstice.dispensary.data.update

import android.content.Context
import android.content.SharedPreferences
import com.solstice.dispensary.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

class AppUpdateChecker(
    context: Context
) {
    private val appContext = context.applicationContext
    private val prefs: SharedPreferences =
        appContext.getSharedPreferences("nativepure_update_prefs", Context.MODE_PRIVATE)

    suspend fun checkForUpdate(force: Boolean = false): UpdateCheckResult = withContext(Dispatchers.IO) {
        try {
            val json = fetchText(MANIFEST_URL)
                ?: return@withContext UpdateCheckResult.Failed(
                    "Could not reach the update server. Check your connection."
                )
            val info = parseManifest(json)
                ?: return@withContext UpdateCheckResult.Failed("Update info looks invalid.")

            if (info.versionCode <= BuildConfig.VERSION_CODE) {
                return@withContext UpdateCheckResult.UpToDate
            }
            if (!force && prefs.getInt(KEY_DISMISSED_VERSION, 0) >= info.versionCode) {
                return@withContext UpdateCheckResult.UpToDate
            }
            UpdateCheckResult.Available(info)
        } catch (t: Throwable) {
            UpdateCheckResult.Failed(t.message ?: "Update check failed.")
        }
    }

    suspend fun downloadApk(
        info: AppUpdateInfo,
        onProgress: (Float?) -> Unit = {}
    ): File = withContext(Dispatchers.IO) {
        val dir = File(appContext.cacheDir, "updates").apply { mkdirs() }
        val outFile = File(dir, "NativePure-update.apk")
        if (outFile.exists()) outFile.delete()

        val connection = (URL(info.apkUrl).openConnection() as HttpURLConnection).apply {
            connectTimeout = 20_000
            readTimeout = 120_000
            instanceFollowRedirects = true
            requestMethod = "GET"
            setRequestProperty("Accept", "application/octet-stream,*/*")
            setRequestProperty("User-Agent", "NativePure-Android/${BuildConfig.VERSION_NAME}")
        }
        try {
            val code = connection.responseCode
            if (code !in 200..299) {
                throw IllegalStateException("Download failed (HTTP $code).")
            }
            val total = connection.contentLengthLong.takeIf { it > 0 }
            connection.inputStream.use { input ->
                outFile.outputStream().use { output ->
                    val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
                    var readTotal = 0L
                    while (true) {
                        val read = input.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        readTotal += read
                        if (total != null) {
                            onProgress((readTotal.toFloat() / total.toFloat()).coerceIn(0f, 1f))
                        } else {
                            onProgress(null)
                        }
                    }
                    output.flush()
                }
            }
            if (outFile.length() < 1_000_000L) {
                outFile.delete()
                throw IllegalStateException("Downloaded file looks too small to be an APK.")
            }
            outFile
        } finally {
            connection.disconnect()
        }
    }

    fun dismiss(info: AppUpdateInfo) {
        prefs.edit().putInt(KEY_DISMISSED_VERSION, info.versionCode).apply()
    }

    fun clearDismissed() {
        prefs.edit().remove(KEY_DISMISSED_VERSION).apply()
    }

    fun cachedApk(): File? {
        val file = File(File(appContext.cacheDir, "updates"), "NativePure-update.apk")
        return file.takeIf { it.exists() && it.length() > 1_000_000L }
    }

    private fun fetchText(url: String): String? {
        val connection = (URL(url).openConnection() as HttpURLConnection).apply {
            connectTimeout = 15_000
            readTimeout = 20_000
            instanceFollowRedirects = true
            requestMethod = "GET"
            setRequestProperty("Accept", "application/json")
            setRequestProperty("User-Agent", "NativePure-Android/${BuildConfig.VERSION_NAME}")
        }
        return try {
            val code = connection.responseCode
            if (code !in 200..299) return null
            connection.inputStream.bufferedReader().use { it.readText() }
        } finally {
            connection.disconnect()
        }
    }

    private fun parseManifest(raw: String): AppUpdateInfo? {
        return try {
            val obj = JSONObject(raw)
            val versionCode = obj.getInt("versionCode")
            val versionName = obj.getString("versionName")
            val apkUrl = obj.getString("apkUrl")
            val notes = obj.optString("releaseNotes", "")
            if (versionCode <= 0 || versionName.isBlank() || apkUrl.isBlank()) null
            else AppUpdateInfo(versionCode, versionName, apkUrl, notes)
        } catch (_: Exception) {
            null
        }
    }

    companion object {
        const val MANIFEST_URL =
            "https://github.com/GutFarms/Japan-central/releases/latest/download/update-manifest.json"
        private const val KEY_DISMISSED_VERSION = "update_dismissed_version_code"
    }
}
