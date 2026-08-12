package com.gutfarms.manager.data.importing

import com.gutfarms.manager.data.model.ApiFeedSource
import com.gutfarms.manager.data.model.ApiHttpMethod
import java.io.BufferedReader
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.nio.charset.StandardCharsets

data class ApiPullResult(
    val ok: Boolean,
    val statusLine: String,
    val preview: String
)

object ApiPullClient {
    private const val CONNECT_MS = 12_000
    private const val READ_MS = 20_000
    private const val MAX_PREVIEW = 4_000

    fun pull(source: ApiFeedSource, body: String = ""): ApiPullResult {
        val urlText = source.baseUrl.trim()
        require(urlText.startsWith("http://") || urlText.startsWith("https://")) {
            "URL must start with http:// or https://"
        }
        val conn = (URL(urlText).openConnection() as HttpURLConnection).apply {
            connectTimeout = CONNECT_MS
            readTimeout = READ_MS
            requestMethod = source.method.name
            instanceFollowRedirects = true
            setRequestProperty("Accept", "application/json, text/plain, */*")
            setRequestProperty("User-Agent", "GutFarms-FarmManager/1.3")
            val auth = source.authHeader.trim()
            if (auth.isNotEmpty()) {
                if (auth.contains(':')) {
                    val idx = auth.indexOf(':')
                    setRequestProperty(auth.substring(0, idx).trim(), auth.substring(idx + 1).trim())
                } else {
                    setRequestProperty("Authorization", auth)
                }
            }
            if (source.method == ApiHttpMethod.POST) {
                doOutput = true
                setRequestProperty("Content-Type", "application/json; charset=utf-8")
                OutputStreamWriter(outputStream, StandardCharsets.UTF_8).use { it.write(body) }
            }
        }

        return try {
            val code = conn.responseCode
            val stream = if (code in 200..299) conn.inputStream else conn.errorStream
            val text = stream?.let { readLimited(it) }.orEmpty()
            val ok = code in 200..299
            ApiPullResult(
                ok = ok,
                statusLine = if (ok) "HTTP $code OK" else "HTTP $code",
                preview = text.ifBlank { "(empty body)" }
            )
        } catch (e: Exception) {
            ApiPullResult(
                ok = false,
                statusLine = "Error: ${e.message ?: e.javaClass.simpleName}",
                preview = ""
            )
        } finally {
            conn.disconnect()
        }
    }

    private fun readLimited(stream: java.io.InputStream): String {
        BufferedReader(InputStreamReader(stream, StandardCharsets.UTF_8)).use { reader ->
            val sb = StringBuilder()
            var line: String?
            while (reader.readLine().also { line = it } != null) {
                if (sb.isNotEmpty()) sb.append('\n')
                sb.append(line)
                if (sb.length >= MAX_PREVIEW) {
                    sb.setLength(MAX_PREVIEW)
                    sb.append("\n…")
                    break
                }
            }
            return sb.toString()
        }
    }
}
