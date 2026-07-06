#!/bin/bash
# Auto-update Cloudflare DNS A record with current EC2 public IP

if [ ! -f /etc/cloudflare/credentials ]; then
    echo "$(date): ERROR - Credentials file /etc/cloudflare/credentials not found"
    exit 1
fi

source /etc/cloudflare/credentials

CF_RECORD_NAME="api.ffoverseas.in"
LOG="/var/log/cloudflare-dns-update.log"

# Get current public IP from EC2 metadata (IMDSv2)
IMDS_TOKEN=$(curl -s -X PUT "http://169.254.169.254/latest/api/token" -H "X-aws-ec2-metadata-token-ttl-seconds: 21600")
CURRENT_IP=$(curl -s -H "X-aws-ec2-metadata-token: $IMDS_TOKEN" http://169.254.169.254/latest/meta-data/public-ipv4)

if [ -z "$CURRENT_IP" ]; then
    echo "$(date): ERROR - Could not get public IP" >> "$LOG"
    exit 1
fi

# Get DNS record ID
RECORD_ID=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records?name=${CF_RECORD_NAME}&type=A" \
    -H "Authorization: Bearer ${CF_API_TOKEN}" -H "Content-Type: application/json" | \
    python3 -c "import sys,json; print(json.load(sys.stdin)['result'][0]['id'])" 2>/dev/null)

if [ -z "$RECORD_ID" ]; then
    echo "$(date): ERROR - Could not find DNS record for ${CF_RECORD_NAME}" >> "$LOG"
    exit 1
fi

# Get current IP in Cloudflare
CF_IP=$(curl -s -X GET "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records/${RECORD_ID}" \
    -H "Authorization: Bearer ${CF_API_TOKEN}" -H "Content-Type: application/json" | \
    python3 -c "import sys,json; print(json.load(sys.stdin)['result']['content'])" 2>/dev/null)

if [ "$CURRENT_IP" = "$CF_IP" ]; then
    echo "$(date): IP unchanged ($CURRENT_IP)" >> "$LOG"
    exit 0
fi

# Update DNS record
RESPONSE=$(curl -s -X PUT "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records/${RECORD_ID}" \
    -H "Authorization: Bearer ${CF_API_TOKEN}" -H "Content-Type: application/json" \
    --data "{\"type\":\"A\",\"name\":\"${CF_RECORD_NAME}\",\"content\":\"${CURRENT_IP}\",\"ttl\":60,\"proxied\":true}")

SUCCESS=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['success'])" 2>/dev/null)

if [ "$SUCCESS" = "True" ]; then
    echo "$(date): SUCCESS - Updated ${CF_RECORD_NAME} to ${CURRENT_IP}" >> "$LOG"
else
    echo "$(date): FAILED - ${RESPONSE}" >> "$LOG"
fi
