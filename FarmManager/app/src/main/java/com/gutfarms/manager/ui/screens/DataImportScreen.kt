package com.gutfarms.manager.ui.screens

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.CloudDownload
import androidx.compose.material.icons.outlined.Delete
import androidx.compose.material.icons.outlined.UploadFile
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import com.gutfarms.manager.data.model.ApiFeedSource
import com.gutfarms.manager.data.model.ApiHttpMethod
import com.gutfarms.manager.data.model.FarmImportFile
import com.gutfarms.manager.ui.components.EmptyHint
import com.gutfarms.manager.ui.components.ScreenHeader
import com.gutfarms.manager.ui.components.formatDate
import com.gutfarms.manager.ui.theme.CreamLeaf
import com.gutfarms.manager.ui.theme.Forest
import com.gutfarms.manager.ui.theme.Mist
import kotlinx.coroutines.flow.StateFlow
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DataImportScreen(
    farmName: StateFlow<String>,
    importFiles: StateFlow<List<FarmImportFile>>,
    apiSources: StateFlow<List<ApiFeedSource>>,
    dataMessage: StateFlow<String?>,
    onClearMessage: () -> Unit,
    onImportUri: (Uri) -> Unit,
    onDeleteFile: (FarmImportFile) -> Unit,
    onSaveApiSource: (ApiFeedSource) -> Unit,
    onDeleteApiSource: (ApiFeedSource) -> Unit,
    onToggleApiSource: (ApiFeedSource) -> Unit,
    onPullApiSource: (ApiFeedSource) -> Unit,
    onBack: () -> Unit
) {
    val context = LocalContext.current
    val brand by farmName.collectAsState()
    val files by importFiles.collectAsState()
    val sources by apiSources.collectAsState()
    val message by dataMessage.collectAsState()
    val snackbar = remember { SnackbarHostState() }

    var showApiSheet by remember { mutableStateOf(false) }
    var editingSource by remember { mutableStateOf<ApiFeedSource?>(null) }
    var previewSource by remember { mutableStateOf<ApiFeedSource?>(null) }

    val picker = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        if (uri != null) {
            runCatching {
                context.contentResolver.takePersistableUriPermission(
                    uri,
                    android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
            }
            onImportUri(uri)
        }
    }

    LaunchedEffect(message) {
        val text = message ?: return@LaunchedEffect
        snackbar.showSnackbar(text)
        onClearMessage()
    }

    Scaffold(
        snackbarHost = { SnackbarHost(snackbar) },
        floatingActionButton = {}
    ) { padding ->
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .background(Brush.verticalGradient(listOf(CreamLeaf, Mist, CreamLeaf)))
                .padding(padding),
            contentPadding = PaddingValues(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)
        ) {
            item {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
                ) {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                    Text("Data & maps", style = MaterialTheme.typography.titleLarge, color = Forest)
                }
                ScreenHeader(
                    brand = brand,
                    title = "Upload files & pull site APIs",
                    subtitle = "Import KMZ / KML / CSV / JSON maps and wire external data feeds."
                )
            }

            item {
                SectionCard(title = "Farm files") {
                    Text(
                        "Upload pasture boundaries, field maps, herd spreadsheets, and other farm documents. KMZ/KML are summarized on import.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.height(12.dp))
                    Button(
                        onClick = {
                            picker.launch(
                                arrayOf(
                                    "application/vnd.google-earth.kmz",
                                    "application/vnd.google-earth.kml+xml",
                                    "application/json",
                                    "text/csv",
                                    "text/comma-separated-values",
                                    "text/plain",
                                    "image/*",
                                    "application/octet-stream",
                                    "*/*"
                                )
                            )
                        },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Icon(Icons.Outlined.UploadFile, contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text("Upload KMZ or other file")
                    }
                }
            }

            if (files.isEmpty()) {
                item {
                    EmptyHint("No files imported yet. Start with a KMZ pasture map or CSV herd list.")
                }
            } else {
                items(files, key = { it.id }) { file ->
                    FileRow(file = file, onDelete = { onDeleteFile(file) })
                }
            }

            item {
                SectionCard(title = "External API feeds") {
                    Text(
                        "Add HTTPS endpoints to pull weather, markets, registries, or your own farm services. Auth header is optional (Bearer token or Header: value).",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                    Spacer(Modifier.height(12.dp))
                    Button(
                        onClick = {
                            editingSource = null
                            showApiSheet = true
                        },
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        Icon(Icons.Outlined.CloudDownload, contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text("Add API source")
                    }
                }
            }

            if (sources.isEmpty()) {
                item {
                    EmptyHint("No API sources yet. Add a URL placeholder for future site integrations.")
                }
            } else {
                items(sources, key = { it.id }) { source ->
                    ApiRow(
                        source = source,
                        onToggle = { onToggleApiSource(source) },
                        onPull = { onPullApiSource(source) },
                        onEdit = {
                            editingSource = source
                            showApiSheet = true
                        },
                        onPreview = { previewSource = source },
                        onDelete = { onDeleteApiSource(source) }
                    )
                }
            }
        }
    }

    if (showApiSheet) {
        ApiSourceSheet(
            initial = editingSource,
            onDismiss = { showApiSheet = false },
            onSave = { source ->
                onSaveApiSource(source)
                showApiSheet = false
            }
        )
    }

    previewSource?.let { source ->
        ModalBottomSheet(onDismissRequest = { previewSource = null }) {
            Column(Modifier = Modifier.padding(20.dp)) {
                Text(source.name, style = MaterialTheme.typography.titleLarge)
                Spacer(Modifier.height(8.dp))
                Text(source.lastStatus.ifBlank { "Not pulled yet" })
                Spacer(Modifier.height(12.dp))
                Text(
                    source.lastPreview.ifBlank { "(no preview)" },
                    style = MaterialTheme.typography.bodySmall
                )
                Spacer(Modifier.height(20.dp))
            }
        }
    }
}

@Composable
private fun SectionCard(title: String, content: @Composable () -> Unit) {
    Column(
        modifier = Modifier
            .padding(horizontal = 16.dp)
            .fillMaxWidth()
            .clip(RoundedCornerShape(18.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(16.dp)
    ) {
        Text(title, style = MaterialTheme.typography.titleMedium, color = Forest)
        Spacer(Modifier.height(8.dp))
        content()
    }
}

@Composable
private fun FileRow(file: FarmImportFile, onDelete: () -> Unit) {
    Column(
        modifier = Modifier
            .padding(horizontal = 16.dp)
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(16.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier = Modifier.weight(1f)) {
                Text(file.displayName, style = MaterialTheme.typography.titleMedium)
                Text(
                    "${file.kind.name} · ${formatBytes(file.byteSize)} · ${formatDate(file.importedAt)}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            IconButton(onClick = onDelete) {
                Icon(Icons.Outlined.Delete, contentDescription = "Delete file")
            }
        }
        if (file.summary.isNotBlank()) {
            Spacer(Modifier.height(6.dp))
            Text(
                file.summary,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

@Composable
private fun ApiRow(
    source: ApiFeedSource,
    onToggle: () -> Unit,
    onPull: () -> Unit,
    onEdit: () -> Unit,
    onPreview: () -> Unit,
    onDelete: () -> Unit
) {
    Column(
        modifier = Modifier
            .padding(horizontal = 16.dp)
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .background(MaterialTheme.colorScheme.surface)
            .padding(16.dp)
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier = Modifier.weight(1f)) {
                Text(source.name, style = MaterialTheme.typography.titleMedium)
                Text(
                    "${source.method.name} · ${source.baseUrl}",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2
                )
            }
            Switch(checked = source.enabled, onCheckedChange = { onToggle() })
        }
        if (source.lastStatus.isNotBlank()) {
            Spacer(Modifier.height(6.dp))
            Text(
                buildString {
                    append(source.lastStatus)
                    source.lastPulledAt?.let { append(" · "); append(formatDate(it)) }
                },
                style = MaterialTheme.typography.bodySmall
            )
        }
        Spacer(Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = onPull, enabled = source.enabled) { Text("Pull now") }
            TextButton(onClick = onEdit) { Text("Edit") }
            TextButton(onClick = onPreview, enabled = source.lastPreview.isNotBlank()) {
                Text("Preview")
            }
            TextButton(onClick = onDelete) { Text("Delete") }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ApiSourceSheet(
    initial: ApiFeedSource?,
    onDismiss: () -> Unit,
    onSave: (ApiFeedSource) -> Unit
) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    var name by remember(initial) { mutableStateOf(initial?.name.orEmpty()) }
    var url by remember(initial) { mutableStateOf(initial?.baseUrl.orEmpty()) }
    var method by remember(initial) { mutableStateOf(initial?.method ?: ApiHttpMethod.GET) }
    var auth by remember(initial) { mutableStateOf(initial?.authHeader.orEmpty()) }
    var notes by remember(initial) { mutableStateOf(initial?.notes.orEmpty()) }

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = sheetState) {
        Column(
            modifier = Modifier
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Text(
                if (initial == null) "Add API source" else "Edit API source",
                style = MaterialTheme.typography.titleLarge
            )
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Name") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth()
            )
            OutlinedTextField(
                value = url,
                onValueChange = { url = it },
                label = { Text("API URL") },
                placeholder = { Text("https://api.example.com/v1/farm") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth()
            )
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ApiHttpMethod.entries.forEach { option ->
                    FilterChip(
                        selected = method == option,
                        onClick = { method = option },
                        label = { Text(option.name) }
                    )
                }
            }
            OutlinedTextField(
                value = auth,
                onValueChange = { auth = it },
                label = { Text("Auth header (optional)") },
                placeholder = { Text("Bearer …  or  X-Api-Key: …") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth()
            )
            OutlinedTextField(
                value = notes,
                onValueChange = { notes = it },
                label = { Text("Notes") },
                modifier = Modifier.fillMaxWidth()
            )
            Button(
                onClick = {
                    onSave(
                        (initial ?: ApiFeedSource(name = "", baseUrl = "")).copy(
                            name = name.trim(),
                            baseUrl = url.trim(),
                            method = method,
                            authHeader = auth.trim(),
                            notes = notes.trim()
                        )
                    )
                },
                enabled = name.isNotBlank() && url.trim().startsWith("http"),
                modifier = Modifier.fillMaxWidth()
            ) {
                Text("Save API source")
            }
        }
    }
}

private fun formatBytes(bytes: Long): String {
    if (bytes < 1024) return "$bytes B"
    val kb = bytes / 1024.0
    if (kb < 1024) return String.format(Locale.US, "%.1f KB", kb)
    return String.format(Locale.US, "%.2f MB", kb / 1024.0)
}
