package com.nativepure.companion.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.compose.material3.Typography

val Charcoal = Color(0xFF121A16)
val CharcoalLift = Color(0xFF1B2620)
val LeafDeep = Color(0xFF2A4A36)
val Fern = Color(0xFF5E8A6A)
val Moss = Color(0xFF7FA388)
val Stem = Color(0xFF9BB59A)
val GreenhouseMist = Color(0xFFE8F0E6)
val CanopyLight = Color(0xFFF3F7F1)
val Terracotta = Color(0xFFB07858)
val TerracottaDeep = Color(0xFF8A5A3C)
val Clay = Color(0xFFA8654A)
val DuskCanopy = Color(0xFF0E1612)
val DuskLeaf = Color(0xFF1E3326)
val Slate = Color(0xFF5A6A5E)

private val LightColors = lightColorScheme(
    primary = LeafDeep,
    onPrimary = CanopyLight,
    primaryContainer = Color(0xFFD5E6D8),
    onPrimaryContainer = Charcoal,
    secondary = Terracotta,
    onSecondary = Charcoal,
    secondaryContainer = Color(0xFFEEDFCC),
    onSecondaryContainer = Color(0xFF4A3A2E),
    tertiary = Fern,
    background = CanopyLight,
    onBackground = Charcoal,
    surface = Color(0xFFFAFCF8),
    onSurface = Charcoal,
    surfaceVariant = GreenhouseMist,
    onSurfaceVariant = Slate,
    error = Clay,
    onError = CanopyLight,
    outline = Color(0xFFA8B8AA)
)

private val DarkColors = darkColorScheme(
    primary = Moss,
    onPrimary = DuskCanopy,
    primaryContainer = LeafDeep,
    onPrimaryContainer = GreenhouseMist,
    secondary = Color(0xFFD0A07A),
    onSecondary = Charcoal,
    secondaryContainer = TerracottaDeep,
    onSecondaryContainer = CanopyLight,
    tertiary = Stem,
    background = DuskCanopy,
    onBackground = GreenhouseMist,
    surface = CharcoalLift,
    onSurface = GreenhouseMist,
    surfaceVariant = DuskLeaf,
    onSurfaceVariant = Color(0xFFB7C8B8),
    error = Clay,
    onError = CanopyLight,
    outline = Color(0xFF6A7C6C)
)

private val AppTypography = Typography(
    headlineLarge = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.Bold,
        fontSize = 34.sp
    ),
    headlineMedium = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 26.sp
    ),
    titleLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 20.sp
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp
    )
)

@Composable
fun CompanionTheme(dark: Boolean = false, content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (dark) DarkColors else LightColors,
        typography = AppTypography,
        content = content
    )
}
