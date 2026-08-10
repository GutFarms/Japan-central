package com.solstice.dispensary.ui.components

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.ui.theme.CanopyLight
import com.solstice.dispensary.ui.theme.Charcoal
import com.solstice.dispensary.ui.theme.DuskCanopy
import com.solstice.dispensary.ui.theme.DuskLeaf
import com.solstice.dispensary.ui.theme.Fern
import com.solstice.dispensary.ui.theme.GreenhouseMist
import com.solstice.dispensary.ui.theme.Leaf
import com.solstice.dispensary.ui.theme.LeafDeep
import com.solstice.dispensary.ui.theme.Moss
import com.solstice.dispensary.ui.theme.Stem
import kotlin.math.cos
import kotlin.math.sin

@Composable
fun BotanicalScreenBackground(
    modifier: Modifier = Modifier,
    content: @Composable BoxScope.() -> Unit
) {
    val isDark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    val base = if (isDark) {
        listOf(DuskCanopy, DuskLeaf, Charcoal)
    } else {
        listOf(CanopyLight, GreenhouseMist, Color(0xFFDCE8D8))
    }
    val transition = rememberInfiniteTransition(label = "canopyLight")
    val drift by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(14000, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "drift"
    )

    Box(
        modifier = modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(base))
    ) {
        LeafMotifOverlay(dark = isDark, drift = drift)
        content()
    }
}

@Composable
fun HeroBackdrop(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit
) {
    val isDark = MaterialTheme.colorScheme.background.luminance() < 0.5f
    val gradient = if (isDark) {
        listOf(DuskCanopy, Color(0xFF15241B), DuskLeaf)
    } else {
        listOf(Color(0xFF1A2E22), LeafDeep, Color(0xFF3A5C44))
    }
    val transition = rememberInfiniteTransition(label = "heroLight")
    val sweep by transition.animateFloat(
        initialValue = 0f,
        targetValue = 1f,
        animationSpec = infiniteRepeatable(
            animation = tween(9000, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "sweep"
    )

    Box(
        modifier = modifier
            .fillMaxWidth()
            .background(Brush.verticalGradient(gradient))
    ) {
        Canvas(modifier = Modifier.matchParentSize()) {
            val leafColor = if (isDark) Moss.copy(alpha = 0.18f) else Stem.copy(alpha = 0.22f)
            drawLeaf(Offset(size.width * 0.08f, size.height * 0.75f), 70.dp.toPx(), -0.4f, leafColor)
            drawLeaf(Offset(size.width * 0.92f, size.height * 0.3f), 90.dp.toPx(), 2.4f, leafColor)
            drawLeaf(Offset(size.width * 0.78f, size.height * 0.85f), 55.dp.toPx(), 1.1f, leafColor)
            val lightX = size.width * (0.2f + sweep * 0.6f)
            drawCircle(
                brush = Brush.radialGradient(
                    colors = listOf(Color.White.copy(alpha = 0.12f), Color.Transparent),
                    center = Offset(lightX, size.height * 0.25f),
                    radius = size.minDimension * 0.55f
                ),
                radius = size.minDimension * 0.55f,
                center = Offset(lightX, size.height * 0.25f)
            )
        }
        Box(modifier = Modifier.padding(24.dp)) {
            content()
        }
    }
}

@Composable
private fun LeafMotifOverlay(dark: Boolean, drift: Float) {
    val stroke = if (dark) Moss.copy(alpha = 0.16f) else Leaf.copy(alpha = 0.14f)
    Canvas(modifier = Modifier.fillMaxSize()) {
        val positions = listOf(
            Offset(size.width * 0.12f, size.height * (0.18f + drift * 0.03f)),
            Offset(size.width * 0.88f, size.height * (0.22f - drift * 0.02f)),
            Offset(size.width * 0.18f, size.height * 0.78f),
            Offset(size.width * 0.82f, size.height * (0.72f + drift * 0.02f)),
            Offset(size.width * 0.5f, size.height * 0.9f)
        )
        positions.forEachIndexed { index, origin ->
            val angle = index * 0.9f + drift
            drawLeaf(origin, 48.dp.toPx() + index * 6f, angle, stroke)
        }
    }
}

private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawLeaf(
    origin: Offset,
    length: Float,
    angle: Float,
    color: Color
) {
    val tip = Offset(
        origin.x + cos(angle) * length,
        origin.y + sin(angle) * length
    )
    val perp = Offset(-sin(angle), cos(angle))
    val mid = Offset((origin.x + tip.x) / 2f, (origin.y + tip.y) / 2f)
    val width = length * 0.28f
    val path = Path().apply {
        moveTo(origin.x, origin.y)
        cubicTo(
            mid.x + perp.x * width,
            mid.y + perp.y * width,
            mid.x + perp.x * width * 0.6f,
            mid.y + perp.y * width * 0.6f,
            tip.x,
            tip.y
        )
        cubicTo(
            mid.x - perp.x * width * 0.6f,
            mid.y - perp.y * width * 0.6f,
            mid.x - perp.x * width,
            mid.y - perp.y * width,
            origin.x,
            origin.y
        )
        close()
    }
    drawPath(path, color = color)
    drawLine(
        color = color.copy(alpha = color.alpha + 0.1f),
        start = origin,
        end = tip,
        strokeWidth = 1.5f,
        cap = StrokeCap.Round
    )
}

@Composable
fun BotanicalDivider(modifier: Modifier = Modifier) {
    val color = MaterialTheme.colorScheme.outline.copy(alpha = 0.55f)
    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .height(18.dp)
            .padding(vertical = 4.dp)
    ) {
        val y = size.height / 2f
        drawLine(color, Offset(0f, y), Offset(size.width * 0.42f, y), strokeWidth = 1.2f)
        drawLeaf(Offset(size.width * 0.5f, y), size.height * 1.4f, -1.2f, color)
        drawLine(color, Offset(size.width * 0.58f, y), Offset(size.width, y), strokeWidth = 1.2f)
    }
}

@Composable
fun LeafEmptyState(
    title: String,
    subtitle: String,
    modifier: Modifier = Modifier,
    leafSize: Dp = 72.dp
) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        val stroke = MaterialTheme.colorScheme.primary.copy(alpha = 0.75f)
        Canvas(modifier = Modifier.size(leafSize)) {
            drawLeaf(
                Offset(this.size.width * 0.32f, this.size.height * 0.78f),
                this.size.minDimension * 0.72f,
                -1.15f,
                stroke
            )
        }
        Spacer(Modifier.height(12.dp))
        Text(title, style = MaterialTheme.typography.headlineMedium, textAlign = TextAlign.Center)
        Spacer(Modifier.height(6.dp))
        Text(
            subtitle,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center
        )
    }
}
