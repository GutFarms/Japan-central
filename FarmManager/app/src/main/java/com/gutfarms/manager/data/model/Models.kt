package com.gutfarms.manager.data.model

import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

enum class AnimalType {
    CATTLE, CHICKEN, GOAT, PIG, SHEEP, OTHER
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
            AnimalType.CATTLE -> 283
            AnimalType.SHEEP -> 147
            AnimalType.GOAT -> 150
            AnimalType.PIG -> 114
            AnimalType.CHICKEN -> 21
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
