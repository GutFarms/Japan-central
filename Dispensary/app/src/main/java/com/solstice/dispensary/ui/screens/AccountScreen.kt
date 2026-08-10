package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
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
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.auth.PasswordPolicy
import com.solstice.dispensary.data.model.AccountRole
import com.solstice.dispensary.data.model.AutoLockTimeout
import com.solstice.dispensary.data.model.CustomerProfile
import com.solstice.dispensary.data.model.SecuritySettings
import com.solstice.dispensary.data.model.ThemeMode
import com.solstice.dispensary.ui.components.BrandLogo
import com.solstice.dispensary.ui.components.MetaPill
import com.solstice.dispensary.ui.components.SectionHeader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun AccountScreen(
    customer: CustomerProfile?,
    customers: List<CustomerProfile>,
    staffAccounts: List<CustomerProfile>,
    themeMode: ThemeMode,
    securitySettings: SecuritySettings,
    message: String?,
    syncExportJson: String?,
    onClearMessage: () -> Unit,
    onSaveProfile: (String, String, String, String, Boolean) -> Unit,
    onThemeModeChange: (ThemeMode) -> Unit,
    onChangePassword: (current: String, newPassword: String, confirm: String) -> Unit,
    onEnableAppLock: (pin: String, confirmPin: String) -> Unit,
    onDisableAppLock: (accountPassword: String) -> Unit,
    onAutoLockChange: (AutoLockTimeout) -> Unit,
    onCreateStaff: (email: String, password: String, fullName: String) -> Unit,
    onSetStaffEnabled: (staffId: String, enabled: Boolean) -> Unit,
    onResetStaffPassword: (staffId: String, newPassword: String) -> Unit,
    onExportSync: () -> Unit,
    onShareSync: (String) -> Unit,
    onImportSync: (String) -> Unit,
    onClearSyncExport: () -> Unit,
    onLogout: () -> Unit,
    onOpenCustomers: () -> Unit,
    onOpenOrders: () -> Unit
) {
    if (customer == null) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(24.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Text("Not signed in", style = MaterialTheme.typography.headlineMedium)
        }
        return
    }

    var fullName by remember(customer.id) { mutableStateOf(customer.fullName) }
    var phone by remember(customer.id) { mutableStateOf(customer.phone) }
    var dob by remember(customer.id) { mutableStateOf(customer.dateOfBirth) }
    var notes by remember(customer.id) { mutableStateOf(customer.notes) }
    var marketing by remember(customer.id) { mutableStateOf(customer.marketingOptIn) }
    var staffEmail by remember { mutableStateOf("") }
    var staffPassword by remember { mutableStateOf("") }
    var staffName by remember { mutableStateOf("") }
    var staffResetPassword by remember { mutableStateOf("") }
    var importJson by remember { mutableStateOf("") }
    var currentPassword by remember { mutableStateOf("") }
    var newPassword by remember { mutableStateOf("") }
    var confirmPassword by remember { mutableStateOf("") }
    var newPin by remember { mutableStateOf("") }
    var confirmPin by remember { mutableStateOf("") }
    var disablePassword by remember { mutableStateOf("") }
    val dateFormat = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }
    val isAdminLike = customer.isAdminLike
    val canManageStaff = customer.role.canManageStaff

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        item {
            Row(verticalAlignment = Alignment.CenterVertically) {
                BrandLogo(size = 64.dp)
                Spacer(Modifier.width(12.dp))
                Column {
                    Text(customer.fullName, style = MaterialTheme.typography.headlineMedium)
                    Text(
                        customer.email,
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    if (customer.username.isNotBlank()) {
                        Text(
                            "Username: ${customer.username}",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Spacer(Modifier.height(4.dp))
                    MetaPill(customer.role.label)
                }
            }
        }

        item {
            SectionHeader(
                title = "Your profile",
                subtitle = "Member since ${dateFormat.format(Date(customer.createdAt))}"
            )
            OutlinedTextField(
                value = fullName,
                onValueChange = { fullName = it },
                label = { Text("Full name") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = phone,
                onValueChange = { phone = it },
                label = { Text("Phone") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = dob,
                onValueChange = { dob = it },
                label = { Text("Date of birth") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = notes,
                onValueChange = { notes = it },
                label = { Text(if (isAdminLike) "Internal notes" else "Notes for the budtender") },
                modifier = Modifier.fillMaxWidth()
            )
            if (customer.role == AccountRole.CUSTOMER) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = marketing, onCheckedChange = { marketing = it })
                    Text("Marketing emails", style = MaterialTheme.typography.bodyMedium)
                }
            }
            Button(
                onClick = {
                    onSaveProfile(fullName, phone, dob, notes, marketing)
                },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp)
            ) {
                Text("Save profile")
            }
            if (message != null) {
                Text(
                    text = message,
                    color = MaterialTheme.colorScheme.primary,
                    style = MaterialTheme.typography.bodyMedium
                )
                TextButton(onClick = onClearMessage) { Text("Dismiss") }
            }
            TextButton(onClick = onOpenOrders) {
                Text(if (isAdminLike) "View all orders" else "View my orders")
            }
        }

        item {
            SectionHeader(
                title = "Security",
                subtitle = "Protects all accounts on this device"
            )
            Text(
                text = PasswordPolicy.requirementsLabel,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = currentPassword,
                onValueChange = { currentPassword = it },
                label = { Text("Current password") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = newPassword,
                onValueChange = { newPassword = it },
                label = { Text("New password") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = confirmPassword,
                onValueChange = { confirmPassword = it },
                label = { Text("Confirm new password") },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
            )
            Spacer(Modifier.height(8.dp))
            Button(
                onClick = {
                    onChangePassword(currentPassword, newPassword, confirmPassword)
                    currentPassword = ""
                    newPassword = ""
                    confirmPassword = ""
                },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(12.dp)
            ) {
                Text("Update password")
            }

            Spacer(Modifier.height(16.dp))
            Text("App PIN lock", style = MaterialTheme.typography.titleLarge)
            Text(
                text = if (securitySettings.appLockEnabled) {
                    "Enabled · auto-lock: ${securitySettings.autoLockTimeout.label.lowercase()}"
                } else {
                    "Off · set a ${PasswordPolicy.PIN_MIN_LENGTH}–${PasswordPolicy.PIN_MAX_LENGTH} digit PIN to lock the app when you leave"
                },
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(Modifier.height(8.dp))
            if (!securitySettings.appLockEnabled) {
                OutlinedTextField(
                    value = newPin,
                    onValueChange = {
                        if (it.length <= PasswordPolicy.PIN_MAX_LENGTH && it.all { ch -> ch.isDigit() }) {
                            newPin = it
                        }
                    },
                    label = { Text("New PIN") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword)
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = confirmPin,
                    onValueChange = {
                        if (it.length <= PasswordPolicy.PIN_MAX_LENGTH && it.all { ch -> ch.isDigit() }) {
                            confirmPin = it
                        }
                    },
                    label = { Text("Confirm PIN") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword)
                )
                Spacer(Modifier.height(8.dp))
                Button(
                    onClick = {
                        onEnableAppLock(newPin, confirmPin)
                        newPin = ""
                        confirmPin = ""
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Enable app lock")
                }
            } else {
                OutlinedTextField(
                    value = disablePassword,
                    onValueChange = { disablePassword = it },
                    label = { Text("Account password to disable lock") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
                )
                Spacer(Modifier.height(8.dp))
                Button(
                    onClick = {
                        onDisableAppLock(disablePassword)
                        disablePassword = ""
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Disable app lock")
                }
            }

            Spacer(Modifier.height(12.dp))
            Text("Auto-lock after", style = MaterialTheme.typography.titleMedium)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                AutoLockTimeout.entries.forEach { timeout ->
                    FilterChip(
                        selected = securitySettings.autoLockTimeout == timeout,
                        onClick = { onAutoLockChange(timeout) },
                        label = { Text(timeout.label) },
                        enabled = securitySettings.appLockEnabled || timeout == AutoLockTimeout.NEVER
                    )
                }
            }

            Spacer(Modifier.height(12.dp))
            Text(
                text = "Login protection is always on: ${PasswordPolicy.MAX_FAILED_ATTEMPTS} failed attempts locks sign-in for 5 minutes.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }

        item {
            SectionHeader(
                title = "Appearance",
                subtitle = "Light, dark, or match your phone"
            )
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                ThemeMode.entries.forEach { mode ->
                    FilterChip(
                        selected = themeMode == mode,
                        onClick = { onThemeModeChange(mode) },
                        label = { Text(mode.label) }
                    )
                }
            }
        }

        if (isAdminLike) {
            item {
                Surface(
                    shape = RoundedCornerShape(14.dp),
                    color = MaterialTheme.colorScheme.secondaryContainer,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(modifier = Modifier.padding(16.dp)) {
                        Text("Customer database", style = MaterialTheme.typography.titleLarge)
                        Text(
                            text = "${customers.size} accounts · includes phone, DOB, and notes",
                            style = MaterialTheme.typography.bodyMedium
                        )
                        Spacer(Modifier.height(8.dp))
                        Button(onClick = onOpenCustomers, shape = RoundedCornerShape(10.dp)) {
                            Text("View all customers")
                        }
                    }
                }
            }
        }

        if (canManageStaff) {
            item {
                SectionHeader(
                    title = "Staff sub-accounts",
                    subtitle = "Temporary password must meet policy; staff change it on first login"
                )
                OutlinedTextField(
                    value = staffName,
                    onValueChange = { staffName = it },
                    label = { Text("Staff full name") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = staffEmail,
                    onValueChange = { staffEmail = it },
                    label = { Text("Staff email") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email)
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = staffPassword,
                    onValueChange = { staffPassword = it },
                    label = { Text("Temporary password") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password)
                )
                Spacer(Modifier.height(8.dp))
                Button(
                    onClick = {
                        onCreateStaff(staffEmail, staffPassword, staffName)
                        staffEmail = ""
                        staffPassword = ""
                        staffName = ""
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Create staff account")
                }

                if (staffAccounts.isNotEmpty()) {
                    Spacer(Modifier.height(16.dp))
                    Text("Active staff", style = MaterialTheme.typography.titleLarge)
                    Spacer(Modifier.height(8.dp))
                    staffAccounts.forEach { staff ->
                        Surface(
                            shape = RoundedCornerShape(12.dp),
                            tonalElevation = 1.dp,
                            modifier = Modifier.fillMaxWidth().padding(bottom = 8.dp)
                        ) {
                            Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                                Text(staff.fullName, style = MaterialTheme.typography.titleMedium)
                                Text(staff.email, color = MaterialTheme.colorScheme.onSurfaceVariant)
                                Text(
                                    if (staff.enabled) "Enabled" else "Disabled",
                                    color = if (staff.enabled) {
                                        MaterialTheme.colorScheme.primary
                                    } else {
                                        MaterialTheme.colorScheme.error
                                    }
                                )
                                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    TextButton(onClick = {
                                        onSetStaffEnabled(staff.id, !staff.enabled)
                                    }) {
                                        Text(if (staff.enabled) "Disable" else "Enable")
                                    }
                                }
                                OutlinedTextField(
                                    value = staffResetPassword,
                                    onValueChange = { staffResetPassword = it },
                                    label = { Text("New temp password") },
                                    modifier = Modifier.fillMaxWidth(),
                                    singleLine = true,
                                    visualTransformation = PasswordVisualTransformation()
                                )
                                TextButton(onClick = {
                                    onResetStaffPassword(staff.id, staffResetPassword)
                                    staffResetPassword = ""
                                }) {
                                    Text("Reset password")
                                }
                            }
                        }
                    }
                }
            }
        }

        if (isAdminLike) {
            item {
                SectionHeader(
                    title = "Desktop sync",
                    subtitle = "Export/import inventory JSON for the Windows companion"
                )
                Button(
                    onClick = onExportSync,
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Export inventory sync file")
                }
                if (syncExportJson != null) {
                    Spacer(Modifier.height(8.dp))
                    Button(
                        onClick = { onShareSync(syncExportJson) },
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(12.dp)
                    ) {
                        Text("Share sync file")
                    }
                    TextButton(onClick = onClearSyncExport) { Text("Clear export") }
                }
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = importJson,
                    onValueChange = { importJson = it },
                    label = { Text("Paste sync JSON to import") },
                    modifier = Modifier.fillMaxWidth().height(120.dp)
                )
                Spacer(Modifier.height(8.dp))
                Button(
                    onClick = {
                        onImportSync(importJson)
                        importJson = ""
                    },
                    enabled = importJson.isNotBlank(),
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Import sync JSON")
                }
            }
        }

        item {
            TextButton(onClick = onLogout) {
                Text("Log out")
            }
        }
    }
}

@Composable
fun CustomersScreen(
    customers: List<CustomerProfile>,
    showSensitive: Boolean,
    onBack: () -> Unit
) {
    val dateFormat = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }
    val visible = if (showSensitive) {
        customers
    } else {
        emptyList()
    }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        item {
            TextButton(onClick = onBack) { Text("← Back") }
            SectionHeader(
                title = "Customers",
                subtitle = if (showSensitive) {
                    "${visible.size} accounts · sensitive fields visible"
                } else {
                    "Access denied"
                }
            )
        }

        if (!showSensitive) {
            item {
                Text(
                    "Only admin and staff accounts can view the customer database.",
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }

        items(visible, key = { it.id }) { customer ->
            Surface(
                shape = RoundedCornerShape(14.dp),
                tonalElevation = 1.dp,
                color = MaterialTheme.colorScheme.surface,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(customer.fullName, style = MaterialTheme.typography.titleLarge)
                        MetaPill(customer.role.label)
                    }
                    Text(customer.email, style = MaterialTheme.typography.bodyMedium)
                    if (customer.username.isNotBlank()) {
                        Text("Username: ${customer.username}", style = MaterialTheme.typography.bodyMedium)
                    }
                    if (customer.phone.isNotBlank()) {
                        Text("Phone: ${customer.phone}", style = MaterialTheme.typography.bodyMedium)
                    }
                    if (customer.dateOfBirth.isNotBlank()) {
                        Text("DOB: ${customer.dateOfBirth}", style = MaterialTheme.typography.bodyMedium)
                    }
                    if (customer.notes.isNotBlank()) {
                        Text("Notes: ${customer.notes}", style = MaterialTheme.typography.bodyMedium)
                    }
                    Text(
                        text = "Joined ${dateFormat.format(Date(customer.createdAt))}" +
                            if (customer.lastLoginAt > 0) {
                                " · Last login ${dateFormat.format(Date(customer.lastLoginAt))}"
                            } else {
                                ""
                            },
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    if (customer.marketingOptIn) {
                        Text(
                            "Marketing opt-in",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.primary
                        )
                    }
                    if (!customer.enabled) {
                        Text(
                            "Account disabled",
                            style = MaterialTheme.typography.labelLarge,
                            color = MaterialTheme.colorScheme.error
                        )
                    }
                }
            }
        }
    }
}
