package com.gutfarms.manager.data.repository

import android.content.Context
import android.net.Uri
import com.gutfarms.manager.data.dao.AnimalArrivalDao
import com.gutfarms.manager.data.dao.AnimalDao
import com.gutfarms.manager.data.dao.ApiFeedSourceDao
import com.gutfarms.manager.data.dao.BreedingScheduleDao
import com.gutfarms.manager.data.dao.FarmImportFileDao
import com.gutfarms.manager.data.dao.FarmProfileDao
import com.gutfarms.manager.data.dao.FeedingScheduleDao
import com.gutfarms.manager.data.dao.TransactionDao
import com.gutfarms.manager.data.importing.ApiPullClient
import com.gutfarms.manager.data.importing.FarmFileImporter
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrival
import com.gutfarms.manager.data.model.AnimalArrivalWithGroup
import com.gutfarms.manager.data.model.ApiFeedSource
import com.gutfarms.manager.data.model.BreedingSchedule
import com.gutfarms.manager.data.model.BreedingScheduleWithAnimal
import com.gutfarms.manager.data.model.FarmImportFile
import com.gutfarms.manager.data.model.FarmProfile
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedingSchedule
import com.gutfarms.manager.data.model.FeedingScheduleWithAnimal
import com.gutfarms.manager.data.model.ProfitSummary
import com.gutfarms.manager.data.model.TransactionType
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.withContext

class FarmRepository(
    private val animalDao: AnimalDao,
    private val feedingScheduleDao: FeedingScheduleDao,
    private val breedingScheduleDao: BreedingScheduleDao,
    private val animalArrivalDao: AnimalArrivalDao,
    private val transactionDao: TransactionDao,
    private val farmProfileDao: FarmProfileDao,
    private val farmImportFileDao: FarmImportFileDao,
    private val apiFeedSourceDao: ApiFeedSourceDao
) {
    val farmName: Flow<String> = farmProfileDao.observe().map { profile ->
        profile?.farmName?.takeIf { it.isNotBlank() } ?: "Gut Farms"
    }

    val animals: Flow<List<Animal>> = animalDao.observeAll()
    val schedules: Flow<List<FeedingSchedule>> = feedingScheduleDao.observeAll()
    val breedingSchedules: Flow<List<BreedingSchedule>> = breedingScheduleDao.observeAll()
    val arrivals: Flow<List<AnimalArrival>> = animalArrivalDao.observeAll()
    val transactions: Flow<List<FarmTransaction>> = transactionDao.observeAll()
    val importFiles: Flow<List<FarmImportFile>> = farmImportFileDao.observeAll()
    val apiSources: Flow<List<ApiFeedSource>> = apiFeedSourceDao.observeAll()

    val schedulesWithAnimals: Flow<List<FeedingScheduleWithAnimal>> =
        combine(schedules, animals) { scheduleList, animalList ->
            val byId = animalList.associateBy { it.id }
            scheduleList.mapNotNull { schedule ->
                val animal = byId[schedule.animalId] ?: return@mapNotNull null
                FeedingScheduleWithAnimal(
                    schedule = schedule,
                    animalName = animal.name,
                    animalType = animal.type
                )
            }
        }

    val breedingWithAnimals: Flow<List<BreedingScheduleWithAnimal>> =
        combine(breedingSchedules, animals) { scheduleList, animalList ->
            val byId = animalList.associateBy { it.id }
            scheduleList.mapNotNull { schedule ->
                val animal = byId[schedule.animalId] ?: return@mapNotNull null
                BreedingScheduleWithAnimal(
                    schedule = schedule,
                    animalName = animal.name,
                    animalType = animal.type
                )
            }
        }

    val arrivalsWithGroups: Flow<List<AnimalArrivalWithGroup>> =
        combine(arrivals, animals) { arrivalList, animalList ->
            val byId = animalList.associateBy { it.id }
            arrivalList.map { arrival ->
                AnimalArrivalWithGroup(
                    arrival = arrival,
                    groupName = arrival.groupAnimalId?.let { byId[it]?.name }
                )
            }
        }

    val profitSummary: Flow<ProfitSummary> =
        combine(
            transactionDao.observeSum(TransactionType.INCOME),
            transactionDao.observeSum(TransactionType.EXPENSE),
            feedingScheduleDao.observeActive()
        ) { income, expenses, activeSchedules ->
            val projectedFeed = activeSchedules.sumOf { it.monthlyCost }
            val totalExpenses = expenses + projectedFeed
            val net = income - totalExpenses
            val margin = if (income > 0) (net / income) * 100.0 else 0.0
            ProfitSummary(
                totalIncome = income,
                totalExpenses = totalExpenses,
                projectedMonthlyFeedCost = projectedFeed,
                netProfit = net,
                marginPercent = margin
            )
        }

    suspend fun updateFarmName(name: String) {
        val trimmed = name.trim().ifBlank { "Gut Farms" }
        farmProfileDao.upsert(FarmProfile(id = 1, farmName = trimmed))
    }

    suspend fun saveAnimal(animal: Animal) = animalDao.upsert(animal)
    suspend fun deleteAnimal(animal: Animal) = animalDao.delete(animal)

    suspend fun saveSchedule(schedule: FeedingSchedule) = feedingScheduleDao.upsert(schedule)
    suspend fun deleteSchedule(schedule: FeedingSchedule) = feedingScheduleDao.delete(schedule)
    suspend fun toggleSchedule(schedule: FeedingSchedule) =
        feedingScheduleDao.update(schedule.copy(active = !schedule.active))

    suspend fun saveBreeding(schedule: BreedingSchedule) = breedingScheduleDao.upsert(schedule)
    suspend fun deleteBreeding(schedule: BreedingSchedule) = breedingScheduleDao.delete(schedule)
    suspend fun toggleBreeding(schedule: BreedingSchedule) =
        breedingScheduleDao.update(schedule.copy(active = !schedule.active))

    suspend fun saveArrival(arrival: AnimalArrival) = animalArrivalDao.upsert(arrival)
    suspend fun deleteArrival(arrival: AnimalArrival) = animalArrivalDao.delete(arrival)

    suspend fun saveTransaction(transaction: FarmTransaction) = transactionDao.upsert(transaction)
    suspend fun deleteTransaction(transaction: FarmTransaction) = transactionDao.delete(transaction)

    suspend fun importFile(context: Context, uri: Uri): Result<FarmImportFile> =
        withContext(Dispatchers.IO) {
            runCatching {
                val payload = FarmFileImporter.importUri(context, uri)
                val row = FarmImportFile(
                    displayName = payload.displayName,
                    kind = payload.kind,
                    mimeType = payload.mimeType,
                    storedPath = payload.storedPath,
                    byteSize = payload.byteSize,
                    summary = payload.summary
                )
                val id = farmImportFileDao.upsert(row)
                row.copy(id = id)
            }
        }

    suspend fun deleteImportFile(file: FarmImportFile) = withContext(Dispatchers.IO) {
        FarmFileImporter.deleteStored(file.storedPath)
        farmImportFileDao.delete(file)
    }

    suspend fun saveApiSource(source: ApiFeedSource) = apiFeedSourceDao.upsert(source)

    suspend fun deleteApiSource(source: ApiFeedSource) = apiFeedSourceDao.delete(source)

    suspend fun toggleApiSource(source: ApiFeedSource) =
        apiFeedSourceDao.update(source.copy(enabled = !source.enabled))

    suspend fun pullApiSource(source: ApiFeedSource): ApiFeedSource = withContext(Dispatchers.IO) {
        val result = ApiPullClient.pull(source)
        val updated = source.copy(
            lastStatus = result.statusLine,
            lastPulledAt = System.currentTimeMillis(),
            lastPreview = result.preview
        )
        apiFeedSourceDao.upsert(updated)
        updated
    }
}
