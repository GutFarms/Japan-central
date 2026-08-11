package com.solstice.dispensary.ui.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.solstice.dispensary.data.update.ApkInstaller
import com.solstice.dispensary.ui.screens.AccountScreen
import com.solstice.dispensary.ui.screens.CartScreen
import com.solstice.dispensary.ui.screens.CustomersScreen
import com.solstice.dispensary.ui.screens.DealsScreen
import com.solstice.dispensary.ui.screens.HomeScreen
import com.solstice.dispensary.ui.screens.InventoryScannerScreen
import com.solstice.dispensary.ui.screens.InventoryScreen
import com.solstice.dispensary.ui.screens.MenuScreen
import com.solstice.dispensary.ui.screens.OrdersScreen
import com.solstice.dispensary.ui.screens.ProductDetailScreen
import com.solstice.dispensary.ui.screens.RequestsScreen
import com.solstice.dispensary.ui.screens.StoreScreen
import com.solstice.dispensary.ui.viewmodel.DispensaryViewModel

object Routes {
    const val HOME = "home"
    const val MENU = "menu"
    const val DEALS = "deals"
    const val CART = "cart"
    const val ORDERS = "orders"
    const val STORE = "store"
    const val INVENTORY = "inventory"
    const val SCANNER = "inventory/scan"
    const val ACCOUNT = "account"
    const val CUSTOMERS = "customers"
    const val REQUESTS = "requests"
    const val PRODUCT = "product/{productId}"

    fun product(id: String) = "product/$id"
}

@Composable
fun DispensaryNavHost(
    navController: NavHostController,
    viewModel: DispensaryViewModel,
    modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    val featured by viewModel.featured.collectAsState()
    val deals by viewModel.deals.collectAsState()
    val cart by viewModel.cart.collectAsState()
    val orders by viewModel.orders.collectAsState()
    val products by viewModel.products.collectAsState()
    val inventory by viewModel.inventory.collectAsState()
    val intakes by viewModel.intakes.collectAsState()
    val customers by viewModel.customers.collectAsState()
    val productRequests by viewModel.productRequests.collectAsState()
    val requestStats by viewModel.requestBoxStats.collectAsState()
    var canInstall by remember { mutableStateOf(ApkInstaller.canInstallPackages(context)) }

    LaunchedEffect(Unit) {
        viewModel.autoCheckForAppUpdate()
    }

    fun openInstallPermission() {
        context.startActivity(ApkInstaller.installPermissionSettingsIntent(context))
        canInstall = ApkInstaller.canInstallPackages(context)
    }

    fun installDownloadedUpdate() {
        canInstall = ApkInstaller.canInstallPackages(context)
        if (!canInstall) {
            openInstallPermission()
            return
        }
        val apk = viewModel.downloadedApk ?: return
        context.startActivity(ApkInstaller.installApk(context, apk))
    }

    NavHost(
        navController = navController,
        startDestination = Routes.HOME,
        modifier = modifier
    ) {
        composable(Routes.HOME) {
            HomeScreen(
                featured = featured,
                cartCount = cart.itemCount,
                showInventory = viewModel.canManageInventory,
                updateState = viewModel.updateState,
                needsInstallPermission = !canInstall,
                requestStats = requestStats,
                productRequests = productRequests,
                isStaff = viewModel.canViewSensitiveInfo,
                requestMessage = viewModel.requestMessage,
                onDownloadUpdate = viewModel::downloadAppUpdate,
                onInstallUpdate = { installDownloadedUpdate() },
                onDismissUpdate = viewModel::dismissAppUpdate,
                onOpenInstallPermission = { openInstallPermission() },
                onSubmitRequest = viewModel::submitProductRequest,
                onMarkRequestDone = viewModel::markProductRequestFulfilled,
                onOpenRequests = { navController.navigate(Routes.REQUESTS) },
                onOpenMenu = { navController.navigate(Routes.MENU) },
                onOpenProduct = { id -> navController.navigate(Routes.product(id)) },
                onOpenCart = { navController.navigate(Routes.CART) },
                onOpenStore = { navController.navigate(Routes.STORE) },
                onOpenInventory = { navController.navigate(Routes.INVENTORY) },
                onSelectCategory = viewModel::setCategory
            )
        }
        composable(Routes.MENU) {
            val q = viewModel.searchQuery.trim().lowercase()
            val visible = products.filter { product ->
                q.isEmpty() ||
                    product.name.lowercase().contains(q) ||
                    product.brand.lowercase().contains(q) ||
                    product.effects.lowercase().contains(q) ||
                    product.category.label.lowercase().contains(q)
            }
            MenuScreen(
                products = visible,
                selectedCategory = viewModel.selectedCategory,
                searchQuery = viewModel.searchQuery,
                onSearchChange = viewModel::setSearch,
                onSelectCategory = viewModel::setCategory,
                onOpenProduct = { id -> navController.navigate(Routes.product(id)) }
            )
        }
        composable(Routes.DEALS) {
            DealsScreen(
                deals = deals,
                onOpenProduct = { id -> navController.navigate(Routes.product(id)) }
            )
        }
        composable(Routes.CART) {
            val customer = viewModel.currentCustomer
            CartScreen(
                cart = cart,
                defaultPickupName = customer?.fullName.orEmpty(),
                loyaltyPoints = customer?.loyaltyPoints ?: 0,
                canRedeemPoints = customer?.role?.canViewSensitiveInfo == false,
                checkoutMessage = viewModel.checkoutMessage,
                onClearMessage = viewModel::clearCheckoutMessage,
                onSetQuantity = viewModel::setQuantity,
                onRemove = viewModel::removeFromCart,
                onClear = viewModel::clearCart,
                onPlaceOrder = viewModel::placeOrder,
                onBrowseMenu = { navController.navigate(Routes.MENU) }
            )
        }
        composable(Routes.ORDERS) {
            OrdersScreen(
                orders = orders,
                canManageOrders = viewModel.canViewSensitiveInfo,
                onUpdateStatus = viewModel::updateOrderStatus,
                onShareReceipt = { orderId ->
                    viewModel.shareOrderReceipt(orderId) { text ->
                        val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                            type = "text/plain"
                            putExtra(android.content.Intent.EXTRA_SUBJECT, "Native Pure receipt")
                            putExtra(android.content.Intent.EXTRA_TEXT, text)
                        }
                        context.startActivity(
                            android.content.Intent.createChooser(send, "Share receipt")
                        )
                    }
                }
            )
        }
        composable(Routes.STORE) {
            StoreScreen(
                onOpenOrders = { navController.navigate(Routes.ORDERS) }
            )
        }
        composable(Routes.INVENTORY) {
            if (!viewModel.canManageInventory) {
                Text(
                    "Inventory is only available to admin and staff accounts.",
                    modifier = Modifier.padding(20.dp)
                )
            } else {
                InventoryScreen(
                    products = inventory,
                    intakes = intakes,
                    intakeMessage = viewModel.intakeMessage,
                    onClearMessage = viewModel::clearIntakeMessage,
                    onOpenScanner = {
                        viewModel.clearScanResult()
                        navController.navigate(Routes.SCANNER)
                    },
                    onAdjustStock = viewModel::adjustStock,
                    onSetPublished = viewModel::setPublished,
                    onSaveProduct = viewModel::saveProduct,
                    onCreateDraft = viewModel::createBlankDraftProduct
                )
            }
        }
        composable(Routes.SCANNER) {
            if (!viewModel.canManageInventory) {
                Text(
                    "Scanner is only available to admin and staff accounts.",
                    modifier = Modifier.padding(20.dp)
                )
            } else {
                InventoryScannerScreen(
                    scanBusy = viewModel.scanBusy,
                    scanResult = viewModel.scanResult,
                    scanError = viewModel.scanError,
                    onBack = { navController.popBackStack() },
                    onCaptureBitmap = viewModel::analyzeLabelBitmap,
                    onClearResult = viewModel::clearScanResult,
                    onConfirm = { qty ->
                        viewModel.confirmScanIntake(qty)
                        navController.popBackStack()
                    }
                )
            }
        }
        composable(Routes.ACCOUNT) {
            val staff by viewModel.staffAccounts.collectAsState()
            AccountScreen(
                customer = viewModel.currentCustomer,
                customers = customers,
                staffAccounts = staff,
                themeMode = viewModel.themeMode,
                securitySettings = viewModel.securitySettings,
                message = viewModel.accountMessage,
                syncExportJson = viewModel.lastSyncExport,
                updateState = viewModel.updateState,
                needsInstallPermission = !canInstall,
                onClearMessage = viewModel::clearAccountMessage,
                onSaveProfile = viewModel::saveProfile,
                onThemeModeChange = viewModel::updateThemeMode,
                onChangePassword = viewModel::changePassword,
                onEnableAppLock = viewModel::enableAppLock,
                onDisableAppLock = viewModel::disableAppLock,
                onAutoLockChange = viewModel::setAutoLockTimeout,
                onCreateStaff = viewModel::createStaffSubAccount,
                onSetStaffEnabled = viewModel::setStaffEnabled,
                onResetStaffPassword = viewModel::resetStaffPassword,
                onExportSync = viewModel::exportSync,
                onShareSync = { json ->
                    val send = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
                        type = "application/json"
                        putExtra(android.content.Intent.EXTRA_SUBJECT, "Native Pure inventory sync")
                        putExtra(android.content.Intent.EXTRA_TEXT, json)
                    }
                    context.startActivity(android.content.Intent.createChooser(send, "Share sync file"))
                },
                onImportSync = viewModel::importSync,
                onClearSyncExport = viewModel::clearSyncExport,
                onCheckUpdate = { viewModel.checkForAppUpdate(force = true) },
                onDownloadUpdate = viewModel::downloadAppUpdate,
                onInstallUpdate = { installDownloadedUpdate() },
                onDismissUpdate = viewModel::dismissAppUpdate,
                onOpenInstallPermission = { openInstallPermission() },
                onLogout = viewModel::logout,
                onOpenCustomers = { navController.navigate(Routes.CUSTOMERS) },
                onOpenOrders = { navController.navigate(Routes.ORDERS) }
            )
        }
        composable(Routes.REQUESTS) {
            if (!viewModel.canViewSensitiveInfo) {
                Text(
                    "Request box details are only available to admin and staff.",
                    modifier = Modifier.padding(20.dp)
                )
            } else {
                RequestsScreen(
                    requests = productRequests,
                    stats = requestStats,
                    onBack = { navController.popBackStack() },
                    onMarkDone = viewModel::markProductRequestFulfilled,
                    onCreateDraft = viewModel::createDraftFromRequest
                )
            }
        }
        composable(Routes.CUSTOMERS) {
            CustomersScreen(
                customers = customers,
                showSensitive = viewModel.canViewSensitiveInfo,
                onBack = { navController.popBackStack() }
            )
        }
        composable(
            route = Routes.PRODUCT,
            arguments = listOf(navArgument("productId") { type = NavType.StringType })
        ) { entry ->
            val id = entry.arguments?.getString("productId").orEmpty()
            val product by viewModel.productFlow(id).collectAsState(initial = null)
            ProductDetailScreen(
                product = product,
                onBack = { navController.popBackStack() },
                onAddToCart = viewModel::addToCart
            )
        }
    }
}
