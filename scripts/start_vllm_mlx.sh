#!/bin/bash
# vLLM-MLX Start Script for macOS
# Usage: ./start_vllm_mlx.sh

set -e

MODEL="mlx-community/Qwen3.5-9B-MLX-4bit"
PORT=8000
GPU_MEMORY_UTILIZATION=0.9
MAX_MODEL_LEN=4096

MODELS_DIR="${HOME}/Library/Application Support/ComputeNode/models"

echo "TokenMeow - Starting vLLM-MLX..."
echo "Model: ${MODEL}"
echo "Port: ${PORT}"
echo "Models Directory: ${MODELS_DIR}"

mkdir -p "${MODELS_DIR}"

if ! command -v pip &> /dev/null; then
    echo "pip not found, installing..."
    brew install python
fi

if ! pip show vllm-mlx &> /dev/null; then
    echo "Installing vllm-mlx..."
    pip install vllm-mlx
fi

echo "Starting vLLM-MLX serve..."
python -m vllm_mlx.serve \
    --model "${MODEL}" \
    --port ${PORT} \
    --gpu-memory-utilization ${GPU_MEMORY_UTILIZATION} \
    --max-model-len ${MAX_MODEL_LEN} \
    --model-path "${MODELS_DIR}"

echo "vLLM-MLX started on http://0.0.0.0:${PORT}"
