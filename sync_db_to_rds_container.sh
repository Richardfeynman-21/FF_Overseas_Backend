#!/bin/bash
# sync_db_to_rds_container.sh
# Exit immediately if a command exits with a non-zero status
set -e

if [ -z "$UNIVERSITIES_DATABASE_URL" ]; then
    echo "ERROR: UNIVERSITIES_DATABASE_URL environment variable is not defined."
    exit 1
fi

DB_URL="$UNIVERSITIES_DATABASE_URL"
# Construct admin connection URL by replacing the database name at the end with '/postgres'
# E.g. postgresql://user:pass@host:port/universities_db -> postgresql://user:pass@host:port/postgres
ADMIN_URL=$(echo "$DB_URL" | sed -E 's|/[a-zA-Z0-9_-]+(\?.*)?$|/postgres\1|')

echo "========================================="
echo "  Container -> RDS Sync Pipeline Started "
echo "========================================="

echo "[1/2] Resetting RDS database 'universities_db'..."
psql -d "$ADMIN_URL" -c "DROP DATABASE IF EXISTS universities_db;"
psql -d "$ADMIN_URL" -c "CREATE DATABASE universities_db;"

echo "[2/2] Restoring dump to AWS RDS..."
pg_restore -d "$DB_URL" -v db_dumps/universities_db.dump

echo "========================================="
echo "  Sync Pipeline Completed Successfully!  "
echo "========================================="
