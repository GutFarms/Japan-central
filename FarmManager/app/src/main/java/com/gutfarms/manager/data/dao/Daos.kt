package com.gutfarms.manager.data.dao

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrival
import com.gutfarms.manager.data.model.ApiFeedSource
import com.gutfarms.manager.data.model.BreedingSchedule
import com.gutfarms.manager.data.model.FarmImportFile
import com.gutfarms.manager.data.model.FarmProfile
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedingSchedule
import com.gutfarms.manager.data.model.RegistrationStatus
import com.gutfarms.manager.data.model.TransactionType
import kotlinx.coroutines.flow.Flow

@Dao
interface FarmProfileDao {
    @Query("SELECT * FROM farm_profile WHERE id = 1")
    fun observe(): Flow<FarmProfile?>

    @Query("SELECT * FROM farm_profile WHERE id = 1")
    suspend fun get(): FarmProfile?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(profile: FarmProfile)
}

@Dao
interface AnimalDao {
    @Query("SELECT * FROM animals ORDER BY name ASC")
    fun observeAll(): Flow<List<Animal>>

    @Query("SELECT * FROM animals WHERE id = :id")
    suspend fun getById(id: Long): Animal?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(animal: Animal): Long

    @Delete
    suspend fun delete(animal: Animal)

    @Query("SELECT COUNT(*) FROM animals")
    fun observeCount(): Flow<Int>
}

@Dao
interface FeedingScheduleDao {
    @Query("SELECT * FROM feeding_schedules ORDER BY timeOfDay ASC")
    fun observeAll(): Flow<List<FeedingSchedule>>

    @Query("SELECT * FROM feeding_schedules WHERE animalId = :animalId ORDER BY timeOfDay ASC")
    fun observeForAnimal(animalId: Long): Flow<List<FeedingSchedule>>

    @Query("SELECT * FROM feeding_schedules WHERE active = 1 ORDER BY timeOfDay ASC")
    fun observeActive(): Flow<List<FeedingSchedule>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(schedule: FeedingSchedule): Long

    @Update
    suspend fun update(schedule: FeedingSchedule)

    @Delete
    suspend fun delete(schedule: FeedingSchedule)
}

@Dao
interface BreedingScheduleDao {
    @Query("SELECT * FROM breeding_schedules ORDER BY expectedDueDateMillis ASC")
    fun observeAll(): Flow<List<BreedingSchedule>>

    @Query("SELECT * FROM breeding_schedules WHERE animalId = :animalId ORDER BY expectedDueDateMillis ASC")
    fun observeForAnimal(animalId: Long): Flow<List<BreedingSchedule>>

    @Query("SELECT * FROM breeding_schedules WHERE active = 1 ORDER BY expectedDueDateMillis ASC")
    fun observeActive(): Flow<List<BreedingSchedule>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(schedule: BreedingSchedule): Long

    @Update
    suspend fun update(schedule: BreedingSchedule)

    @Delete
    suspend fun delete(schedule: BreedingSchedule)
}

@Dao
interface AnimalArrivalDao {
    @Query("SELECT * FROM animal_arrivals ORDER BY eventDateMillis DESC")
    fun observeAll(): Flow<List<AnimalArrival>>

    @Query("SELECT * FROM animal_arrivals WHERE registrationStatus = :status ORDER BY eventDateMillis DESC")
    fun observeByRegistration(status: RegistrationStatus): Flow<List<AnimalArrival>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(arrival: AnimalArrival): Long

    @Delete
    suspend fun delete(arrival: AnimalArrival)
}

@Dao
interface TransactionDao {
    @Query("SELECT * FROM transactions ORDER BY dateMillis DESC")
    fun observeAll(): Flow<List<FarmTransaction>>

    @Query("SELECT * FROM transactions WHERE type = :type ORDER BY dateMillis DESC")
    fun observeByType(type: TransactionType): Flow<List<FarmTransaction>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(transaction: FarmTransaction): Long

    @Delete
    suspend fun delete(transaction: FarmTransaction)

    @Query("SELECT COALESCE(SUM(amount), 0) FROM transactions WHERE type = :type")
    fun observeSum(type: TransactionType): Flow<Double>
}

@Dao
interface FarmImportFileDao {
    @Query("SELECT * FROM farm_import_files ORDER BY importedAt DESC")
    fun observeAll(): Flow<List<FarmImportFile>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(file: FarmImportFile): Long

    @Delete
    suspend fun delete(file: FarmImportFile)

    @Query("SELECT * FROM farm_import_files WHERE id = :id")
    suspend fun getById(id: Long): FarmImportFile?
}

@Dao
interface ApiFeedSourceDao {
    @Query("SELECT * FROM api_feed_sources ORDER BY name ASC")
    fun observeAll(): Flow<List<ApiFeedSource>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(source: ApiFeedSource): Long

    @Update
    suspend fun update(source: ApiFeedSource)

    @Delete
    suspend fun delete(source: ApiFeedSource)

    @Query("SELECT * FROM api_feed_sources WHERE id = :id")
    suspend fun getById(id: Long): ApiFeedSource?
}
