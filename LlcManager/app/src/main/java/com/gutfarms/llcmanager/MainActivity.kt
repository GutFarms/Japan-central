package com.gutfarms.llcmanager

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.rememberNavController
import com.gutfarms.llcmanager.ui.navigation.LlcNavHost
import com.gutfarms.llcmanager.ui.theme.LlcManagerTheme
import com.gutfarms.llcmanager.ui.viewmodel.LlcViewModel
import com.gutfarms.llcmanager.ui.viewmodel.LlcViewModelFactory

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val app = application as LlcManagerApplication
        setContent {
            LlcManagerTheme {
                val navController = rememberNavController()
                val viewModel: LlcViewModel = viewModel(
                    factory = LlcViewModelFactory(app.repository)
                )
                LlcNavHost(
                    navController = navController,
                    viewModel = viewModel,
                    modifier = Modifier.fillMaxSize()
                )
            }
        }
    }
}
