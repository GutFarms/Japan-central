package com.gutfarms.llcmanager.data.repository

import com.gutfarms.llcmanager.data.dao.DeductionDao
import com.gutfarms.llcmanager.data.dao.IncomeDao
import com.gutfarms.llcmanager.data.dao.InventoryDao
import com.gutfarms.llcmanager.data.dao.LlcDao
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.Income
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.data.model.Llc
import com.gutfarms.llcmanager.data.model.LlcSummary
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

class LlcRepository(
    private val llcDao: LlcDao,
    private val deductionDao: DeductionDao,
    private val incomeDao: IncomeDao,
    private val inventoryDao: InventoryDao
) {
    fun observeLlcs(): Flow<List<Llc>> = llcDao.observeAll()

    fun observeLlc(id: Long): Flow<Llc?> = llcDao.observeById(id)

    fun observeDeductions(llcId: Long): Flow<List<Deduction>> = deductionDao.observeForLlc(llcId)

    fun observeIncomes(llcId: Long): Flow<List<Income>> = incomeDao.observeForLlc(llcId)

    fun observeInventory(llcId: Long): Flow<List<InventoryItem>> = inventoryDao.observeForLlc(llcId)

    fun observeSummaries(): Flow<List<LlcSummary>> {
        return combine(
            llcDao.observeAll(),
            deductionDao.observeAll(),
            incomeDao.observeAll(),
            inventoryDao.observeAll()
        ) { llcs, deductions, incomes, inventory ->
            llcs.map { llc ->
                val llcDeductions = deductions.filter { it.llcId == llc.id }
                val llcIncomes = incomes.filter { it.llcId == llc.id }
                val llcInventory = inventory.filter { it.llcId == llc.id }
                LlcSummary(
                    llc = llc,
                    deductionCount = llcDeductions.size,
                    deductionTotal = llcDeductions.sumOf { it.amount },
                    incomeCount = llcIncomes.size,
                    incomeTotal = llcIncomes.sumOf { it.amount },
                    inventoryCount = llcInventory.size,
                    inventoryValue = llcInventory.sumOf { it.totalValue },
                    lowStockCount = llcInventory.count { it.isLowStock }
                )
            }
        }
    }

    suspend fun saveLlc(llc: Llc): Long = llcDao.upsert(llc)

    suspend fun deleteLlc(id: Long) = llcDao.deleteById(id)

    suspend fun saveDeduction(deduction: Deduction): Long = deductionDao.upsert(deduction)

    suspend fun deleteDeduction(id: Long) = deductionDao.deleteById(id)

    suspend fun saveIncome(income: Income): Long = incomeDao.upsert(income)

    suspend fun deleteIncome(id: Long) = incomeDao.deleteById(id)

    suspend fun saveInventoryItem(item: InventoryItem): Long = inventoryDao.upsert(item)

    suspend fun deleteInventoryItem(id: Long) = inventoryDao.deleteById(id)

    suspend fun adjustInventoryQuantity(item: InventoryItem, delta: Double) {
        inventoryDao.update(
            item.copy(
                quantity = (item.quantity + delta).coerceAtLeast(0.0),
                updatedAt = System.currentTimeMillis()
            )
        )
    }

    suspend fun moveInventoryItem(item: InventoryItem, targetLlcId: Long) {
        if (item.llcId == targetLlcId) return
        inventoryDao.update(
            item.copy(llcId = targetLlcId, updatedAt = System.currentTimeMillis())
        )
    }

    fun formatReport(
        llc: Llc,
        deductions: List<Deduction>,
        incomes: List<Income>,
        inventory: List<InventoryItem>
    ): String {
        val dateFmt = SimpleDateFormat("yyyy-MM-dd", Locale.US)
        val money = { v: Double -> String.format(Locale.US, "$%,.2f", v) }
        val sb = StringBuilder()
        sb.appendLine("LLC Manager Report")
        sb.appendLine(llc.name)
        if (llc.ein.isNotBlank()) sb.appendLine("EIN: ${llc.ein}")
        if (llc.state.isNotBlank()) sb.appendLine("State: ${llc.state}")
        sb.appendLine("Generated: ${dateFmt.format(Date())}")
        sb.appendLine()

        val incomeTotal = incomes.sumOf { it.amount }
        val deductionTotal = deductions.sumOf { it.amount }
        sb.appendLine("Income: ${money(incomeTotal)} (${incomes.size})")
        sb.appendLine("Deductions: ${money(deductionTotal)} (${deductions.size})")
        sb.appendLine("Net: ${money(incomeTotal - deductionTotal)}")
        sb.appendLine("Inventory value: ${money(inventory.sumOf { it.totalValue })}")
        sb.appendLine()

        sb.appendLine("=== Income ===")
        if (incomes.isEmpty()) sb.appendLine("(none)")
        incomes.forEach {
            sb.appendLine(
                "${dateFmt.format(Date(it.dateEpochMs))} | ${it.category} | ${it.name} | ${money(it.amount)}"
            )
        }
        sb.appendLine()
        sb.appendLine("=== Deductions ===")
        if (deductions.isEmpty()) sb.appendLine("(none)")
        deductions.forEach {
            sb.appendLine(
                "${dateFmt.format(Date(it.dateEpochMs))} | ${it.category} | ${it.name} | ${money(it.amount)}"
            )
        }
        sb.appendLine()
        sb.appendLine("=== Inventory ===")
        if (inventory.isEmpty()) sb.appendLine("(none)")
        inventory.forEach {
            val low = if (it.isLowStock) " [LOW STOCK]" else ""
            sb.appendLine(
                "${it.name} | ${it.quantity} ${it.unit} | ${money(it.totalValue)}$low"
            )
        }
        return sb.toString()
    }
}
