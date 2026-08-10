package com.nativepure.companion.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.VerticalScrollbar
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.rememberScrollbarAdapter
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationRail
import androidx.compose.material3.NavigationRailItem
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.nativepure.companion.data.AccountRole
import com.nativepure.companion.data.AuthResult
import com.nativepure.companion.data.CompanionRepository
import com.nativepure.companion.data.CustomerProfile
import com.nativepure.companion.data.NavSection
import com.nativepure.companion.data.OpResult
import com.nativepure.companion.data.PasswordPolicy
import com.nativepure.companion.data.Product
import com.nativepure.companion.data.ProductCategory
import java.awt.FileDialog
import java.awt.Frame
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun CompanionApp(repository: CompanionRepository) {
    var ageVerified by remember { mutableStateOf(repository.ageVerified) }
    var customer by remember { mutableStateOf(repository.currentCustomer()) }
    var authError by remember { mutableStateOf<String?>(null) }
    var message by remember { mutableStateOf<String?>(null) }
    var section by remember { mutableStateOf(NavSection.HOME) }
    var tick by remember { mutableStateOf(0) }

    fun refresh() {
        customer = repository.currentCustomer()
        tick++
    }

    when {
        !ageVerified -> AgeGate {
            repository.setAgeVerified()
            ageVerified = true
        }
        customer == null -> AuthPane(
            error = authError,
            onLogin = { email, password ->
                when (val result = repository.login(email, password)) {
                    is AuthResult.Success -> {
                        customer = result.customer
                        authError = null
                        section = NavSection.HOME
                    }
                    is AuthResult.Error -> authError = result.message
                }
            },
            onRegister = { email, password, name, phone, dob ->
                when (val result = repository.register(email, password, name, phone, dob)) {
                    is AuthResult.Success -> {
                        customer = result.customer
                        authError = null
                    }
                    is AuthResult.Error -> authError = result.message
                }
            },
            onClearError = { authError = null }
        )
        customer!!.mustChangePassword -> ForcePasswordPane(
            error = authError,
            onSubmit = { newPass, confirm ->
                when (val result = repository.forceChangePassword(newPass, confirm)) {
                    is AuthResult.Success -> {
                        customer = result.customer
                        authError = null
                        message = "Password updated."
                    }
                    is AuthResult.Error -> authError = result.message
                }
            },
            onLogout = {
                repository.logout()
                customer = null
            }
        )
        else -> MainShell(
            repository = repository,
            customer = customer!!,
            section = section,
            message = message,
            tick = tick,
            onSection = { section = it },
            onMessage = { message = it },
            onRefresh = { refresh() },
            onLogout = {
                repository.logout()
                customer = null
                section = NavSection.HOME
            }
        )
    }
}

@Composable
private fun AgeGate(onVerified: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(colors.background, colors.surfaceVariant, colors.background))),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            modifier = Modifier.width(420.dp).padding(28.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            BrandMark(120.dp)
            Text("Native Pure", style = MaterialTheme.typography.headlineLarge)
            Text("Are you 18 or older?", style = MaterialTheme.typography.headlineMedium)
            Text(
                "You must be of legal age to use the Native Pure desktop companion.",
                style = MaterialTheme.typography.bodyLarge,
                color = colors.onSurfaceVariant,
                textAlign = TextAlign.Center
            )
            Button(
                onClick = onVerified,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp),
                colors = ButtonDefaults.buttonColors(containerColor = colors.secondary, contentColor = colors.onSecondary)
            ) { Text("Yes, I am 18+") }
            Text(
                "Windows companion for menu, pickup orders, inventory, and account security.",
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                textAlign = TextAlign.Center
            )
        }
    }
}

@Composable
private fun AuthPane(
    error: String?,
    onLogin: (String, String) -> Unit,
    onRegister: (String, String, String, String, String) -> Unit,
    onClearError: () -> Unit
) {
    var create by remember { mutableStateOf(false) }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var phone by remember { mutableStateOf("") }
    var dob by remember { mutableStateOf("") }

    Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background), contentAlignment = Alignment.Center) {
        Surface(shape = RoundedCornerShape(18.dp), tonalElevation = 2.dp, modifier = Modifier.width(440.dp)) {
            Column(Modifier.padding(28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                BrandMark(72.dp)
                Text("Native Pure Companion", style = MaterialTheme.typography.headlineMedium)
                Text(
                    if (create) "Create a customer account" else "Sign in to continue",
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                if (create) {
                    Field(name, { name = it; onClearError() }, "Full name")
                    Field(phone, { phone = it; onClearError() }, "Phone")
                    Field(dob, { dob = it; onClearError() }, "Date of birth")
                }
                Field(email, { email = it; onClearError() }, if (create) "Email" else "Email or username")
                Field(password, { password = it; onClearError() }, "Password", password = true)
                if (error != null) Text(error, color = MaterialTheme.colorScheme.error)
                Button(
                    onClick = {
                        if (create) onRegister(email, password, name, phone, dob)
                        else onLogin(email, password)
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) { Text(if (create) "Create account" else "Log in") }
                TextButton(onClick = { create = !create; onClearError() }) {
                    Text(if (create) "Already have an account? Log in" else "New here? Create an account")
                }
                Text(
                    PasswordPolicy.requirementsLabel,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

@Composable
private fun ForcePasswordPane(
    error: String?,
    onSubmit: (String, String) -> Unit,
    onLogout: () -> Unit
) {
    var newPass by remember { mutableStateOf("") }
    var confirm by remember { mutableStateOf("") }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Surface(shape = RoundedCornerShape(18.dp), tonalElevation = 2.dp, modifier = Modifier.width(420.dp)) {
            Column(Modifier.padding(28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text("Create a new password", style = MaterialTheme.typography.headlineMedium)
                Text(PasswordPolicy.requirementsLabel, color = MaterialTheme.colorScheme.onSurfaceVariant)
                Field(newPass, { newPass = it }, "New password", password = true)
                Field(confirm, { confirm = it }, "Confirm password", password = true)
                if (error != null) Text(error, color = MaterialTheme.colorScheme.error)
                Button(onClick = { onSubmit(newPass, confirm) }, modifier = Modifier.fillMaxWidth()) {
                    Text("Save password")
                }
                TextButton(onClick = onLogout) { Text("Log out") }
            }
        }
    }
}

@Composable
private fun MainShell(
    repository: CompanionRepository,
    customer: CustomerProfile,
    section: NavSection,
    message: String?,
    tick: Int,
    onSection: (NavSection) -> Unit,
    onMessage: (String?) -> Unit,
    onRefresh: () -> Unit,
    onLogout: () -> Unit
) {
    val sections = buildList {
        add(NavSection.HOME)
        add(NavSection.MENU)
        add(NavSection.CART)
        add(NavSection.ORDERS)
        if (customer.role.canManageInventory) add(NavSection.INVENTORY)
        if (customer.role.canViewSensitiveInfo) add(NavSection.CUSTOMERS)
        add(NavSection.STORE)
        add(NavSection.ACCOUNT)
    }

    Row(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        NavigationRail(
            modifier = Modifier.fillMaxHeight().background(MaterialTheme.colorScheme.surfaceVariant),
            header = {
                Column(
                    modifier = Modifier.padding(12.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    BrandMark(48.dp)
                    Spacer(Modifier.height(6.dp))
                    Text("Native Pure", style = MaterialTheme.typography.titleLarge)
                    Text(customer.role.label, style = MaterialTheme.typography.bodyMedium)
                }
            }
        ) {
            sections.forEach { item ->
                NavigationRailItem(
                    selected = section == item,
                    onClick = { onSection(item) },
                    icon = { Text(item.label.take(1)) },
                    label = { Text(item.label) }
                )
            }
        }

        Column(Modifier.fillMaxSize().padding(24.dp)) {
            if (message != null) {
                Surface(
                    color = MaterialTheme.colorScheme.primaryContainer,
                    shape = RoundedCornerShape(10.dp),
                    modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp)
                ) {
                    Row(
                        Modifier.padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(message, modifier = Modifier.weight(1f))
                        TextButton(onClick = { onMessage(null) }) { Text("Dismiss") }
                    }
                }
            }

            // tick forces recomposition after mutations
            @Suppress("UNUSED_EXPRESSION")
            tick

            when (section) {
                NavSection.HOME -> HomePane(
                    featured = repository.featuredProducts(),
                    cartCount = repository.cartSummary().itemCount,
                    onOpenMenu = { onSection(NavSection.MENU) },
                    onAdd = {
                        repository.addToCart(it)
                        onRefresh()
                        onMessage("Added to bag.")
                    }
                )
                NavSection.MENU -> MenuPane(
                    products = repository.catalogProducts(),
                    onAdd = {
                        repository.addToCart(it)
                        onRefresh()
                        onMessage("Added to bag.")
                    }
                )
                NavSection.CART -> CartPane(
                    repository = repository,
                    defaultName = customer.fullName,
                    onRefresh = onRefresh,
                    onMessage = onMessage
                )
                NavSection.ORDERS -> OrdersPane(repository)
                NavSection.INVENTORY -> InventoryPane(repository, onRefresh, onMessage)
                NavSection.CUSTOMERS -> CustomersPane(repository)
                NavSection.STORE -> StorePane()
                NavSection.ACCOUNT -> AccountPane(
                    repository = repository,
                    customer = customer,
                    onRefresh = onRefresh,
                    onMessage = onMessage,
                    onLogout = onLogout
                )
            }
        }
    }
}

@Composable
private fun HomePane(
    featured: List<Product>,
    cartCount: Int,
    onOpenMenu: () -> Unit,
    onAdd: (String) -> Unit
) {
    Column(Modifier.fillMaxSize()) {
        Text("Native Pure", style = MaterialTheme.typography.headlineLarge)
        Text(
            "Desktop companion for browsing the menu, placing pickup orders, and managing stock.",
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Button(onClick = onOpenMenu, shape = RoundedCornerShape(10.dp)) { Text("Browse menu") }
            Text("$cartCount in bag", modifier = Modifier.align(Alignment.CenterVertically))
        }
        Spacer(Modifier.height(20.dp))
        Text("Featured", style = MaterialTheme.typography.headlineMedium)
        Spacer(Modifier.height(10.dp))
        LazyVerticalGrid(
            columns = GridCells.Adaptive(220.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxSize()
        ) {
            items(featured, key = { it.id }) { product ->
                ProductCard(product, onAdd)
            }
        }
    }
}

@Composable
private fun MenuPane(products: List<Product>, onAdd: (String) -> Unit) {
    var query by remember { mutableStateOf("") }
    var category by remember { mutableStateOf<ProductCategory?>(null) }
    val filtered = products.filter { product ->
        (category == null || product.category == category) &&
            (query.isBlank() ||
                product.name.contains(query, true) ||
                product.brand.contains(query, true) ||
                product.effects.contains(query, true))
    }
    Column(Modifier.fillMaxSize()) {
        Text("Menu", style = MaterialTheme.typography.headlineLarge)
        Spacer(Modifier.height(8.dp))
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            label = { Text("Search") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true
        )
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(selected = category == null, onClick = { category = null }, label = { Text("All") })
            ProductCategory.entries.forEach { cat ->
                FilterChip(
                    selected = category == cat,
                    onClick = { category = cat },
                    label = { Text(cat.label) }
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        LazyVerticalGrid(
            columns = GridCells.Adaptive(240.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxSize()
        ) {
            items(filtered, key = { it.id }) { ProductCard(it, onAdd) }
        }
    }
}

@Composable
private fun ProductCard(product: Product, onAdd: (String) -> Unit) {
    Surface(shape = RoundedCornerShape(14.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(product.name, style = MaterialTheme.typography.titleLarge)
            Text("${product.brand} · ${product.category.label}", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text("${product.strainType.label} · THC ${product.thcPercent}%")
            if (!product.published) {
                Text("Draft — not visible to customers", color = MaterialTheme.colorScheme.error)
            }
            Text(product.effects, style = MaterialTheme.typography.bodyMedium)
            Text("$${ "%.2f".format(product.price) } / ${product.unitLabel}", style = MaterialTheme.typography.titleLarge)
            Text("Stock ${product.stockQuantity}", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Button(
                onClick = { onAdd(product.id) },
                enabled = product.stockQuantity > 0,
                shape = RoundedCornerShape(10.dp)
            ) { Text("Add to bag") }
        }
    }
}

@Composable
private fun CartPane(
    repository: CompanionRepository,
    defaultName: String,
    onRefresh: () -> Unit,
    onMessage: (String?) -> Unit
) {
    var pickup by remember { mutableStateOf(defaultName) }
    var notes by remember { mutableStateOf("") }
    val cart = repository.cartSummary()
    Column(Modifier.fillMaxSize()) {
        Text("Cart & pickup", style = MaterialTheme.typography.headlineLarge)
        Spacer(Modifier.height(12.dp))
        if (cart.lines.isEmpty()) {
            Text("Your bag is empty.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            return
        }
        val listState = rememberLazyListState()
        Box(Modifier.weight(1f)) {
            LazyColumn(state = listState, verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(cart.lines, key = { it.product.id }) { line ->
                    Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp) {
                        Row(
                            Modifier.fillMaxWidth().padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text(line.product.name, style = MaterialTheme.typography.titleLarge)
                                Text("$${ "%.2f".format(line.product.price) } each")
                            }
                            OutlinedButton(onClick = {
                                repository.setCartQuantity(line.product.id, line.quantity - 1)
                                onRefresh()
                            }) { Text("−") }
                            Text("${line.quantity}", modifier = Modifier.padding(horizontal = 10.dp))
                            OutlinedButton(onClick = {
                                repository.setCartQuantity(line.product.id, line.quantity + 1)
                                onRefresh()
                            }) { Text("+") }
                        }
                    }
                }
            }
            VerticalScrollbar(
                adapter = rememberScrollbarAdapter(listState),
                modifier = Modifier.align(Alignment.CenterEnd).fillMaxHeight()
            )
        }
        Spacer(Modifier.height(12.dp))
        Text("Subtotal $${"%.2f".format(cart.subtotal)} · Tax $${"%.2f".format(cart.tax)} · Total $${"%.2f".format(cart.total)}")
        Field(pickup, { pickup = it }, "Pickup name")
        Field(notes, { notes = it }, "Order notes")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = {
                val order = repository.placePickupOrder(pickup, notes)
                onRefresh()
                onMessage(if (order != null) "Order ${order.id} ready for pickup." else "Cart is empty.")
            }) { Text("Place pickup order") }
            OutlinedButton(onClick = {
                repository.clearCart()
                onRefresh()
            }) { Text("Clear bag") }
        }
    }
}

@Composable
private fun OrdersPane(repository: CompanionRepository) {
    val orders = repository.visibleOrders()
    val dateFormat = remember { SimpleDateFormat("MMM d, yyyy h:mm a", Locale.US) }
    Column(Modifier.fillMaxSize()) {
        Text("Orders", style = MaterialTheme.typography.headlineLarge)
        Spacer(Modifier.height(12.dp))
        if (orders.isEmpty()) {
            Text("No orders yet.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            return
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(orders, key = { it.id }) { order ->
                Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text("Order ${order.id}", style = MaterialTheme.typography.titleLarge)
                        Text(order.status)
                        Text("${order.itemCount} items · $${"%.2f".format(order.total)}")
                        Text("Pickup: ${order.pickupName}")
                        if (order.customerEmail.isNotBlank()) Text(order.customerEmail)
                        Text(dateFormat.format(Date(order.createdAt)), color = MaterialTheme.colorScheme.onSurfaceVariant)
                        repository.orderLinesFor(order.id).forEach { line ->
                            Text("• ${line.quantity} × ${line.productName}")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun InventoryPane(
    repository: CompanionRepository,
    onRefresh: () -> Unit,
    onMessage: (String?) -> Unit
) {
    val products = repository.allProducts().sortedWith(
        compareBy<Product> { it.published }.thenBy { it.name }
    )
    val drafts = products.count { !it.published }
    Column(Modifier.fillMaxSize()) {
        Text("Stock", style = MaterialTheme.typography.headlineLarge)
        Text(
            "Customers only see published products. $drafts unpublished draft(s).",
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(12.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(products, key = { it.id }) { product ->
                Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(product.name, style = MaterialTheme.typography.titleLarge)
                                Text(
                                    "${product.sku} · ${product.category.label} · " +
                                        if (product.published) "Published" else "Draft",
                                    color = MaterialTheme.colorScheme.onSurfaceVariant
                                )
                            }
                            Text("${product.stockQuantity}", modifier = Modifier.padding(horizontal = 12.dp))
                            OutlinedButton(onClick = {
                                repository.adjustStock(product.id, -1)
                                onRefresh()
                            }) { Text("−") }
                            Spacer(Modifier.width(6.dp))
                            OutlinedButton(onClick = {
                                repository.adjustStock(product.id, 1)
                                onRefresh()
                            }) { Text("+") }
                        }
                        Button(onClick = {
                            val next = !product.published
                            when (val result = repository.setPublished(product.id, next)) {
                                is OpResult.Success -> {
                                    onRefresh()
                                    onMessage(result.message)
                                }
                                is OpResult.Error -> onMessage(result.message)
                            }
                        }) {
                            Text(if (product.published) "Unpublish from menu" else "Publish to customers")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CustomersPane(repository: CompanionRepository) {
    val customers = repository.allCustomers()
    val dateFormat = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }
    Column(Modifier.fillMaxSize()) {
        Text("Customers", style = MaterialTheme.typography.headlineLarge)
        Text("${customers.size} accounts", color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.height(12.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(customers, key = { it.id }) { c ->
                Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text(c.fullName, style = MaterialTheme.typography.titleLarge)
                            Text(c.role.label, color = MaterialTheme.colorScheme.primary)
                        }
                        Text(c.email)
                        if (c.phone.isNotBlank()) Text("Phone: ${c.phone}")
                        if (c.dateOfBirth.isNotBlank()) Text("DOB: ${c.dateOfBirth}")
                        if (c.notes.isNotBlank()) Text("Notes: ${c.notes}")
                        Text("Joined ${dateFormat.format(Date(c.createdAt))}", color = MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
            }
        }
    }
}

@Composable
private fun StorePane() {
    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(14.dp)
    ) {
        Text("Store", style = MaterialTheme.typography.headlineLarge)
        InfoBlock("Address", "1847 Grove Avenue\nPortland, OR 97214")
        InfoBlock("Hours", "Mon–Thu 10am–8pm\nFri–Sat 10am–9pm\nSun 11am–6pm")
        InfoBlock("Contact", "hello@nativepure.example\n(503) 555-0187")
        InfoBlock("Pickup", "Order in the companion or phone app, then pick up in store. Bring a valid ID.")
        InfoBlock("Responsible use", "18+ only with valid government ID. Do not drive impaired. Keep products away from children and pets.")
    }
}

@Composable
private fun InfoBlock(title: String, body: String) {
    Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, style = MaterialTheme.typography.titleLarge)
            Text(body)
        }
    }
}

@Composable
private fun AccountPane(
    repository: CompanionRepository,
    customer: CustomerProfile,
    onRefresh: () -> Unit,
    onMessage: (String?) -> Unit,
    onLogout: () -> Unit
) {
    var fullName by remember(customer.id) { mutableStateOf(customer.fullName) }
    var phone by remember(customer.id) { mutableStateOf(customer.phone) }
    var dob by remember(customer.id) { mutableStateOf(customer.dateOfBirth) }
    var notes by remember(customer.id) { mutableStateOf(customer.notes) }
    var currentPw by remember { mutableStateOf("") }
    var newPw by remember { mutableStateOf("") }
    var confirmPw by remember { mutableStateOf("") }
    var staffName by remember { mutableStateOf("") }
    var staffEmail by remember { mutableStateOf("") }
    var staffPw by remember { mutableStateOf("") }

    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Text("Account", style = MaterialTheme.typography.headlineLarge)
        Text("${customer.email} · ${customer.role.label}", color = MaterialTheme.colorScheme.onSurfaceVariant)
        Field(fullName, { fullName = it }, "Full name")
        Field(phone, { phone = it }, "Phone")
        Field(dob, { dob = it }, "Date of birth")
        Field(notes, { notes = it }, "Notes")
        Button(onClick = {
            when (val result = repository.updateProfile(fullName, phone, dob, notes, customer.marketingOptIn)) {
                is AuthResult.Success -> {
                    onRefresh()
                    onMessage("Profile saved.")
                }
                is AuthResult.Error -> onMessage(result.message)
            }
        }) { Text("Save profile") }

        HorizontalDivider(Modifier.padding(vertical = 8.dp))
        Text("Security", style = MaterialTheme.typography.headlineMedium)
        Text(PasswordPolicy.requirementsLabel, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Field(currentPw, { currentPw = it }, "Current password", password = true)
        Field(newPw, { newPw = it }, "New password", password = true)
        Field(confirmPw, { confirmPw = it }, "Confirm new password", password = true)
        Button(onClick = {
            when (val result = repository.changePassword(currentPw, newPw, confirmPw)) {
                is AuthResult.Success -> {
                    currentPw = ""; newPw = ""; confirmPw = ""
                    onRefresh()
                    onMessage("Password updated.")
                }
                is AuthResult.Error -> onMessage(result.message)
            }
        }) { Text("Update password") }
        Text(
            "Login protection: ${PasswordPolicy.MAX_FAILED_ATTEMPTS} failed attempts lock sign-in for 5 minutes.",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.bodyMedium
        )

        if (customer.role.canManageStaff) {
            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            Text("Staff sub-accounts", style = MaterialTheme.typography.headlineMedium)
            Field(staffName, { staffName = it }, "Staff full name")
            Field(staffEmail, { staffEmail = it }, "Staff email")
            Field(staffPw, { staffPw = it }, "Temporary password", password = true)
            Button(onClick = {
                when (val result = repository.createStaff(staffEmail, staffPw, staffName)) {
                    is AuthResult.Success -> {
                        staffName = ""; staffEmail = ""; staffPw = ""
                        onMessage("Staff created for ${result.customer.email}. They must change password on first login.")
                        onRefresh()
                    }
                    is AuthResult.Error -> onMessage(result.message)
                }
            }) { Text("Create staff account") }

            repository.staffAccounts().forEach { staff ->
                Surface(shape = RoundedCornerShape(10.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text("${staff.fullName} · ${staff.email}")
                        Text(if (staff.enabled) "Enabled" else "Disabled")
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedButton(onClick = {
                                when (val result = repository.setStaffEnabled(staff.id, !staff.enabled)) {
                                    is OpResult.Success -> { onMessage(result.message); onRefresh() }
                                    is OpResult.Error -> onMessage(result.message)
                                }
                            }) { Text(if (staff.enabled) "Disable" else "Enable") }
                        }
                    }
                }
            }
        }

        if (customer.role.canManageInventory) {
            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            Text("Desktop sync", style = MaterialTheme.typography.headlineMedium)
            Text("Share inventory JSON with the Android app.", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Button(onClick = {
                try {
                    val json = repository.exportSyncJson()
                    val dialog = FileDialog(null as Frame?, "Export sync", FileDialog.SAVE)
                    dialog.file = "nativepure-sync.json"
                    dialog.isVisible = true
                    val dir = dialog.directory ?: return@Button
                    val name = dialog.file ?: return@Button
                    File(dir, name).writeText(json)
                    onMessage("Exported sync file.")
                } catch (t: Throwable) {
                    onMessage(t.message ?: "Export failed.")
                }
            }) { Text("Export sync JSON") }
            Button(onClick = {
                val dialog = FileDialog(null as Frame?, "Import sync", FileDialog.LOAD)
                dialog.isVisible = true
                val dir = dialog.directory ?: return@Button
                val name = dialog.file ?: return@Button
                val text = File(dir, name).readText()
                when (val result = repository.importSyncJson(text)) {
                    is OpResult.Success -> { onMessage(result.message); onRefresh() }
                    is OpResult.Error -> onMessage(result.message)
                }
            }) { Text("Import sync JSON") }
        }

        Spacer(Modifier.height(8.dp))
        OutlinedButton(onClick = onLogout) { Text("Log out") }
    }
}

@Composable
private fun BrandMark(size: androidx.compose.ui.unit.Dp) {
    Image(
        painter = painterResource("logo_main.png"),
        contentDescription = "Native Pure",
        modifier = Modifier.size(size).clip(RoundedCornerShape(size / 5)),
        contentScale = ContentScale.Crop
    )
}

@Composable
private fun Field(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    password: Boolean = false
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        visualTransformation = if (password) PasswordVisualTransformation() else androidx.compose.ui.text.input.VisualTransformation.None,
        keyboardOptions = KeyboardOptions(
            keyboardType = if (password) KeyboardType.Password else KeyboardType.Text
        )
    )
}
