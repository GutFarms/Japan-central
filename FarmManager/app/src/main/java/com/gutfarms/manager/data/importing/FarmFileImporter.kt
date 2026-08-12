package com.gutfarms.manager.data.importing

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import com.gutfarms.manager.data.model.FarmFileKind
import java.io.BufferedInputStream
import java.io.File
import java.io.FileOutputStream
import java.util.zip.ZipInputStream

data class ImportedFilePayload(
    val displayName: String,
    val kind: FarmFileKind,
    val mimeType: String,
    val storedPath: String,
    val byteSize: Long,
    val summary: String
)

object FarmFileImporter {
    private const val IMPORT_DIR = "imports"

    fun importUri(context: Context, uri: Uri): ImportedFilePayload {
        val resolver = context.contentResolver
        val mime = resolver.getType(uri).orEmpty()
        val displayName = queryDisplayName(context, uri) ?: "import-${System.currentTimeMillis()}"
        val kind = classify(displayName, mime)

        val dir = File(context.filesDir, IMPORT_DIR).also { it.mkdirs() }
        val safeName = displayName.replace(Regex("[^A-Za-z0-9._-]"), "_")
        val dest = File(dir, "${System.currentTimeMillis()}_$safeName")

        resolver.openInputStream(uri)?.use { input ->
            FileOutputStream(dest).use { output -> input.copyTo(output) }
        } ?: error("Could not open selected file")

        val summary = when (kind) {
            FarmFileKind.KMZ -> summarizeKmz(dest)
            FarmFileKind.KML -> summarizeKmlText(dest.readText(Charsets.UTF_8))
            FarmFileKind.GEOJSON, FarmFileKind.JSON -> "JSON document stored for later mapping."
            FarmFileKind.CSV -> "CSV spreadsheet stored for later import mapping."
            FarmFileKind.IMAGE -> "Image stored on device."
            FarmFileKind.OTHER -> "File stored on device."
        }

        return ImportedFilePayload(
            displayName = displayName,
            kind = kind,
            mimeType = mime,
            storedPath = dest.absolutePath,
            byteSize = dest.length(),
            summary = summary
        )
    }

    fun deleteStored(path: String) {
        runCatching { File(path).takeIf { it.exists() }?.delete() }
    }

    fun classify(name: String, mime: String): FarmFileKind {
        val lower = name.lowercase()
        return when {
            lower.endsWith(".kmz") || mime.contains("kmz") -> FarmFileKind.KMZ
            lower.endsWith(".kml") || mime.contains("kml") -> FarmFileKind.KML
            lower.endsWith(".geojson") || lower.endsWith(".json") && lower.contains("geo") ->
                FarmFileKind.GEOJSON
            lower.endsWith(".json") || mime.contains("json") -> FarmFileKind.JSON
            lower.endsWith(".csv") || mime.contains("csv") -> FarmFileKind.CSV
            mime.startsWith("image/") ||
                lower.endsWith(".png") ||
                lower.endsWith(".jpg") ||
                lower.endsWith(".jpeg") ||
                lower.endsWith(".webp") -> FarmFileKind.IMAGE
            else -> FarmFileKind.OTHER
        }
    }

    private fun queryDisplayName(context: Context, uri: Uri): String? {
        context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (idx >= 0 && cursor.moveToFirst()) {
                return cursor.getString(idx)
            }
        }
        return uri.lastPathSegment?.substringAfterLast('/')
    }

    private fun summarizeKmz(file: File): String {
        return try {
            ZipInputStream(BufferedInputStream(file.inputStream())).use { zis ->
                var entry = zis.nextEntry
                while (entry != null) {
                    if (!entry.isDirectory && entry.name.lowercase().endsWith(".kml")) {
                        val text = zis.readBytes().toString(Charsets.UTF_8)
                        return summarizeKmlText(text)
                    }
                    entry = zis.nextEntry
                }
            }
            "KMZ stored (no KML entry found yet)."
        } catch (e: Exception) {
            "KMZ stored (${e.message ?: "unreadable zip"})."
        }
    }

    private fun summarizeKmlText(text: String): String {
        val placemarks = Regex("<Placemark\\b", RegexOption.IGNORE_CASE).findAll(text).count()
        val folders = Regex("<Folder\\b", RegexOption.IGNORE_CASE).findAll(text).count()
        val name = Regex("<name>(.*?)</name>", setOf(RegexOption.IGNORE_CASE, RegexOption.DOT_MATCHES_ALL))
            .find(text)
            ?.groupValues
            ?.getOrNull(1)
            ?.replace(Regex("\\s+"), " ")
            ?.trim()
            ?.take(80)
        val title = name?.takeIf { it.isNotBlank() }?.let { "“$it” · " } ?: ""
        return "${title}$placemarks placemark(s), $folders folder(s)"
    }
}
