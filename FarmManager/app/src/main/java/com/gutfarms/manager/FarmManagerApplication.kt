package com.gutfarms.manager

import android.app.Application
import com.gutfarms.manager.data.db.FarmDatabase
import com.gutfarms.manager.data.db.seedSampleDataIfEmpty
import com.gutfarms.manager.data.repository.FarmRepository
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

class FarmManagerApplication : Application() {
    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    lateinit var repository: FarmRepository
        private set

    override fun onCreate() {
        super.onCreate()
        val database = FarmDatabase.getInstance(this)
        repository = FarmRepository(
            animalDao = database.animalDao(),
            feedingScheduleDao = database.feedingScheduleDao(),
            breedingScheduleDao = database.breedingScheduleDao(),
            animalArrivalDao = database.animalArrivalDao(),
            transactionDao = database.transactionDao(),
            farmProfileDao = database.farmProfileDao(),
            farmImportFileDao = database.farmImportFileDao(),
            apiFeedSourceDao = database.apiFeedSourceDao()
        )
        appScope.launch {
            seedSampleDataIfEmpty(database)
        }
    }
}
