package com.nativepure.mail

import com.icegreen.greenmail.configuration.GreenMailConfiguration
import com.icegreen.greenmail.util.GreenMail
import com.icegreen.greenmail.util.ServerSetup
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpStatusCode
import io.ktor.serialization.kotlinx.json.json
import io.ktor.server.application.call
import io.ktor.server.application.install
import io.ktor.server.engine.embeddedServer
import io.ktor.server.netty.Netty
import io.ktor.server.plugins.contentnegotiation.ContentNegotiation
import io.ktor.server.plugins.cors.routing.CORS
import io.ktor.server.plugins.statuspages.StatusPages
import io.ktor.server.request.receive
import io.ktor.server.response.respond
import io.ktor.server.response.respondText
import io.ktor.server.routing.get
import io.ktor.server.routing.post
import io.ktor.server.routing.routing
import jakarta.mail.Message
import jakarta.mail.Session
import jakarta.mail.Transport
import jakarta.mail.internet.InternetAddress
import jakarta.mail.internet.MimeMessage
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import org.slf4j.LoggerFactory
import java.time.Instant
import java.util.Properties
import java.util.UUID
import java.util.concurrent.CopyOnWriteArrayList

private val log = LoggerFactory.getLogger("NativePureMail")

fun main() {
    val config = MailConfig.fromEnv()
    val store = MessageStore()
    val embeddedSmtp = if (config.embedSmtp) {
        startEmbeddedSmtp(config).also {
            log.info("Embedded SMTP listening on {}:{}", config.smtpHost, config.smtpPort)
        }
    } else {
        null
    }

    Runtime.getRuntime().addShutdownHook(
        Thread {
            embeddedSmtp?.stop()
        }
    )

    log.info(
        "Native Pure Mail Server starting on :{} (SMTP {}:{}, from={})",
        config.httpPort,
        config.smtpHost,
        config.smtpPort,
        config.fromAddress
    )

    embeddedServer(Netty, port = config.httpPort, host = "0.0.0.0") {
        install(ContentNegotiation) {
            json(
                Json {
                    prettyPrint = true
                    ignoreUnknownKeys = true
                }
            )
        }
        install(CORS) {
            anyHost()
            allowHeader(HttpHeaders.ContentType)
        }
        install(StatusPages) {
            exception<Throwable> { call, cause ->
                log.error("Request failed", cause)
                call.respond(
                    HttpStatusCode.InternalServerError,
                    ApiError(cause.message ?: "Internal error")
                )
            }
        }
        routing {
            get("/health") {
                call.respond(
                    HealthResponse(
                        ok = true,
                        service = "nativepure-mail",
                        smtpHost = config.smtpHost,
                        smtpPort = config.smtpPort,
                        from = config.fromAddress,
                        embeddedSmtp = config.embedSmtp,
                        messagesCaptured = store.size()
                    )
                )
            }

            get("/") {
                call.respondText(inboxHtml(store, config), ContentType.Text.Html)
            }

            get("/v1/messages") {
                call.respond(store.snapshot())
            }

            get("/v1/messages/{id}") {
                val id = call.parameters["id"].orEmpty()
                val found = store.get(id)
                if (found == null) {
                    call.respond(HttpStatusCode.NotFound, ApiError("Message not found"))
                } else {
                    call.respond(found)
                }
            }

            post("/v1/mail/send") {
                val req = call.receive<SendMailRequest>()
                val result = sendMail(config, store, req)
                if (result.ok) {
                    call.respond(HttpStatusCode.OK, result)
                } else {
                    call.respond(HttpStatusCode.BadGateway, result)
                }
            }

            post("/v1/mail/verification") {
                val req = call.receive<VerificationRequest>()
                val code = req.code.trim()
                if (req.to.isBlank() || "@" !in req.to) {
                    call.respond(HttpStatusCode.BadRequest, ApiError("Valid 'to' email is required."))
                    return@post
                }
                if (code.length !in 4..8 || code.any { !it.isDigit() }) {
                    call.respond(HttpStatusCode.BadRequest, ApiError("Valid numeric 'code' is required."))
                    return@post
                }
                val minutes = req.expiresInMinutes.coerceIn(5, 60)
                val subject = "Native Pure verification code"
                val text = buildString {
                    appendLine("Your Native Pure verification code is: $code")
                    appendLine()
                    appendLine("This code expires in $minutes minutes.")
                    appendLine("If you did not request this, you can ignore this email.")
                    appendLine()
                    appendLine("— Native Pure")
                }
                val html = """
                    <div style="font-family:Georgia,serif;max-width:480px;margin:0 auto;padding:24px;background:#f4f7f2;color:#1f2a24">
                      <h1 style="color:#2f5d3a;margin:0 0 12px">Native Pure</h1>
                      <p>Your verification code:</p>
                      <p style="font-size:32px;letter-spacing:6px;font-weight:700;color:#2f5d3a;margin:16px 0">$code</p>
                      <p style="color:#5b6b62">Expires in $minutes minutes. If you did not request this, ignore this email.</p>
                    </div>
                """.trimIndent()
                val result = sendMail(
                    config,
                    store,
                    SendMailRequest(
                        to = req.to.trim().lowercase(),
                        subject = subject,
                        text = text,
                        html = html
                    )
                )
                if (result.ok) {
                    call.respond(HttpStatusCode.OK, result)
                } else {
                    call.respond(HttpStatusCode.BadGateway, result)
                }
            }
        }
    }.start(wait = true)
}

data class MailConfig(
    val httpPort: Int,
    val smtpHost: String,
    val smtpPort: Int,
    val smtpUser: String,
    val smtpPass: String,
    val fromAddress: String,
    val fromName: String,
    val embedSmtp: Boolean
) {
    companion object {
        fun fromEnv(): MailConfig {
            val rawEmbed = System.getenv("MAIL_EMBED_SMTP")
            val embed = when {
                rawEmbed == null -> true
                rawEmbed.equals("false", true) || rawEmbed == "0" -> false
                else -> true
            }
            return MailConfig(
                httpPort = System.getenv("MAIL_HTTP_PORT")?.toIntOrNull() ?: 8787,
                smtpHost = System.getenv("SMTP_HOST") ?: "127.0.0.1",
                smtpPort = System.getenv("SMTP_PORT")?.toIntOrNull() ?: 1025,
                smtpUser = System.getenv("SMTP_USER").orEmpty(),
                smtpPass = System.getenv("SMTP_PASS").orEmpty(),
                fromAddress = System.getenv("SMTP_FROM") ?: "noreply@nativepure.local",
                fromName = System.getenv("SMTP_FROM_NAME") ?: "Native Pure",
                embedSmtp = embed
            )
        }
    }
}

@Serializable
data class HealthResponse(
    val ok: Boolean,
    val service: String,
    val smtpHost: String,
    val smtpPort: Int,
    val from: String,
    val embeddedSmtp: Boolean,
    val messagesCaptured: Int
)

@Serializable
data class ApiError(val error: String)

@Serializable
data class SendMailRequest(
    val to: String,
    val subject: String,
    val text: String,
    val html: String? = null
)

@Serializable
data class VerificationRequest(
    val to: String,
    val code: String,
    val expiresInMinutes: Int = 15
)

@Serializable
data class SendMailResponse(
    val ok: Boolean,
    val messageId: String? = null,
    val error: String? = null
)

@Serializable
data class StoredMessage(
    val id: String,
    val to: String,
    val from: String,
    val subject: String,
    val text: String,
    val html: String?,
    val createdAt: String
)

class MessageStore {
    private val items = CopyOnWriteArrayList<StoredMessage>()

    fun add(message: StoredMessage) {
        items.add(0, message)
        if (items.size > 200) {
            while (items.size > 200) items.removeAt(items.lastIndex)
        }
    }

    fun snapshot(): List<StoredMessage> = items.toList()

    fun get(id: String): StoredMessage? = items.find { it.id == id }

    fun size(): Int = items.size
}

fun startEmbeddedSmtp(config: MailConfig): GreenMail {
    val setup = ServerSetup(config.smtpPort, config.smtpHost, ServerSetup.PROTOCOL_SMTP)
    val green = GreenMail(setup)
    if (config.smtpUser.isNotBlank()) {
        green.withConfiguration(
            GreenMailConfiguration.aConfig().withUser(config.smtpUser, config.smtpPass)
        )
    }
    green.start()
    return green
}

fun sendMail(config: MailConfig, store: MessageStore, req: SendMailRequest): SendMailResponse {
    return try {
        val props = Properties().apply {
            put("mail.smtp.host", config.smtpHost)
            put("mail.smtp.port", config.smtpPort.toString())
            put("mail.smtp.auth", (config.smtpUser.isNotBlank()).toString())
            put("mail.smtp.starttls.enable", "false")
            put("mail.smtp.connectiontimeout", "8000")
            put("mail.smtp.timeout", "8000")
        }
        val session = Session.getInstance(props)
        val message = MimeMessage(session).apply {
            setFrom(InternetAddress(config.fromAddress, config.fromName))
            setRecipients(Message.RecipientType.TO, InternetAddress.parse(req.to))
            subject = req.subject
            if (!req.html.isNullOrBlank()) {
                setContent(req.html, "text/html; charset=utf-8")
            } else {
                setText(req.text, "utf-8")
            }
        }
        if (config.smtpUser.isNotBlank()) {
            Transport.send(message, config.smtpUser, config.smtpPass)
        } else {
            Transport.send(message)
        }
        val id = UUID.randomUUID().toString().take(12)
        store.add(
            StoredMessage(
                id = id,
                to = req.to,
                from = config.fromAddress,
                subject = req.subject,
                text = req.text,
                html = req.html,
                createdAt = Instant.now().toString()
            )
        )
        log.info("Sent mail to {} subject={}", req.to, req.subject)
        SendMailResponse(ok = true, messageId = id)
    } catch (t: Throwable) {
        log.error("Failed to send mail to {}", req.to, t)
        SendMailResponse(ok = false, error = t.message ?: "SMTP send failed")
    }
}

fun inboxHtml(store: MessageStore, config: MailConfig): String {
    val rows = store.snapshot().joinToString("") { msg ->
        """
        <tr>
          <td>${msg.createdAt}</td>
          <td>${msg.to}</td>
          <td>${msg.subject}</td>
          <td><pre style="white-space:pre-wrap;margin:0">${msg.text.replace("<", "&lt;")}</pre></td>
        </tr>
        """.trimIndent()
    }
    return """
        <!doctype html>
        <html>
        <head>
          <meta charset="utf-8"/>
          <title>Native Pure Mail</title>
          <style>
            body{font-family:Georgia,serif;background:#f4f7f2;color:#1f2a24;margin:0;padding:24px}
            h1{color:#2f5d3a}
            .meta{color:#5b6b62;margin-bottom:16px}
            table{width:100%;border-collapse:collapse;background:#fff;border-radius:12px;overflow:hidden}
            th,td{border-bottom:1px solid #dce5dd;padding:10px 12px;text-align:left;vertical-align:top}
            th{background:#e7efe8}
            a{color:#2f5d3a}
          </style>
        </head>
        <body>
          <h1>Native Pure Mail Server</h1>
          <p class="meta">
            HTTP :${config.httpPort} · SMTP ${config.smtpHost}:${config.smtpPort} ·
            from ${config.fromAddress} ·
            embedded SMTP: ${config.embedSmtp}<br/>
            API: <code>POST /v1/mail/verification</code> · <a href="/v1/messages">JSON inbox</a> ·
            <a href="/health">health</a>
          </p>
          <table>
            <thead><tr><th>When</th><th>To</th><th>Subject</th><th>Body</th></tr></thead>
            <tbody>$rows</tbody>
          </table>
        </body>
        </html>
    """.trimIndent()
}
