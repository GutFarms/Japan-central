package com.gutfarms.manager.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import com.gutfarms.manager.ui.screens.AnimalsScreen
import com.gutfarms.manager.ui.screens.ArrivalsScreen
import com.gutfarms.manager.ui.screens.BreedingScreen
import com.gutfarms.manager.ui.screens.ContactsScreen
import com.gutfarms.manager.ui.screens.FarmInfoScreen
import com.gutfarms.manager.ui.screens.FeedingScreen
import com.gutfarms.manager.ui.screens.HealthScreen
import com.gutfarms.manager.ui.screens.HomeScreen
import com.gutfarms.manager.ui.screens.InventoryScreen
import com.gutfarms.manager.ui.screens.JournalScreen
import com.gutfarms.manager.ui.screens.ProfitsScreen
import com.gutfarms.manager.ui.screens.RecordsHubScreen
import com.gutfarms.manager.ui.viewmodel.FarmViewModel

object Routes {
    const val HOME = "home"
    const val ANIMALS = "animals"
    const val ARRIVALS = "arrivals"
    const val FEEDING = "feeding"
    const val BREEDING = "breeding"
    const val PROFITS = "profits"
    const val RECORDS = "records"
    const val FARM_INFO = "farm_info"
    const val HEALTH = "health"
    const val INVENTORY = "inventory"
    const val JOURNAL = "journal"
    const val CONTACTS = "contacts"
}

@Composable
fun FarmNavHost(
    navController: NavHostController,
    viewModel: FarmViewModel,
    modifier: Modifier = Modifier
) {
    val healthRecords by viewModel.healthRecords.collectAsState()
    val inventoryItems by viewModel.inventoryItems.collectAsState()
    val journalEntries by viewModel.journalEntries.collectAsState()
    val contacts by viewModel.contacts.collectAsState()
    val lowStockCount = inventoryItems.count { it.needsReorder }

    NavHost(
        navController = navController,
        startDestination = Routes.HOME,
        modifier = modifier
    ) {
        composable(Routes.HOME) {
            HomeScreen(
                farmName = viewModel.farmName,
                animals = viewModel.animals,
                schedules = viewModel.schedules,
                breedingSchedules = viewModel.breedingSchedules,
                arrivals = viewModel.arrivals,
                transactions = viewModel.transactions,
                profitSummary = viewModel.profitSummary,
                onUpdateFarmName = viewModel::updateFarmName,
                onOpenAnimals = { navController.navigate(Routes.ANIMALS) },
                onOpenArrivals = { navController.navigate(Routes.ARRIVALS) },
                onOpenFeeding = { navController.navigate(Routes.FEEDING) },
                onOpenBreeding = { navController.navigate(Routes.BREEDING) },
                onOpenProfits = { navController.navigate(Routes.PROFITS) },
                onOpenRecords = { navController.navigate(Routes.RECORDS) }
            )
        }
        composable(Routes.ANIMALS) {
            AnimalsScreen(
                farmName = viewModel.farmName,
                animals = viewModel.animals,
                onSave = viewModel::saveAnimal,
                onDelete = viewModel::deleteAnimal,
                onOpenArrivals = { navController.navigate(Routes.ARRIVALS) }
            )
        }
        composable(Routes.ARRIVALS) {
            ArrivalsScreen(
                farmName = viewModel.farmName,
                animals = viewModel.animals,
                arrivals = viewModel.arrivals,
                onSave = viewModel::saveArrival,
                onDelete = viewModel::deleteArrival,
                onBack = { navController.popBackStack() }
            )
        }
        composable(Routes.FEEDING) {
            FeedingScreen(
                farmName = viewModel.farmName,
                animals = viewModel.animals,
                schedules = viewModel.schedules,
                onSave = viewModel::saveSchedule,
                onDelete = viewModel::deleteSchedule,
                onToggle = viewModel::toggleSchedule
            )
        }
        composable(Routes.BREEDING) {
            BreedingScreen(
                farmName = viewModel.farmName,
                animals = viewModel.animals,
                breedingSchedules = viewModel.breedingSchedules,
                onSave = viewModel::saveBreeding,
                onDelete = viewModel::deleteBreeding,
                onToggle = viewModel::toggleBreeding
            )
        }
        composable(Routes.PROFITS) {
            ProfitsScreen(
                farmName = viewModel.farmName,
                profitSummary = viewModel.profitSummary,
                transactions = viewModel.transactions,
                onSave = viewModel::saveTransaction,
                onDelete = viewModel::deleteTransaction
            )
        }
        composable(Routes.RECORDS) {
            RecordsHubScreen(
                farmName = viewModel.farmName,
                healthCount = healthRecords.size,
                inventoryCount = inventoryItems.size,
                journalCount = journalEntries.size,
                contactCount = contacts.size,
                lowStockCount = lowStockCount,
                onOpenFarmInfo = { navController.navigate(Routes.FARM_INFO) },
                onOpenHealth = { navController.navigate(Routes.HEALTH) },
                onOpenInventory = { navController.navigate(Routes.INVENTORY) },
                onOpenJournal = { navController.navigate(Routes.JOURNAL) },
                onOpenContacts = { navController.navigate(Routes.CONTACTS) },
                onBack = { navController.popBackStack() }
            )
        }
        composable(Routes.FARM_INFO) {
            FarmInfoScreen(
                farmName = viewModel.farmName,
                farmProfile = viewModel.farmProfile,
                onSave = viewModel::saveFarmProfile,
                onBack = { navController.popBackStack() }
            )
        }
        composable(Routes.HEALTH) {
            HealthScreen(
                farmName = viewModel.farmName,
                animals = viewModel.animals,
                records = viewModel.healthRecords,
                onSave = viewModel::saveHealthRecord,
                onDelete = viewModel::deleteHealthRecord,
                onBack = { navController.popBackStack() }
            )
        }
        composable(Routes.INVENTORY) {
            InventoryScreen(
                farmName = viewModel.farmName,
                items = viewModel.inventoryItems,
                onSave = viewModel::saveInventoryItem,
                onDelete = viewModel::deleteInventoryItem,
                onBack = { navController.popBackStack() }
            )
        }
        composable(Routes.JOURNAL) {
            JournalScreen(
                farmName = viewModel.farmName,
                entries = viewModel.journalEntries,
                onSave = viewModel::saveJournalEntry,
                onDelete = viewModel::deleteJournalEntry,
                onBack = { navController.popBackStack() }
            )
        }
        composable(Routes.CONTACTS) {
            ContactsScreen(
                farmName = viewModel.farmName,
                contacts = viewModel.contacts,
                onSave = viewModel::saveContact,
                onDelete = viewModel::deleteContact,
                onBack = { navController.popBackStack() }
            )
        }
    }
}
