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
    val location: String = "",
    val notes: String = "",
    val updatedAt: Long = System.currentTimeMillis(),
    val createdAt: Long = System.currentTimeMillis()
) {
    val totalValue: Double
        get() = quantity * unitCost
}

data class LlcSummary(
    val llc: Llc,
    val deductionCount: Int,
    val deductionTotal: Double,
    val inventoryCount: Int,
    val inventoryValue: Double
)
