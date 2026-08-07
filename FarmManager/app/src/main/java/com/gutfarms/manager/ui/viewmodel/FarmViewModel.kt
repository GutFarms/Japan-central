package com.gutfarms.manager.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.gutfarms.manager.data.model.Animal
import com.gutfarms.manager.data.model.AnimalArrival
import com.gutfarms.manager.data.model.AnimalArrivalWithGroup
import com.gutfarms.manager.data.model.BreedingSchedule
import com.gutfarms.manager.data.model.BreedingScheduleWithAnimal
import com.gutfarms.manager.data.model.FarmTransaction
import com.gutfarms.manager.data.model.FeedingSchedule
import com.gutfarms.manager.data.model.FeedingScheduleWithAnimal
import com.gutfarms.manager.data.model.ProfitSummary
import com.gutfarms.manager.data.repository.FarmRepository
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class FarmViewModel(private val repository: FarmRepository) : ViewModel() {
    val animals: StateFlow<List<Animal>> = repository.animals.stateIn(
        viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList()
    )

    val schedules: StateFlow<List<FeedingScheduleWithAnimal>> = repository.schedulesWithAnimals.stateIn(
        viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList()
    )

    val breedingSchedules: StateFlow<List<BreedingScheduleWithAnimal>> =
        repository.breedingWithAnimals.stateIn(
            viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList()
        )

    val arrivals: StateFlow<List<AnimalArrivalWithGroup>> = repository.arrivalsWithGroups.stateIn(
        viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList()
    )

    val transactions: StateFlow<List<FarmTransaction>> = repository.transactions.stateIn(
        viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList()
    )

    val profitSummary: StateFlow<ProfitSummary> = repository.profitSummary.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        ProfitSummary(0.0, 0.0, 0.0, 0.0, 0.0)
    )

    fun saveAnimal(animal: Animal) = viewModelScope.launch { repository.saveAnimal(animal) }
    fun deleteAnimal(animal: Animal) = viewModelScope.launch { repository.deleteAnimal(animal) }

    fun saveSchedule(schedule: FeedingSchedule) =
        viewModelScope.launch { repository.saveSchedule(schedule) }

    fun deleteSchedule(schedule: FeedingSchedule) =
        viewModelScope.launch { repository.deleteSchedule(schedule) }

    fun toggleSchedule(schedule: FeedingSchedule) =
        viewModelScope.launch { repository.toggleSchedule(schedule) }

    fun saveBreeding(schedule: BreedingSchedule) =
        viewModelScope.launch { repository.saveBreeding(schedule) }

    fun deleteBreeding(schedule: BreedingSchedule) =
        viewModelScope.launch { repository.deleteBreeding(schedule) }

    fun toggleBreeding(schedule: BreedingSchedule) =
        viewModelScope.launch { repository.toggleBreeding(schedule) }

    fun saveArrival(arrival: AnimalArrival) =
        viewModelScope.launch { repository.saveArrival(arrival) }

    fun deleteArrival(arrival: AnimalArrival) =
        viewModelScope.launch { repository.deleteArrival(arrival) }

    fun saveTransaction(transaction: FarmTransaction) =
        viewModelScope.launch { repository.saveTransaction(transaction) }

    fun deleteTransaction(transaction: FarmTransaction) =
        viewModelScope.launch { repository.deleteTransaction(transaction) }
}

class FarmViewModelFactory(
    private val repository: FarmRepository
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(FarmViewModel::class.java)) {
            return FarmViewModel(repository) as T
        }
        throw IllegalArgumentException("Unknown ViewModel class")
    }
}
