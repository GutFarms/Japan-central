package com.solstice.dispensary.scan

import android.graphics.Bitmap
import com.google.mlkit.vision.barcode.BarcodeScanning
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import com.solstice.dispensary.data.model.LabelScanResult
import com.solstice.dispensary.data.model.Product
import com.solstice.dispensary.data.model.ProductCategory
import com.solstice.dispensary.data.model.StrainType
import kotlinx.coroutines.tasks.await

/**
 * On-device AI label scanner using ML Kit OCR + barcode recognition,
 * then heuristic matching against the local inventory catalog.
 */
class LabelAiScanner {

    private val textRecognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
    private val barcodeScanner = BarcodeScanning.getClient()

    suspend fun analyze(bitmap: Bitmap, catalog: List<Product>): LabelScanResult {
        val image = InputImage.fromBitmap(bitmap, 0)
        val textTask = textRecognizer.process(image)
        val barcodeTask = barcodeScanner.process(image)

        val visionText = textTask.await()
        val barcodes = barcodeTask.await()

        val rawText = visionText.text.trim()
        val lines = visionText.textBlocks
            .flatMap { it.lines }
            .map { it.text.trim() }
            .filter { it.isNotBlank() }

        val barcode = barcodes
            .mapNotNull { it.rawValue?.trim() }
            .firstOrNull { it.isNotBlank() }

        val parsed = LabelParser.parse(rawText, lines)
        val match = InventoryMatcher.match(catalog, barcode, parsed)

        return LabelScanResult(
            rawText = rawText.ifBlank { barcode.orEmpty() },
            barcode = barcode,
            detectedName = parsed.name,
            detectedBrand = parsed.brand,
            detectedCategory = parsed.category,
            detectedStrain = parsed.strain,
            detectedThc = parsed.thc,
            detectedCbd = parsed.cbd,
            matchedProduct = match.product,
            matchConfidence = match.confidence,
            suggestedQuantity = parsed.quantity ?: 1,
            isNewProduct = match.product == null
        )
    }

    fun close() {
        textRecognizer.close()
        barcodeScanner.close()
    }
}

data class ParsedLabel(
    val name: String?,
    val brand: String?,
    val category: ProductCategory?,
    val strain: StrainType?,
    val thc: Double?,
    val cbd: Double?,
    val quantity: Int?,
    val skuHint: String?
)

object LabelParser {
    private val thcRegex = Regex("""(?i)\bTHC\b[^0-9]{0,8}(\d{1,2}(?:\.\d{1,2})?)\s*%?""")
    private val cbdRegex = Regex("""(?i)\bCBD\b[^0-9]{0,8}(\d{1,2}(?:\.\d{1,2})?)\s*%?""")
    private val qtyRegex = Regex("""(?i)\b(?:qty|quantity|count|pack|units?)[:\s]*(\d{1,3})\b""")
    private val skuRegex = Regex("""(?i)\b(?:sku|lot|batch)[:\s#-]*([A-Z0-9-]{4,})\b""")

    private val brandHints = listOf(
        "Solstice Farms", "North Reach", "Hearth Kitchen", "Lumen Lab", "Solstice Gear", "Solstice"
    )

    fun parse(rawText: String, lines: List<String>): ParsedLabel {
        val joined = rawText.ifBlank { lines.joinToString("\n") }
        val thc = thcRegex.find(joined)?.groupValues?.getOrNull(1)?.toDoubleOrNull()
        val cbd = cbdRegex.find(joined)?.groupValues?.getOrNull(1)?.toDoubleOrNull()
        val quantity = qtyRegex.find(joined)?.groupValues?.getOrNull(1)?.toIntOrNull()
        val skuHint = skuRegex.find(joined)?.groupValues?.getOrNull(1)?.uppercase()

        val category = detectCategory(joined)
        val strain = detectStrain(joined)
        val brand = brandHints.firstOrNull { joined.contains(it, ignoreCase = true) }
            ?: lines.firstOrNull { line ->
                brandHints.any { hint -> line.contains(hint, ignoreCase = true) }
            }

        val name = lines
            .asSequence()
            .map { it.trim() }
            .filter { it.length in 3..48 }
            .filterNot { it.contains('%') }
            .filterNot { it.contains("THC", ignoreCase = true) && it.length < 12 }
            .filterNot { it.contains("CBD", ignoreCase = true) && it.length < 12 }
            .filterNot { skuRegex.containsMatchIn(it) }
            .filterNot { brandHints.any { hint -> it.equals(hint, ignoreCase = true) } }
            .firstOrNull()

        return ParsedLabel(
            name = name,
            brand = brand,
            category = category,
            strain = strain,
            thc = thc,
            cbd = cbd,
            quantity = quantity,
            skuHint = skuHint
        )
    }

    private fun detectCategory(text: String): ProductCategory? {
        val t = text.lowercase()
        return when {
            listOf("pre-roll", "preroll", "joint").any { it in t } -> ProductCategory.PREROLL
            listOf("gummy", "edible", "chocolate", "softgel").any { it in t } -> ProductCategory.EDIBLE
            listOf("rosin", "shatter", "wax", "concentrate", "live resin").any { it in t } -> ProductCategory.CONCENTRATE
            listOf("vape", "cart", "cartridge", "disposable").any { it in t } -> ProductCategory.VAPE
            listOf("balm", "lotion", "topical", "cream").any { it in t } -> ProductCategory.TOPICAL
            listOf("grinder", "tray", "papers", "accessory").any { it in t } -> ProductCategory.ACCESSORY
            listOf("flower", "bud", "eighth", "3.5g", "cannabis flower").any { it in t } -> ProductCategory.FLOWER
            else -> null
        }
    }

    private fun detectStrain(text: String): StrainType? {
        val t = text.lowercase()
        return when {
            "indica" in t -> StrainType.INDICA
            "sativa" in t -> StrainType.SATIVA
            "hybrid" in t -> StrainType.HYBRID
            Regex("""\bcbd\b""").containsMatchIn(t) && "thc" !in t -> StrainType.CBD
            else -> null
        }
    }
}

data class MatchResult(val product: Product?, val confidence: Float)

object InventoryMatcher {
    fun match(catalog: List<Product>, barcode: String?, parsed: ParsedLabel): MatchResult {
        if (!barcode.isNullOrBlank()) {
            val bySku = catalog.firstOrNull {
                it.sku.equals(barcode, ignoreCase = true) ||
                    it.id.equals(barcode, ignoreCase = true)
            }
            if (bySku != null) return MatchResult(bySku, 0.98f)
        }

        parsed.skuHint?.let { hint ->
            val byHint = catalog.firstOrNull { it.sku.equals(hint, ignoreCase = true) }
            if (byHint != null) return MatchResult(byHint, 0.94f)
        }

        var best: Product? = null
        var bestScore = 0f

        for (product in catalog) {
            var score = 0f
            val name = parsed.name
            if (!name.isNullOrBlank()) {
                score += nameSimilarity(name, product.name) * 0.55f
                score += nameSimilarity(name, product.brand) * 0.1f
            }
            parsed.brand?.let { brand ->
                if (product.brand.contains(brand, ignoreCase = true) ||
                    brand.contains(product.brand, ignoreCase = true)
                ) {
                    score += 0.2f
                }
            }
            parsed.category?.let { if (it == product.category) score += 0.1f }
            parsed.strain?.let { if (it == product.strainType) score += 0.05f }
            parsed.thc?.let { detected ->
                if (product.thcPercent > 0) {
                    val delta = kotlin.math.abs(product.thcPercent - detected)
                    if (delta <= 2.0) score += 0.1f
                    else if (delta <= 5.0) score += 0.04f
                }
            }

            if (score > bestScore) {
                bestScore = score
                best = product
            }
        }

        return if (best != null && bestScore >= 0.35f) {
            MatchResult(best, bestScore.coerceAtMost(0.97f))
        } else {
            MatchResult(null, bestScore)
        }
    }

    private fun nameSimilarity(a: String, b: String): Float {
        val left = a.lowercase().trim()
        val right = b.lowercase().trim()
        if (left == right) return 1f
        if (left in right || right in left) return 0.85f

        val leftTokens = left.split(Regex("""\W+""")).filter { it.length > 2 }.toSet()
        val rightTokens = right.split(Regex("""\W+""")).filter { it.length > 2 }.toSet()
        if (leftTokens.isEmpty() || rightTokens.isEmpty()) return 0f
        val overlap = leftTokens.intersect(rightTokens).size.toFloat()
        val union = leftTokens.union(rightTokens).size.toFloat()
        return overlap / union
    }
}

object NewProductFactory {
    fun fromScan(result: LabelScanResult): Product {
        val name = result.detectedName?.takeIf { it.isNotBlank() } ?: "Scanned Product"
        val brand = result.detectedBrand?.takeIf { it.isNotBlank() } ?: "Unknown Brand"
        val category = result.detectedCategory ?: ProductCategory.FLOWER
        val id = "scan-" + System.currentTimeMillis().toString(36)
        val sku = result.barcode?.takeIf { it.length in 4..32 }
            ?: "SCAN-${id.takeLast(6).uppercase()}"
        return Product(
            id = id,
            name = name,
            brand = brand,
            category = category,
            strainType = result.detectedStrain ?: StrainType.NONE,
            thcPercent = result.detectedThc ?: 0.0,
            cbdPercent = result.detectedCbd ?: 0.0,
            price = 0.0,
            unitLabel = "unit",
            description = "Added via AI camera scan.\n\nLabel text:\n${result.rawText.take(400)}",
            effects = "—",
            featured = false,
            inStock = true,
            stockQuantity = 0,
            sku = sku,
            published = false
        )
    }
}
