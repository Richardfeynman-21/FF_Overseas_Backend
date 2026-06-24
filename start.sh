#!/bin/bash
set -e

echo "Starting FastAPI Chatbot application on port 8000..."
exec uvicorn main:app --host 0.0.0.0 --port 8000
