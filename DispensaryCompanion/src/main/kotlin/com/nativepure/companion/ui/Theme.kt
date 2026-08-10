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

val Charcoal = Color(0xFF0B0B0B)
val CharcoalLift = Color(0xFF1A1714)
val Terracotta = Color(0xFFC47A4E)
val TerracottaDeep = Color(0xFF9E5A35)
val Teal = Color(0xFF5B8A8A)
val TealDeep = Color(0xFF3E6464)
val Cream = Color(0xFFF0E6D8)
val Ivory = Color(0xFFF7F1E8)
val Slate = Color(0xFF6B6258)
val Clay = Color(0xFFB56A4C)

private val LightColors = lightColorScheme(
    primary = TealDeep,
    onPrimary = Ivory,
    primaryContainer = Color(0xFFD9E4E2),
    onPrimaryContainer = Charcoal,
    secondary = Terracotta,
    onSecondary = Charcoal,
    secondaryContainer = Color(0xFFF3E0D2),
    onSecondaryContainer = Color(0xFF3A2415),
    tertiary = Teal,
    background = Color(0xFFF8F4EE),
    onBackground = Charcoal,
    surface = Color.White,
    onSurface = Charcoal,
    surfaceVariant = Color(0xFFECE6DC),
    onSurfaceVariant = Slate,
    error = Clay,
    onError = Ivory,
    outline = Color(0xFFB8A99A)
)

private val DarkColors = darkColorScheme(
    primary = Teal,
    onPrimary = Charcoal,
    primaryContainer = TealDeep,
    onPrimaryContainer = Ivory,
    secondary = Color(0xFFD4A07A),
    onSecondary = Charcoal,
    secondaryContainer = TerracottaDeep,
    onSecondaryContainer = Ivory,
    tertiary = Cream,
    background = Charcoal,
    onBackground = Ivory,
    surface = CharcoalLift,
    onSurface = Ivory,
    surfaceVariant = Color(0xFF2A2420),
    onSurfaceVariant = Color(0xFFC9BDB0),
    error = Clay,
    onError = Ivory,
    outline = Color(0xFF7A6E62)
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
