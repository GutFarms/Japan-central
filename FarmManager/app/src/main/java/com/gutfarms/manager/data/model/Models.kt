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

enum class FeedUnit {
    KG, LB, BAG, SCOOP, BALE, GALLON, BALE_HAY;

    val label: String
        get() = when (this) {
            KG -> "kg"
            LB -> "lb"
            BAG -> "bags"
            SCOOP -> "scoops"
            BALE -> "bales"
            GALLON -> "gallons"
            BALE_HAY -> "hay bales"
        }
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
    /** Quantity of feed given each feeding. */
    val feedQuantity: Double,
    val quantityUnit: FeedUnit = FeedUnit.KG,
    val costPerUnit: Double = 0.0,
    /** How many animals this ration covers. */
    val animalsFed: Int = 1,
    /** Remaining stock of this feed on hand. */
    val stockOnHand: Double = 0.0,
    val frequency: FeedFrequency,
    val timeOfDay: String,
    val notes: String = "",
    val active: Boolean = true
) {
    val quantityLabel: String
        get() = "${trimQty(feedQuantity)} ${quantityUnit.label}"

    val stockLabel: String
        get() = "${trimQty(stockOnHand)} ${quantityUnit.label} on hand"

    val perHeadLabel: String
        get() = if (animalsFed > 0) {
            "${trimQty(feedQuantity / animalsFed)} ${quantityUnit.label}/head"
        } else {
            quantityLabel
        }

    val dailyCost: Double
        get() = when (frequency) {
            FeedFrequency.DAILY -> feedQuantity * costPerUnit
            FeedFrequency.TWICE_DAILY -> feedQuantity * costPerUnit * 2
            FeedFrequency.WEEKLY -> (feedQuantity * costPerUnit) / 7.0
            FeedFrequency.CUSTOM -> feedQuantity * costPerUnit
        }

    val monthlyCost: Double
        get() = dailyCost * 30.0

    companion object {
        fun trimQty(value: Double): String =
            if (value == value.toLong().toDouble()) value.toLong().toString()
            else String.format("%.2f", value)
    }
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
    val farmName: String = "Gut Farms",
    val location: String = "",
    val ownerName: String = "",
    val phone: String = "",
    val notes: String = ""
)

enum class HealthRecordType {
    VACCINATION, TREATMENT, CHECKUP, INJURY, ILLNESS, OTHER
}

@Entity(
    tableName = "health_records",
    foreignKeys = [
        ForeignKey(
            entity = Animal::class,
            parentColumns = ["id"],
            childColumns = ["animalId"],
            onDelete = ForeignKey.SET_NULL
        )
    ],
    indices = [Index("animalId"), Index("dateMillis")]
)
data class HealthRecord(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val animalId: Long? = null,
    val animalLabel: String = "",
    val type: HealthRecordType = HealthRecordType.CHECKUP,
    val title: String,
    val dateMillis: Long = System.currentTimeMillis(),
    val provider: String = "",
    val cost: Double = 0.0,
    val notes: String = ""
)

enum class InventoryCategory {
    FEED, MEDICINE, EQUIPMENT, SUPPLIES, FUEL, OTHER
}

@Entity(tableName = "inventory_items")
data class InventoryItem(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val category: InventoryCategory = InventoryCategory.SUPPLIES,
    val quantity: Double = 0.0,
    val unit: String = "units",
    val reorderLevel: Double = 0.0,
    val unitCost: Double = 0.0,
    val location: String = "",
    val notes: String = "",
    val updatedAt: Long = System.currentTimeMillis()
) {
    val needsReorder: Boolean
        get() = reorderLevel > 0 && quantity <= reorderLevel
}

enum class JournalCategory {
    DAILY_LOG, WEATHER, PASTURE, OBSERVATION, TASK, OTHER
}

@Entity(tableName = "journal_entries")
data class JournalEntry(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val title: String,
    val category: JournalCategory = JournalCategory.DAILY_LOG,
    val body: String = "",
    val dateMillis: Long = System.currentTimeMillis(),
    val tags: String = ""
)

enum class ContactRole {
    VETERINARIAN, SUPPLIER, BUYER, WORKER, ADVISOR, OTHER
}

@Entity(tableName = "farm_contacts")
data class FarmContact(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val role: ContactRole = ContactRole.OTHER,
    val phone: String = "",
    val email: String = "",
    val organization: String = "",
    val notes: String = ""
)
