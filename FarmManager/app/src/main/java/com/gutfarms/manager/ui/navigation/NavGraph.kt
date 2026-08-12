package com.gutfarms.manager.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import com.gutfarms.manager.ui.screens.AnimalsScreen
import com.gutfarms.manager.ui.screens.ArrivalsScreen
import com.gutfarms.manager.ui.screens.BreedingScreen
import com.gutfarms.manager.ui.screens.DataImportScreen
import com.gutfarms.manager.ui.screens.FeedingScreen
import com.gutfarms.manager.ui.screens.HomeScreen
import com.gutfarms.manager.ui.screens.ProfitsScreen
import com.gutfarms.manager.ui.viewmodel.FarmViewModel

object Routes {
    const val HOME = "home"
    const val ANIMALS = "animals"
    const val ARRIVALS = "arrivals"
    const val FEEDING = "feeding"
    const val BREEDING = "breeding"
    const val PROFITS = "profits"
    const val DATA_IMPORT = "data_import"
}

@Composable
fun FarmNavHost(
    navController: NavHostController,
    viewModel: FarmViewModel,
    modifier: Modifier = Modifier
) {
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
                onOpenDataImport = { navController.navigate(Routes.DATA_IMPORT) }
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
        composable(Routes.DATA_IMPORT) {
            val context = LocalContext.current
            DataImportScreen(
                farmName = viewModel.farmName,
                importFiles = viewModel.importFiles,
                apiSources = viewModel.apiSources,
                dataMessage = viewModel.dataMessage,
                onClearMessage = viewModel::clearDataMessage,
                onImportUri = { uri -> viewModel.importFile(context, uri) },
                onDeleteFile = viewModel::deleteImportFile,
                onSaveApiSource = viewModel::saveApiSource,
                onDeleteApiSource = viewModel::deleteApiSource,
                onToggleApiSource = viewModel::toggleApiSource,
                onPullApiSource = viewModel::pullApiSource,
                onBack = { navController.popBackStack() }
            )
        }
    }
}
