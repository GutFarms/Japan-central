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
import com.nativepure.companion.data.EmailCodeIssue
import com.nativepure.companion.data.LoyaltyPoints
import com.nativepure.companion.data.NavSection
import com.nativepure.companion.data.OpResult
import com.nativepure.companion.data.OrderStatus
import com.nativepure.companion.data.PasswordPolicy
import com.nativepure.companion.data.Product
import com.nativepure.companion.data.ProductCategory
import com.nativepure.companion.data.StrainType
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
    var pendingEmailCode by remember { mutableStateOf(repository.peekIssuedEmailCode()) }
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
                        pendingEmailCode = repository.peekIssuedEmailCode()
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
                        pendingEmailCode = repository.peekIssuedEmailCode()
                        authError = null
                    }
                    is AuthResult.Error -> authError = result.message
                }
            },
            onClearError = { authError = null }
        )
        !customer!!.emailVerified -> EmailVerifyPane(
            email = customer!!.email,
            issuedCode = pendingEmailCode,
            error = authError,
            onVerify = { code ->
                when (val result = repository.verifyEmailCode(code)) {
                    is AuthResult.Success -> {
                        customer = result.customer
                        pendingEmailCode = null
                        authError = null
                    }
                    is AuthResult.Error -> authError = result.message
                }
            },
            onResend = {
                when (val result = repository.resendEmailVerificationCode()) {
                    is AuthResult.Success -> {
                        pendingEmailCode = repository.peekIssuedEmailCode()
                        authError = null
                    }
                    is AuthResult.Error -> authError = result.message
                }
            },
            onClearError = { authError = null },
            onLogout = {
                repository.logout()
                customer = null
                pendingEmailCode = null
            }
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
                pendingEmailCode = null
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
            Text(
                "grown calm · picked fresh",
                style = MaterialTheme.typography.bodyMedium,
                color = colors.primary
            )
            Text("Are you 18 or older?", style = MaterialTheme.typography.headlineMedium)
            Text(
                "Enter only if you are of legal age to browse our living menu.",
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
                "Desktop companion for menu, pickup orders, inventory, and account security.",
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
private fun EmailVerifyPane(
    email: String,
    issuedCode: EmailCodeIssue?,
    error: String?,
    onVerify: (String) -> Unit,
    onResend: () -> Unit,
    onClearError: () -> Unit,
    onLogout: () -> Unit
) {
    var code by remember { mutableStateOf("") }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Surface(shape = RoundedCornerShape(18.dp), tonalElevation = 2.dp, modifier = Modifier.width(420.dp)) {
            Column(Modifier.padding(28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text("Verify your email", style = MaterialTheme.typography.headlineMedium)
                Text(
                    "Enter the 6-digit code sent to $email",
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                if (issuedCode != null) {
                    Surface(
                        color = MaterialTheme.colorScheme.secondaryContainer,
                        shape = RoundedCornerShape(12.dp)
                    ) {
                        Column(Modifier.padding(12.dp)) {
                            Text("Offline delivery", style = MaterialTheme.typography.titleSmall)
                            Text(
                                "Code: ${issuedCode.code}",
                                style = MaterialTheme.typography.headlineMedium
                            )
                        }
                    }
                }
                Field(code, {
                    code = it.filter { ch -> ch.isDigit() }.take(6)
                    onClearError()
                }, "6-digit code")
                if (error != null) Text(error, color = MaterialTheme.colorScheme.error)
                Button(
                    onClick = { onVerify(code) },
                    enabled = code.length == 6,
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Verify email") }
                TextButton(onClick = onResend) { Text("Resend code") }
                TextButton(onClick = onLogout) { Text("Log out") }
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
        if (customer.role.canViewSensitiveInfo) {
            add(NavSection.CUSTOMERS)
            add(NavSection.REQUESTS)
        }
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
                    repository = repository,
                    customer = customer,
                    featured = repository.featuredProducts(),
                    cartCount = repository.cartSummary().itemCount,
                    onOpenMenu = { onSection(NavSection.MENU) },
                    onAdd = {
                        repository.addToCart(it)
                        onRefresh()
                        onMessage("Added to bag.")
                    },
                    onRefresh = onRefresh,
                    onMessage = onMessage
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
                NavSection.ORDERS -> OrdersPane(repository, onRefresh, onMessage)
                NavSection.INVENTORY -> InventoryPane(repository, onRefresh, onMessage)
                NavSection.CUSTOMERS -> CustomersPane(repository)
                NavSection.REQUESTS -> RequestsPane(repository, onRefresh, onMessage)
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
    repository: CompanionRepository,
    customer: CustomerProfile,
    featured: List<Product>,
    cartCount: Int,
    onOpenMenu: () -> Unit,
    onAdd: (String) -> Unit,
    onRefresh: () -> Unit,
    onMessage: (String?) -> Unit
) {
    var requestName by remember { mutableStateOf("") }
    var requestNotes by remember { mutableStateOf("") }
    val stats = repository.requestBoxStats()
    val requests = repository.visibleProductRequests().take(5)

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(4.dp)) {
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
        Spacer(Modifier.height(16.dp))
        Surface(shape = RoundedCornerShape(14.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
            Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Request box", style = MaterialTheme.typography.headlineMedium)
                Text(
                    "${stats.requesterToCustomerPercent.toInt()}% customer ratio · " +
                        "${stats.uniqueRequesters}/${stats.totalCustomers} customers · " +
                        "${stats.totalRequests} requests"
                )
                if (!customer.role.canViewSensitiveInfo) {
                    Field(requestName, { requestName = it }, "What are you looking for?")
                    Field(requestNotes, { requestNotes = it }, "Notes (optional)")
                    Button(
                        onClick = {
                            when (val result = repository.submitProductRequest(requestName, requestNotes)) {
                                is OpResult.Success -> {
                                    requestName = ""
                                    requestNotes = ""
                                    onMessage(result.message)
                                    onRefresh()
                                }
                                is OpResult.Error -> onMessage(result.message)
                            }
                        },
                        enabled = requestName.trim().length >= 2
                    ) { Text("Send request") }
                }
                requests.forEach { req ->
                    Text("• ${req.productName} (${req.status})" +
                        if (customer.role.canViewSensitiveInfo) " — ${req.customerName}" else "")
                }
            }
        }
        Spacer(Modifier.height(20.dp))
        Text("Featured", style = MaterialTheme.typography.headlineMedium)
        Spacer(Modifier.height(10.dp))
        LazyVerticalGrid(
            columns = GridCells.Adaptive(220.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.height(420.dp)
        ) {
            items(featured, key = { it.id }) { product ->
                ProductCard(product, onAdd)
            }
        }
    }
}

@Composable
private fun RequestsPane(
    repository: CompanionRepository,
    onRefresh: () -> Unit,
    onMessage: (String?) -> Unit
) {
    val stats = repository.requestBoxStats()
    val requests = repository.visibleProductRequests()
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Text("Request box", style = MaterialTheme.typography.headlineLarge)
        Text(
            "${stats.requesterToCustomerPercent.toInt()}% of customers have requested · " +
                "${stats.uniqueRequesters} / ${stats.totalCustomers}"
        )
        Spacer(Modifier.height(12.dp))
        requests.forEach { req ->
            Surface(
                shape = RoundedCornerShape(12.dp),
                tonalElevation = 1.dp,
                modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp)
            ) {
                Column(Modifier.padding(12.dp)) {
                    Text(req.productName, style = MaterialTheme.typography.titleLarge)
                    Text("${req.customerName} · ${req.customerEmail}")
                    if (req.notes.isNotBlank()) Text(req.notes)
                    Text("Status: ${req.status}")
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        if (req.status == "Open") {
                            TextButton(onClick = {
                                when (val result = repository.markRequestFulfilled(req.id)) {
                                    is OpResult.Success -> {
                                        onMessage(result.message)
                                        onRefresh()
                                    }
                                    is OpResult.Error -> onMessage(result.message)
                                }
                            }) { Text("Mark fulfilled") }
                        }
                        if (req.status != "Declined") {
                            TextButton(onClick = {
                                when (val result = repository.createDraftFromRequest(req.id)) {
                                    is OpResult.Success -> {
                                        onMessage(result.message)
                                        onRefresh()
                                    }
                                    is OpResult.Error -> onMessage(result.message)
                                }
                            }) { Text("Create draft in Stock") }
                        }
                    }
                }
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
    var redeemPoints by remember { mutableStateOf(0) }
    val cart = repository.cartSummary()
    val customer = repository.currentCustomer()
    val canRedeem = customer?.role == AccountRole.CUSTOMER
    val maxRedeem = if (canRedeem) {
        LoyaltyPoints.maxRedeemablePoints(customer!!.loyaltyPoints, cart.total)
    } else {
        0
    }
    val safeRedeem = redeemPoints.coerceAtMost(maxRedeem).let { it - (it % LoyaltyPoints.REDEEM_POINTS_PER_DOLLAR) }
    val discount = LoyaltyPoints.discountForPoints(safeRedeem)
    val payable = (cart.total - discount).coerceAtLeast(0.0)
    val earnPreview = LoyaltyPoints.pointsForSpend(payable)

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
        if (canRedeem && customer != null) {
            Text(
                "You have ${customer.loyaltyPoints} points · redeem 100 pts = $1 off",
                color = MaterialTheme.colorScheme.secondary
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                FilterChip(
                    selected = safeRedeem == 0,
                    onClick = { redeemPoints = 0 },
                    label = { Text("No redeem") }
                )
                listOf(100, 200, 500, maxRedeem).distinct().filter { it in 100..maxRedeem }.forEach { pts ->
                    FilterChip(
                        selected = safeRedeem == pts,
                        onClick = { redeemPoints = pts },
                        label = { Text("$pts pts") }
                    )
                }
            }
            if (discount > 0) {
                Text("Discount −$${"%.2f".format(discount)} · Pay $${"%.2f".format(payable)}")
            }
        }
        Text(
            "You’ll earn $earnPreview points on this order (1 pt per $1 paid)",
            color = MaterialTheme.colorScheme.secondary
        )
        Field(pickup, { pickup = it }, "Pickup name")
        Field(notes, { notes = it }, "Order notes")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = {
                val order = repository.placePickupOrder(pickup, notes, safeRedeem)
                onRefresh()
                redeemPoints = 0
                onMessage(
                    if (order != null) {
                        buildString {
                            append("Order ${order.id} ready for pickup.")
                            if (order.discount > 0) {
                                append(" Saved $${"%.2f".format(order.discount)}.")
                            }
                            if (order.pointsEarned > 0) {
                                append(" +${order.pointsEarned} points!")
                            }
                        }
                    } else {
                        "Cart is empty."
                    }
                )
            }) { Text("Place pickup order") }
            OutlinedButton(onClick = {
                repository.clearCart()
                onRefresh()
            }) { Text("Clear bag") }
        }
    }
}

@Composable
private fun OrdersPane(repository: CompanionRepository, onRefresh: () -> Unit, onMessage: (String?) -> Unit) {
    val orders = repository.visibleOrders()
    val canManage = repository.currentCustomer()?.role?.canViewSensitiveInfo == true
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
                        if (order.discount > 0) {
                            Text(
                                "Redeemed ${order.pointsRedeemed} pts (−$${"%.2f".format(order.discount)})",
                                color = MaterialTheme.colorScheme.secondary
                            )
                        }
                        if (order.pointsEarned > 0) {
                            Text("+${order.pointsEarned} loyalty points", color = MaterialTheme.colorScheme.secondary)
                        }
                        Text("Pickup: ${order.pickupName}")
                        if (order.customerEmail.isNotBlank()) Text(order.customerEmail)
                        Text(dateFormat.format(Date(order.createdAt)), color = MaterialTheme.colorScheme.onSurfaceVariant)
                        repository.orderLinesFor(order.id).forEach { line ->
                            Text("• ${line.quantity} × ${line.productName}")
                        }
                        TextButton(onClick = {
                            val text = repository.orderReceiptText(order.id)
                            onMessage(text ?: "Could not build receipt.")
                        }) { Text("Show receipt") }
                        if (canManage) {
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                OrderStatus.staffActions.forEach { status ->
                                    FilterChip(
                                        selected = order.status == status,
                                        onClick = {
                                            when (val result = repository.updateOrderStatus(order.id, status)) {
                                                is OpResult.Success -> {
                                                    onMessage(result.message)
                                                    onRefresh()
                                                }
                                                is OpResult.Error -> onMessage(result.message)
                                            }
                                        },
                                        label = { Text(status) }
                                    )
                                }
                            }
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
    var query by remember { mutableStateOf("") }
    var filterPublished by remember { mutableStateOf("All") }
    var editing by remember { mutableStateOf<Product?>(null) }
    val products = repository.allProducts().sortedWith(
        compareBy<Product> { it.published }.thenBy { it.name }
    )
    val filtered = products.filter { product ->
        val matchesQuery = query.isBlank() ||
            product.name.contains(query, true) ||
            product.brand.contains(query, true) ||
            product.sku.contains(query, true)
        val matchesPublished = when (filterPublished) {
            "Live" -> product.published
            "Draft" -> !product.published
            else -> true
        }
        matchesQuery && matchesPublished
    }
    val drafts = products.count { !it.published }
    Column(Modifier.fillMaxSize()) {
        Text("Stock", style = MaterialTheme.typography.headlineLarge)
        Text(
            "Admin/staff can edit price, SKU, and stock. $drafts unpublished draft(s).",
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(onClick = {
                when (val result = repository.createBlankDraftProduct()) {
                    is OpResult.Success -> {
                        onMessage(result.message)
                        onRefresh()
                    }
                    is OpResult.Error -> onMessage(result.message)
                }
            }) { Text("Add product") }
        }
        Spacer(Modifier.height(8.dp))
        OutlinedTextField(
            value = query,
            onValueChange = { query = it },
            label = { Text("Search name, brand, or SKU") },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true
        )
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf("All", "Live", "Draft").forEach { status ->
                FilterChip(
                    selected = filterPublished == status,
                    onClick = { filterPublished = status },
                    label = { Text(status) }
                )
            }
        }
        Text(
            "Showing ${filtered.size} of ${products.size}",
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(12.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            editing?.let { product ->
                item {
                    CompanionProductEditor(
                        product = product,
                        onCancel = { editing = null },
                        onSave = { updated ->
                            when (val result = repository.saveProduct(updated)) {
                                is OpResult.Success -> {
                                    onMessage(result.message)
                                    editing = null
                                    onRefresh()
                                }
                                is OpResult.Error -> onMessage(result.message)
                            }
                        }
                    )
                }
            }
            items(filtered, key = { it.id }) { product ->
                Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text(product.name, style = MaterialTheme.typography.titleLarge)
                                Text(
                                    "${product.sku.ifBlank { "no SKU" }} · $${"%.2f".format(product.price)} · " +
                                        "${product.category.label} · " +
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
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedButton(onClick = { editing = product }) { Text("Edit") }
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
                                Text(if (product.published) "Unpublish" else "Publish")
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CompanionProductEditor(
    product: Product,
    onCancel: () -> Unit,
    onSave: (Product) -> Unit
) {
    var name by remember(product.id) { mutableStateOf(product.name) }
    var brand by remember(product.id) { mutableStateOf(product.brand) }
    var sku by remember(product.id) { mutableStateOf(product.sku) }
    var price by remember(product.id) { mutableStateOf(if (product.price > 0) product.price.toString() else "") }
    var stock by remember(product.id) { mutableStateOf(product.stockQuantity.toString()) }
    var unit by remember(product.id) { mutableStateOf(product.unitLabel) }
    var description by remember(product.id) { mutableStateOf(product.description) }
    var category by remember(product.id) { mutableStateOf(product.category) }
    var published by remember(product.id) { mutableStateOf(product.published) }

    Surface(shape = RoundedCornerShape(12.dp), color = MaterialTheme.colorScheme.secondaryContainer) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Edit product", style = MaterialTheme.typography.titleLarge)
            Field(name, { name = it }, "Name")
            Field(brand, { brand = it }, "Brand")
            Field(sku, { sku = it }, "SKU")
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = price,
                    onValueChange = { price = it },
                    label = { Text("Price") },
                    modifier = Modifier.weight(1f),
                    singleLine = true
                )
                OutlinedTextField(
                    value = stock,
                    onValueChange = { stock = it.filter { ch -> ch.isDigit() } },
                    label = { Text("Stock") },
                    modifier = Modifier.weight(1f),
                    singleLine = true
                )
            }
            Field(unit, { unit = it }, "Unit")
            Field(description, { description = it }, "Description")
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ProductCategory.entries.forEach { cat ->
                    FilterChip(
                        selected = category == cat,
                        onClick = { category = cat },
                        label = { Text(cat.label) }
                    )
                }
            }
            FilterChip(
                selected = published,
                onClick = { published = !published },
                label = { Text(if (published) "Published" else "Draft") }
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = onCancel) { Text("Cancel") }
                Button(
                    onClick = {
                        onSave(
                            product.copy(
                                name = name,
                                brand = brand,
                                sku = sku,
                                price = price.toDoubleOrNull() ?: 0.0,
                                stockQuantity = stock.toIntOrNull() ?: 0,
                                unitLabel = unit,
                                description = description,
                                category = category,
                                strainType = product.strainType.takeIf { it != StrainType.NONE }
                                    ?: StrainType.HYBRID,
                                published = published
                            )
                        )
                    },
                    enabled = name.trim().length >= 2
                ) { Text("Save changes") }
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
                        if (c.loyaltyPoints > 0 || c.lifetimeSpend > 0) {
                            Text("Points: ${c.loyaltyPoints} · Spent $${"%.2f".format(c.lifetimeSpend)}")
                        }
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
        Text(
            "Loyalty: ${customer.loyaltyPoints} pts · Lifetime spend $${"%.2f".format(customer.lifetimeSpend)} " +
                "(1 pt per $1)",
            color = MaterialTheme.colorScheme.secondary
        )
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
