package com.solstice.dispensary.ui.screens

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.ui.components.BotanicalScreenBackground
import com.solstice.dispensary.ui.components.BrandLogo
import com.solstice.dispensary.ui.components.BotanicalDivider

@Composable
fun AgeGateScreen(
    onVerified: () -> Unit,
    onExit: () -> Unit
) {
    val colors = MaterialTheme.colorScheme
    var visible by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { visible = true }
    val fade by animateFloatAsState(
        targetValue = if (visible) 1f else 0f,
        animationSpec = tween(900),
        label = "ageFade"
    )
    val rise by animateFloatAsState(
        targetValue = if (visible) 0f else 18f,
        animationSpec = tween(900),
        label = "ageRise"
    )

    BotanicalScreenBackground {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(28.dp)
                .graphicsLayer { translationY = rise }
                .alpha(fade),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            BrandLogo(size = 156.dp)
            Spacer(Modifier.height(18.dp))
            Text(
                text = "Native Pure",
                style = MaterialTheme.typography.displayMedium,
                color = colors.onBackground,
                textAlign = TextAlign.Center
            )
            Text(
                text = "grown calm · picked fresh",
                style = MaterialTheme.typography.bodyMedium,
                color = colors.primary,
                textAlign = TextAlign.Center
            )
            BotanicalDivider(modifier = Modifier.padding(vertical = 14.dp))
            Text(
                text = "Are you 18 or older?",
                style = MaterialTheme.typography.headlineMedium,
                color = colors.onBackground,
                textAlign = TextAlign.Center
            )
            Text(
                text = "Enter only if you are of legal age to browse our living menu.",
                style = MaterialTheme.typography.bodyLarge,
                color = colors.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(top = 8.dp)
            )
            Spacer(Modifier.height(20.dp))
            Button(
                onClick = onVerified,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = colors.secondary,
                    contentColor = colors.onSecondary
                )
            ) {
                Text(
                    text = "Yes, I am 18+",
                    modifier = Modifier.padding(vertical = 6.dp),
                    style = MaterialTheme.typography.titleMedium
                )
            }
            Spacer(Modifier.height(8.dp))
            OutlinedButton(
                onClick = onExit,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp)
            ) {
                Text(
                    text = "No, exit",
                    modifier = Modifier.padding(vertical = 6.dp)
                )
            }
            Spacer(Modifier.height(18.dp))
            Text(
                text = "Cannabis products have not been evaluated by the FDA. Keep out of reach of children.",
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant.copy(alpha = 0.8f),
                textAlign = TextAlign.Center
            )
        }
    }
}
