package com.solstice.dispensary.data.images

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import java.io.File
import java.io.FileOutputStream

/** Local product photos captured by staff (filesDir / product-images). */
object ProductImageStore {
    private const val DIR = "product-images"
    private const val MAX_EDGE = 1280
    private const val JPEG_QUALITY = 85

    fun relativePathFor(productId: String): String = "$DIR/$productId.jpg"

    fun resolve(context: Context, imagePath: String): File? {
        if (imagePath.isBlank()) return null
        val file = if (imagePath.startsWith("/")) {
            File(imagePath)
        } else {
            File(context.filesDir, imagePath)
        }
        return file.takeIf { it.exists() && it.length() > 0L }
    }

    fun loadBitmap(context: Context, imagePath: String, maxEdge: Int = MAX_EDGE): Bitmap? {
        val file = resolve(context, imagePath) ?: return null
        return decodeSampled(file, maxEdge)
    }

    /**
     * Saves a downscaled JPEG and returns the relative path to store on [Product.imagePath].
     */
    fun save(context: Context, productId: String, source: Bitmap): String {
        val dir = File(context.filesDir, DIR).also { it.mkdirs() }
        val relative = relativePathFor(productId)
        val outFile = File(context.filesDir, relative)
        val scaled = scaleDown(source, MAX_EDGE)
        FileOutputStream(outFile).use { stream ->
            scaled.compress(Bitmap.CompressFormat.JPEG, JPEG_QUALITY, stream)
        }
        if (scaled !== source && !scaled.isRecycled) {
            scaled.recycle()
        }
        // Remove stale absolute-path leftovers from older builds if any.
        File(dir, "$productId.jpg")
        return relative
    }

    fun delete(context: Context, imagePath: String) {
        resolve(context, imagePath)?.delete()
        // Also clear by id-style relative path if given a bare id.
        if (imagePath.isNotBlank() && !imagePath.contains('/')) {
            File(context.filesDir, relativePathFor(imagePath)).delete()
        }
    }

    fun deleteForProduct(context: Context, productId: String) {
        File(context.filesDir, relativePathFor(productId)).delete()
    }

    private fun scaleDown(bitmap: Bitmap, maxEdge: Int): Bitmap {
        val w = bitmap.width
        val h = bitmap.height
        val longest = maxOf(w, h)
        if (longest <= maxEdge) return bitmap
        val scale = maxEdge.toFloat() / longest
        val nw = (w * scale).toInt().coerceAtLeast(1)
        val nh = (h * scale).toInt().coerceAtLeast(1)
        return Bitmap.createScaledBitmap(bitmap, nw, nh, true)
    }

    private fun decodeSampled(file: File, maxEdge: Int): Bitmap? {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.absolutePath, bounds)
        val sample = run {
            var s = 1
            var w = bounds.outWidth
            var h = bounds.outHeight
            while (w / s > maxEdge || h / s > maxEdge) s *= 2
            s
        }
        val opts = BitmapFactory.Options().apply { inSampleSize = sample }
        return BitmapFactory.decodeFile(file.absolutePath, opts)
    }
}
