package com.solstice.dispensary.ui.screens

import android.Manifest
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.camera.core.CameraSelector
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.camera.view.PreviewView
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.ArrowBack
import androidx.compose.material.icons.outlined.CameraAlt
import androidx.compose.material.icons.outlined.DocumentScanner
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.ContextCompat
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.solstice.dispensary.data.model.LabelScanResult
import com.solstice.dispensary.ui.components.MetaPill
import com.solstice.dispensary.ui.components.QuantityStepper
import com.solstice.dispensary.ui.theme.Amber
import com.solstice.dispensary.ui.theme.Charcoal
import com.solstice.dispensary.ui.theme.Ivory
import java.io.File
import java.util.concurrent.Executors
import kotlin.math.roundToInt

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun InventoryScannerScreen(
    scanBusy: Boolean,
    scanResult: LabelScanResult?,
    scanError: String?,
    onBack: () -> Unit,
    onCaptureBitmap: (Bitmap) -> Unit,
    onClearResult: () -> Unit,
    onConfirm: (Int) -> Unit
) {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    var hasCameraPermission by remember {
        mutableStateOf(
            ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) ==
                PackageManager.PERMISSION_GRANTED
        )
    }
    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted -> hasCameraPermission = granted }

    LaunchedEffect(Unit) {
        if (!hasCameraPermission) {
            permissionLauncher.launch(Manifest.permission.CAMERA)
        }
    }

    val imageCapture = remember {
        ImageCapture.Builder()
            .setCaptureMode(ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY)
            .build()
    }
    val cameraExecutor = remember { Executors.newSingleThreadExecutor() }
    var capturing by remember { mutableStateOf(false) }
    var quantity by remember(scanResult) {
        mutableIntStateOf(scanResult?.suggestedQuantity?.coerceAtLeast(1) ?: 1)
    }

    DisposableEffect(Unit) {
        onDispose { cameraExecutor.shutdown() }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("AI inventory scan") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Outlined.ArrowBack, contentDescription = "Back")
                    }
                },
                actions = {
                    if (scanResult != null) {
                        TextButton(onClick = onClearResult) { Text("Rescan") }
                    }
                }
            )
        }
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            when {
                !hasCameraPermission -> {
                    Column(
                        modifier = Modifier
                            .fillMaxSize()
                            .padding(24.dp),
                        verticalArrangement = Arrangement.Center,
                        horizontalAlignment = Alignment.CenterHorizontally
                    ) {
                        Icon(Icons.Outlined.CameraAlt, contentDescription = null)
                        Spacer(Modifier.height(12.dp))
                        Text(
                            "Camera access is required to scan product labels and barcodes.",
                            style = MaterialTheme.typography.bodyLarge
                        )
                        Spacer(Modifier.height(16.dp))
                        Button(onClick = { permissionLauncher.launch(Manifest.permission.CAMERA) }) {
                            Text("Allow camera")
                        }
                    }
                }

                scanResult != null -> {
                    ScanResultPanel(
                        result = scanResult,
                        quantity = quantity,
                        onQuantityChange = { quantity = it },
                        onConfirm = { onConfirm(quantity) },
                        onRescan = onClearResult
                    )
                }

                else -> {
                    Box(modifier = Modifier.fillMaxSize()) {
                        AndroidView(
                            modifier = Modifier.fillMaxSize(),
                            factory = { ctx ->
                                val previewView = PreviewView(ctx)
                                val providerFuture = ProcessCameraProvider.getInstance(ctx)
                                providerFuture.addListener({
                                    val provider = providerFuture.get()
                                    val preview = Preview.Builder().build()
                                    preview.setSurfaceProvider(previewView.getSurfaceProvider())
                                    provider.unbindAll()
                                    provider.bindToLifecycle(
                                        lifecycleOwner,
                                        CameraSelector.DEFAULT_BACK_CAMERA,
                                        preview,
                                        imageCapture
                                    )
                                }, ContextCompat.getMainExecutor(ctx))
                                previewView
                            }
                        )

                        Column(
                            modifier = Modifier
                                .align(Alignment.BottomCenter)
                                .fillMaxWidth()
                                .background(Charcoal.copy(alpha = 0.72f))
                                .padding(20.dp),
                            horizontalAlignment = Alignment.CenterHorizontally
                        ) {
                            Text(
                                text = "Point at a product label or barcode",
                                color = Ivory,
                                style = MaterialTheme.typography.titleMedium
                            )
                            Text(
                                text = "On-device AI reads text + barcodes, then matches inventory",
                                color = Ivory.copy(alpha = 0.75f),
                                style = MaterialTheme.typography.bodyMedium
                            )
                            Spacer(Modifier.height(16.dp))
                            IconButton(
                                onClick = {
                                    if (capturing || scanBusy) return@IconButton
                                    capturing = true
                                    val photoFile = File(
                                        context.cacheDir,
                                        "inventory-scan-${System.currentTimeMillis()}.jpg"
                                    )
                                    val output = ImageCapture.OutputFileOptions.Builder(photoFile).build()
                                    imageCapture.takePicture(
                                        output,
                                        cameraExecutor,
                                        object : ImageCapture.OnImageSavedCallback {
                                            override fun onImageSaved(
                                                outputFileResults: ImageCapture.OutputFileResults
                                            ) {
                                                val bitmap = BitmapFactory.decodeFile(photoFile.absolutePath)
                                                photoFile.delete()
                                                capturing = false
                                                if (bitmap != null) {
                                                    onCaptureBitmap(bitmap)
                                                }
                                            }

                                            override fun onError(exception: ImageCaptureException) {
                                                capturing = false
                                            }
                                        }
                                    )
                                },
                                modifier = Modifier
                                    .clip(CircleShape)
                                    .background(Amber)
                                    .padding(8.dp)
                            ) {
                                Icon(
                                    Icons.Outlined.DocumentScanner,
                                    contentDescription = "Scan label",
                                    tint = Charcoal
                                )
                            }
                            Spacer(Modifier.height(8.dp))
                            Text("Tap to scan", color = Amber, style = MaterialTheme.typography.labelLarge)
                        }

                        if (scanBusy || capturing) {
                            Box(
                                modifier = Modifier
                                    .fillMaxSize()
                                    .background(Color.Black.copy(alpha = 0.45f)),
                                contentAlignment = Alignment.Center
                            ) {
                                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                                    CircularProgressIndicator(color = Amber)
                                    Spacer(Modifier.height(12.dp))
                                    Text("Reading label…", color = Ivory)
                                }
                            }
                        }

                        if (scanError != null) {
                            Surface(
                                modifier = Modifier
                                    .align(Alignment.TopCenter)
                                    .padding(16.dp),
                                color = MaterialTheme.colorScheme.errorContainer,
                                shape = RoundedCornerShape(12.dp)
                            ) {
                                Text(
                                    text = scanError,
                                    modifier = Modifier.padding(12.dp),
                                    color = MaterialTheme.colorScheme.onErrorContainer
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ScanResultPanel(
    result: LabelScanResult,
    quantity: Int,
    onQuantityChange: (Int) -> Unit,
    onConfirm: () -> Unit,
    onRescan: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        Text(
            text = if (result.isNewProduct) "New product detected" else "Matched inventory item",
            style = MaterialTheme.typography.headlineMedium
        )
        Text(
            text = "Confidence ${(result.matchConfidence * 100).roundToInt()}%",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )

        val title = result.matchedProduct?.name
            ?: result.detectedName
            ?: "Unrecognized label"
        val subtitle = result.matchedProduct?.let { "${it.brand} · SKU ${it.sku}" }
            ?: listOfNotNull(result.detectedBrand, result.barcode).joinToString(" · ")

        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surface,
            tonalElevation = 1.dp,
            modifier = Modifier.fillMaxWidth()
        ) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(title, style = MaterialTheme.typography.titleLarge)
                if (subtitle.isNotBlank()) {
                    Text(subtitle, style = MaterialTheme.typography.bodyMedium)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    result.detectedCategory?.let { MetaPill(it.label) }
                    result.detectedStrain?.let { MetaPill(it.label) }
                    result.detectedThc?.let { MetaPill("THC $it%") }
                    result.detectedCbd?.let { MetaPill("CBD $it%") }
                }
                if (result.matchedProduct != null) {
                    Text(
                        text = "Current stock: ${result.matchedProduct.stockQuantity}",
                        style = MaterialTheme.typography.bodyMedium
                    )
                } else {
                    Text(
                        text = "No catalog match — confirming will create a new inventory item from the label.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
            }
        }

        if (result.rawText.isNotBlank()) {
            Surface(
                shape = RoundedCornerShape(14.dp),
                color = MaterialTheme.colorScheme.surfaceVariant,
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(modifier = Modifier.padding(14.dp)) {
                    Text("AI read text", style = MaterialTheme.typography.titleMedium)
                    Spacer(Modifier.height(6.dp))
                    Text(
                        text = result.rawText.take(500),
                        style = MaterialTheme.typography.bodyMedium
                    )
                }
            }
        }

        Text("Quantity to add", style = MaterialTheme.typography.titleMedium)
        QuantityStepper(
            quantity = quantity,
            onDecrease = { if (quantity > 1) onQuantityChange(quantity - 1) },
            onIncrease = { onQuantityChange(quantity + 1) }
        )

        Button(
            onClick = onConfirm,
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(14.dp)
        ) {
            Text(
                if (result.isNewProduct) "Create item + add $quantity"
                else "Add $quantity to inventory"
            )
        }
        TextButton(onClick = onRescan, modifier = Modifier.fillMaxWidth()) {
            Text("Scan another label")
        }
    }
}
