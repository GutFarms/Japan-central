package com.solstice.dispensary.data.mail

import android.util.Log
import com.solstice.dispensary.BuildConfig
import org.json.JSONObject
import java.io.BufferedReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.nio.charset.StandardCharsets
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

data class MailSendResult(
    val ok: Boolean,
    val messageId: String? = null,
    val error: String? = null
)

object MailApiClient {
    private const val TAG = "MailApiClient"

    suspend fun sendVerificationCode(
        email: String,
        code: String,
        expiresInMinutes: Int = 15
    ): MailSendResult = withContext(Dispatchers.IO) {
        val base = BuildConfig.MAIL_API_BASE_URL.trim().trimEnd('/')
        if (base.isBlank()) {
            return@withContext MailSendResult(ok = false, error = "Mail API not configured")
        }
        try {
            val url = URL("$base/v1/mail/verification")
            val body = JSONObject()
                .put("to", email)
                .put("code", code)
                .put("expiresInMinutes", expiresInMinutes)
                .toString()
            val conn = (url.openConnection() as HttpURLConnection).apply {
                requestMethod = "POST"
                connectTimeout = 8_000
                readTimeout = 8_000
                doOutput = true
                setRequestProperty("Content-Type", "application/json; charset=utf-8")
                setRequestProperty("Accept", "application/json")
            }
            OutputStreamWriter(conn.outputStream, StandardCharsets.UTF_8).use { it.write(body) }
            val codeHttp = conn.responseCode
            val stream = if (codeHttp in 200..299) conn.inputStream else conn.errorStream
            val text = stream?.bufferedReader()?.use(BufferedReader::readText).orEmpty()
            conn.disconnect()
            val json = runCatching { JSONObject(text) }.getOrNull()
            if (codeHttp in 200..299 && json?.optBoolean("ok") == true) {
                MailSendResult(ok = true, messageId = json.optString("messageId").ifBlank { null })
            } else {
                MailSendResult(
                    ok = false,
                    error = json?.optString("error")?.ifBlank { null }
                        ?: json?.optString("message")?.ifBlank { null }
                        ?: "Mail server HTTP $codeHttp"
                )
            }
        } catch (t: Throwable) {
            Log.w(TAG, "Mail send failed: ${t.message}")
            MailSendResult(ok = false, error = t.message ?: "Mail unreachable")
        }
    }
}
