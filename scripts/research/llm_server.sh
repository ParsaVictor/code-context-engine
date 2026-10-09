#!/usr/bin/env bash
# Start a local OpenAI-compatible LLM endpoint (llama.cpp `llama-server`) for
# scripts/research/llm_localize.py. Uses the integrated GPU through Vulkan by default; BACKEND=cpu for
# CPU only.
#
#   bash scripts/research/llm_server.sh [model.gguf] [port]
#
# Binaries and models live under %LOCALAPPDATA%/neuromesh/llm (not in the repo):
#   llm/llama-b11172-cpu/llama-server.exe, llm/llama-b11172-vulkan/...,
#   llm/models/qwen2.5-coder-3b-instruct-q4_k_m.gguf
set -euo pipefail
LLM="${LOCALAPPDATA:-$HOME/.local/share}/neuromesh/llm"
MODEL="${1:-$LLM/models/qwen2.5-coder-3b-instruct-q4_k_m.gguf}"
PORT="${2:-8089}"
BACKEND="${BACKEND:-vulkan}"
BIN="$LLM/llama-b11172-$BACKEND/llama-server.exe"
EXTRA=()
[ "$BACKEND" = "vulkan" ] && EXTRA=(-ngl 99)
exec "$BIN" -m "$MODEL" --port "$PORT" -c "${CTX:-8192}" -t "${THREADS:-6}" \
  --temp 0 "${EXTRA[@]}"
