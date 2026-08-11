package com.nativepure.companion.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay

/** Sample pickup activity shown when the real order queue is empty. */
data class FeedOrderLine(
    val id: String,
    val name: String,
    val summary: String,
    val status: String
)

object BackgroundOrderFeed {
    val samples = listOf(
        FeedOrderLine("A1F3", "Maya R.", "2 items · $48.60 · phone pickup", "Ready for pickup"),
        FeedOrderLine("B82C", "Jordan L.", "1 item · $22.00 · app order", "Ready for pickup"),
        FeedOrderLine("C40E", "Sam K.", "3 items · $91.25 · deal applied", "Ready for pickup"),
        FeedOrderLine("D19A", "Alex P.", "1 item · $38.00 · loyalty redeem", "Ready for pickup"),
        FeedOrderLine("E7B2", "Riley M.", "4 items · $112.40 · flower + edibles", "Ready for pickup"),
        FeedOrderLine("F55D", "Casey T.", "2 items · $55.00 · cart special", "Ready for pickup"),
        FeedOrderLine("G03C", "Quinn H.", "1 item · $28.00 · pre-roll pack", "Ready for pickup"),
        FeedOrderLine("H91F", "Avery S.", "3 items · $67.80 · evening order", "Ready for pickup")
    )
}

/**
 * Soft scrolling sample order feed for empty queue / empty orders screens.
 * Not real sales — only fills the background until live orders arrive.
 */
@Composable
fun OrderBackgroundFeed(
    compact: Boolean = false,
    modifier: Modifier = Modifier
) {
    val colors = MaterialTheme.colorScheme
    var highlight by remember { mutableIntStateOf(0) }
    val lines = BackgroundOrderFeed.samples

    LaunchedEffect(Unit) {
        while (true) {
            delay(2_400)
            highlight = (highlight + 1) % lines.size
        }
    }

    val shimmer by rememberInfiniteTransition(label = "feedShimmer").animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(4_800, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "feedShimmerAnim"
    )

    Box(
        modifier = modifier
            .clip(RoundedCornerShape(if (compact) 10.dp else 14.dp))
            .background(
                Brush.verticalGradient(
                    listOf(
                        colors.surfaceVariant.copy(alpha = 0.55f),
                        colors.primaryContainer.copy(alpha = 0.35f),
                        colors.surfaceVariant.copy(alpha = 0.45f)
                    )
                )
            )
            .fillMaxWidth()
    ) {
        // Soft drifting accent bar
        Box(
            Modifier
                .align(Alignment.TopStart)
                .offset(x = (shimmer * 280).dp - 40.dp)
                .fillMaxWidth(0.35f)
                .height(if (compact) 88.dp else 160.dp)
                .background(
                    Brush.horizontalGradient(
                        listOf(
                            colors.primary.copy(alpha = 0.0f),
                            colors.primary.copy(alpha = 0.08f),
                            colors.primary.copy(alpha = 0.0f)
                        )
                    )
                )
        )

        Column(
            Modifier
                .fillMaxWidth()
                .padding(if (compact) 10.dp else 16.dp),
            verticalArrangement = Arrangement.spacedBy(if (compact) 4.dp else 8.dp)
        ) {
            Text(
                "Sample pickup feed",
                style = if (compact) MaterialTheme.typography.titleMedium else MaterialTheme.typography.titleLarge,
                color = colors.onSurfaceVariant
            )
            Text(
                "Waiting for live orders — this background clears when real pickups arrive.",
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant.copy(alpha = 0.85f)
            )
            Spacer(Modifier.height(2.dp))
            val visible = if (compact) 3 else 5
            val start = highlight
            repeat(visible) { i ->
                val line = lines[(start + i) % lines.size]
                val active = i == 0
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = if (active) {
                        colors.primary.copy(alpha = 0.14f)
                    } else {
                        colors.surface.copy(alpha = 0.55f)
                    },
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Row(
                        Modifier.padding(horizontal = 10.dp, vertical = if (compact) 6.dp else 8.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(
                                "#${line.id} · ${line.name}",
                                style = MaterialTheme.typography.titleMedium,
                                color = if (active) colors.primary else colors.onSurface
                            )
                            Text(
                                line.summary,
                                style = MaterialTheme.typography.bodyMedium,
                                color = colors.onSurfaceVariant
                            )
                        }
                        Text(
                            line.status,
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.secondary
                        )
                    }
                }
            }
        }
    }
}
