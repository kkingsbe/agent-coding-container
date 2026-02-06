@echo off
set AGENT_CONTAINER_PATH=C:\Users\Kyle\Documents\code\agent-coding-container

for %%I in (.) do set DIRNAME=%%~nxI
if "%~1"=="" (
    set COMPOSE_PROJECT_NAME=%DIRNAME%-dev
) else (
    set COMPOSE_PROJECT_NAME=%~1-dev
)

set LOOP_TYPE=development
set MOUNT_HOST_DIR=%cd%

docker-compose -f "%AGENT_CONTAINER_PATH%\docker-compose.yml" up -d --build