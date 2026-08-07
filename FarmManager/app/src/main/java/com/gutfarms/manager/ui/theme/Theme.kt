package com.gutfarms.manager.ui.theme

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

val Forest = Color(0xFF2F5D3A)
val ForestDeep = Color(0xFF1E3D28)
val Moss = Color(0xFF6B8F71)
val Wheat = Color(0xFFF4C95F)
val Soil = Color(0xFF5C4033)
val CreamLeaf = Color(0xFFF3F7F0)
val Mist = Color(0xFFE4EDE3)
val Ink = Color(0xFF1A241C)
val SoftRed = Color(0xFFB85C38)
val SoftTeal = Color(0xFF3D7A6A)

private val LightColors = lightColorScheme(
    primary = Forest,
    onPrimary = Color.White,
    primaryContainer = Mist,
    onPrimaryContainer = ForestDeep,
    secondary = SoftTeal,
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFD5E8E2),
    onSecondaryContainer = Color(0xFF0F3A31),
    tertiary = Wheat,
    onTertiary = Ink,
    tertiaryContainer = Color(0xFFFFE9B0),
    onTertiaryContainer = Soil,
    background = CreamLeaf,
    onBackground = Ink,
    surface = Color.White,
    onSurface = Ink,
    surfaceVariant = Mist,
    onSurfaceVariant = Color(0xFF3F4A41),
    error = SoftRed,
    onError = Color.White,
    outline = Moss
)

private val DarkColors = darkColorScheme(
    primary = Moss,
    onPrimary = ForestDeep,
    primaryContainer = ForestDeep,
    onPrimaryContainer = Mist,
    secondary = SoftTeal,
    onSecondary = Color.White,
    background = Color(0xFF121812),
    onBackground = CreamLeaf,
    surface = Color(0xFF1A221A),
    onSurface = CreamLeaf,
    error = SoftRed
)

private val AppTypography = Typography(
    displayLarge = TextStyle(
        fontFamily = FontFamily.Serif,
        fontWeight = FontWeight.Bold,
        fontSize = 40.sp,
        lineHeight = 46.sp,
        letterSpacing = (-0.5).sp
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
        fontSize = 14.sp,
        letterSpacing = 0.2.sp
    )
)

@Composable
fun FarmManagerTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        typography = AppTypography,
        content = content
    )
}
