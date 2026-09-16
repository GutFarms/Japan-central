package com.gutfarms.llcmanager.data.model

import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(tableName = "llcs")
data class Llc(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val name: String,
    val ein: String = "",
    val state: String = "",
    val notes: String = "",
    val createdAt: Long = System.currentTimeMillis()
)

@Entity(
    tableName = "deductions",
    foreignKeys = [
        ForeignKey(
            entity = Llc::class,
            parentColumns = ["id"],
            childColumns = ["llcId"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("llcId")]
)
data class Deduction(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val llcId: Long,
    val name: String,
    val category: String = "General",
    val vendor: String = "",
    val amount: Double,
    val dateEpochMs: Long = System.currentTimeMillis(),
    val notes: String = "",
    val createdAt: Long = System.currentTimeMillis()
)

@Entity(
    tableName = "incomes",
    foreignKeys = [
        ForeignKey(
            entity = Llc::class,
            parentColumns = ["id"],
            childColumns = ["llcId"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("llcId")]
)
data class Income(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val llcId: Long,
    val name: String,
    val category: String = "Sales",
    val source: String = "",
    val amount: Double,
    val dateEpochMs: Long = System.currentTimeMillis(),
    val notes: String = "",
    val createdAt: Long = System.currentTimeMillis()
)

@Entity(
    tableName = "inventory_items",
    foreignKeys = [
        ForeignKey(
            entity = Llc::class,
            parentColumns = ["id"],
            childColumns = ["llcId"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("llcId")]
)
data class InventoryItem(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val llcId: Long,
    val name: String,
    val sku: String = "",
    val quantity: Double = 0.0,
    val unit: String = "ea",
    val unitCost: Double = 0.0,
    val reorderLevel: Double = 0.0,
    val location: String = "",
    val notes: String = "",
    val updatedAt: Long = System.currentTimeMillis(),
    val createdAt: Long = System.currentTimeMillis()
) {
    val totalValue: Double
        get() = quantity * unitCost

    val isLowStock: Boolean
        get() = reorderLevel > 0.0 && quantity <= reorderLevel
}

enum class PayType {
    HOURLY,
    SALARY
}

enum class EmploymentStatus {
    ACTIVE,
    ON_LEAVE,
    TERMINATED
}

@Entity(
    tableName = "employees",
    foreignKeys = [
        ForeignKey(
            entity = Llc::class,
            parentColumns = ["id"],
            childColumns = ["llcId"],
            onDelete = ForeignKey.CASCADE
        )
    ],
    indices = [Index("llcId")]
)
data class Employee(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val llcId: Long,
    val name: String,
    val role: String = "",
    val department: String = "",
    val email: String = "",
    val phone: String = "",
    val employeeCode: String = "",
    val payType: PayType = PayType.HOURLY,
    val payRate: Double = 0.0,
    val status: EmploymentStatus = EmploymentStatus.ACTIVE,
    val hireDateEpochMs: Long = System.currentTimeMillis(),
    val notes: String = "",
    val createdAt: Long = System.currentTimeMillis()
)

data class LlcSummary(
    val llc: Llc,
    val deductionCount: Int,
    val deductionTotal: Double,
    val incomeCount: Int,
    val incomeTotal: Double,
    val inventoryCount: Int,
    val inventoryValue: Double,
    val lowStockCount: Int,
    val employeeCount: Int,
    val activeEmployeeCount: Int
) {
    val net: Double
        get() = incomeTotal - deductionTotal
}

object DeductionCategories {
    val defaults = listOf(
        "General", "Office", "Travel", "Supplies", "Utilities",
        "Insurance", "Professional", "Vehicle", "Marketing", "Other"
    )
}

object IncomeCategories {
    val defaults = listOf("Sales", "Services", "Interest", "Other")
}

object EmployeeRoles {
    val defaults = listOf(
        "Owner", "Manager", "Admin", "Bookkeeper",
        "Sales", "Operations", "Labor", "Contractor", "Other"
    )
}
