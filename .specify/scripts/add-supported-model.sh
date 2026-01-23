#!/usr/bin/env bash
# Add verified model to supported_models.json
# Usage: ./add-supported-model.sh --model-id <id> --repo <repo> --filename <file> --format <format> [options]

set -euo pipefail

# Default values
MODEL_ID=""
REPO=""
FILENAME=""
FORMAT=""
CAPABILITY="TextGeneration"
PLATFORM="macos-metal"
DESCRIPTION=""

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
    --model-id)
      MODEL_ID="$2"
      shift 2
      ;;
    --repo)
      REPO="$2"
      shift 2
      ;;
    --filename)
      FILENAME="$2"
      shift 2
      ;;
    --format)
      FORMAT="$2"
      shift 2
      ;;
    --capability)
      CAPABILITY="$2"
      shift 2
      ;;
    --platform)
      PLATFORM="$2"
      shift 2
      ;;
    --description)
      DESCRIPTION="$2"
      shift 2
      ;;
    -h|--help)
      echo "Usage: $0 --model-id <id> --repo <repo> --filename <file> --format <format> [options]"
      echo ""
      echo "Required arguments:"
      echo "  --model-id     Model identifier (e.g., phi4-14b)"
      echo "  --repo         HuggingFace repository (e.g., bartowski/phi-4-GGUF)"
      echo "  --filename     Model filename (e.g., phi-4-Q4_K_M.gguf)"
      echo "  --format       Model format: safetensors or gguf"
      echo ""
      echo "Optional arguments:"
      echo "  --capability   Model capability: TextGeneration, Vision, Audio, Embedding, Reranker"
      echo "                 (default: TextGeneration)"
      echo "  --platform     Verified platform: macos-metal, linux-cuda, windows-directml"
      echo "                 (default: macos-metal)"
      echo "  --description  Model description (default: auto-generated)"
      echo "  -h, --help     Show this help message"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

# Validate required arguments
if [[ -z "$MODEL_ID" ]]; then
  echo "Error: --model-id is required"
  exit 1
fi

if [[ -z "$REPO" ]]; then
  echo "Error: --repo is required"
  exit 1
fi

if [[ -z "$FILENAME" ]]; then
  echo "Error: --filename is required"
  exit 1
fi

if [[ -z "$FORMAT" ]]; then
  echo "Error: --format is required"
  exit 1
fi

# Validate format
case "$FORMAT" in
  safetensors|gguf)
    ;;
  *)
    echo "Error: --format must be 'safetensors' or 'gguf'"
    exit 1
    ;;
esac

# Validate capability
case "$CAPABILITY" in
  TextGeneration|Vision|Audio|Embedding|Reranker)
    ;;
  *)
    echo "Error: --capability must be one of: TextGeneration, Vision, Audio, Embedding, Reranker"
    exit 1
    ;;
esac

# Validate platform
case "$PLATFORM" in
  macos-metal|linux-cuda|windows-directml)
    ;;
  *)
    echo "Error: --platform must be one of: macos-metal, linux-cuda, windows-directml"
    exit 1
    ;;
esac

# Determine engine based on format
if [[ "$FORMAT" == "safetensors" ]]; then
  ENGINE="gptoss_cpp"
else
  ENGINE="llama_cpp"
fi

# Auto-generate description if not provided
if [[ -z "$DESCRIPTION" ]]; then
  DESCRIPTION="Verified $CAPABILITY model"
fi

# Find JSON file
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# Navigate to repository root
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
JSON_FILE="$REPO_ROOT/llmlb/src/supported_models.json"

if [[ ! -f "$JSON_FILE" ]]; then
  echo "Error: $JSON_FILE not found"
  exit 1
fi

# Check if jq is available
if ! command -v jq &> /dev/null; then
  echo "Error: jq is required but not installed"
  echo "Install with: brew install jq (macOS) or apt-get install jq (Linux)"
  exit 1
fi

# Check if model already exists
EXISTING=$(jq -r --arg id "$MODEL_ID" '.[] | select(.id == $id) | .id' "$JSON_FILE")
if [[ -n "$EXISTING" ]]; then
  echo "Model $MODEL_ID already exists in $JSON_FILE"

  # Check if platform already registered
  EXISTING_PLATFORMS=$(jq -r --arg id "$MODEL_ID" '.[] | select(.id == $id) | .platforms | join(",")' "$JSON_FILE")
  if echo "$EXISTING_PLATFORMS" | grep -q "$PLATFORM"; then
    echo "Platform $PLATFORM already registered for $MODEL_ID"
    exit 0
  fi

  # Add platform to existing model
  echo "Adding platform $PLATFORM to existing model $MODEL_ID"
  jq --arg id "$MODEL_ID" \
     --arg platform "$PLATFORM" \
     'map(if .id == $id then .platforms += [$platform] else . end)' \
     "$JSON_FILE" > "${JSON_FILE}.tmp" && mv "${JSON_FILE}.tmp" "$JSON_FILE"

  echo "Updated $MODEL_ID with platform $PLATFORM"
  exit 0
fi

# Add new model entry
echo "Adding new model: $MODEL_ID"
echo "  Repository: $REPO"
echo "  Filename: $FILENAME"
echo "  Format: $FORMAT"
echo "  Engine: $ENGINE"
echo "  Capability: $CAPABILITY"
echo "  Platform: $PLATFORM"

jq --arg id "$MODEL_ID" \
   --arg name "$MODEL_ID" \
   --arg desc "$DESCRIPTION" \
   --arg repo "$REPO" \
   --arg filename "$FILENAME" \
   --arg format "$FORMAT" \
   --arg engine "$ENGINE" \
   --arg capability "$CAPABILITY" \
   --arg platform "$PLATFORM" \
   '. += [{
     "id": $id,
     "name": $name,
     "description": $desc,
     "repo": $repo,
     "recommended_filename": $filename,
     "format": $format,
     "engine": $engine,
     "capability": $capability,
     "platforms": [$platform]
   }]' "$JSON_FILE" > "${JSON_FILE}.tmp" && mv "${JSON_FILE}.tmp" "$JSON_FILE"

echo ""
echo "Added $MODEL_ID to $JSON_FILE"
echo "Don't forget to commit and create PR!"
