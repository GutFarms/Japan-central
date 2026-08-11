package com.solstice.dispensary.data.update

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Re-schedules the daily 1:00 AM GitHub update check after device reboot. */
class BootCompletedReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent?) {
        if (intent?.action != Intent.ACTION_BOOT_COMPLETED &&
            intent?.action != Intent.ACTION_LOCKED_BOOT_COMPLETED
        ) {
            return
        }
        AppAutoUpdateScheduler.ensureScheduled(context.applicationContext)
        AppAutoUpdateWorker.ensureChannel(context.applicationContext)
    }
}
