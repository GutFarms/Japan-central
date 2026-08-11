package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.ProductRequest
import com.solstice.dispensary.data.model.RequestBoxStats
import com.solstice.dispensary.ui.components.SectionHeader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
fun RequestBoxCard(
    stats: RequestBoxStats,
    myRequests: List<ProductRequest>,
    isStaff: Boolean,
    message: String?,
    onSubmit: (productName: String, notes: String) -> Unit,
    onMarkDone: (String) -> Unit = {},
    onOpenAll: (() -> Unit)? = null
) {
    var name by remember { mutableStateOf("") }
    var notes by remember { mutableStateOf("") }
    val pct = stats.requesterToCustomerPercent.coerceIn(0f, 100f)

    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text(
                "Request box",
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold
            )
            Text(
                "Ask for a strain, brand, or product you’d like on the menu.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            // Percentage → customer ratio
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = MaterialTheme.colorScheme.secondaryContainer,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(12.dp)) {
                    Text(
                        "Customer request ratio",
                        style = MaterialTheme.typography.titleSmall,
                        color = MaterialTheme.colorScheme.onSecondaryContainer
                    )
                    Text(
                        "${pct.toInt()}%",
                        style = MaterialTheme.typography.displaySmall,
                        fontWeight = FontWeight.Bold,
                        color = MaterialTheme.colorScheme.onSecondaryContainer
                    )
                    LinearProgressIndicator(
                        progress = { (pct / 100f).coerceIn(0f, 1f) },
                        modifier = Modifier.fillMaxWidth()
                    )
                    Spacer(Modifier.height(6.dp))
                    Text(
                        "${stats.uniqueRequesters} of ${stats.totalCustomers} customers · " +
                            "${stats.totalRequests} total requests",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSecondaryContainer
                    )
                    Text(
                        stats.ratioLabel,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSecondaryContainer
                    )
                }
            }

            if (!isStaff) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text("What are you looking for?") },
                    modifier = Modifier.fillMaxWidth(),
                    singleLine = true
                )
                OutlinedTextField(
                    value = notes,
                    onValueChange = { notes = it },
                    label = { Text("Notes (optional)") },
                    modifier = Modifier.fillMaxWidth()
                )
                Button(
                    onClick = {
                        onSubmit(name, notes)
                        name = ""
                        notes = ""
                    },
                    enabled = name.trim().length >= 2,
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(12.dp)
                ) {
                    Text("Send request")
                }
            }

            if (message != null) {
                Text(message, color = MaterialTheme.colorScheme.primary)
            }

            if (myRequests.isNotEmpty()) {
                Text(
                    if (isStaff) "Latest requests" else "Your recent requests",
                    style = MaterialTheme.typography.titleSmall
                )
                myRequests.take(5).forEach { req ->
                    RequestRow(
                        request = req,
                        showCustomer = isStaff,
                        onMarkDone = if (isStaff && req.status == "Open") {
                            { onMarkDone(req.id) }
                        } else null
                    )
                }
            }

            if (isStaff && onOpenAll != null) {
                TextButton(onClick = onOpenAll) { Text("Open full request box →") }
            }
        }
    }
}

@Composable
fun RequestsScreen(
    requests: List<ProductRequest>,
    stats: RequestBoxStats,
    onBack: () -> Unit,
    onMarkDone: (String) -> Unit
) {
    val dateFormat = remember { SimpleDateFormat("MMM d · h:mm a", Locale.US) }
    LazyColumn(
        modifier = Modifier
            .fillMaxSize()
            .imePadding(),
        contentPadding = PaddingValues(20.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        item {
            TextButton(onClick = onBack) { Text("← Back") }
            SectionHeader(
                title = "Request box",
                subtitle = "${stats.requesterToCustomerPercent.toInt()}% customer ratio · " +
                    "${stats.totalRequests} requests"
            )
        }
        item {
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.secondaryContainer,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Text(
                        "${stats.requesterToCustomerPercent.toInt()}% of customers requesting",
                        style = MaterialTheme.typography.headlineMedium
                    )
                    LinearProgressIndicator(
                        progress = { (stats.requesterToCustomerPercent / 100f).coerceIn(0f, 1f) },
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 8.dp)
                    )
                    Text(
                        "${stats.uniqueRequesters} unique requesters / ${stats.totalCustomers} customers",
                        modifier = Modifier.padding(top = 6.dp)
                    )
                }
            }
        }
        items(requests, key = { it.id }) { req ->
            RequestRow(
                request = req,
                showCustomer = true,
                detailTime = dateFormat.format(Date(req.createdAt)),
                onMarkDone = if (req.status == "Open") {{ onMarkDone(req.id) }} else null
            )
        }
    }
}

@Composable
private fun RequestRow(
    request: ProductRequest,
    showCustomer: Boolean,
    detailTime: String? = null,
    onMarkDone: (() -> Unit)?
) {
    Surface(
        shape = RoundedCornerShape(12.dp),
        tonalElevation = 1.dp,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(modifier = Modifier.padding(12.dp)) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(request.productName, style = MaterialTheme.typography.titleMedium)
                Text(request.status, style = MaterialTheme.typography.labelLarge)
            }
            if (showCustomer) {
                Text(
                    "${request.customerName} · ${request.customerEmail}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            if (request.notes.isNotBlank()) {
                Text(request.notes, style = MaterialTheme.typography.bodyMedium)
            }
            if (detailTime != null) {
                Text(
                    detailTime,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            if (onMarkDone != null) {
                TextButton(onClick = onMarkDone) { Text("Mark fulfilled") }
            }
        }
    }
}
