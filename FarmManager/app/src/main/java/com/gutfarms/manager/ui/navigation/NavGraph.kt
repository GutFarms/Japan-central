package com.gutfarms.manager.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import com.gutfarms.manager.ui.screens.AnimalsScreen
import com.gutfarms.manager.ui.screens.BreedingScreen
import com.gutfarms.manager.ui.screens.FeedingScreen
import com.gutfarms.manager.ui.screens.HomeScreen
import com.gutfarms.manager.ui.screens.ProfitsScreen
import com.gutfarms.manager.ui.viewmodel.FarmViewModel

object Routes {
    const val HOME = "home"
    const val ANIMALS = "animals"
    const val FEEDING = "feeding"
    const val BREEDING = "breeding"
    const val PROFITS = "profits"
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
                animals = viewModel.animals,
                schedules = viewModel.schedules,
                breedingSchedules = viewModel.breedingSchedules,
                profitSummary = viewModel.profitSummary,
                onOpenAnimals = { navController.navigate(Routes.ANIMALS) },
                onOpenFeeding = { navController.navigate(Routes.FEEDING) },
                onOpenBreeding = { navController.navigate(Routes.BREEDING) },
                onOpenProfits = { navController.navigate(Routes.PROFITS) }
            )
        }
        composable(Routes.ANIMALS) {
            AnimalsScreen(
                animals = viewModel.animals,
                onSave = viewModel::saveAnimal,
                onDelete = viewModel::deleteAnimal
            )
        }
        composable(Routes.FEEDING) {
            FeedingScreen(
                animals = viewModel.animals,
                schedules = viewModel.schedules,
                onSave = viewModel::saveSchedule,
                onDelete = viewModel::deleteSchedule,
                onToggle = viewModel::toggleSchedule
            )
        }
        composable(Routes.BREEDING) {
            BreedingScreen(
                animals = viewModel.animals,
                breedingSchedules = viewModel.breedingSchedules,
                onSave = viewModel::saveBreeding,
                onDelete = viewModel::deleteBreeding,
                onToggle = viewModel::toggleBreeding
            )
        }
        composable(Routes.PROFITS) {
            ProfitsScreen(
                profitSummary = viewModel.profitSummary,
                transactions = viewModel.transactions,
                onSave = viewModel::saveTransaction,
                onDelete = viewModel::deleteTransaction
            )
        }
    }
}
