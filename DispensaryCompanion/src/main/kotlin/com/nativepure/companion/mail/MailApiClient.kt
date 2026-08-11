package com.nativepure.companion.mail

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.net.HttpURLConnection
import java.net.URL
import java.nio.charset.StandardCharsets

data class MailSendResult(
    val ok: Boolean,
    val messageId: String? = null,
    val error: String? = null
)

object MailApiClient {
    private val json = Json { ignoreUnknownKeys = true }

    var baseUrl: String = System.getenv("NATIVEPURE_MAIL_API")
        ?.trim()
        ?.trimEnd('/')
        ?: "http://127.0.0.1:8787"

    fun sendVerificationCodeBlocking(
        email: String,
        code: String,
        expiresInMinutes: Int = 15
    ): MailSendResult = runBlocking {
        sendVerificationCode(email, code, expiresInMinutes)
    }

    suspend fun sendVerificationCode(
        email: String,
        code: String,
        expiresInMinutes: Int = 15
    ): MailSendResult = withContext(Dispatchers.IO) {
        val base = baseUrl.trim().trimEnd('/')
        if (base.isBlank()) {
            return@withContext MailSendResult(ok = false, error = "Mail API not configured")
        }
        try {
            val url = URL("$base/v1/mail/verification")
            val payload = json.encodeToString(
                VerificationBody.serializer(),
                VerificationBody(email, code, expiresInMinutes)
            )
            val conn = (url.openConnection() as HttpURLConnection).apply {
                requestMethod = "POST"
                connectTimeout = 8_000
                readTimeout = 8_000
                doOutput = true
                setRequestProperty("Content-Type", "application/json; charset=utf-8")
                setRequestProperty("Accept", "application/json")
            }
            conn.outputStream.use { it.write(payload.toByteArray(StandardCharsets.UTF_8)) }
            val codeHttp = conn.responseCode
            val text = (if (codeHttp in 200..299) conn.inputStream else conn.errorStream)
                ?.bufferedReader()
                ?.readText()
                .orEmpty()
            conn.disconnect()
            val parsed = runCatching { json.decodeFromString(SendResponse.serializer(), text) }.getOrNull()
            if (codeHttp in 200..299 && parsed?.ok == true) {
                MailSendResult(ok = true, messageId = parsed.messageId)
            } else {
                MailSendResult(
                    ok = false,
                    error = parsed?.error ?: "Mail server HTTP $codeHttp"
                )
            }
        } catch (t: Throwable) {
            MailSendResult(ok = false, error = t.message ?: "Mail unreachable")
        }
    }
}

@Serializable
private data class VerificationBody(
    val to: String,
    val code: String,
    val expiresInMinutes: Int = 15
)

@Serializable
private data class SendResponse(
    val ok: Boolean = false,
    val messageId: String? = null,
    val error: String? = null
)
