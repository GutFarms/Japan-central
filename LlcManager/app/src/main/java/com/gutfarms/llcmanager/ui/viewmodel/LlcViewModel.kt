package com.gutfarms.llcmanager.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.data.model.Llc
import com.gutfarms.llcmanager.data.model.LlcSummary
import com.gutfarms.llcmanager.data.repository.LlcRepository
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

class LlcViewModel(private val repository: LlcRepository) : ViewModel() {

    val summaries: StateFlow<List<LlcSummary>> = repository.observeSummaries()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    private val selectedLlcId = MutableStateFlow<Long?>(null)

    @OptIn(ExperimentalCoroutinesApi::class)
    val selectedLlc: StateFlow<Llc?> = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(null) else repository.observeLlc(id)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    @OptIn(ExperimentalCoroutinesApi::class)
    val deductions: StateFlow<List<Deduction>> = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(emptyList()) else repository.observeDeductions(id)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    @OptIn(ExperimentalCoroutinesApi::class)
    val inventory: StateFlow<List<InventoryItem>> = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(emptyList()) else repository.observeInventory(id)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun selectLlc(id: Long?) {
        selectedLlcId.value = id
    }

    fun saveLlc(
        id: Long = 0,
        name: String,
        ein: String,
        state: String,
        notes: String,
        onDone: (Long) -> Unit = {}
    ) {
        viewModelScope.launch {
            val savedId = repository.saveLlc(
                Llc(
                    id = id,
                    name = name.trim(),
                    ein = ein.trim(),
                    state = state.trim(),
                    notes = notes.trim()
                )
            )
            onDone(if (id == 0L) savedId else id)
        }
    }

    fun deleteLlc(id: Long, onDone: () -> Unit = {}) {
        viewModelScope.launch {
            repository.deleteLlc(id)
            if (selectedLlcId.value == id) selectedLlcId.value = null
            onDone()
        }
    }

    fun addDeduction(
        llcId: Long,
        name: String,
        category: String,
        amount: Double,
        notes: String,
        onDone: () -> Unit = {}
    ) {
        viewModelScope.launch {
            repository.saveDeduction(
                Deduction(
                    llcId = llcId,
                    name = name.trim(),
                    category = category.trim().ifBlank { "General" },
                    amount = amount,
                    notes = notes.trim()
                )
            )
            onDone()
        }
    }

    fun removeDeduction(id: Long) {
        viewModelScope.launch { repository.deleteDeduction(id) }
    }

    fun saveInventoryItem(
        id: Long = 0,
        llcId: Long,
        name: String,
        sku: String,
        quantity: Double,
        unit: String,
        unitCost: Double,
        location: String,
        notes: String,
        onDone: () -> Unit = {}
    ) {
        viewModelScope.launch {
            repository.saveInventoryItem(
                InventoryItem(
                    id = id,
                    llcId = llcId,
                    name = name.trim(),
                    sku = sku.trim(),
                    quantity = quantity,
                    unit = unit.trim().ifBlank { "ea" },
                    unitCost = unitCost,
                    location = location.trim(),
                    notes = notes.trim(),
                    updatedAt = System.currentTimeMillis()
                )
            )
            onDone()
        }
    }

    fun removeInventoryItem(id: Long) {
        viewModelScope.launch { repository.deleteInventoryItem(id) }
    }

    fun adjustInventory(item: InventoryItem, delta: Double) {
        viewModelScope.launch { repository.adjustInventoryQuantity(item, delta) }
    }
}

class LlcViewModelFactory(
    private val repository: LlcRepository
) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        if (modelClass.isAssignableFrom(LlcViewModel::class.java)) {
            return LlcViewModel(repository) as T
        }
        throw IllegalArgumentException("Unknown ViewModel: ${modelClass.name}")
    }
}
