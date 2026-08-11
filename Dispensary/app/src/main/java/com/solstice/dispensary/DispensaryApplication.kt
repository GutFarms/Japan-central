package com.solstice.dispensary

import android.app.Application
import com.solstice.dispensary.data.repository.DispensaryRepository
import com.solstice.dispensary.data.update.AppUpdateChecker
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class DispensaryApplication : Application() {
    lateinit var repository: DispensaryRepository
        private set

    lateinit var updateChecker: AppUpdateChecker
        private set

    override fun onCreate() {
        super.onCreate()
        repository = DispensaryRepository(this)
        updateChecker = AppUpdateChecker(this)
        CoroutineScope(Dispatchers.IO).launch {
            repository.ensureSeeded()
        }
    }
}
