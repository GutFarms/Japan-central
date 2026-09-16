package com.gutfarms.llcmanager.data.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import com.gutfarms.llcmanager.data.dao.DeductionDao
import com.gutfarms.llcmanager.data.dao.IncomeDao
import com.gutfarms.llcmanager.data.dao.InventoryDao
import com.gutfarms.llcmanager.data.dao.LlcDao
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.Income
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.data.model.Llc

@Database(
    entities = [Llc::class, Deduction::class, Income::class, InventoryItem::class],
    version = 2,
    exportSchema = false
)
abstract class LlcDatabase : RoomDatabase() {
    abstract fun llcDao(): LlcDao
    abstract fun deductionDao(): DeductionDao
    abstract fun incomeDao(): IncomeDao
    abstract fun inventoryDao(): InventoryDao

    companion object {
        @Volatile
        private var instance: LlcDatabase? = null

        fun get(context: Context): LlcDatabase {
            return instance ?: synchronized(this) {
                instance ?: Room.databaseBuilder(
                    context.applicationContext,
                    LlcDatabase::class.java,
                    "llc_manager.db"
                ).fallbackToDestructiveMigration().build().also { instance = it }
            }
        }
    }
}
