package com.gutfarms.manager.data.model

import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

enum class AnimalType {
    CATTLE,
    DAIRY_COW,
    BEEF_CATTLE,
    CHICKEN,
    DUCK,
    TURKEY,
    GOOSE,
    QUAIL,
    GUINEA_FOWL,
    GOAT,
    SHEEP,
    PIG,
    HORSE,
    DONKEY,
    MULE,
    RABBIT,
    LLAMA,
    ALPACA,
    BISON,
    WATER_BUFFALO,
    DEER,
    EMU,
    OSTRICH,
    FISH,
    BEE_COLONY,
    OTHER
}

@Entity(tableName = "animals")
data class Animal(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val type: AnimalType,
    val count: Int,
    val notes: String = "",
    val purchaseCost: Double = 0.0,
    val createdAt: Long = System.currentTimeMillis()
)

enum class FeedFrequency {
    DAILY, TWICE_DAILY, WEEKLY, CUSTOM
}

@Entity(
    tableName = "feeding_schedules",
    foreignKeys = [
        ForeignKey(
            entity = Animal::class,
            parentColumns = ["id"],
            childColumns = ["animalId"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("animalId")]
)
data class FeedingSchedule(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val animalId: Long,
    val feedName: String,
    val amountKg: Double,
    val costPerKg: Double,
    val frequency: FeedFrequency,
    val timeOfDay: String,
    val notes: String = "",
    val active: Boolean = true
) {
    val dailyCost: Double
        get() = when (frequency) {
            FeedFrequency.DAILY -> amountKg * costPerKg
            FeedFrequency.TWICE_DAILY -> amountKg * costPerKg * 2
            FeedFrequency.WEEKLY -> (amountKg * costPerKg) / 7.0
            FeedFrequency.CUSTOM -> amountKg * costPerKg
        }

    val monthlyCost: Double
        get() = dailyCost * 30.0
}

enum class TransactionType {
    INCOME, EXPENSE
}

enum class ExpenseCategory {
    FEED, VETERINARY, LABOR, EQUIPMENT, UTILITIES, LIVESTOCK_PURCHASE, OTHER
}

enum class IncomeCategory {
    LIVESTOCK_SALE, EGGS, MILK, MEAT, PRODUCE, OTHER
}

@Entity(tableName = "transactions")
data class FarmTransaction(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val type: TransactionType,
    val amount: Double,
    val description: String,
    val expenseCategory: ExpenseCategory? = null,
    val incomeCategory: IncomeCategory? = null,
    val animalId: Long? = null,
    val dateMillis: Long = System.currentTimeMillis()
)

data class FeedingScheduleWithAnimal(
    val schedule: FeedingSchedule,
    val animalName: String,
    val animalType: AnimalType
)

enum class BreedingMethod {
    NATURAL, ARTIFICIAL_INSEMINATION, EMBRYO_TRANSFER
}

enum class BreedingStatus {
    PLANNED, BRED, PREGNANT, DUE_SOON, COMPLETED, FAILED
}

@Entity(
    tableName = "breeding_schedules",
    foreignKeys = [
        ForeignKey(
            entity = Animal::class,
            parentColumns = ["id"],
            childColumns = ["animalId"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("animalId"), Index("expectedDueDateMillis")]
)
data class BreedingSchedule(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val animalId: Long,
    val femaleLabel: String,
    val sireName: String = "",
    val method: BreedingMethod = BreedingMethod.NATURAL,
    val status: BreedingStatus = BreedingStatus.PLANNED,
    val breedingDateMillis: Long,
    val expectedDueDateMillis: Long,
    val expectedOffspring: Int = 1,
    val notes: String = "",
    val active: Boolean = true
) {
    val daysUntilDue: Long
        get() = ((expectedDueDateMillis - System.currentTimeMillis()) / DayMillis)

    companion object {
        const val DayMillis = 24L * 60L * 60L * 1000L

        fun gestationDaysFor(type: AnimalType): Int = when (type) {
            AnimalType.CATTLE,
            AnimalType.DAIRY_COW,
            AnimalType.BEEF_CATTLE -> 283
            AnimalType.SHEEP -> 147
            AnimalType.GOAT -> 150
            AnimalType.PIG -> 114
            AnimalType.HORSE,
            AnimalType.DONKEY,
            AnimalType.MULE -> 340
            AnimalType.RABBIT -> 31
            AnimalType.LLAMA,
            AnimalType.ALPACA -> 345
            AnimalType.BISON -> 285
            AnimalType.WATER_BUFFALO -> 310
            AnimalType.DEER -> 230
            AnimalType.CHICKEN -> 21
            AnimalType.DUCK -> 28
            AnimalType.TURKEY -> 28
            AnimalType.GOOSE -> 30
            AnimalType.QUAIL -> 17
            AnimalType.GUINEA_FOWL -> 28
            AnimalType.EMU -> 50
            AnimalType.OSTRICH -> 42
            AnimalType.FISH -> 0
            AnimalType.BEE_COLONY -> 0
            AnimalType.OTHER -> 120
        }

        fun expectedDueDate(breedingDateMillis: Long, type: AnimalType): Long =
            breedingDateMillis + gestationDaysFor(type) * DayMillis
    }
}

data class BreedingScheduleWithAnimal(
    val schedule: BreedingSchedule,
    val animalName: String,
    val animalType: AnimalType
)

data class ProfitSummary(
    val totalIncome: Double,
    val totalExpenses: Double,
    val projectedMonthlyFeedCost: Double,
    val netProfit: Double,
    val marginPercent: Double
)

enum class ArrivalOrigin {
    PURCHASED,
    BORN_ON_FARM,
    TRANSFERRED_IN,
    OTHER
}

enum class RegistrationStatus {
    NOT_REQUIRED,
    PENDING,
    REGISTERED,
    EXPIRED
}

@Entity(
    tableName = "animal_arrivals",
    foreignKeys = [
        ForeignKey(
            entity = Animal::class,
            parentColumns = ["id"],
            childColumns = ["groupAnimalId"],
            onDelete = ForeignKey.SET_NULL
        )
    ],
    indices = [Index("groupAnimalId"), Index("eventDateMillis")]
)
data class AnimalArrival(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String = "",
    val type: AnimalType,
    val origin: ArrivalOrigin,
    val eventDateMillis: Long,
    val registrationStatus: RegistrationStatus = RegistrationStatus.PENDING,
    val registrationId: String = "",
    val groupAnimalId: Long? = null,
    val notes: String = "",
    val createdAt: Long = System.currentTimeMillis()
) {
    val displayName: String
        get() = name.ifBlank { "Unnamed ${type.name.lowercase()}" }

    val eventDateLabel: String
        get() = when (origin) {
            ArrivalOrigin.BORN_ON_FARM -> "Birth"
            ArrivalOrigin.PURCHASED -> "Acquire"
            ArrivalOrigin.TRANSFERRED_IN -> "Transfer"
            ArrivalOrigin.OTHER -> "Arrival"
        }
}

data class AnimalArrivalWithGroup(
    val arrival: AnimalArrival,
    val groupName: String?
)

@Entity(tableName = "farm_profile")
data class FarmProfile(
    @PrimaryKey val id: Int = 1,
    val farmName: String = "Gut Farms"
)

enum class FarmFileKind {
    KMZ,
    KML,
    GEOJSON,
    CSV,
    JSON,
    IMAGE,
    OTHER
}

@Entity(tableName = "farm_import_files")
data class FarmImportFile(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val displayName: String,
    val kind: FarmFileKind,
    val mimeType: String = "",
    /** Absolute path under app filesDir/imports/ */
    val storedPath: String,
    val byteSize: Long = 0,
    val notes: String = "",
    /** Lightweight summary (e.g. placemark count for KMZ/KML). */
    val summary: String = "",
    val importedAt: Long = System.currentTimeMillis()
)

enum class ApiHttpMethod {
    GET, POST
}

@Entity(tableName = "api_feed_sources")
data class ApiFeedSource(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val baseUrl: String,
    val method: ApiHttpMethod = ApiHttpMethod.GET,
    /** Optional bearer / API key header value (stored on-device). */
    val authHeader: String = "",
    val notes: String = "",
    val enabled: Boolean = true,
    val lastStatus: String = "",
    val lastPulledAt: Long? = null,
    /** Truncated body from last successful pull for preview. */
    val lastPreview: String = "",
    val createdAt: Long = System.currentTimeMillis()
)
