package com.gutfarms.llcmanager.data.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.Income
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.data.model.Llc
import kotlinx.coroutines.flow.Flow

@Dao
interface LlcDao {
    @Query("SELECT * FROM llcs ORDER BY name COLLATE NOCASE ASC")
    fun observeAll(): Flow<List<Llc>>

    @Query("SELECT * FROM llcs WHERE id = :id")
    fun observeById(id: Long): Flow<Llc?>

    @Query("SELECT * FROM llcs WHERE id = :id")
    suspend fun getById(id: Long): Llc?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(llc: Llc): Long

    @Update
    suspend fun update(llc: Llc)

    @Delete
    suspend fun delete(llc: Llc)

    @Query("DELETE FROM llcs WHERE id = :id")
    suspend fun deleteById(id: Long)
}

@Dao
interface DeductionDao {
    @Query("SELECT * FROM deductions WHERE llcId = :llcId ORDER BY dateEpochMs DESC, id DESC")
    fun observeForLlc(llcId: Long): Flow<List<Deduction>>

    @Query("SELECT * FROM deductions ORDER BY dateEpochMs DESC")
    fun observeAll(): Flow<List<Deduction>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(deduction: Deduction): Long

    @Delete
    suspend fun delete(deduction: Deduction)

    @Query("DELETE FROM deductions WHERE id = :id")
    suspend fun deleteById(id: Long)
}

@Dao
interface IncomeDao {
    @Query("SELECT * FROM incomes WHERE llcId = :llcId ORDER BY dateEpochMs DESC, id DESC")
    fun observeForLlc(llcId: Long): Flow<List<Income>>

    @Query("SELECT * FROM incomes ORDER BY dateEpochMs DESC")
    fun observeAll(): Flow<List<Income>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(income: Income): Long

    @Delete
    suspend fun delete(income: Income)

    @Query("DELETE FROM incomes WHERE id = :id")
    suspend fun deleteById(id: Long)
}

@Dao
interface InventoryDao {
    @Query("SELECT * FROM inventory_items WHERE llcId = :llcId ORDER BY name COLLATE NOCASE ASC")
    fun observeForLlc(llcId: Long): Flow<List<InventoryItem>>

    @Query("SELECT * FROM inventory_items ORDER BY name COLLATE NOCASE ASC")
    fun observeAll(): Flow<List<InventoryItem>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(item: InventoryItem): Long

    @Update
    suspend fun update(item: InventoryItem)

    @Delete
    suspend fun delete(item: InventoryItem)

    @Query("DELETE FROM inventory_items WHERE id = :id")
    suspend fun deleteById(id: Long)
}
