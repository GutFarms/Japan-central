package com.solstice.dispensary.ui.components

import android.graphics.Bitmap
import android.graphics.Color as AndroidColor
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

object AppDownloadLinks {
    const val ANDROID_APK =
        "https://github.com/GutFarms/Japan-central/releases/latest/download/NativePure-Dispensary.apk"
    const val DOWNLOADS_PAGE =
        "https://github.com/GutFarms/Japan-central/blob/master/DOWNLOADS.md"
    const val COMPANION_DOCS =
        "https://github.com/GutFarms/Japan-central/blob/master/DOWNLOADS.md#desktop-companion"
}

fun qrBitmap(
    content: String,
    sizePx: Int = 720,
    margin: Int = 1
): Bitmap {
    val hints = mapOf(
        EncodeHintType.CHARACTER_SET to "UTF-8",
        EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
        EncodeHintType.MARGIN to margin
    )
    val matrix = QRCodeWriter().encode(content, BarcodeFormat.QR_CODE, sizePx, sizePx, hints)
    val bitmap = Bitmap.createBitmap(sizePx, sizePx, Bitmap.Config.ARGB_8888)
    for (x in 0 until sizePx) {
        for (y in 0 until sizePx) {
            bitmap.setPixel(
                x,
                y,
                if (matrix[x, y]) AndroidColor.BLACK else AndroidColor.WHITE
            )
        }
    }
    return bitmap
}

@Composable
fun DownloadQrDialog(
    title: String,
    subtitle: String,
    url: String,
    onDismiss: () -> Unit
) {
    val bitmap = remember(url) { qrBitmap(url) }
    Dialog(onDismissRequest = onDismiss) {
        Surface(
            shape = RoundedCornerShape(20.dp),
            tonalElevation = 3.dp,
            modifier = Modifier.fillMaxWidth()
        ) {
            Column(
                modifier = Modifier.padding(20.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Text(title, style = MaterialTheme.typography.titleLarge, textAlign = TextAlign.Center)
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
                Image(
                    bitmap = bitmap.asImageBitmap(),
                    contentDescription = "Download QR code",
                    modifier = Modifier.size(240.dp),
                    contentScale = ContentScale.Fit
                )
                Text(
                    url,
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
                Spacer(Modifier.height(4.dp))
                TextButton(onClick = onDismiss) { Text("Close") }
            }
        }
    }
}
