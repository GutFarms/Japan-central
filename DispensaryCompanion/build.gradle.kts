import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {
    kotlin("jvm") version "1.9.24"
    id("org.jetbrains.compose") version "1.6.11"
    kotlin("plugin.serialization") version "1.9.24"
}

repositories {
    google()
    mavenCentral()
    maven("https://maven.pkg.jetbrains.space/public/p/compose/dev")
}

dependencies {
    implementation(compose.desktop.currentOs)
    implementation(compose.material3)
    implementation(compose.materialIconsExtended)
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-swing:1.8.1")
    testImplementation("junit:junit:4.13.2")
}

kotlin {
    jvmToolchain(21)
}

compose.desktop {
    application {
        mainClass = "com.nativepure.companion.MainKt"
        nativeDistributions {
            targetFormats(TargetFormat.Exe, TargetFormat.Msi, TargetFormat.Deb)
            packageName = "NativePureCompanion"
            packageVersion = "1.14.0"
            description = "Native Pure dispensary point of sale"
            copyright = "© Native Pure"
            vendor = "Native Pure"
            // App + Start Menu / taskbar icon (from logo_main.png)
            windows {
                iconFile.set(project.file("icon.ico"))
                menu = true
                menuGroup = "Native Pure"
                shortcut = true
                dirChooser = true
                perUserInstall = false
                upgradeUuid = "A7C3E91F-4B2D-4E8A-9F1C-6D5B8A0E2C44"
                console = false
            }
            linux {
                iconFile.set(project.file("icon.png"))
            }
        }
    }
}
