package com.gutfarms.manager.data.repository

import com.gutfarms.manager.data.dao.AnimalArrivalDao
import com.gutfarms.manager.data.dao.AnimalDao
import com.gutfarms.manager.data.dao.BreedingScheduleDao
import com.gutfarms.manager.data.dao.FarmContactDao
import com.gutfarms.manager.data.dao.FarmProfileDao
import com.gutfarms.manager.data.dao.FeedingScheduleDao
import com.gutfarms.manager.data.dao.HealthRecordDao
import com.gutfarms.manager.data.dao.InventoryItemDao
import com.gutfarms.manager.data.dao.JournalEntryDao
import com.gutfarms.manager.data.dao.TransactionDao
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrival
import com.gutfarms.manager.data.model.AnimalArrivalWithGroup
import com.gutfarms.manager.data.model.BreedingSchedule
import com.gutfarms.manager.data.model.BreedingScheduleWithAnimal
import com.gutfarms.manager.data.model.FarmContact
import com.gutfarms.manager.data.model.FarmProfile
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedingSchedule
import com.gutfarms.manager.data.model.FeedingScheduleWithAnimal
import com.gutfarms.manager.data.model.HealthRecord
import com.gutfarms.manager.data.model.InventoryItem
import com.gutfarms.manager.data.model.JournalEntry
import com.gutfarms.manager.data.model.ProfitSummary
import com.gutfarms.manager.data.model.TransactionType
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map

class FarmRepository(
    private val animalDao: AnimalDao,
    private val feedingScheduleDao: FeedingScheduleDao,
    private val breedingScheduleDao: BreedingScheduleDao,
    private val animalArrivalDao: AnimalArrivalDao,
    private val transactionDao: TransactionDao,
    private val farmProfileDao: FarmProfileDao,
    private val healthRecordDao: HealthRecordDao,
    private val inventoryItemDao: InventoryItemDao,
    private val journalEntryDao: JournalEntryDao,
    private val farmContactDao: FarmContactDao
) {
    val farmProfile: Flow<FarmProfile> = farmProfileDao.observe().map { profile ->
        profile ?: FarmProfile(farmName = "Gut Farms")
    }

    val farmName: Flow<String> = farmProfile.map { profile ->
        profile.farmName.takeIf { it.isNotBlank() } ?: "Gut Farms"
    }

    val animals: Flow<List<Animal>> = animalDao.observeAll()
    val schedules: Flow<List<FeedingSchedule>> = feedingScheduleDao.observeAll()
    val breedingSchedules: Flow<List<BreedingSchedule>> = breedingScheduleDao.observeAll()
    val arrivals: Flow<List<AnimalArrival>> = animalArrivalDao.observeAll()
    val transactions: Flow<List<FarmTransaction>> = transactionDao.observeAll()
    val healthRecords: Flow<List<HealthRecord>> = healthRecordDao.observeAll()
    val inventoryItems: Flow<List<InventoryItem>> = inventoryItemDao.observeAll()
    val journalEntries: Flow<List<JournalEntry>> = journalEntryDao.observeAll()
    val contacts: Flow<List<FarmContact>> = farmContactDao.observeAll()

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
        val existing = farmProfileDao.get() ?: FarmProfile()
        val trimmed = name.trim().ifBlank { "Gut Farms" }
        farmProfileDao.upsert(existing.copy(farmName = trimmed))
    }

    suspend fun saveFarmProfile(profile: FarmProfile) {
        farmProfileDao.upsert(
            profile.copy(
                id = 1,
                farmName = profile.farmName.trim().ifBlank { "Gut Farms" }
            )
        )
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

    suspend fun saveHealthRecord(record: HealthRecord) = healthRecordDao.upsert(record)
    suspend fun deleteHealthRecord(record: HealthRecord) = healthRecordDao.delete(record)

    suspend fun saveInventoryItem(item: InventoryItem) = inventoryItemDao.upsert(item)
    suspend fun deleteInventoryItem(item: InventoryItem) = inventoryItemDao.delete(item)

    suspend fun saveJournalEntry(entry: JournalEntry) = journalEntryDao.upsert(entry)
    suspend fun deleteJournalEntry(entry: JournalEntry) = journalEntryDao.delete(entry)

    suspend fun saveContact(contact: FarmContact) = farmContactDao.upsert(contact)
    suspend fun deleteContact(contact: FarmContact) = farmContactDao.delete(contact)
}
