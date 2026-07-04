#!/bin/bash
# sync_db_to_rds.sh
# Exit immediately if a command exits with a non-zero status
set -e

# Database Configuration
LOCAL_DB="universities_db"
LOCAL_USER="ffoverseas"
RDS_HOST="database-1.czo2ggw2c7f3.ap-south-1.rds.amazonaws.com"
RDS_USER="${RDS_USER:-postgres}" # Default to postgres if not defined
RDS_DB="universities_db"
DUMP_FILE="./tmp_db_sync.dump"

# Confirm RDS_PASSWORD is set in environment
if [ -z "$RDS_PASSWORD" ]; then
    echo "ERROR: \$RDS_PASSWORD environment variable is not set."
    echo "Please run: export RDS_PASSWORD='your_password'"
    exit 1
fi

echo "========================================="
echo "  FF Overseas DB Sync Pipeline Started   "
echo "========================================="

echo "[1/4] Dumping local database '$LOCAL_DB'..."
pg_dump -U "$LOCAL_USER" -h localhost -F c -b -v -f "$DUMP_FILE" "$LOCAL_DB"

echo "[2/4] Resetting RDS Database '$RDS_DB' on Host..."
PGPASSWORD="$RDS_PASSWORD" psql -h "$RDS_HOST" -U "$RDS_USER" -d postgres -c "DROP DATABASE IF EXISTS $RDS_DB WITH (FORCE);"
PGPASSWORD="$RDS_PASSWORD" psql -h "$RDS_HOST" -U "$RDS_USER" -d postgres -c "CREATE DATABASE $RDS_DB;"

echo "[3/4] Restoring dump to AWS RDS..."
PGPASSWORD="$RDS_PASSWORD" pg_restore -h "$RDS_HOST" -U "$RDS_USER" -d "$RDS_DB" -v "$DUMP_FILE"

echo "[4/4] Cleaning up temporary files..."
rm -f "$DUMP_FILE"

echo "========================================="
echo "  Sync Pipeline Completed Successfully!  "
echo "========================================="
