package com.nativepure.companion.data

import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption

/**
 * Local hard-drive location for Native Pure Companion data.
 *
 * Windows: `%LOCALAPPDATA%\NativePure\Companion\`
 * macOS/Linux: `~/.nativepure-companion/`
 *
 * Migrates automatically from the legacy home-folder path when present.
 */
object LocalDataStore {
    const val STORE_FILE_NAME = "store.json"
    const val INVENTORY_BACKUP = "inventory.json"
    const val CUSTOMERS_BACKUP = "customers.json"
    const val LEGACY_DIR_NAME = ".nativepure-companion"

    fun dataDirectory(): File {
        val preferred = preferredDirectory()
        preferred.mkdirs()
        migrateFromLegacyIfNeeded(preferred)
        return preferred
    }

    fun storeFile(): File = File(dataDirectory(), STORE_FILE_NAME)

    fun inventoryBackupFile(): File = File(dataDirectory(), INVENTORY_BACKUP)

    fun customersBackupFile(): File = File(dataDirectory(), CUSTOMERS_BACKUP)

    /** Human-readable absolute path for UI / docs. */
    fun dataDirectoryPath(): String = dataDirectory().absolutePath

    fun openInFileManager(): Boolean {
        return try {
            val dir = dataDirectory()
            if (DesktopHelper.open(dir)) return true
            false
        } catch (_: Throwable) {
            false
        }
    }

    private fun preferredDirectory(): File {
        val localAppData = System.getenv("LOCALAPPDATA")?.takeIf { it.isNotBlank() }
        return if (localAppData != null) {
            // Windows — visible under the user's Local AppData hard-drive folder
            File(localAppData, "NativePure${File.separator}Companion")
        } else {
            File(System.getProperty("user.home"), LEGACY_DIR_NAME)
        }
    }

    private fun legacyDirectory(): File =
        File(System.getProperty("user.home"), LEGACY_DIR_NAME)

    private fun migrateFromLegacyIfNeeded(preferred: File) {
        val legacy = legacyDirectory()
        if (legacy.absolutePath == preferred.absolutePath) return
        if (!legacy.isDirectory) return
        val legacyStore = File(legacy, STORE_FILE_NAME)
        val preferredStore = File(preferred, STORE_FILE_NAME)
        if (legacyStore.isFile && !preferredStore.exists()) {
            runCatching {
                preferred.mkdirs()
                Files.copy(
                    legacyStore.toPath(),
                    preferredStore.toPath(),
                    StandardCopyOption.REPLACE_EXISTING
                )
                // Copy any sibling json backups too
                legacy.listFiles()?.filter { it.isFile && it.name.endsWith(".json") }?.forEach { file ->
                    if (file.name == STORE_FILE_NAME) return@forEach
                    Files.copy(
                        file.toPath(),
                        File(preferred, file.name).toPath(),
                        StandardCopyOption.REPLACE_EXISTING
                    )
                }
            }
        }
    }

    /**
     * Atomic write: write to `.tmp` then move over the target so a crash cannot
     * leave a half-written store on disk.
     */
    fun writeAtomic(target: File, text: String) {
        target.parentFile?.mkdirs()
        val tmp = File(target.parentFile, "${target.name}.tmp")
        tmp.writeText(text)
        try {
            Files.move(
                tmp.toPath(),
                target.toPath(),
                StandardCopyOption.REPLACE_EXISTING,
                StandardCopyOption.ATOMIC_MOVE
            )
        } catch (_: java.nio.file.AtomicMoveNotSupportedException) {
            Files.move(
                tmp.toPath(),
                target.toPath(),
                StandardCopyOption.REPLACE_EXISTING
            )
        } catch (_: Throwable) {
            // Last resort for odd filesystems
            target.writeText(text)
            tmp.delete()
        }
    }
}

/** Best-effort open of a folder in the OS file manager. */
internal object DesktopHelper {
    fun open(dir: File): Boolean {
        return try {
            if (java.awt.Desktop.isDesktopSupported() &&
                java.awt.Desktop.getDesktop().isSupported(java.awt.Desktop.Action.OPEN)
            ) {
                java.awt.Desktop.getDesktop().open(dir)
                true
            } else {
                val os = System.getProperty("os.name").orEmpty().lowercase()
                val pb = when {
                    os.contains("win") -> ProcessBuilder("explorer.exe", dir.absolutePath)
                    os.contains("mac") -> ProcessBuilder("open", dir.absolutePath)
                    else -> ProcessBuilder("xdg-open", dir.absolutePath)
                }
                pb.start()
                true
            }
        } catch (_: Throwable) {
            false
        }
    }
}
