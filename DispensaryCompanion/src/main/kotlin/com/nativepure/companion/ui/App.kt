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
import com.nativepure.companion.data.Privacy
import com.nativepure.companion.data.Product
import com.nativepure.companion.data.ProductCategory
import com.nativepure.companion.data.ProductSize
import com.nativepure.companion.data.SizePricing
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
    var sessionLocked by remember { mutableStateOf(repository.isSessionLocked()) }
    var lockError by remember { mutableStateOf<String?>(null) }

    fun refresh() {
        customer = repository.currentCustomer()
        sessionLocked = repository.isSessionLocked()
        tick++
    }

    // Poll idle lock while signed in.
    androidx.compose.runtime.LaunchedEffect(customer?.id) {
        while (customer != null) {
            kotlinx.coroutines.delay(15_000)
            if (repository.isSessionLocked()) {
                sessionLocked = true
            }
        }
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
                        sessionLocked = false
                        section = if (result.customer.role.canViewSensitiveInfo) {
                            NavSection.POS
                        } else {
                            NavSection.HOME
                        }
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
            email = Privacy.maskEmail(customer!!.email),
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
        sessionLocked -> SessionLockPane(
            name = customer!!.fullName,
            error = lockError,
            onUnlock = { password ->
                when (val result = repository.unlockSession(password)) {
                    is AuthResult.Success -> {
                        sessionLocked = false
                        lockError = null
                        customer = result.customer
                    }
                    is AuthResult.Error -> lockError = result.message
                }
            },
            onLogout = {
                repository.logout()
                customer = null
                sessionLocked = false
                lockError = null
            }
        )
        else -> MainShell(
            repository = repository,
            customer = customer!!,
            section = section,
            message = message,
            tick = tick,
            onSection = {
                repository.touchActivity()
                section = it
            },
            onMessage = { message = it },
            onRefresh = {
                repository.touchActivity()
                refresh()
            },
            onLogout = {
                repository.logout()
                customer = null
                pendingEmailCode = null
                section = NavSection.HOME
            },
            onLock = {
                repository.lockSessionNow()
                sessionLocked = true
            }
        )
    }
}

@Composable
private fun SessionLockPane(
    name: String,
    error: String?,
    onUnlock: (String) -> Unit,
    onLogout: () -> Unit
) {
    var password by remember { mutableStateOf("") }
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Surface(shape = RoundedCornerShape(18.dp), tonalElevation = 2.dp, modifier = Modifier.width(420.dp)) {
            Column(Modifier.padding(28.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text("Session locked", style = MaterialTheme.typography.headlineMedium)
                Text(
                    "Idle lock protects customer personal information on this register. Re-enter the password for $name.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
                Field(password, { password = it }, "Password", password = true)
                if (error != null) Text(error, color = MaterialTheme.colorScheme.error)
                Button(
                    onClick = { onUnlock(password) },
                    enabled = password.isNotBlank(),
                    modifier = Modifier.fillMaxWidth()
                ) { Text("Unlock") }
                TextButton(onClick = onLogout) { Text("Log out") }
            }
        }
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
                "Desktop point of sale for register sales, pickup handoff, inventory, and account security.",
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
                Text("Native Pure POS", style = MaterialTheme.typography.headlineMedium)
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
                        color = if (issuedCode.deliveredByMail) {
                            MaterialTheme.colorScheme.primaryContainer
                        } else {
                            MaterialTheme.colorScheme.secondaryContainer
                        },
                        shape = RoundedCornerShape(12.dp)
                    ) {
                        Column(Modifier.padding(12.dp)) {
                            if (issuedCode.deliveredByMail) {
                                Text("Code emailed", style = MaterialTheme.typography.titleSmall)
                                Text(
                                    "Check your inbox for the 6-digit code. It is never shown on this screen.",
                                    color = MaterialTheme.colorScheme.onPrimaryContainer
                                )
                            } else {
                                Text(
                                    "Mail server offline",
                                    style = MaterialTheme.typography.titleSmall
                                )
                                Text(
                                    "Ask an admin to check the local mail server or the secure verification-code.dev.txt file in the data folder. Codes are never shown on the register.",
                                    color = MaterialTheme.colorScheme.onSecondaryContainer
                                )
                                issuedCode.mailError?.let {
                                    Text("($it)", color = MaterialTheme.colorScheme.onSecondaryContainer)
                                }
                            }
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
    onLogout: () -> Unit,
    onLock: () -> Unit
) {
    val sections = buildList {
        if (customer.role.canViewSensitiveInfo) {
            add(NavSection.POS)
            add(NavSection.ORDERS)
            if (customer.role.canManageInventory) add(NavSection.INVENTORY)
            add(NavSection.CUSTOMERS)
            add(NavSection.REQUESTS)
            add(NavSection.MENU)
            add(NavSection.DEALS)
        } else {
            add(NavSection.HOME)
            add(NavSection.MENU)
            add(NavSection.DEALS)
            add(NavSection.CART)
            add(NavSection.ORDERS)
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
                NavSection.POS -> PosPane(
                    repository = repository,
                    cashier = customer,
                    onRefresh = onRefresh,
                    onMessage = onMessage
                )
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
                        onMessage(if (customer.role.canViewSensitiveInfo) "Added to ticket." else "Added to bag.")
                    }
                )
                NavSection.DEALS -> DealsPane(
                    products = repository.dealProducts(),
                    onAdd = {
                        repository.addToCart(it)
                        onRefresh()
                        onMessage(if (customer.role.canViewSensitiveInfo) "Added deal to ticket." else "Added deal item to bag.")
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
                    onLogout = onLogout,
                    onLock = onLock
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
                    Text("${req.customerName} · ${Privacy.maskEmail(req.customerEmail)}")
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
private fun DealsPane(products: List<Product>, onAdd: (String) -> Unit) {
    Column(Modifier.fillMaxSize()) {
        Text("Deals", style = MaterialTheme.typography.headlineLarge)
        Text(
            if (products.isEmpty()) "No active promos right now."
            else "${products.size} active deals — savings apply at checkout.",
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(12.dp))
        if (products.isEmpty()) {
            Text("Staff can mark products On Deals from Stock → Edit.")
            return
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(products, key = { it.id }) { product ->
                Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Row(
                        Modifier.padding(14.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Text(product.name, style = MaterialTheme.typography.titleLarge)
                            Text(
                                product.dealLabel.ifBlank { "${product.dealPercent}% off" },
                                color = MaterialTheme.colorScheme.secondary
                            )
                            Text(
                                "$${ "%.2f".format(product.price) } → $${"%.2f".format(product.effectivePrice)}",
                                style = MaterialTheme.typography.titleMedium
                            )
                        }
                        Button(
                            onClick = { onAdd(product.id) },
                            enabled = product.stockQuantity > 0
                        ) { Text("Add") }
                    }
                }
            }
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
            if (product.hasActiveDeal) {
                Text(
                    "${product.dealLabel.ifBlank { "Deal" }} · −${product.dealPercent}%",
                    color = MaterialTheme.colorScheme.secondary
                )
            }
            Text(product.effects, style = MaterialTheme.typography.bodyMedium)
            if (product.hasActiveDeal) {
                Text(
                    "$${ "%.2f".format(product.effectivePrice) } (was $${"%.2f".format(product.price)}) / ${product.displayUnitLabel()}",
                    style = MaterialTheme.typography.titleLarge
                )
            } else {
                Text("$${ "%.2f".format(product.price) } / ${product.displayUnitLabel()}", style = MaterialTheme.typography.titleLarge)
            }
            Text("Stock ${product.stockQuantity}", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Button(
                onClick = { onAdd(product.id) },
                enabled = product.stockQuantity > 0
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
            OrderBackgroundFeed(
                compact = false,
                modifier = Modifier.fillMaxWidth().weight(1f, fill = true)
            )
            return
        }
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(orders, key = { it.id }) { order ->
                Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text("Order ${order.id}", style = MaterialTheme.typography.titleLarge)
                        Text(order.status)
                        if (order.channel.isNotBlank()) {
                            val channelLabel = when (order.channel) {
                                "POS" -> "In-store POS"
                                "PICKUP" -> "Pickup order"
                                else -> order.channel
                            }
                            Text(channelLabel, color = MaterialTheme.colorScheme.secondary)
                        }
                        Text("${order.itemCount} items · $${"%.2f".format(order.total)}")
                        if (order.paymentMethod.isNotBlank()) {
                            Text(
                                "Paid ${order.paymentMethod.lowercase()}" +
                                    if (order.changeDue > 0) " · change $${"%.2f".format(order.changeDue)}" else ""
                            )
                        }
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
                        if (order.customerEmail.isNotBlank()) {
                            Text(Privacy.maskEmail(order.customerEmail))
                        }
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
    var onDeal by remember(product.id) { mutableStateOf(product.onDeal) }
    var dealPercent by remember(product.id) {
        mutableStateOf(if (product.dealPercent > 0) product.dealPercent.toString() else "15")
    }
    var dealLabel by remember(product.id) { mutableStateOf(product.dealLabel) }
    var sizeInventory by remember(product.id) { mutableStateOf(product.sizeInventoryEnabled) }
    var priceGram by remember(product.id) {
        mutableStateOf(if (product.priceGram > 0) product.priceGram.toString() else "")
    }
    var stockGram by remember(product.id) { mutableStateOf(product.stockGram.toString()) }
    var priceEighth by remember(product.id) {
        mutableStateOf(if (product.priceEighth > 0) product.priceEighth.toString() else "")
    }
    var stockEighth by remember(product.id) { mutableStateOf(product.stockEighth.toString()) }
    var priceQuarter by remember(product.id) {
        mutableStateOf(if (product.priceQuarter > 0) product.priceQuarter.toString() else "")
    }
    var stockQuarter by remember(product.id) { mutableStateOf(product.stockQuarter.toString()) }
    var priceOunce by remember(product.id) {
        mutableStateOf(if (product.priceOunce > 0) product.priceOunce.toString() else "")
    }
    var stockOunce by remember(product.id) { mutableStateOf(product.stockOunce.toString()) }

    fun fillFromEighth() {
        val base = priceEighth.toDoubleOrNull() ?: price.toDoubleOrNull() ?: return
        val prices = SizePricing.fromEighth(base)
        priceGram = prices.getValue(ProductSize.GRAM).toString()
        priceEighth = prices.getValue(ProductSize.EIGHTH).toString()
        priceQuarter = prices.getValue(ProductSize.QUARTER).toString()
        priceOunce = prices.getValue(ProductSize.OUNCE).toString()
        price = prices.getValue(ProductSize.EIGHTH).toString()
    }

    Surface(shape = RoundedCornerShape(12.dp), color = MaterialTheme.colorScheme.secondaryContainer) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("Edit product", style = MaterialTheme.typography.titleLarge)
            Field(name, { name = it }, "Name")
            Field(brand, { brand = it }, "Brand")
            Field(sku, { sku = it }, "SKU")
            FilterChip(
                selected = sizeInventory,
                onClick = {
                    sizeInventory = !sizeInventory
                    if (sizeInventory && priceGram.isBlank() && priceEighth.isBlank()) fillFromEighth()
                },
                label = { Text(if (sizeInventory) "Size inventory on" else "Size inventory off") }
            )
            if (sizeInventory) {
                Text("1g / 3.5g / 7g / oz", style = MaterialTheme.typography.labelLarge)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = priceGram, onValueChange = { priceGram = it }, label = { Text("1g $") }, modifier = Modifier.weight(1f), singleLine = true)
                    OutlinedTextField(value = stockGram, onValueChange = { stockGram = it.filter(Char::isDigit) }, label = { Text("1g qty") }, modifier = Modifier.weight(1f), singleLine = true)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = priceEighth, onValueChange = { priceEighth = it }, label = { Text("3.5g $") }, modifier = Modifier.weight(1f), singleLine = true)
                    OutlinedTextField(value = stockEighth, onValueChange = { stockEighth = it.filter(Char::isDigit) }, label = { Text("3.5g qty") }, modifier = Modifier.weight(1f), singleLine = true)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = priceQuarter, onValueChange = { priceQuarter = it }, label = { Text("7g $") }, modifier = Modifier.weight(1f), singleLine = true)
                    OutlinedTextField(value = stockQuarter, onValueChange = { stockQuarter = it.filter(Char::isDigit) }, label = { Text("7g qty") }, modifier = Modifier.weight(1f), singleLine = true)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = priceOunce, onValueChange = { priceOunce = it }, label = { Text("1oz $") }, modifier = Modifier.weight(1f), singleLine = true)
                    OutlinedTextField(value = stockOunce, onValueChange = { stockOunce = it.filter(Char::isDigit) }, label = { Text("1oz qty") }, modifier = Modifier.weight(1f), singleLine = true)
                }
                TextButton(onClick = { fillFromEighth() }) { Text("Fill prices from 3.5g") }
            } else {
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
            }
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
            FilterChip(
                selected = onDeal,
                onClick = { onDeal = !onDeal },
                label = { Text(if (onDeal) "On Deals tab" else "No deal") }
            )
            if (onDeal) {
                Field(dealPercent, { dealPercent = it.filter { ch -> ch.isDigit() }.take(2) }, "Deal % off")
                Field(dealLabel, { dealLabel = it }, "Deal label")
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = onCancel) { Text("Cancel") }
                Button(
                    onClick = {
                        val pct = dealPercent.toIntOrNull() ?: 0
                        val eighth = priceEighth.toDoubleOrNull() ?: price.toDoubleOrNull() ?: 0.0
                        onSave(
                            product.copy(
                                name = name,
                                brand = brand,
                                sku = sku,
                                price = if (sizeInventory) eighth else (price.toDoubleOrNull() ?: 0.0),
                                stockQuantity = if (sizeInventory) {
                                    (stockGram.toIntOrNull() ?: 0) +
                                        (stockEighth.toIntOrNull() ?: 0) +
                                        (stockQuarter.toIntOrNull() ?: 0) +
                                        (stockOunce.toIntOrNull() ?: 0)
                                } else {
                                    stock.toIntOrNull() ?: 0
                                },
                                unitLabel = if (sizeInventory) "1g–1oz" else unit,
                                description = description,
                                category = category,
                                strainType = product.strainType.takeIf { it != StrainType.NONE }
                                    ?: StrainType.HYBRID,
                                published = published,
                                onDeal = onDeal && pct > 0,
                                dealPercent = if (onDeal) pct.coerceIn(0, 90) else 0,
                                dealLabel = dealLabel,
                                sizeInventoryEnabled = sizeInventory,
                                priceGram = priceGram.toDoubleOrNull() ?: 0.0,
                                stockGram = stockGram.toIntOrNull() ?: 0,
                                priceEighth = if (sizeInventory) eighth else 0.0,
                                stockEighth = stockEighth.toIntOrNull() ?: 0,
                                priceQuarter = priceQuarter.toDoubleOrNull() ?: 0.0,
                                stockQuarter = stockQuarter.toIntOrNull() ?: 0,
                                priceOunce = priceOunce.toDoubleOrNull() ?: 0.0,
                                stockOunce = stockOunce.toIntOrNull() ?: 0
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
    val me = repository.currentCustomer()
    val canReveal = me?.role?.canRevealFullPii == true
    val dateFormat = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }
    var revealIds by remember { mutableStateOf(setOf<String>()) }
    Column(Modifier.fillMaxSize()) {
        Text("Customers", style = MaterialTheme.typography.headlineLarge)
        Text(
            if (canReveal) {
                "${customers.size} accounts · admin can reveal personal details"
            } else {
                "${customers.size} accounts · contact details masked (admin-only reveal)"
            },
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(Modifier.height(12.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items(customers, key = { it.id }) { c ->
                val revealed = canReveal && c.id in revealIds
                Surface(shape = RoundedCornerShape(12.dp), tonalElevation = 1.dp, modifier = Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text(c.fullName, style = MaterialTheme.typography.titleLarge)
                            Text(c.role.label, color = MaterialTheme.colorScheme.primary)
                        }
                        Text(if (revealed) c.email else Privacy.maskEmail(c.email))
                        if (c.phone.isNotBlank()) {
                            Text("Phone: ${if (revealed) c.phone else Privacy.maskPhone(c.phone)}")
                        }
                        if (c.dateOfBirth.isNotBlank()) {
                            Text("DOB: ${if (revealed) c.dateOfBirth else Privacy.maskDob(c.dateOfBirth)}")
                        }
                        if (revealed && c.notes.isNotBlank()) Text("Notes: ${c.notes}")
                        if (c.loyaltyPoints > 0 || c.lifetimeSpend > 0) {
                            Text("Points: ${c.loyaltyPoints} · Spent $${"%.2f".format(c.lifetimeSpend)}")
                        }
                        Text("Joined ${dateFormat.format(Date(c.createdAt))}", color = MaterialTheme.colorScheme.onSurfaceVariant)
                        if (canReveal) {
                            TextButton(onClick = {
                                revealIds = if (revealed) revealIds - c.id else revealIds + c.id
                            }) {
                                Text(if (revealed) "Hide personal details" else "Reveal personal details")
                            }
                        }
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
    onLogout: () -> Unit,
    onLock: () -> Unit
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
        Text("Privacy & security", style = MaterialTheme.typography.headlineMedium)
        Text(
            "Customer personal information is staff-only. Full reveal (email/phone/DOB/notes) is admin-only. " +
                "Passwords use PBKDF2; the store is AES-GCM encrypted on disk. Sync never exports password hashes. " +
                "Staff sessions are not restored after restart. This register auto-locks after 5 minutes idle.",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.bodyMedium
        )
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
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onLock) { Text("Lock register now") }
            OutlinedButton(onClick = onLogout) { Text("Log out") }
        }

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
            Text("Local hard drive", style = MaterialTheme.typography.headlineMedium)
            Text(
                "Inventory and customer records save automatically to this PC:",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                repository.localDataFolderPath(),
                style = MaterialTheme.typography.bodyMedium
            )
            Text(
                "Files: store.enc · inventory.json · customers.json · .store-key",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                style = MaterialTheme.typography.bodyMedium
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Button(onClick = {
                    when (val result = repository.saveAllToHardDrive()) {
                        is OpResult.Success -> onMessage(result.message)
                        is OpResult.Error -> onMessage(result.message)
                    }
                }) { Text("Save now") }
                OutlinedButton(onClick = {
                    if (!repository.openLocalDataFolder()) {
                        onMessage("Could not open folder. Path: ${repository.localDataFolderPath()}")
                    }
                }) { Text("Open data folder") }
            }

            HorizontalDivider(Modifier.padding(vertical = 8.dp))
            Text("Desktop sync", style = MaterialTheme.typography.headlineMedium)
            Text(
                "Share inventory + customers JSON with the Android app. Customer passwords are never included. Import writes into the local hard-drive store.",
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Button(onClick = {
                try {
                    val json = repository.exportSyncJson()
                    val dialog = FileDialog(null as Frame?, "Export sync", FileDialog.SAVE)
                    dialog.file = "nativepure-sync.json"
                    dialog.isVisible = true
                    val dir = dialog.directory ?: return@Button
                    val name = dialog.file ?: return@Button
                    File(dir, name).writeText(json)
                    onMessage("Exported sync file (products + customers + orders).")
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
