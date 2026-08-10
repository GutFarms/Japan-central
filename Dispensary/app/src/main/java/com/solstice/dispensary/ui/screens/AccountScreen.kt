package com.solstice.dispensary.ui.screens

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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
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
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.CustomerProfile
import com.solstice.dispensary.ui.components.BrandLogo
import com.solstice.dispensary.ui.components.SectionHeader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun AccountScreen(
    customer: CustomerProfile?,
    customers: List<CustomerProfile>,
    message: String?,
    onClearMessage: () -> Unit,
    onSaveProfile: (String, String, String, String, Boolean) -> Unit,
    onLogout: () -> Unit,
    onOpenCustomers: () -> Unit
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
    val dateFormat = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }

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
                label = { Text("Notes for the budtender") },
                modifier = Modifier.fillMaxWidth()
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = marketing, onCheckedChange = { marketing = it })
                Text("Marketing emails", style = MaterialTheme.typography.bodyMedium)
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
        }

        item {
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.secondaryContainer,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Customer database", style = MaterialTheme.typography.titleLarge)
                    Text(
                        text = "${customers.size} registered customers on this device",
                        style = MaterialTheme.typography.bodyMedium
                    )
                    Spacer(Modifier.height(8.dp))
                    Button(onClick = onOpenCustomers, shape = RoundedCornerShape(10.dp)) {
                        Text("View customers")
                    }
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
    onBack: () -> Unit
) {
    val dateFormat = remember { SimpleDateFormat("MMM d, yyyy", Locale.US) }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        item {
            TextButton(onClick = onBack) { Text("← Back") }
            SectionHeader(
                title = "Customers",
                subtitle = "${customers.size} accounts in the local database"
            )
        }
        items(customers, key = { it.id }) { customer ->
            Surface(
                shape = RoundedCornerShape(14.dp),
                tonalElevation = 1.dp,
                color = MaterialTheme.colorScheme.surface,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Text(customer.fullName, style = MaterialTheme.typography.titleLarge)
                    Text(customer.email, style = MaterialTheme.typography.bodyMedium)
                    if (customer.phone.isNotBlank()) {
                        Text(customer.phone, style = MaterialTheme.typography.bodyMedium)
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
                }
            }
        }
    }
}
