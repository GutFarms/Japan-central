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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import com.solstice.dispensary.data.model.ThemeMode

// Plant-inspired Native Pure palette — canopy greens + soil warmth
val Charcoal = Color(0xFF121A16)
val CharcoalLift = Color(0xFF1B2620)
val Leaf = Color(0xFF3F6B4F)
val LeafDeep = Color(0xFF2A4A36)
val Fern = Color(0xFF5E8A6A)
val Moss = Color(0xFF7FA388)
val Stem = Color(0xFF9BB59A)
val GreenhouseMist = Color(0xFFE8F0E6)
val CanopyLight = Color(0xFFF3F7F1)
val Soil = Color(0xFF4A3A2E)
val DuskCanopy = Color(0xFF0E1612)
val DuskLeaf = Color(0xFF1E3326)

// Keep brand-adjacent accents from logo
val Terracotta = Color(0xFFB07858)
val TerracottaDeep = Color(0xFF8A5A3C)
val Teal = Fern
val TealDeep = LeafDeep
val Cream = GreenhouseMist
val Ivory = CanopyLight
val Slate = Color(0xFF5A6A5E)
val Clay = Color(0xFFA8654A)

// Legacy aliases used across screens
val Sage = Fern
val SageDeep = LeafDeep
val Amber = Terracotta
val AmberSoft = Color(0xFFD0A07A)
val MistGreen = Stem

private val LightColors = lightColorScheme(
    primary = LeafDeep,
    onPrimary = CanopyLight,
    primaryContainer = Color(0xFFD5E6D8),
    onPrimaryContainer = Charcoal,
    secondary = Terracotta,
    onSecondary = Charcoal,
    secondaryContainer = Color(0xFFEEDFCC),
    onSecondaryContainer = Soil,
    tertiary = Fern,
    onTertiary = CanopyLight,
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
    secondary = AmberSoft,
    onSecondary = Charcoal,
    secondaryContainer = TerracottaDeep,
    onSecondaryContainer = CanopyLight,
    tertiary = Stem,
    onTertiary = DuskCanopy,
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
    displayLarge = TextStyle(
        fontFamily = FrauncesFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 44.sp,
        lineHeight = 50.sp,
        letterSpacing = (-0.6).sp
    ),
    displayMedium = TextStyle(
        fontFamily = FrauncesFamily,
        fontWeight = FontWeight.Bold,
        fontSize = 34.sp,
        lineHeight = 40.sp,
        letterSpacing = (-0.4).sp
    ),
    headlineLarge = TextStyle(
        fontFamily = FrauncesFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 28.sp,
        lineHeight = 34.sp
    ),
    headlineMedium = TextStyle(
        fontFamily = FrauncesFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 22.sp,
        lineHeight = 28.sp
    ),
    titleLarge = TextStyle(
        fontFamily = NunitoSansFamily,
        fontWeight = FontWeight.SemiBold,
        fontSize = 18.sp,
        lineHeight = 24.sp
    ),
    titleMedium = TextStyle(
        fontFamily = NunitoSansFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 16.sp,
        lineHeight = 22.sp
    ),
    bodyLarge = TextStyle(
        fontFamily = NunitoSansFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp,
        lineHeight = 24.sp
    ),
    bodyMedium = TextStyle(
        fontFamily = NunitoSansFamily,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        lineHeight = 20.sp
    ),
    labelLarge = TextStyle(
        fontFamily = NunitoSansFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 13.sp,
        letterSpacing = 0.3.sp
    ),
    labelSmall = TextStyle(
        fontFamily = NunitoSansFamily,
        fontWeight = FontWeight.Medium,
        fontSize = 11.sp,
        letterSpacing = 0.5.sp
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
