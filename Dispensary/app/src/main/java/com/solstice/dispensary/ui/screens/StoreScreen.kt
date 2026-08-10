package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.ui.components.HeroBackdrop
import com.solstice.dispensary.ui.components.SectionHeader
import com.solstice.dispensary.ui.theme.Amber
import com.solstice.dispensary.ui.theme.Ivory
import com.solstice.dispensary.ui.theme.Sage

@Composable
fun StoreScreen(
    onOpenOrders: () -> Unit = {}
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
    ) {
        HeroBackdrop {
            Column {
                Text("SOLSTICE", style = MaterialTheme.typography.displayMedium, color = Ivory)
                Text("DISPENSARY", style = MaterialTheme.typography.labelLarge, color = Amber)
                Spacer(Modifier.height(12.dp))
                Text(
                    text = "Adult-use cannabis · Pickup only",
                    style = MaterialTheme.typography.bodyLarge,
                    color = Sage
                )
            }
        }

        Column(modifier = Modifier.padding(20.dp)) {
            SectionHeader(title = "Visit us", subtitle = "Licensed retail · ID required")
            InfoBlock(
                title = "Address",
                body = "1847 Grove Avenue\nPortland, OR 97214"
            )
            Spacer(Modifier.height(12.dp))
            InfoBlock(
                title = "Hours",
                body = "Mon–Thu  10:00 AM – 8:00 PM\nFri–Sat  10:00 AM – 10:00 PM\nSun      11:00 AM – 7:00 PM"
            )
            Spacer(Modifier.height(12.dp))
            InfoBlock(
                title = "Contact",
                body = "(503) 555-0184\nhello@solsticedispensary.example"
            )
            Spacer(Modifier.height(12.dp))
            InfoBlock(
                title = "Pickup",
                body = "Order in the app, show ID at the counter, and collect your bag. Please allow 20–30 minutes during peak hours."
            )
            Spacer(Modifier.height(12.dp))
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.secondaryContainer,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onOpenOrders)
            ) {
                Column(modifier = Modifier.padding(16.dp)) {
                    Text("Pickup orders", style = MaterialTheme.typography.titleLarge)
                    Text(
                        text = "View recent orders placed on this device",
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSecondaryContainer
                    )
                }
            }
            Spacer(Modifier.height(12.dp))
            InfoBlock(
                title = "Reminders",
                body = "21+ only with valid government ID. Do not drive impaired. Keep products away from children and pets. Consume responsibly."
            )
        }
    }
}

@Composable
private fun InfoBlock(title: String, body: String) {
    Surface(
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.surface,
        tonalElevation = 1.dp,
        modifier = Modifier.fillMaxWidth()
    ) {
        Column(modifier = Modifier.padding(16.dp)) {
            Text(title, style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(6.dp))
            Text(
                text = body,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
