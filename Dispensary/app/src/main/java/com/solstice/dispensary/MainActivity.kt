package com.solstice.dispensary

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Inventory2
import androidx.compose.material.icons.outlined.LocalMall
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material.icons.outlined.ShoppingBag
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Badge
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.solstice.dispensary.ui.navigation.DispensaryNavHost
import com.solstice.dispensary.ui.navigation.Routes
import com.solstice.dispensary.ui.screens.AgeGateScreen
import com.solstice.dispensary.ui.screens.AppLockScreen
import com.solstice.dispensary.ui.screens.AuthScreen
import com.solstice.dispensary.ui.screens.EmailVerificationScreen
import com.solstice.dispensary.ui.screens.ForcePasswordChangeScreen
import com.solstice.dispensary.ui.theme.SolsticeTheme
import com.solstice.dispensary.ui.viewmodel.DispensaryViewModel
import com.solstice.dispensary.ui.viewmodel.DispensaryViewModelFactory

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        val app = application as DispensaryApplication
        setContent {
            val viewModel: DispensaryViewModel = viewModel(
                factory = DispensaryViewModelFactory(app.repository, app.updateChecker)
            )

            val lifecycleOwner = LocalLifecycleOwner.current
            DisposableEffect(lifecycleOwner, viewModel) {
                val observer = LifecycleEventObserver { _, event ->
                    when (event) {
                        Lifecycle.Event.ON_STOP -> viewModel.onAppBackgrounded()
                        Lifecycle.Event.ON_START -> viewModel.onAppResumed()
                        else -> Unit
                    }
                }
                lifecycleOwner.lifecycle.addObserver(observer)
                onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
            }

            SolsticeTheme(themeMode = viewModel.themeMode) {
                if (!viewModel.ageVerified) {
                    AgeGateScreen(
                        onVerified = viewModel::verifyAge,
                        onExit = { finish() }
                    )
                    return@SolsticeTheme
                }

                if (!viewModel.isLoggedIn) {
                    AuthScreen(
                        busy = viewModel.authBusy,
                        error = viewModel.authError,
                        onLogin = viewModel::login,
                        onRegister = viewModel::register,
                        onClearError = viewModel::clearAuthError
                    )
                    return@SolsticeTheme
                }

                if (viewModel.needsEmailVerification) {
                    EmailVerificationScreen(
                        email = viewModel.currentCustomer?.email.orEmpty(),
                        issuedCode = viewModel.pendingEmailCode,
                        busy = viewModel.authBusy,
                        error = viewModel.authError,
                        onVerify = viewModel::verifyEmailCode,
                        onResend = viewModel::resendEmailVerificationCode,
                        onClearError = viewModel::clearAuthError,
                        onLogout = viewModel::logout
                    )
                    return@SolsticeTheme
                }

                if (viewModel.mustChangePassword) {
                    ForcePasswordChangeScreen(
                        busy = viewModel.authBusy,
                        error = viewModel.authError,
                        onSubmit = viewModel::forceChangePassword,
                        onClearError = viewModel::clearAuthError,
                        onLogout = viewModel::logout
                    )
                    return@SolsticeTheme
                }

                if (viewModel.appLocked) {
                    AppLockScreen(
                        error = viewModel.lockError,
                        onUnlock = viewModel::unlockWithPin,
                        onClearError = viewModel::clearLockError,
                        onLogout = viewModel::logout
                    )
                    return@SolsticeTheme
                }

                val navController = rememberNavController()
                val backStack by navController.currentBackStackEntryAsState()
                val current = backStack?.destination?.route ?: Routes.HOME
                val cart by viewModel.cart.collectAsState()

                val destinations = buildList {
                    add(NavItem(Routes.HOME, "Home", Icons.Outlined.Home))
                    add(NavItem(Routes.MENU, "Menu", Icons.Outlined.LocalMall))
                    add(NavItem(Routes.CART, "Bag", Icons.Outlined.ShoppingBag))
                    if (viewModel.canManageInventory) {
                        add(NavItem(Routes.INVENTORY, "Stock", Icons.Outlined.Inventory2))
                    }
                    add(NavItem(Routes.ACCOUNT, "Account", Icons.Outlined.Person))
                }

                val hideBottomBar = current.startsWith("product/") ||
                    current == Routes.SCANNER ||
                    current == Routes.CUSTOMERS ||
                    current == Routes.ORDERS ||
                    current == Routes.STORE

                Scaffold(
                    modifier = Modifier.fillMaxSize(),
                    bottomBar = {
                        if (!hideBottomBar) {
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
                                        icon = {
                                            if (item.route == Routes.CART && cart.itemCount > 0) {
                                                BadgedBox(
                                                    badge = { Badge { Text(cart.itemCount.toString()) } }
                                                ) {
                                                    Icon(item.icon, contentDescription = item.label)
                                                }
                                            } else {
                                                Icon(item.icon, contentDescription = item.label)
                                            }
                                        },
                                        label = { Text(item.label) }
                                    )
                                }
                            }
                        }
                    }
                ) { padding ->
                    DispensaryNavHost(
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
