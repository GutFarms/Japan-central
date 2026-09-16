package com.gutfarms.llcmanager

import android.app.Application
import com.gutfarms.llcmanager.data.db.LlcDatabase
import com.gutfarms.llcmanager.data.repository.LlcRepository

class LlcManagerApplication : Application() {
    val repository: LlcRepository by lazy {
        val db = LlcDatabase.get(this)
        LlcRepository(
            db.llcDao(),
            db.deductionDao(),
            db.incomeDao(),
            db.inventoryDao(),
            db.employeeDao()
        )
    }
}
