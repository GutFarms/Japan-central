package com.solstice.dispensary.ui.navigation

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.solstice.dispensary.ui.screens.CartScreen
import com.solstice.dispensary.ui.screens.HomeScreen
import com.solstice.dispensary.ui.screens.MenuScreen
import com.solstice.dispensary.ui.screens.OrdersScreen
import com.solstice.dispensary.ui.screens.ProductDetailScreen
import com.solstice.dispensary.ui.screens.StoreScreen
import com.solstice.dispensary.ui.viewmodel.DispensaryViewModel

object Routes {
    const val HOME = "home"
    const val MENU = "menu"
    const val CART = "cart"
    const val ORDERS = "orders"
    const val STORE = "store"
    const val PRODUCT = "product/{productId}"

    fun product(id: String) = "product/$id"
}

@Composable
fun DispensaryNavHost(
    navController: NavHostController,
    viewModel: DispensaryViewModel,
    modifier: Modifier = Modifier
) {
    val featured by viewModel.featured.collectAsState()
    val cart by viewModel.cart.collectAsState()
    val orders by viewModel.orders.collectAsState()
    val products by viewModel.products.collectAsState()

    NavHost(
        navController = navController,
        startDestination = Routes.HOME,
        modifier = modifier
    ) {
        composable(Routes.HOME) {
            HomeScreen(
                featured = featured,
                cartCount = cart.itemCount,
                onOpenMenu = { navController.navigate(Routes.MENU) },
                onOpenProduct = { id -> navController.navigate(Routes.product(id)) },
                onOpenCart = { navController.navigate(Routes.CART) },
                onOpenStore = { navController.navigate(Routes.STORE) },
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
        composable(Routes.CART) {
            CartScreen(
                cart = cart,
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
            OrdersScreen(orders = orders)
        }
        composable(Routes.STORE) {
            StoreScreen()
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
