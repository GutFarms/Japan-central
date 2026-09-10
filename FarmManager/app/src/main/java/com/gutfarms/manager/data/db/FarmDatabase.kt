package com.gutfarms.manager.data.db

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.gutfarms.manager.data.dao.AnimalArrivalDao
import com.gutfarms.manager.data.dao.AnimalDao
import com.gutfarms.manager.data.dao.BreedingScheduleDao
import com.gutfarms.manager.data.dao.FarmContactDao
import com.gutfarms.manager.data.dao.FarmProfileDao
import com.gutfarms.manager.data.dao.FeedingScheduleDao
import com.gutfarms.manager.data.dao.HealthRecordDao
import com.gutfarms.manager.data.dao.InventoryItemDao
import com.gutfarms.manager.data.dao.JournalEntryDao
import com.gutfarms.manager.data.dao.TransactionDao
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrival
import com.gutfarms.manager.data.model.AnimalType
import com.gutfarms.manager.data.model.ArrivalOrigin
import com.gutfarms.manager.data.model.BreedingMethod
import com.gutfarms.manager.data.model.BreedingSchedule
import com.gutfarms.manager.data.model.BreedingStatus
import com.gutfarms.manager.data.model.ContactRole
import com.gutfarms.manager.data.model.ExpenseCategory
import com.gutfarms.manager.data.model.FarmContact
import com.gutfarms.manager.data.model.FarmProfile
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedFrequency
import com.gutfarms.manager.data.model.FeedUnit
import com.gutfarms.manager.data.model.FeedingSchedule
import com.gutfarms.manager.data.model.HealthRecord
import com.gutfarms.manager.data.model.HealthRecordType
import com.gutfarms.manager.data.model.IncomeCategory
import com.gutfarms.manager.data.model.InventoryCategory
import com.gutfarms.manager.data.model.InventoryItem
import com.gutfarms.manager.data.model.JournalCategory
import com.gutfarms.manager.data.model.JournalEntry
import com.gutfarms.manager.data.model.RegistrationStatus
import com.gutfarms.manager.data.model.TransactionType

class Converters {
    @TypeConverter fun fromAnimalType(value: AnimalType): String = value.name
    @TypeConverter fun toAnimalType(value: String): AnimalType = AnimalType.valueOf(value)

    @TypeConverter fun fromFeedFrequency(value: FeedFrequency): String = value.name
    @TypeConverter fun toFeedFrequency(value: String): FeedFrequency = FeedFrequency.valueOf(value)

    @TypeConverter fun fromFeedUnit(value: FeedUnit): String = value.name
    @TypeConverter fun toFeedUnit(value: String): FeedUnit =
        runCatching { FeedUnit.valueOf(value) }.getOrDefault(FeedUnit.KG)

    @TypeConverter fun fromTransactionType(value: TransactionType): String = value.name
    @TypeConverter fun toTransactionType(value: String): TransactionType = TransactionType.valueOf(value)

    @TypeConverter fun fromExpenseCategory(value: ExpenseCategory?): String? = value?.name
    @TypeConverter fun toExpenseCategory(value: String?): ExpenseCategory? =
        value?.let { ExpenseCategory.valueOf(it) }

    @TypeConverter fun fromIncomeCategory(value: IncomeCategory?): String? = value?.name
    @TypeConverter fun toIncomeCategory(value: String?): IncomeCategory? =
        value?.let { IncomeCategory.valueOf(it) }

    @TypeConverter fun fromBreedingMethod(value: BreedingMethod): String = value.name
    @TypeConverter fun toBreedingMethod(value: String): BreedingMethod = BreedingMethod.valueOf(value)

    @TypeConverter fun fromBreedingStatus(value: BreedingStatus): String = value.name
    @TypeConverter fun toBreedingStatus(value: String): BreedingStatus = BreedingStatus.valueOf(value)

    @TypeConverter fun fromArrivalOrigin(value: ArrivalOrigin): String = value.name
    @TypeConverter fun toArrivalOrigin(value: String): ArrivalOrigin = ArrivalOrigin.valueOf(value)

    @TypeConverter fun fromRegistrationStatus(value: RegistrationStatus): String = value.name
    @TypeConverter fun toRegistrationStatus(value: String): RegistrationStatus =
        RegistrationStatus.valueOf(value)

    @TypeConverter fun fromHealthRecordType(value: HealthRecordType): String = value.name
    @TypeConverter fun toHealthRecordType(value: String): HealthRecordType =
        HealthRecordType.valueOf(value)

    @TypeConverter fun fromInventoryCategory(value: InventoryCategory): String = value.name
    @TypeConverter fun toInventoryCategory(value: String): InventoryCategory =
        InventoryCategory.valueOf(value)

    @TypeConverter fun fromJournalCategory(value: JournalCategory): String = value.name
    @TypeConverter fun toJournalCategory(value: String): JournalCategory =
        JournalCategory.valueOf(value)

    @TypeConverter fun fromContactRole(value: ContactRole): String = value.name
    @TypeConverter fun toContactRole(value: String): ContactRole = ContactRole.valueOf(value)
}

@Database(
    entities = [
        Animal::class,
        FeedingSchedule::class,
        BreedingSchedule::class,
        AnimalArrival::class,
        FarmTransaction::class,
        FarmProfile::class,
        HealthRecord::class,
        InventoryItem::class,
        JournalEntry::class,
        FarmContact::class
    ],
    version = 5,
    exportSchema = false
)
@TypeConverters(Converters::class)
abstract class FarmDatabase : RoomDatabase() {
    abstract fun animalDao(): AnimalDao
    abstract fun feedingScheduleDao(): FeedingScheduleDao
    abstract fun breedingScheduleDao(): BreedingScheduleDao
    abstract fun animalArrivalDao(): AnimalArrivalDao
    abstract fun transactionDao(): TransactionDao
    abstract fun farmProfileDao(): FarmProfileDao
    abstract fun healthRecordDao(): HealthRecordDao
    abstract fun inventoryItemDao(): InventoryItemDao
    abstract fun journalEntryDao(): JournalEntryDao
    abstract fun farmContactDao(): FarmContactDao

    companion object {
        @Volatile private var INSTANCE: FarmDatabase? = null

        fun getInstance(context: Context): FarmDatabase {
            return INSTANCE ?: synchronized(this) {
                INSTANCE ?: Room.databaseBuilder(
                    context.applicationContext,
                    FarmDatabase::class.java,
                    "farm_manager.db"
                )
                    .fallbackToDestructiveMigration()
                    .build()
                    .also { INSTANCE = it }
            }
        }
    }
}

suspend fun ensureFarmProfile(database: FarmDatabase) {
    if (database.farmProfileDao().get() == null) {
        database.farmProfileDao().upsert(FarmProfile(farmName = "Gut Farms"))
    }
}

suspend fun seedSampleDataIfEmpty(database: FarmDatabase) {
    ensureFarmProfile(database)
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
            feedQuantity = 140.0,
            quantityUnit = FeedUnit.KG,
            costPerUnit = 0.28,
            animalsFed = 12,
            stockOnHand = 900.0,
            frequency = FeedFrequency.DAILY,
            timeOfDay = "07:00",
            notes = "Morning pasture top-up"
        )
    )
    database.feedingScheduleDao().upsert(
        FeedingSchedule(
            animalId = cattleId,
            feedName = "Mineral lick check",
            feedQuantity = 2.0,
            quantityUnit = FeedUnit.KG,
            costPerUnit = 1.10,
            animalsFed = 12,
            stockOnHand = 40.0,
            frequency = FeedFrequency.DAILY,
            timeOfDay = "17:30"
        )
    )
    database.feedingScheduleDao().upsert(
        FeedingSchedule(
            animalId = chickenId,
            feedName = "Layer pellets",
            feedQuantity = 10.0,
            quantityUnit = FeedUnit.KG,
            costPerUnit = 0.55,
            animalsFed = 80,
            stockOnHand = 200.0,
            frequency = FeedFrequency.TWICE_DAILY,
            timeOfDay = "08:00"
        )
    )

    val now = System.currentTimeMillis()
    val cattleBreeding = now - (30L * BreedingSchedule.DayMillis)
    database.breedingScheduleDao().upsert(
        BreedingSchedule(
            animalId = cattleId,
            femaleLabel = "Cow #14",
            sireName = "Bull Ranger",
            method = BreedingMethod.NATURAL,
            status = BreedingStatus.PREGNANT,
            breedingDateMillis = cattleBreeding,
            expectedDueDateMillis = BreedingSchedule.expectedDueDate(cattleBreeding, AnimalType.CATTLE),
            expectedOffspring = 1,
            notes = "First calf for #14"
        )
    )
    val chickenBreeding = now - (5L * BreedingSchedule.DayMillis)
    database.breedingScheduleDao().upsert(
        BreedingSchedule(
            animalId = chickenId,
            femaleLabel = "Broody hen group",
            sireName = "Rooster pen B",
            method = BreedingMethod.NATURAL,
            status = BreedingStatus.DUE_SOON,
            breedingDateMillis = chickenBreeding,
            expectedDueDateMillis = BreedingSchedule.expectedDueDate(chickenBreeding, AnimalType.CHICKEN),
            expectedOffspring = 12,
            notes = "Incubator tray 2"
        )
    )

    database.animalArrivalDao().upsert(
        AnimalArrival(
            name = "Maple",
            type = AnimalType.CATTLE,
            origin = ArrivalOrigin.PURCHASED,
            eventDateMillis = now - (12L * BreedingSchedule.DayMillis),
            registrationStatus = RegistrationStatus.REGISTERED,
            registrationId = "US-CA-4412",
            groupAnimalId = cattleId,
            notes = "Bought at county sale"
        )
    )
    database.animalArrivalDao().upsert(
        AnimalArrival(
            name = "",
            type = AnimalType.CHICKEN,
            origin = ArrivalOrigin.BORN_ON_FARM,
            eventDateMillis = now - (2L * BreedingSchedule.DayMillis),
            registrationStatus = RegistrationStatus.NOT_REQUIRED,
            groupAnimalId = chickenId,
            notes = "Clutch from incubator tray 1"
        )
    )
    database.animalArrivalDao().upsert(
        AnimalArrival(
            name = "Pepper",
            type = AnimalType.GOAT,
            origin = ArrivalOrigin.TRANSFERRED_IN,
            eventDateMillis = now - (40L * BreedingSchedule.DayMillis),
            registrationStatus = RegistrationStatus.PENDING,
            registrationId = "",
            notes = "Awaiting herd book paperwork"
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

    database.healthRecordDao().upsert(
        HealthRecord(
            animalId = cattleId,
            animalLabel = "Cow #14",
            type = HealthRecordType.VACCINATION,
            title = "Clostridial booster",
            dateMillis = now - (20L * BreedingSchedule.DayMillis),
            provider = "Valley Vet",
            cost = 85.0,
            notes = "Annual herd round"
        )
    )
    database.inventoryItemDao().upsert(
        InventoryItem(
            name = "Layer pellets",
            category = InventoryCategory.FEED,
            quantity = 200.0,
            unit = "kg",
            reorderLevel = 50.0,
            unitCost = 0.55,
            location = "Feed shed"
        )
    )
    database.inventoryItemDao().upsert(
        InventoryItem(
            name = "Ivermectin",
            category = InventoryCategory.MEDICINE,
            quantity = 4.0,
            unit = "bottles",
            reorderLevel = 2.0,
            unitCost = 32.0,
            location = "Med cabinet"
        )
    )
    database.journalEntryDao().upsert(
        JournalEntry(
            title = "Pasture rotation",
            category = JournalCategory.PASTURE,
            body = "Moved herd A to east paddock. Grass height good.",
            dateMillis = now - BreedingSchedule.DayMillis
        )
    )
    database.farmContactDao().upsert(
        FarmContact(
            name = "Dr. Helen Park",
            role = ContactRole.VETERINARIAN,
            phone = "555-0142",
            email = "helen@valleyvet.example",
            organization = "Valley Vet"
        )
    )
    database.farmContactDao().upsert(
        FarmContact(
            name = "Midwest Feed Co.",
            role = ContactRole.SUPPLIER,
            phone = "555-0199",
            organization = "Midwest Feed"
        )
    )
}
