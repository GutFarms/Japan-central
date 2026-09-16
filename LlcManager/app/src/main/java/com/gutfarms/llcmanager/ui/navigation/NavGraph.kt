package com.gutfarms.llcmanager.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.gutfarms.llcmanager.ui.screens.HomeScreen
import com.gutfarms.llcmanager.ui.screens.LlcDetailScreen
import com.gutfarms.llcmanager.ui.viewmodel.LlcViewModel

object Routes {
    const val HOME = "home"
    const val LLC_DETAIL = "llc/{llcId}"

    fun llcDetail(llcId: Long) = "llc/$llcId"
}

@Composable
fun LlcNavHost(
    navController: NavHostController,
    viewModel: LlcViewModel,
    modifier: Modifier = Modifier
) {
    NavHost(
        navController = navController,
        startDestination = Routes.HOME,
        modifier = modifier
    ) {
        composable(Routes.HOME) {
            HomeScreen(
                viewModel = viewModel,
                onOpenLlc = { id ->
                    viewModel.selectLlc(id)
                    navController.navigate(Routes.llcDetail(id))
                }
            )
        }
        composable(
            route = Routes.LLC_DETAIL,
            arguments = listOf(navArgument("llcId") { type = NavType.LongType })
        ) { entry ->
            val llcId = entry.arguments?.getLong("llcId") ?: return@composable
            viewModel.selectLlc(llcId)
            LlcDetailScreen(
                llcId = llcId,
                viewModel = viewModel,
                onBack = { navController.popBackStack() }
            )
        }
    }
}
