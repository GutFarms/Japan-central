package com.solstice.dispensary.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.solstice.dispensary.ui.components.BrandLogo

@Composable
fun AuthScreen(
    busy: Boolean,
    error: String?,
    onLogin: (email: String, password: String) -> Unit,
    onRegister: (
        email: String,
        password: String,
        fullName: String,
        phone: String,
        dateOfBirth: String,
        marketingOptIn: Boolean
    ) -> Unit,
    onClearError: () -> Unit
) {
    var modeCreate by remember { mutableStateOf(false) }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var fullName by remember { mutableStateOf("") }
    var phone by remember { mutableStateOf("") }
    var dob by remember { mutableStateOf("") }
    var marketing by remember { mutableStateOf(true) }
    val colors = MaterialTheme.colorScheme

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    listOf(colors.background, colors.surfaceVariant, colors.background)
                )
            )
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Spacer(Modifier.height(12.dp))
            BrandLogo(size = 120.dp)
            Spacer(Modifier.height(16.dp))
            Text(
                text = if (modeCreate) "Create customer account" else "Welcome back",
                style = MaterialTheme.typography.headlineMedium,
                color = colors.onBackground
            )
            Text(
                text = if (modeCreate) {
                    "Join Native Pure to save pickup details and your order history."
                } else {
                    "Sign in with email, or admin username."
                },
                style = MaterialTheme.typography.bodyMedium,
                color = colors.onSurfaceVariant,
                textAlign = TextAlign.Center,
                modifier = Modifier.padding(top = 6.dp, bottom = 18.dp)
            )

            if (modeCreate) {
                AuthField(fullName, { fullName = it; onClearError() }, "Full name")
                Spacer(Modifier.height(10.dp))
            }
            AuthField(
                email,
                { email = it; onClearError() },
                if (modeCreate) "Email" else "Email or username",
                keyboard = KeyboardType.Email
            )
            Spacer(Modifier.height(10.dp))
            AuthField(
                password,
                { password = it; onClearError() },
                "Password",
                keyboard = KeyboardType.Password,
                password = true
            )
            if (modeCreate) {
                Spacer(Modifier.height(10.dp))
                AuthField(phone, { phone = it; onClearError() }, "Phone (optional)", KeyboardType.Phone)
                Spacer(Modifier.height(10.dp))
                AuthField(
                    dob,
                    { dob = it; onClearError() },
                    "Date of birth YYYY-MM-DD (optional)"
                )
                Spacer(Modifier.height(8.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = marketing, onCheckedChange = { marketing = it })
                    Text(
                        "Email me deals and menu drops",
                        color = colors.onBackground,
                        style = MaterialTheme.typography.bodyMedium
                    )
                }
            }

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
                    if (modeCreate) {
                        onRegister(email, password, fullName, phone, dob, marketing)
                    } else {
                        onLogin(email, password)
                    }
                },
                enabled = !busy,
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
                        text = if (modeCreate) "Create account" else "Log in",
                        modifier = Modifier.padding(vertical = 4.dp),
                        style = MaterialTheme.typography.titleMedium
                    )
                }
            }

            TextButton(
                onClick = {
                    modeCreate = !modeCreate
                    onClearError()
                }
            ) {
                Text(
                    text = if (modeCreate) {
                        "Already have an account? Log in"
                    } else {
                        "New here? Create an account"
                    },
                    color = colors.secondary
                )
            }

            if (!modeCreate) {
                Spacer(Modifier.height(8.dp))
                Text(
                    text = "Sign in with your email or username.\nNew passwords need 8+ characters with a letter and a number.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
            } else {
                Spacer(Modifier.height(8.dp))
                Text(
                    text = "Password: at least 8 characters, with a letter and a number.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = colors.onSurfaceVariant,
                    textAlign = TextAlign.Center
                )
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun AuthField(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    keyboard: KeyboardType = KeyboardType.Text,
    password: Boolean = false
) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text(label) },
        keyboardOptions = KeyboardOptions(keyboardType = keyboard),
        visualTransformation = if (password) PasswordVisualTransformation() else VisualTransformation.None
    )
}
