FROM python:3.11-slim

# Prevent Python from writing .pyc files and enable unbuffered logging
ENV PYTHONDONTWRITEBYTECODE=1
ENV PYTHONUNBUFFERED=1

WORKDIR /app

# Install system dependencies (curl for healthchecks, postgresql-client for DB sync tools)
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl postgresql-client \
    && rm -rf /var/lib/apt/lists/*

# Install python dependencies
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

# Copy application files
COPY main.py .
COPY app/ app/
COPY db_dumps/ db_dumps/

# Copy startup and migration scripts and make them executable
COPY start.sh .
COPY sync_db_to_rds_container.sh .
RUN chmod +x start.sh sync_db_to_rds_container.sh

# Expose port
EXPOSE 8000

# Run startup script
CMD ["./start.sh"]
