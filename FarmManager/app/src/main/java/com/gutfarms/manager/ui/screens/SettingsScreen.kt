package com.gutfarms.manager.ui.screens

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.SectionLabel
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Mist
import com.gutfarms.manager.update.AppUpdateChecker
import com.gutfarms.manager.update.AppUpdateInfo
import com.gutfarms.manager.update.UpdateCheckResult
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import androidx.compose.runtime.collectAsState

private sealed class UpdateUiState {
    data object Idle : UpdateUiState()
    data object Checking : UpdateUiState()
    data class Ready(val info: AppUpdateInfo) : UpdateUiState()
    data class Message(val text: String) : UpdateUiState()
    data object Downloading : UpdateUiState()
}

@Composable
fun SettingsScreen(
    farmName: StateFlow<String>,
    onBack: () -> Unit
) {
    val brand by farmName.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var state by remember { mutableStateOf<UpdateUiState>(UpdateUiState.Idle) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Brush.verticalGradient(listOf(CreamLeaf, Mist, CreamLeaf)))
            .verticalScroll(rememberScrollState())
    ) {
        ScreenHeader(
            brand = brand,
            title = "Settings",
            subtitle = "App version and updates for sideloaded installs."
        )

        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp)
        ) {
            TextButton(onClick = onBack) { Text("← Back") }

            SectionLabel("About")
            Text(
                text = "Installed version: ${AppUpdateChecker.currentVersionLabel()}",
                style = MaterialTheme.typography.bodyLarge
            )
            Text(
                text = "Updates install over the current app when a newer release is published.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            SectionLabel("Fetch update")
            when (val s = state) {
                UpdateUiState.Idle -> {
                    Button(
                        onClick = {
                            scope.launch {
                                state = UpdateUiState.Checking
                                state = when (val result = AppUpdateChecker.checkForUpdate()) {
                                    is UpdateCheckResult.Available ->
                                        UpdateUiState.Ready(result.info)
                                    is UpdateCheckResult.UpToDate ->
                                        UpdateUiState.Message(
                                            "You're on the latest build (${result.currentVersion})."
                                        )
                                    is UpdateCheckResult.Error ->
                                        UpdateUiState.Message(result.message)
                                }
                            }
                        },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text("Check for updates")
                    }
                }

                UpdateUiState.Checking -> {
                    Column(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        CircularProgressIndicator()
                        Spacer(Modifier.height(8.dp))
                        Text("Checking GitHub releases…")
                    }
                }

                is UpdateUiState.Ready -> {
                    Text(
                        text = "Update available: ${s.info.versionName} (${s.info.versionCode})",
                        style = MaterialTheme.typography.titleMedium
                    )
                    if (s.info.releaseNotes.isNotBlank()) {
                        Text(
                            text = s.info.releaseNotes,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant
                        )
                    }
                    Button(
                        onClick = {
                            scope.launch {
                                if (!AppUpdateChecker.canRequestInstall(context)) {
                                    context.startActivity(
                                        AppUpdateChecker.installPermissionSettingsIntent(context)
                                    )
                                    state = UpdateUiState.Message(
                                        "Allow “Install unknown apps” for Gut Farms, then tap Download & install again."
                                    )
                                    return@launch
                                }
                                state = UpdateUiState.Downloading
                                try {
                                    val file = AppUpdateChecker.downloadApk(context, s.info)
                                    AppUpdateChecker.installApk(context, file)
                                    state = UpdateUiState.Message(
                                        "Installer opened for ${s.info.versionName}. Confirm install on the next screen."
                                    )
                                } catch (e: Exception) {
                                    state = UpdateUiState.Message(
                                        e.message ?: "Download failed"
                                    )
                                }
                            }
                        },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text("Download & install")
                    }
                    OutlinedButton(
                        onClick = { state = UpdateUiState.Idle },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text("Cancel")
                    }
                }

                UpdateUiState.Downloading -> {
                    Column(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        CircularProgressIndicator()
                        Spacer(Modifier.height(8.dp))
                        Text("Downloading APK…")
                    }
                }

                is UpdateUiState.Message -> {
                    Text(
                        text = s.text,
                        style = MaterialTheme.typography.bodyLarge
                    )
                    Button(
                        onClick = { state = UpdateUiState.Idle },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Text("Check again")
                    }
                }
            }

            SectionLabel("Manual download")
            Text(
                text = "You can also sideload the APK from GitHub if in-app update is unavailable.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            OutlinedButton(
                onClick = {
                    val intent = Intent(
                        Intent.ACTION_VIEW,
                        Uri.parse(
                            "https://github.com/GutFarms/Japan-central/blob/cursor/farm-management-android-115a/FarmManager/dist/GutFarms-FarmManager.apk"
                        )
                    )
                    context.startActivity(intent)
                },
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Open APK on GitHub")
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}
