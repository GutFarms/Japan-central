package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.ui.components.BrandLogo
import com.solstice.dispensary.ui.theme.Amber
import com.solstice.dispensary.ui.theme.Charcoal
import com.solstice.dispensary.ui.theme.Ivory
import com.solstice.dispensary.ui.theme.Sage

@Composable
fun AgeGateScreen(
    onVerified: () -> Unit,
    onExit: () -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    listOf(Charcoal, Color(0xFF1A1410), Charcoal)
                )
            )
            .padding(28.dp),
        contentAlignment = Alignment.Center
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            BrandLogo(size = 168.dp)
            Spacer(Modifier.height(4.dp))
            Text(
                text = "Are you 21 or older?",
                style = MaterialTheme.typography.headlineMedium,
                color = Ivory,
                textAlign = TextAlign.Center
            )
            Text(
                text = "You must be of legal age to enter Native Pure and browse our menu.",
                style = MaterialTheme.typography.bodyLarge,
                color = Sage,
                textAlign = TextAlign.Center,
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(8.dp))
            Button(
                onClick = onVerified,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = Amber,
                    contentColor = Charcoal
                )
            ) {
                Text(
                    text = "Yes, I am 21+",
                    modifier = Modifier.padding(vertical = 6.dp),
                    style = MaterialTheme.typography.titleMedium
                )
            }
            OutlinedButton(
                onClick = onExit,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp),
                colors = ButtonDefaults.outlinedButtonColors(contentColor = Ivory)
            ) {
                Text(
                    text = "No, exit",
                    modifier = Modifier.padding(vertical = 6.dp)
                )
            }
            Text(
                text = "Cannabis products have not been evaluated by the FDA. Keep out of reach of children.",
                style = MaterialTheme.typography.bodyMedium,
                color = Ivory.copy(alpha = 0.55f),
                textAlign = TextAlign.Center
            )
        }
    }
}
