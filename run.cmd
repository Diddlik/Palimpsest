@echo off
REM Build and run the Palimpsest desktop app (tray + settings UI).
REM Usage:  run.cmd [--release]
REM   (no args)   build and run a debug build
REM   --release   build and run an optimized release build

setlocal
cd /d "%~dp0"

set "PROFILE=%~1"
if /i "%PROFILE%"=="--release" (
    echo Building Palimpsest ^(release^)...
    cargo run --release -p palimpsest-app
) else (
    echo Building Palimpsest ^(debug^)...
    cargo run -p palimpsest-app
)

if errorlevel 1 (
    echo.
    echo Build or run failed ^(exit code %errorlevel%^).
    exit /b %errorlevel%
)

endlocal
