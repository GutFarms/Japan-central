package com.solstice.dispensary.data.update

import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.WorkManager
import java.time.Duration
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.time.ZonedDateTime
import java.util.concurrent.TimeUnit

/** Schedules a GitHub Releases check/download every day at 1:00 AM local time. */
object AppAutoUpdateScheduler {
    const val UNIQUE_WORK_NAME = "nativepure-daily-app-update"
    const val WORK_TAG = "nativepure-auto-update"
    private val UPDATE_TIME = LocalTime.of(1, 0)

    fun ensureScheduled(context: Context) {
        scheduleNext(context.applicationContext)
    }

    fun scheduleNext(context: Context) {
        val appContext = context.applicationContext
        val delayMs = millisUntilNextUpdate().coerceAtLeast(5_000L)
        val constraints = Constraints.Builder()
            .setRequiredNetworkType(NetworkType.CONNECTED)
            .build()
        val request = OneTimeWorkRequestBuilder<AppAutoUpdateWorker>()
            .setInitialDelay(delayMs, TimeUnit.MILLISECONDS)
            .setConstraints(constraints)
            .addTag(WORK_TAG)
            .build()
        WorkManager.getInstance(appContext).enqueueUniqueWork(
            UNIQUE_WORK_NAME,
            ExistingWorkPolicy.REPLACE,
            request
        )
    }

    /** Milliseconds from now until the next 1:00 AM local time. */
    fun millisUntilNextUpdate(now: ZonedDateTime = ZonedDateTime.now()): Long {
        var next = ZonedDateTime.of(LocalDate.from(now), UPDATE_TIME, now.zone)
        if (!next.isAfter(now)) {
            next = next.plusDays(1)
        }
        return Duration.between(now, next).toMillis()
    }

    fun nextRunLabel(zoneId: ZoneId = ZoneId.systemDefault()): String {
        val now = ZonedDateTime.now(zoneId)
        var next = ZonedDateTime.of(LocalDate.from(now), UPDATE_TIME, zoneId)
        if (!next.isAfter(now)) next = next.plusDays(1)
        return "Daily at 1:00 AM · next ${next.toLocalDate()}"
    }
}
