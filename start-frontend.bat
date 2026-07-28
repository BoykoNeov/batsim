@echo off
REM Double-click this to build (if needed) and start the batsim browser demo.
REM It serves web/ at http://127.0.0.1:8080/app/ and opens it in your browser.
REM Close this window (or press Ctrl+C) to stop the server.

cd /d "%~dp0"
title batsim frontend

echo === batsim frontend ===
echo.

REM --- 1. the wasm bundle (a build artifact, not committed) -------------------
if not exist "web\pkg\sim_wasm_bg.wasm" (
    echo The wasm bundle is missing. Building it with wasm-pack...
    where wasm-pack >nul 2>&1
    if errorlevel 1 (
        echo.
        echo   wasm-pack was not found on PATH, so the page cannot be built.
        echo   Install it ^(cargo install wasm-pack^) and then run:
        echo.
        echo     wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg
        echo.
        pause
        exit /b 1
    )
    call wasm-pack build crates/sim-wasm --target web --out-dir ../../web/pkg
    if errorlevel 1 (
        echo.
        echo   wasm-pack build failed - see the output above.
        echo.
        pause
        exit /b 1
    )
    echo.
)

REM --- 2. the server ---------------------------------------------------------
echo Building sim-server ^(fast if nothing changed^)...
cargo build --release -p sim-server
if errorlevel 1 (
    echo.
    echo   cargo build failed - see the output above.
    echo   If the server is already running, close that window first: Windows
    echo   cannot overwrite a running .exe, and that shows up as a linker error.
    echo.
    pause
    exit /b 1
)
echo.

REM --- 3. the browser, once the port has had a moment to come up -------------
start "" /min cmd /c "ping -n 3 127.0.0.1 >nul & start http://127.0.0.1:8080/app/"

REM --- 4. the server in the foreground, so this window shows its log ---------
echo Serving http://127.0.0.1:8080/app/  ^(Ctrl+C to stop^)
echo.
target\release\sim-server.exe

echo.
echo Server stopped.
pause
