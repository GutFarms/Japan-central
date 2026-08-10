package com.solstice.dispensary.ui.theme

import android.app.Activity
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import com.solstice.dispensary.data.model.ThemeMode

// Native Pure brand palette (from main logo)
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

// Legacy aliases used across screens
val Sage = Teal
val SageDeep = TealDeep
val Amber = Terracotta
val AmberSoft = Color(0xFFD4A07A)
val MistGreen = Color(0xFFD9E4E2)

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
    onTertiary = Ivory,
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
    secondary = AmberSoft,
    onSecondary = Charcoal,
    secondaryContainer = TerracottaDeep,
    onSecondaryContainer = Ivory,
    tertiary = Cream,
    onTertiary = Charcoal,
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
    displayLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 42.sp,
        lineHeight = 48.sp,
        letterSpacing = (-0.8).sp
    ),
    displayMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.Bold,
        fontSize = 32.sp,
        lineHeight = 38.sp
    ),
    headlineLarge = TextStyle(
        fontFamily = FontFamily.SansSerif,
        fontWeight = FontWeight.SemiBold,
        fontSize = 28.sp,
        lineHeight = 34.sp
    ),
    headlineMedium = TextStyle(
        fontFamily = FontFamily.SansSerif,
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
    themeMode: ThemeMode = ThemeMode.SYSTEM,
    darkTheme: Boolean = when (themeMode) {
        ThemeMode.LIGHT -> false
        ThemeMode.DARK -> true
        ThemeMode.SYSTEM -> isSystemInDarkTheme()
    },
    content: @Composable () -> Unit
) {
    val colorScheme = if (darkTheme) DarkColors else LightColors
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            WindowCompat.getInsetsController(window, view).isAppearanceLightStatusBars = !darkTheme
            WindowCompat.getInsetsController(window, view).isAppearanceLightNavigationBars = !darkTheme
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        typography = AppTypography,
        content = content
    )
}
