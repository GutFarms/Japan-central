package com.solstice.dispensary.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

val Charcoal = Color(0xFF141816)
val CharcoalLift = Color(0xFF1E2420)
val Sage = Color(0xFF7FA187)
val SageDeep = Color(0xFF3E5C45)
val Amber = Color(0xFFC9A45C)
val AmberSoft = Color(0xFFE2C88A)
val Ivory = Color(0xFFF2EFE6)
val MistGreen = Color(0xFFD8E3D7)
val Slate = Color(0xFF5C675F)
val Clay = Color(0xFFB56A4C)

private val LightColors = lightColorScheme(
    primary = SageDeep,
    onPrimary = Ivory,
    primaryContainer = MistGreen,
    onPrimaryContainer = Charcoal,
    secondary = Amber,
    onSecondary = Charcoal,
    secondaryContainer = Color(0xFFF3E6C8),
    onSecondaryContainer = Color(0xFF3A2E12),
    tertiary = Sage,
    onTertiary = Charcoal,
    background = Color(0xFFF7F5F0),
    onBackground = Charcoal,
    surface = Color.White,
    onSurface = Charcoal,
    surfaceVariant = Color(0xFFE8EDE7),
    onSurfaceVariant = Slate,
    error = Clay,
    onError = Ivory,
    outline = Color(0xFF9AAB9C)
)

private val DarkColors = darkColorScheme(
    primary = Sage,
    onPrimary = Charcoal,
    primaryContainer = SageDeep,
    onPrimaryContainer = Ivory,
    secondary = AmberSoft,
    onSecondary = Charcoal,
    secondaryContainer = Color(0xFF4A3B1C),
    onSecondaryContainer = AmberSoft,
    tertiary = MistGreen,
    onTertiary = Charcoal,
    background = Charcoal,
    onBackground = Ivory,
    surface = CharcoalLift,
    onSurface = Ivory,
    surfaceVariant = Color(0xFF2A332C),
    onSurfaceVariant = Color(0xFFB7C4B8),
    error = Clay,
    onError = Ivory,
    outline = Color(0xFF6E7B70)
)

private val AppTypography = Typography(
    displayLarge = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.Bold,
        fontSize = 42.sp,
        lineHeight = 48.sp,
        letterSpacing = (-0.8).sp
    ),
    displayMedium = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.Bold,
        fontSize = 32.sp,
        lineHeight = 38.sp
    ),
    headlineLarge = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 28.sp,
        lineHeight = 34.sp
    ),
    headlineMedium = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 22.sp,
        lineHeight = 28.sp
    ),
    titleLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 18.sp,
        lineHeight = 24.sp
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 16.sp,
        lineHeight = 22.sp
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp,
        lineHeight = 24.sp
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 13.sp,
        letterSpacing = 0.4.sp
    ),
    labelSmall = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Medium,
        fontSize = 11.sp,
        letterSpacing = 0.6.sp
    )
)

@Composable
fun SolsticeTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        typography = AppTypography,
        content = content
    )
}
