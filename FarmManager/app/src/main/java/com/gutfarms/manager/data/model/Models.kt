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

data class ProfitSummary(
    val totalIncome: Double,
    val totalExpenses: Double,
    val projectedMonthlyFeedCost: Double,
    val netProfit: Double,
    val marginPercent: Double
)
