@echo off
set ROOT=%~dp0
set JAR=%ROOT%dist\NativePure-Companion.jar
if not exist "%JAR%" (
  echo Missing %JAR% — build with: gradlew.bat packageUberJarForCurrentOS
  exit /b 1
)
java -jar "%JAR%" %*
