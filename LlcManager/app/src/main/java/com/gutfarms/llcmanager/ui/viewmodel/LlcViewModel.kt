package com.gutfarms.llcmanager.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.gutfarms.llcmanager.data.model.Deduction
import com.gutfarms.llcmanager.data.model.Employee
import com.gutfarms.llcmanager.data.model.EmploymentStatus
import com.gutfarms.llcmanager.data.model.Income
import com.gutfarms.llcmanager.data.model.InventoryItem
import com.gutfarms.llcmanager.data.model.Llc
import com.gutfarms.llcmanager.data.model.LlcSummary
import com.gutfarms.llcmanager.data.model.PayType
import com.gutfarms.llcmanager.data.repository.LlcRepository
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.util.Calendar

class LlcViewModel(private val repository: LlcRepository) : ViewModel() {

    val searchQuery = MutableStateFlow("")
    val yearFilter = MutableStateFlow(Calendar.getInstance().get(Calendar.YEAR))

    val summaries: StateFlow<List<LlcSummary>> = combine(
        repository.observeSummaries(),
        searchQuery
    ) { list, query ->
        val q = query.trim()
        if (q.isEmpty()) list
        else list.filter {
            it.llc.name.contains(q, ignoreCase = true) ||
                it.llc.ein.contains(q, ignoreCase = true) ||
                it.llc.state.contains(q, ignoreCase = true) ||
                it.llc.notes.contains(q, ignoreCase = true)
        }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val allSummaries: StateFlow<List<LlcSummary>> = repository.observeSummaries()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    private val selectedLlcId = MutableStateFlow<Long?>(null)

    @OptIn(ExperimentalCoroutinesApi::class)
    val selectedLlc: StateFlow<Llc?> = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(null) else repository.observeLlc(id)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), null)

    @OptIn(ExperimentalCoroutinesApi::class)
    private val rawDeductions = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(emptyList()) else repository.observeDeductions(id)
        }

    @OptIn(ExperimentalCoroutinesApi::class)
    private val rawIncomes = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(emptyList()) else repository.observeIncomes(id)
        }

    @OptIn(ExperimentalCoroutinesApi::class)
    val inventory: StateFlow<List<InventoryItem>> = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(emptyList()) else repository.observeInventory(id)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    @OptIn(ExperimentalCoroutinesApi::class)
    val employees: StateFlow<List<Employee>> = selectedLlcId
        .flatMapLatest { id ->
            if (id == null) flowOf(emptyList()) else repository.observeEmployees(id)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val deductions: StateFlow<List<Deduction>> = combine(rawDeductions, yearFilter) { list, year ->
        list.filter { yearOf(it.dateEpochMs) == year }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val incomes: StateFlow<List<Income>> = combine(rawIncomes, yearFilter) { list, year ->
        list.filter { yearOf(it.dateEpochMs) == year }
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    val availableYears: StateFlow<List<Int>> = combine(rawDeductions, rawIncomes) { d, i ->
        val years = (d.map { yearOf(it.dateEpochMs) } + i.map { yearOf(it.dateEpochMs) } +
            Calendar.getInstance().get(Calendar.YEAR)).distinct().sortedDescending()
        years
    }.stateIn(
        viewModelScope,
        SharingStarted.WhileSubscribed(5_000),
        listOf(Calendar.getInstance().get(Calendar.YEAR))
    )

    val llcsForMove: StateFlow<List<Llc>> = repository.observeLlcs()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    fun selectLlc(id: Long?) {
        selectedLlcId.value = id
    }

    fun setSearchQuery(query: String) {
        searchQuery.value = query
    }

    fun setYearFilter(year: Int) {
        yearFilter.value = year
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

    fun saveDeduction(
        id: Long = 0,
        llcId: Long,
        name: String,
        category: String,
        vendor: String,
        amount: Double,
        dateEpochMs: Long,
        notes: String,
        onDone: () -> Unit = {}
    ) {
        viewModelScope.launch {
            repository.saveDeduction(
                Deduction(
                    id = id,
                    llcId = llcId,
                    name = name.trim(),
                    category = category.trim().ifBlank { "General" },
                    vendor = vendor.trim(),
                    amount = amount,
                    dateEpochMs = dateEpochMs,
                    notes = notes.trim()
                )
            )
            onDone()
        }
    }

    fun removeDeduction(id: Long) {
        viewModelScope.launch { repository.deleteDeduction(id) }
    }

    fun saveIncome(
        id: Long = 0,
        llcId: Long,
        name: String,
        category: String,
        source: String,
        amount: Double,
        dateEpochMs: Long,
        notes: String,
        onDone: () -> Unit = {}
    ) {
        viewModelScope.launch {
            repository.saveIncome(
                Income(
                    id = id,
                    llcId = llcId,
                    name = name.trim(),
                    category = category.trim().ifBlank { "Sales" },
                    source = source.trim(),
                    amount = amount,
                    dateEpochMs = dateEpochMs,
                    notes = notes.trim()
                )
            )
            onDone()
        }
    }

    fun removeIncome(id: Long) {
        viewModelScope.launch { repository.deleteIncome(id) }
    }

    fun saveInventoryItem(
        id: Long = 0,
        llcId: Long,
        name: String,
        sku: String,
        quantity: Double,
        unit: String,
        unitCost: Double,
        reorderLevel: Double,
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
                    reorderLevel = reorderLevel,
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

    fun moveInventory(item: InventoryItem, targetLlcId: Long, onDone: () -> Unit = {}) {
        viewModelScope.launch {
            repository.moveInventoryItem(item, targetLlcId)
            onDone()
        }
    }

    fun saveEmployee(
        id: Long = 0,
        llcId: Long,
        name: String,
        role: String,
        department: String,
        email: String,
        phone: String,
        employeeCode: String,
        payType: PayType,
        payRate: Double,
        status: EmploymentStatus,
        hireDateEpochMs: Long,
        notes: String,
        onDone: () -> Unit = {}
    ) {
        viewModelScope.launch {
            repository.saveEmployee(
                Employee(
                    id = id,
                    llcId = llcId,
                    name = name.trim(),
                    role = role.trim(),
                    department = department.trim(),
                    email = email.trim(),
                    phone = phone.trim(),
                    employeeCode = employeeCode.trim(),
                    payType = payType,
                    payRate = payRate,
                    status = status,
                    hireDateEpochMs = hireDateEpochMs,
                    notes = notes.trim()
                )
            )
            onDone()
        }
    }

    fun removeEmployee(id: Long) {
        viewModelScope.launch { repository.deleteEmployee(id) }
    }

    fun buildShareText(
        llc: Llc,
        deductions: List<Deduction>,
        incomes: List<Income>,
        inventory: List<InventoryItem>,
        employees: List<Employee>
    ): String = repository.formatReport(llc, deductions, incomes, inventory, employees)

    private fun yearOf(epochMs: Long): Int {
        val cal = Calendar.getInstance()
        cal.timeInMillis = epochMs
        return cal.get(Calendar.YEAR)
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
