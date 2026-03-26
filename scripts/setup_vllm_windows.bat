@echo off
REM TokenMeow - vLLM Docker Setup Script for Windows
REM Run as Administrator

echo TokenMeow - Setting up vLLM Docker...

REM Check if Docker is installed
where docker >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Docker not found, installing...
    powershell -Command "Invoke-WebRequest -Uri https://get.docker.com -OutFile C:\docker-install.ps1"
    powershell -ExecutionPolicy Bypass -File C:\docker-install.ps1
    del C:\docker-install.ps1
    net start docker
)

REM Create models directory
if not exist "C:\ProgramData\ComputeNode\models" (
    mkdir "C:\ProgramData\ComputeNode\models"
)

REM Stop and remove existing container
docker stop vllm-server 2>nul
docker rm vllm-server 2>nul

REM Pull vLLM image
echo Pulling vLLM image...
docker pull vllm/vllm-openai:latest

REM Start vLLM container
echo Starting vLLM container...
docker run -d ^
    --name vllm-server ^
    --gpus all ^
    -p 8000:8000 ^
    -v "C:\ProgramData\ComputeNode\models:/models" ^
    vllm/vllm-openai:latest ^
    --model Qwen/Qwen3.5-9B-Instruct ^
    --port 8000 ^
    --gpu-memory-utilization 0.9 ^
    --max-model-len 4096

echo vLLM started on http://0.0.0.0:8000
echo.
echo To check logs: docker logs -f vllm-server
echo To stop: docker stop vllm-server
