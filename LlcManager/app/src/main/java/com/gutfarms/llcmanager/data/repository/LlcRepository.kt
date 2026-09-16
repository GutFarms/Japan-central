package com.gutfarms.llcmanager.data.repository

import com.gutfarms.llcmanager.data.dao.DeductionDao
import com.gutfarms.llcmanager.data.dao.InventoryDao
import com.gutfarms.llcmanager.data.dao.LlcDao
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.data.model.Llc
import com.gutfarms.llcmanager.data.model.LlcSummary
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine

class LlcRepository(
    private val llcDao: LlcDao,
    private val deductionDao: DeductionDao,
    private val inventoryDao: InventoryDao
) {
    fun observeLlcs(): Flow<List<Llc>> = llcDao.observeAll()

    fun observeLlc(id: Long): Flow<Llc?> = llcDao.observeById(id)

    fun observeDeductions(llcId: Long): Flow<List<Deduction>> = deductionDao.observeForLlc(llcId)

    fun observeInventory(llcId: Long): Flow<List<InventoryItem>> = inventoryDao.observeForLlc(llcId)

    fun observeSummaries(): Flow<List<LlcSummary>> {
        return combine(
            llcDao.observeAll(),
            deductionDao.observeAll(),
            inventoryDao.observeAll()
        ) { llcs, deductions, inventory ->
            llcs.map { llc ->
                val llcDeductions = deductions.filter { it.llcId == llc.id }
                val llcInventory = inventory.filter { it.llcId == llc.id }
                LlcSummary(
                    llc = llc,
                    deductionCount = llcDeductions.size,
                    deductionTotal = llcDeductions.sumOf { it.amount },
                    inventoryCount = llcInventory.size,
                    inventoryValue = llcInventory.sumOf { it.totalValue }
                )
            }
        }
    }

    suspend fun saveLlc(llc: Llc): Long = llcDao.upsert(llc)

    suspend fun deleteLlc(id: Long) = llcDao.deleteById(id)

    suspend fun saveDeduction(deduction: Deduction): Long = deductionDao.upsert(deduction)

    suspend fun deleteDeduction(id: Long) = deductionDao.deleteById(id)

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
}
