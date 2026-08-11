package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.data.model.EmailCodeIssue
import com.solstice.dispensary.ui.components.BotanicalScreenBackground
import com.solstice.dispensary.ui.components.BrandLogo

@Composable
fun EmailVerificationScreen(
    email: String,
    issuedCode: EmailCodeIssue?,
    busy: Boolean,
    error: String?,
    onVerify: (code: String) -> Unit,
    onResend: () -> Unit,
    onClearError: () -> Unit,
    onLogout: () -> Unit
) {
    var code by remember { mutableStateOf("") }
    val focusRequester = remember { FocusRequester() }
    val keyboard = LocalSoftwareKeyboardController.current
    val colors = MaterialTheme.colorScheme
    val scroll = rememberScrollState()

    LaunchedEffect(Unit) {
        focusRequester.requestFocus()
    }

    BotanicalScreenBackground {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .statusBarsPadding()
                .navigationBarsPadding()
                .imePadding()
                .verticalScroll(scroll)
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Spacer(Modifier.height(12.dp))
            BrandLogo(size = 88.dp)
            Spacer(Modifier.height(14.dp))
            Text(
                text = "Verify your email",
                style = MaterialTheme.typography.displayMedium,
                color = colors.onBackground
            )
            Text(
                text = "Enter the 6-digit code sent to\n$email",
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 8.dp, bottom = 18.dp)
            )

            if (issuedCode != null) {
                Surface(
                    color = colors.secondaryContainer,
                    shape = RoundedCornerShape(14.dp),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Column(modifier = Modifier.padding(14.dp)) {
                        Text(
                            text = "Offline delivery",
                            style = MaterialTheme.typography.titleSmall,
                            color = colors.onSecondaryContainer
                        )
                        Text(
                            text = "This build has no mail server, so your code is shown here:",
                            style = MaterialTheme.typography.bodyMedium,
                            color = colors.onSecondaryContainer,
                            modifier = Modifier.padding(top = 4.dp)
                        )
                        Text(
                            text = issuedCode.code,
                            style = MaterialTheme.typography.headlineLarge.copy(
                                fontFamily = FontFamily.Monospace
                            ),
                            color = colors.onSecondaryContainer,
                            modifier = Modifier.padding(top = 8.dp)
                        )
                    }
                }
                Spacer(Modifier.height(16.dp))
            }

            OutlinedTextField(
                value = code,
                onValueChange = { raw ->
                    val digits = raw.filter { it.isDigit() }.take(6)
                    code = digits
                    onClearError()
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .focusRequester(focusRequester),
                singleLine = true,
                label = { Text("6-digit code") },
                keyboardOptions = KeyboardOptions(
                    keyboardType = KeyboardType.NumberPassword,
                    imeAction = ImeAction.Done
                ),
                keyboardActions = KeyboardActions(
                    onDone = {
                        if (code.length == 6 && !busy) {
                            keyboard?.hide()
                            onVerify(code)
                        }
                    }
                )
            )

            if (error != null) {
                Spacer(Modifier.height(10.dp))
                Text(
                    text = error,
                    color = colors.error,
                    style = MaterialTheme.typography.bodyMedium,
                    textAlign = TextAlign.Center
                )
            }

            Spacer(Modifier.height(18.dp))
            Button(
                onClick = {
                    keyboard?.hide()
                    onVerify(code)
                },
                enabled = !busy && code.length == 6,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(14.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = colors.secondary,
                    contentColor = colors.onSecondary
                )
            ) {
                if (busy) {
                    CircularProgressIndicator(
                        modifier = Modifier.height(22.dp),
                        color = colors.onSecondary,
                        strokeWidth = 2.dp
                    )
                } else {
                    Text(
                        text = "Verify email",
                        modifier = Modifier.padding(vertical = 4.dp),
                        style = MaterialTheme.typography.titleMedium
                    )
                }
            }

            TextButton(
                onClick = onResend,
                enabled = !busy
            ) {
                Text("Resend code", color = colors.secondary)
            }
            TextButton(onClick = onLogout) {
                Text("Log out", color = colors.onSurfaceVariant)
            }
            Spacer(Modifier.height(32.dp))
        }
    }
}
