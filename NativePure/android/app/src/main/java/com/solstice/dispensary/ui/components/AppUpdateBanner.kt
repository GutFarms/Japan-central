package com.solstice.dispensary.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.update.UpdateUiState

/**
 * Simple 4-step download wizard: Check → Download (%) → Allow install → Install.
 */
@Composable
fun AppUpdateWizard(
    state: UpdateUiState,
    onCheck: () -> Unit = {},
    onDownload: () -> Unit,
    onInstall: () -> Unit,
    onDismiss: () -> Unit,
    onOpenInstallPermission: () -> Unit,
    needsInstallPermission: Boolean,
    compact: Boolean = false,
    showIdleCheck: Boolean = false
) {
    val step = when (state) {
        UpdateUiState.Checking -> 1
        is UpdateUiState.Available -> 1
        is UpdateUiState.Downloading -> 2
        is UpdateUiState.ReadyToInstall -> if (needsInstallPermission) 3 else 4
        else -> 0
    }
    val showShell = step > 0 ||
        showIdleCheck ||
        state is UpdateUiState.UpToDate ||
        state is UpdateUiState.Error ||
        state is UpdateUiState.Idle && showIdleCheck

    if (!showShell && step == 0) return

    Surface(
        color = when (state) {
            is UpdateUiState.ReadyToInstall -> MaterialTheme.colorScheme.tertiaryContainer
            is UpdateUiState.Available, is UpdateUiState.Downloading ->
                MaterialTheme.colorScheme.secondaryContainer
            else -> MaterialTheme.colorScheme.surfaceVariant
        },
        shape = RoundedCornerShape(16.dp),
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = if (compact) 0.dp else 16.dp, vertical = 8.dp)
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text(
                "Download wizard",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold
            )
            if (step > 0) {
                WizardSteps(currentStep = step)
            }

            when (state) {
                UpdateUiState.Idle, UpdateUiState.Checking -> {
                    Text(
                        if (state is UpdateUiState.Checking) {
                            "Checking GitHub for a newer app…"
                        } else {
                            "Check for a newer Native Pure APK, then follow the steps."
                        },
                        style = MaterialTheme.typography.bodyMedium
                    )
                    if (state !is UpdateUiState.Checking && showIdleCheck) {
                        Button(onClick = onCheck, modifier = Modifier.fillMaxWidth()) {
                            Text("1 · Check for update")
                        }
                    }
                }
                UpdateUiState.UpToDate -> {
                    Text("You’re on the latest version.", style = MaterialTheme.typography.bodyMedium)
                    if (showIdleCheck) {
                        Button(onClick = onCheck, modifier = Modifier.fillMaxWidth()) {
                            Text("Check again")
                        }
                    }
                }
                is UpdateUiState.Error -> {
                    Text(state.message, color = MaterialTheme.colorScheme.error)
                    Button(onClick = onCheck, modifier = Modifier.fillMaxWidth()) {
                        Text("Try again")
                    }
                }
                is UpdateUiState.Available -> {
                    Text(
                        "Step 1 of 4 — Update ready: v${state.info.versionName}",
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = FontWeight.Medium
                    )
                    if (state.info.releaseNotes.isNotBlank()) {
                        Text(state.info.releaseNotes, style = MaterialTheme.typography.bodyMedium)
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = onDownload) { Text("2 · Download") }
                        TextButton(onClick = onDismiss) { Text("Later") }
                    }
                }
                is UpdateUiState.Downloading -> {
                    val pct = state.progress?.let { (it * 100).toInt().coerceIn(0, 100) }
                    Text(
                        if (pct != null) "Step 2 of 4 — Downloading $pct%" else "Step 2 of 4 — Downloading…",
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = FontWeight.Medium
                    )
                    if (state.progress != null) {
                        LinearProgressIndicator(
                            progress = { state.progress },
                            modifier = Modifier.fillMaxWidth()
                        )
                        Text(
                            "$pct%",
                            style = MaterialTheme.typography.headlineSmall,
                            fontWeight = FontWeight.Bold
                        )
                    } else {
                        LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                    }
                }
                is UpdateUiState.ReadyToInstall -> {
                    if (needsInstallPermission) {
                        Text(
                            "Step 3 of 4 — Allow installs from this app",
                            style = MaterialTheme.typography.bodyLarge,
                            fontWeight = FontWeight.Medium
                        )
                        Text(
                            "Android needs permission before Native Pure can install the update.",
                            style = MaterialTheme.typography.bodyMedium
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(onClick = onOpenInstallPermission) { Text("Allow installs") }
                            Button(onClick = onInstall) { Text("4 · Install") }
                        }
                    } else {
                        Text(
                            "Step 4 of 4 — Install v${state.info.versionName}",
                            style = MaterialTheme.typography.bodyLarge,
                            fontWeight = FontWeight.Medium
                        )
                        Text(
                            "Download finished. Tap Install and confirm on the system screen.",
                            style = MaterialTheme.typography.bodyMedium
                        )
                        Button(onClick = onInstall, modifier = Modifier.fillMaxWidth()) {
                            Text("Install update")
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun WizardSteps(currentStep: Int) {
    val labels = listOf("Check", "Download", "Allow", "Install")
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        labels.forEachIndexed { index, label ->
            val n = index + 1
            val active = n <= currentStep
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Surface(
                    color = if (active) {
                        MaterialTheme.colorScheme.secondary
                    } else {
                        MaterialTheme.colorScheme.outlineVariant
                    },
                    shape = CircleShape,
                    modifier = Modifier.size(28.dp)
                ) {
                    Text(
                        text = n.toString(),
                        color = if (active) {
                            MaterialTheme.colorScheme.onSecondary
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                        style = MaterialTheme.typography.labelMedium,
                        modifier = Modifier.padding(6.dp)
                    )
                }
                Spacer(Modifier.height(4.dp))
                Text(
                    label,
                    style = MaterialTheme.typography.labelSmall,
                    color = if (active) {
                        MaterialTheme.colorScheme.onSurface
                    } else {
                        MaterialTheme.colorScheme.onSurfaceVariant
                    }
                )
            }
        }
    }
}

/** Back-compat alias used by older call sites. */
@Composable
fun AppUpdateBanner(
    state: UpdateUiState,
    onDownload: () -> Unit,
    onInstall: () -> Unit,
    onDismiss: () -> Unit,
    onOpenInstallPermission: () -> Unit,
    needsInstallPermission: Boolean,
    compact: Boolean = false
) {
    AppUpdateWizard(
        state = state,
        onDownload = onDownload,
        onInstall = onInstall,
        onDismiss = onDismiss,
        onOpenInstallPermission = onOpenInstallPermission,
        needsInstallPermission = needsInstallPermission,
        compact = compact
    )
}
