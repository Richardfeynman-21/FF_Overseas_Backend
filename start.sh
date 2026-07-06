#!/bin/bash
set -e

echo "Starting FastAPI Chatbot application on port 8000..."
exec uvicorn main:app --host 0.0.0.0 --port 8000 --proxy-headers --forwarded-allow-ips="127.0.0.1"
