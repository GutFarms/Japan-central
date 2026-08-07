package com.gutfarms.manager.data.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.gutfarms.manager.data.dao.AnimalDao
import com.gutfarms.manager.data.dao.FeedingScheduleDao
import com.gutfarms.manager.data.dao.TransactionDao
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalType
import com.gutfarms.manager.data.model.ExpenseCategory
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedFrequency
import com.gutfarms.manager.data.model.FeedingSchedule
import com.gutfarms.manager.data.model.IncomeCategory
import com.gutfarms.manager.data.model.TransactionType

class Converters {
    @TypeConverter fun fromAnimalType(value: AnimalType): String = value.name
    @TypeConverter fun toAnimalType(value: String): AnimalType = AnimalType.valueOf(value)

    @TypeConverter fun fromFeedFrequency(value: FeedFrequency): String = value.name
    @TypeConverter fun toFeedFrequency(value: String): FeedFrequency = FeedFrequency.valueOf(value)

    @TypeConverter fun fromTransactionType(value: TransactionType): String = value.name
    @TypeConverter fun toTransactionType(value: String): TransactionType = TransactionType.valueOf(value)

    @TypeConverter fun fromExpenseCategory(value: ExpenseCategory?): String? = value?.name
    @TypeConverter fun toExpenseCategory(value: String?): ExpenseCategory? =
        value?.let { ExpenseCategory.valueOf(it) }

    @TypeConverter fun fromIncomeCategory(value: IncomeCategory?): String? = value?.name
    @TypeConverter fun toIncomeCategory(value: String?): IncomeCategory? =
        value?.let { IncomeCategory.valueOf(it) }
}

@Database(
    entities = [Animal::class, FeedingSchedule::class, FarmTransaction::class],
    version = 1,
    exportSchema = false
)
@TypeConverters(Converters::class)
abstract class FarmDatabase : RoomDatabase() {
    abstract fun animalDao(): AnimalDao
    abstract fun feedingScheduleDao(): FeedingScheduleDao
    abstract fun transactionDao(): TransactionDao

    companion object {
        @Volatile private var INSTANCE: FarmDatabase? = null

        fun getInstance(context: Context): FarmDatabase {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: Room.databaseBuilder(
                    context.applicationContext,
                    FarmDatabase::class.java,
                    "farm_manager.db"
                ).build().also { INSTANCE = it }
            }
        }
    }
}

suspend fun seedSampleDataIfEmpty(database: FarmDatabase) {
    val animals = database.openHelper.readableDatabase.query("SELECT COUNT(*) FROM animals").use { cursor ->
        cursor.moveToFirst()
        cursor.getInt(0)
    }
    if (animals > 0) return

    val cattleId = database.animalDao().upsert(
        Animal(
            name = "Pasture Herd A",
            type = AnimalType.CATTLE,
            count = 12,
            notes = "Mixed beef cattle",
            purchaseCost = 18000.0
        )
    )
    val chickenId = database.animalDao().upsert(
        Animal(
            name = "Layer Coop 1",
            type = AnimalType.CHICKEN,
            count = 80,
            notes = "Rhode Island Reds",
            purchaseCost = 960.0
        )
    )
    database.feedingScheduleDao().upsert(
        FeedingSchedule(
            animalId = cattleId,
            feedName = "Hay + Grain mix",
            amountKg = 140.0,
            costPerKg = 0.28,
            frequency = FeedFrequency.DAILY,
            timeOfDay = "07:00",
            notes = "Morning pasture top-up"
        )
    )
    database.feedingScheduleDao().upsert(
        FeedingSchedule(
            animalId = cattleId,
            feedName = "Mineral lick check",
            amountKg = 2.0,
            costPerKg = 1.10,
            frequency = FeedFrequency.DAILY,
            timeOfDay = "17:30"
        )
    )
    database.feedingScheduleDao().upsert(
        FeedingSchedule(
            animalId = chickenId,
            feedName = "Layer pellets",
            amountKg = 10.0,
            costPerKg = 0.55,
            frequency = FeedFrequency.TWICE_DAILY,
            timeOfDay = "08:00"
        )
    )
    database.transactionDao().upsert(
        FarmTransaction(
            type = TransactionType.INCOME,
            amount = 420.0,
            description = "Egg sales — weekly market",
            incomeCategory = IncomeCategory.EGGS
        )
    )
    database.transactionDao().upsert(
        FarmTransaction(
            type = TransactionType.INCOME,
            amount = 2400.0,
            description = "Two steers sold",
            incomeCategory = IncomeCategory.LIVESTOCK_SALE
        )
    )
    database.transactionDao().upsert(
        FarmTransaction(
            type = TransactionType.EXPENSE,
            amount = 310.0,
            description = "Bulk feed delivery",
            expenseCategory = ExpenseCategory.FEED
        )
    )
    database.transactionDao().upsert(
        FarmTransaction(
            type = TransactionType.EXPENSE,
            amount = 150.0,
            description = "Vet visit — herd check",
            expenseCategory = ExpenseCategory.VETERINARY
        )
    )
}
