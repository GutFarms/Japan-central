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
import com.gutfarms.llcmanager.ui.screens.TaxCalculatorScreen
import com.gutfarms.llcmanager.ui.viewmodel.LlcViewModel

object Routes {
    const val HOME = "home"
    const val LLC_DETAIL = "llc/{llcId}"
    const val TAX_CALCULATOR = "tax-calculator?gross={gross}&deductions={deductions}"

    fun llcDetail(llcId: Long) = "llc/$llcId"

    fun taxCalculator(gross: Double = 0.0, deductions: Double = 0.0): String {
        return "tax-calculator?gross=$gross&deductions=$deductions"
    }
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
                },
                onOpenTaxCalculator = {
                    navController.navigate(Routes.taxCalculator())
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
                onBack = { navController.popBackStack() },
                onOpenTaxCalculator = { gross, deductions ->
                    navController.navigate(Routes.taxCalculator(gross, deductions))
                }
            )
        }
        composable(
            route = Routes.TAX_CALCULATOR,
            arguments = listOf(
                navArgument("gross") {
                    type = NavType.StringType
                    defaultValue = "0"
                },
                navArgument("deductions") {
                    type = NavType.StringType
                    defaultValue = "0"
                }
            )
        ) { entry ->
            val gross = entry.arguments?.getString("gross")?.toDoubleOrNull() ?: 0.0
            val deductions = entry.arguments?.getString("deductions")?.toDoubleOrNull() ?: 0.0
            TaxCalculatorScreen(
                onBack = { navController.popBackStack() },
                initialGrossIncome = gross,
                initialDeductions = deductions
            )
        }
    }
}
