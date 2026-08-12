package com.solstice.dispensary.data.update

data class AppUpdateInfo(
    val versionCode: Int,
    val versionName: String,
    val apkUrl: String,
    val releaseNotes: String = ""
)

sealed class UpdateCheckResult {
    data class Available(val info: AppUpdateInfo) : UpdateCheckResult()
    data object UpToDate : UpdateCheckResult()
    data class Failed(val message: String) : UpdateCheckResult()
}

sealed class UpdateUiState {
    data object Idle : UpdateUiState()
    data object Checking : UpdateUiState()
    data class Available(val info: AppUpdateInfo) : UpdateUiState()
    data object UpToDate : UpdateUiState()
    data class Downloading(val progress: Float?) : UpdateUiState()
    data class ReadyToInstall(val info: AppUpdateInfo) : UpdateUiState()
    data class Error(val message: String) : UpdateUiState()
}
