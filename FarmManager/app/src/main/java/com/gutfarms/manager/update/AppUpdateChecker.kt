package com.gutfarms.manager.update

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import androidx.core.content.FileProvider
import com.gutfarms.manager.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.BufferedInputStream
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL

data class AppUpdateInfo(
    val versionCode: Int,
    val versionName: String,
    val apkUrl: String,
    val releaseNotes: String = ""
)

sealed class UpdateCheckResult {
    data class Available(val info: AppUpdateInfo) : UpdateCheckResult()
    data class UpToDate(val currentVersion: String) : UpdateCheckResult()
    data class Error(val message: String) : UpdateCheckResult()
}

object AppUpdateChecker {
    private const val REPO_API =
        "https://api.github.com/repos/GutFarms/Japan-central/releases?per_page=40"

    private val MANIFEST_FALLBACKS = listOf(
        "https://raw.githubusercontent.com/GutFarms/Japan-central/master/FarmManager/dist/update-manifest.json",
        "https://raw.githubusercontent.com/GutFarms/Japan-central/cursor/farm-management-android-115a/FarmManager/dist/update-manifest.json"
    )

    fun currentVersionLabel(): String =
        "${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})"

    suspend fun checkForUpdate(): UpdateCheckResult = withContext(Dispatchers.IO) {
        try {
            val remote = fetchLatestManifest()
                ?: return@withContext UpdateCheckResult.Error(
                    "Could not reach the Gut Farms update feed. Check your connection."
                )
            if (remote.versionCode > BuildConfig.VERSION_CODE) {
                UpdateCheckResult.Available(remote)
            } else {
                UpdateCheckResult.UpToDate(currentVersionLabel())
            }
        } catch (e: Exception) {
            UpdateCheckResult.Error(e.message ?: "Update check failed")
        }
    }

    suspend fun downloadApk(context: Context, info: AppUpdateInfo): File =
        withContext(Dispatchers.IO) {
            val dir = File(context.cacheDir, "updates").apply { mkdirs() }
            val outFile = File(dir, "GutFarms-FarmManager-${info.versionName}.apk")
            if (outFile.exists()) outFile.delete()

            val connection = (URL(info.apkUrl).openConnection() as HttpURLConnection).apply {
                instanceFollowRedirects = true
                connectTimeout = 30_000
                readTimeout = 120_000
                setRequestProperty("User-Agent", "GutFarms-FarmManager/${BuildConfig.VERSION_NAME}")
                setRequestProperty("Accept", "application/octet-stream,*/*")
            }
            try {
                if (connection.responseCode !in 200..299) {
                    error("Download failed (HTTP ${connection.responseCode})")
                }
                BufferedInputStream(connection.inputStream).use { input ->
                    FileOutputStream(outFile).use { output ->
                        input.copyTo(output)
                    }
                }
            } finally {
                connection.disconnect()
            }
            if (outFile.length() < 50_000L) {
                outFile.delete()
                error("Downloaded file looks incomplete")
            }
            outFile
        }

    fun installApk(context: Context, apkFile: File) {
        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            apkFile
        )
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, "application/vnd.android.package-archive")
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        context.packageManager.queryIntentActivities(intent, PackageManager.MATCH_DEFAULT_ONLY)
            .forEach { resolve ->
                context.grantUriPermission(
                    resolve.activityInfo.packageName,
                    uri,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
            }
        context.startActivity(intent)
    }

    fun canRequestInstall(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            context.packageManager.canRequestPackageInstalls()
        } else {
            true
        }
    }

    fun installPermissionSettingsIntent(context: Context): Intent {
        return Intent(
            android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
            Uri.parse("package:${context.packageName}")
        ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }

    private fun fetchLatestManifest(): AppUpdateInfo? {
        fetchFromGitHubReleases()?.let { return it }
        for (url in MANIFEST_FALLBACKS) {
            runCatching { parseManifest(fetchText(url)) }.getOrNull()?.let { return it }
        }
        return null
    }

    private fun fetchFromGitHubReleases(): AppUpdateInfo? {
        val body = fetchText(REPO_API)
        val releases = JSONArray(body)
        for (i in 0 until releases.length()) {
            val release = releases.getJSONObject(i)
            val tag = release.optString("tag_name")
            if (!tag.startsWith("gutfarms-v")) continue

            val assets = release.optJSONArray("assets") ?: continue
            var manifestUrl: String? = null
            var apkUrl: String? = null
            for (a in 0 until assets.length()) {
                val asset = assets.getJSONObject(a)
                val name = asset.optString("name")
                val url = asset.optString("browser_download_url")
                when {
                    name.equals("update-manifest.json", ignoreCase = true) -> manifestUrl = url
                    name.equals("GutFarms-FarmManager.apk", ignoreCase = true) -> apkUrl = url
                    name.endsWith(".apk", ignoreCase = true) &&
                        name.contains("GutFarms", ignoreCase = true) &&
                        apkUrl == null -> apkUrl = url
                }
            }
            if (manifestUrl != null) {
                return parseManifest(fetchText(manifestUrl))
            }
            if (apkUrl != null) {
                val versionName = tag.removePrefix("gutfarms-v")
                return AppUpdateInfo(
                    versionCode = BuildConfig.VERSION_CODE + 1,
                    versionName = versionName,
                    apkUrl = apkUrl,
                    releaseNotes = release.optString("name")
                )
            }
        }
        return null
    }

    private fun parseManifest(json: String): AppUpdateInfo {
        val obj = JSONObject(json)
        return AppUpdateInfo(
            versionCode = obj.getInt("versionCode"),
            versionName = obj.getString("versionName"),
            apkUrl = obj.getString("apkUrl"),
            releaseNotes = obj.optString("releaseNotes", "")
        )
    }

    private fun fetchText(url: String): String {
        val connection = (URL(url).openConnection() as HttpURLConnection).apply {
            instanceFollowRedirects = true
            connectTimeout = 20_000
            readTimeout = 30_000
            setRequestProperty("User-Agent", "GutFarms-FarmManager/${BuildConfig.VERSION_NAME}")
            setRequestProperty("Accept", "application/json,text/plain,*/*")
        }
        try {
            if (connection.responseCode !in 200..299) {
                error("HTTP ${connection.responseCode} for $url")
            }
            return connection.inputStream.bufferedReader().use { it.readText() }
        } finally {
            connection.disconnect()
        }
    }
}
