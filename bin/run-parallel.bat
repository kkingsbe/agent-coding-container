@echo off
set AGENT_CONTAINER_PATH=%~dp0..\automation-parallel

for %%I in (.) do set DIRNAME=%%~nxI
if "%~1"=="" (
    set COMPOSE_PROJECT_NAME=%DIRNAME%-dev
) else (
    set COMPOSE_PROJECT_NAME=%~1-dev
)

rem Set CONTAINER_PREFIX to unique prefix based on workspace directory
rem This prevents container name conflicts when running from different workspaces
set CONTAINER_PREFIX=%DIRNAME%-

set MOUNT_HOST_DIR=%cd%

rem Set prompt interval (default: 0 - no delay for main dev loop)
if "%~2"=="" (
    set PROMPT_INTERVAL=0
) else (
    set PROMPT_INTERVAL=%~2
)

rem Set janitor interval (default: 1200000)
if "%~3"=="" (
    set JANITOR_INTERVAL=1200000
) else (
    set JANITOR_INTERVAL=%~3
)

rem Set architect interval (default: 2400000)
if "%~4"=="" (
    set ARCHITECT_INTERVAL=2400000
) else (
    set ARCHITECT_INTERVAL=%~4
)

rem Set prompt timeout (default: 1800000 = 30 minutes)
if "%~5"=="" (
    set PROMPT_TIMEOUT=1800000
) else (
    set PROMPT_TIMEOUT=%~5
)

rem Set janitor timeout (default: 1800000 = 30 minutes)
if "%~6"=="" (
    set JANITOR_TIMEOUT=1800000
) else (
    set JANITOR_TIMEOUT=%~6
)

rem Set architect timeout (default: 1800000 = 30 minutes)
if "%~7"=="" (
    set ARCHITECT_TIMEOUT=1800000
) else (
    set ARCHITECT_TIMEOUT=%~7
)

docker-compose -f "%AGENT_CONTAINER_PATH%\docker-compose.yml" -p %COMPOSE_PROJECT_NAME% up -d --build
