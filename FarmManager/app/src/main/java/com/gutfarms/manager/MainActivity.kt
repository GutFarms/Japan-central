package com.gutfarms.manager

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.TrendingUp
import androidx.compose.material.icons.outlined.Favorite
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Pets
import androidx.compose.material.icons.outlined.Restaurant
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.gutfarms.manager.ui.navigation.FarmNavHost
import com.gutfarms.manager.ui.navigation.Routes
import com.gutfarms.manager.ui.theme.FarmManagerTheme
import com.gutfarms.manager.ui.viewmodel.FarmViewModel
import com.gutfarms.manager.ui.viewmodel.FarmViewModelFactory

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val app = application as FarmManagerApplication
        setContent {
            FarmManagerTheme {
                val navController = rememberNavController()
                val viewModel: FarmViewModel = viewModel(
                    factory = FarmViewModelFactory(app.repository)
                )
                val backStack by navController.currentBackStackEntryAsState()
                val current = backStack?.destination?.route ?: Routes.HOME

                val destinations = listOf(
                    NavItem(Routes.HOME, "Home", Icons.Outlined.Home),
                    NavItem(Routes.ANIMALS, "Herd", Icons.Outlined.Pets),
                    NavItem(Routes.FEEDING, "Feed", Icons.Outlined.Restaurant),
                    NavItem(Routes.BREEDING, "Breed", Icons.Outlined.Favorite),
                    NavItem(Routes.PROFITS, "Profit", Icons.AutoMirrored.Outlined.TrendingUp)
                )

                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                    bottomBar = {
                        NavigationBar {
                            destinations.forEach { item ->
                                NavigationBarItem(
                                    selected = current == item.route,
                                    onClick = {
                                        if (current != item.route) {
                                            navController.navigate(item.route) {
                                                popUpTo(Routes.HOME) { saveState = true }
                                                launchSingleTop = true
                                                restoreState = true
                                            }
                                        }
                                    },
                                    icon = { Icon(item.icon, contentDescription = item.label) },
                                    label = { Text(item.label) }
                                )
                            }
                        }
                    }
                ) { padding ->
                    FarmNavHost(
                        navController = navController,
                        viewModel = viewModel,
                        modifier = Modifier.padding(padding)
                    )
                }
            }
        }
    }
}

private data class NavItem(
    val route: String,
    val label: String,
    val icon: ImageVector
)
