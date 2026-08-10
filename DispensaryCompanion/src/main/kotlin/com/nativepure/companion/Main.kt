package com.nativepure.companion

import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import com.nativepure.companion.data.CompanionRepository
import com.nativepure.companion.ui.CompanionApp
import com.nativepure.companion.ui.CompanionTheme
import java.awt.Dimension

fun main() = application {
    val repository = CompanionRepository()
    val state = rememberWindowState(size = DpSize(1180.dp, 760.dp))
    Window(
        onCloseRequest = ::exitApplication,
        title = "Native Pure — Desktop Companion",
        state = state,
        icon = painterResource("logo_main.png")
    ) {
        window.minimumSize = Dimension(960, 640)
        CompanionTheme {
            CompanionApp(repository = repository)
        }
    }
}
