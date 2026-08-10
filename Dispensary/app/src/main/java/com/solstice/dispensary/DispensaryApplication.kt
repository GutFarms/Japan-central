package com.solstice.dispensary

import android.app.Application
import com.solstice.dispensary.data.repository.DispensaryRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class DispensaryApplication : Application() {
    lateinit var repository: DispensaryRepository
        private set

    override fun onCreate() {
        super.onCreate()
        repository = DispensaryRepository(this)
        CoroutineScope(Dispatchers.IO).launch {
            repository.ensureSeeded()
        }
    }
}
