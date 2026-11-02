@echo off
rem Cargo resolves a runner program relative to its config, but not a script argument.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0cargo-ffmpeg-windows.ps1" %*
exit /b %ERRORLEVEL%
