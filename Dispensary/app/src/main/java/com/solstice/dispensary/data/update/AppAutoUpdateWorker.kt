package com.solstice.dispensary.data.update

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters
import com.solstice.dispensary.MainActivity
import com.solstice.dispensary.R

/**
 * Pulls the latest release manifest from GitHub and downloads the APK when newer.
 * Runs on the daily 1:00 AM schedule from [AppAutoUpdateScheduler].
 */
class AppAutoUpdateWorker(
    appContext: Context,
    params: WorkerParameters
) : CoroutineWorker(appContext, params) {

    override suspend fun doWork(): Result {
        return try {
            val checker = AppUpdateChecker(applicationContext)
            when (val result = checker.checkForUpdate(force = true)) {
                UpdateCheckResult.UpToDate -> {
                    checker.markAutoUpdateChecked("up_to_date")
                }
                is UpdateCheckResult.Failed -> {
                    checker.markAutoUpdateChecked("failed:${result.message}")
                    // Retry later today if network blipped; still reschedule 1 AM tomorrow.
                    AppAutoUpdateScheduler.scheduleNext(applicationContext)
                    return Result.retry()
                }
                is UpdateCheckResult.Available -> {
                    val file = checker.downloadApk(result.info)
                    checker.markPendingInstall(result.info)
                    checker.markAutoUpdateChecked("downloaded:${result.info.versionName}")
                    notifyUpdateReady(applicationContext, result.info)
                    // Attempt to open the system installer when allowed (may be blocked in background).
                    if (ApkInstaller.canInstallPackages(applicationContext)) {
                        try {
                            applicationContext.startActivity(
                                ApkInstaller.installApk(applicationContext, file)
                            )
                        } catch (_: Throwable) {
                            // Notification remains the reliable path.
                        }
                    }
                }
            }
            AppAutoUpdateScheduler.scheduleNext(applicationContext)
            Result.success()
        } catch (t: Throwable) {
            AppAutoUpdateScheduler.scheduleNext(applicationContext)
            Result.retry()
        }
    }

    companion object {
        const val CHANNEL_ID = "nativepure_updates"
        const val NOTIFICATION_ID = 1701
        const val EXTRA_INSTALL_UPDATE = "extra_install_update"

        fun ensureChannel(context: Context) {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
            val manager = context.getSystemService(NotificationManager::class.java) ?: return
            val channel = NotificationChannel(
                CHANNEL_ID,
                "App updates",
                NotificationManager.IMPORTANCE_HIGH
            ).apply {
                description = "Daily Native Pure updates from GitHub Releases"
            }
            manager.createNotificationChannel(channel)
        }

        private fun notifyUpdateReady(context: Context, info: AppUpdateInfo) {
            ensureChannel(context)
            val launch = Intent(context, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP
                putExtra(EXTRA_INSTALL_UPDATE, true)
            }
            val pending = PendingIntent.getActivity(
                context,
                NOTIFICATION_ID,
                launch,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
            val notification = NotificationCompat.Builder(context, CHANNEL_ID)
                .setSmallIcon(R.mipmap.ic_launcher)
                .setContentTitle("Native Pure ${info.versionName} ready")
                .setContentText("Tap to install the update downloaded at 1:00 AM.")
                .setStyle(
                    NotificationCompat.BigTextStyle().bigText(
                        buildString {
                            append("A newer build was pulled from GitHub Releases.")
                            if (info.releaseNotes.isNotBlank()) {
                                append(' ')
                                append(info.releaseNotes)
                            }
                        }
                    )
                )
                .setPriority(NotificationCompat.PRIORITY_HIGH)
                .setContentIntent(pending)
                .setAutoCancel(true)
                .build()
            try {
                NotificationManagerCompat.from(context).notify(NOTIFICATION_ID, notification)
            } catch (_: SecurityException) {
                // POST_NOTIFICATIONS may not be granted yet on Android 13+.
            }
        }
    }
}
