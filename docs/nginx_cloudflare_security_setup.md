# Secured Backend Deployment — Cloudflare + Nginx (No Elastic IP)

Complete setup guide for securing the FF Overseas Backend on AWS EC2 with Cloudflare edge proxy, Nginx reverse proxy, and server-side API key authentication.

---

## Architecture

```
[ User Browser ]
       ↓ HTTPS
[ Vercel Frontend (ffoverseas.in) ]
       ↓ internal call (no secrets exposed to browser)
[ Next.js API Route (/api/chat) — Vercel server-side ]
       ↓ HTTPS + secret X-ORBIT-API-KEY header
[ Cloudflare Edge (api.ffoverseas.in) — Free SSL, WAF, DDoS ]
       ↓ HTTPS + Authenticated Origin Pull (mutual TLS)
[ Nginx on EC2 :443 — origin cert, header enforcement ]
       ↓ HTTP over loopback only
[ FastAPI in Docker (127.0.0.1:8000) ]
```

### Security Layers

| Layer | What it does | Stops what |
|-------|-------------|------------|
| **1. AWS Security Groups** | Only allows Cloudflare IP ranges on ports 80/443 | Direct access from any non-Cloudflare IP |
| **2. Cloudflare Edge** | WAF, DDoS mitigation, Bot Fight Mode | Bots, L7 DDoS, malicious traffic |
| **3. Authenticated Origin Pulls** | Nginx requires Cloudflare's client certificate | Anyone connecting directly to EC2 IP (even if they guess it) |
| **4. Nginx Header Enforcement** | Rejects requests missing `CF-Connecting-IP` | Requests that somehow bypass Cloudflare |
| **5. Global API Key Middleware** | FastAPI rejects requests without valid `X-ORBIT-API-KEY` on ALL endpoints | Unauthorized API access |
| **6. Server-Side Proxy (Vercel)** | API key never reaches browser; validated input only forwarded | Key extraction from browser, payload injection |

---

## Code Changes Already Applied to This Repository

The following files have been modified and are ready to commit + deploy:

### 1. `app/main.py` — Global API key middleware + CORS hardening

- **Global API key middleware**: Every endpoint (chat, universities, courses, scholarships) now requires a valid `X-ORBIT-API-KEY` header. Only `/api/health` and CORS preflight `OPTIONS` requests are exempt.
- **No wildcard CORS fallback**: If the `ALLOWED_ORIGINS` env var is missing, CORS defaults to `https://ffoverseas.in` and `https://www.ffoverseas.in` — never `*`.
- **Proxy-aware IP extraction**: Rate limiter reads `X-Real-IP` header (set by Nginx from Cloudflare's `CF-Connecting-IP`).

### 2. `app/config.py` — Safe defaults

- `ALLOWED_ORIGINS` default changed from including `localhost` to production domains only.

### 3. `docker-compose.yml` — Localhost-only binding

- Port binding changed from `"8000:8000"` to `"127.0.0.1:8000:8000"`.
- FastAPI container is unreachable from outside the EC2 host.

### 4. `start.sh` — Proxy header trust

- Uvicorn now starts with `--proxy-headers --forwarded-allow-ips="127.0.0.1"`.

---

## YOUR MANUAL STEPS — What You Need to Do

Everything below happens outside this codebase — on Cloudflare, AWS, your EC2 terminal, and your Vercel dashboard.

---

### PART A: Cloudflare Setup (Your Browser — ~15 minutes)

#### A1. Create Cloudflare Account and Add Domain

1. Go to https://cloudflare.com → Sign up (free).
2. Click **"Add a Site"** → type `ffoverseas.in` → select **Free plan**.
3. Cloudflare will scan and import your existing DNS records. **Keep all of them.**
4. Cloudflare will show you 2 nameservers like:
   ```
   ana.ns.cloudflare.com
   bob.ns.cloudflare.com
   ```
5. Go to wherever you bought `ffoverseas.in` (your domain registrar — GoDaddy, Namecheap, Hostinger, etc.).
6. Find **DNS Settings** or **Nameservers** and **replace** the current nameservers with the two Cloudflare gave you.
7. Save. Wait 5-30 minutes for propagation.

#### A2. Add API Subdomain DNS Record

In Cloudflare Dashboard → **DNS** → **Records** → **Add Record**:

- **Type**: `A`
- **Name**: `api`
- **IPv4 address**: Your EC2 instance's current public IP (find it in AWS Console → EC2 → Instances → your instance → "Public IPv4 address")
- **Proxy status**: Click to make it **Proxied** (orange cloud ☁️) — this is critical, it hides your real IP

#### A3. Configure SSL

1. **SSL/TLS** → **Overview** → Set encryption mode to **Full (Strict)**
2. **SSL/TLS** → **Edge Certificates**:
   - Turn ON **"Always Use HTTPS"**
   - Set **Minimum TLS Version** to `TLS 1.2`

#### A4. Enable Origin Security

1. **SSL/TLS** → **Origin Server** → Turn ON **"Authenticated Origin Pulls"**
2. **Security** → **Bots** → Turn ON **"Bot Fight Mode"**

#### A5. Generate Origin Certificate

1. **SSL/TLS** → **Origin Server** → click **"Create Certificate"**
2. Choose **"Generate private key and CSR with Cloudflare"** (RSA)
3. Hostnames: make sure `api.ffoverseas.in` is listed
4. Certificate validity: **15 years**
5. Click **Create**
6. **IMPORTANT**: You will see two text blocks:
   - **Origin Certificate** — copy and save this (you'll paste it on EC2)
   - **Private Key** — copy and save this (you'll paste it on EC2). **You cannot view this again after closing the page.**

#### A6. Create Cloudflare API Token (for DNS auto-update script)

1. Click your profile icon → **My Profile** → **API Tokens** → **Create Token**
2. Select template: **"Edit zone DNS"**
3. Under **Zone Resources** → `Include` → `Specific zone` → `ffoverseas.in`
4. Click **Continue to summary** → **Create Token**
5. **Copy and save the token** — you'll need it on EC2.
6. Also note your **Zone ID** — visible on the **Overview** page of your domain in Cloudflare, right sidebar.

---

### PART B: AWS Security Groups (AWS Console — ~10 minutes)

#### B1. Lock Down Your Security Group

1. Go to **AWS Console** → **EC2** → **Instances** → click your instance
2. Under **Security** tab → click your **Security Group** link
3. Click **Edit inbound rules**
4. **Delete ALL existing rules** and add ONLY these:

**SSH Rule (1 rule):**

| Type | Protocol | Port | Source | Description |
|------|----------|------|--------|-------------|
| SSH | TCP | 22 | My IP | SSH access |

**HTTP Rules (15 rules — one per Cloudflare IPv4 range):**

For each of these IP ranges, add a rule with Type=HTTP, Port=80:

```
173.245.48.0/20
103.21.244.0/22
103.22.200.0/22
103.31.4.0/22
141.101.64.0/18
108.162.192.0/18
190.93.240.0/20
188.114.96.0/20
197.234.240.0/22
198.41.128.0/17
162.158.0.0/15
104.16.0.0/13
104.24.0.0/14
172.64.0.0/13
131.0.72.0/22
```

**HTTPS Rules (15 rules — same IP ranges, port 443):**

Add the same 15 IP ranges again, but with Type=HTTPS, Port=443.

**IPv6 Rules (optional but recommended — 7 ranges for each of port 80 and 443):**

```
2400:cb00::/32
2606:4700::/32
2803:f800::/32
2405:b500::/32
2405:8100::/32
2a06:98c0::/29
2c0f:f248::/32
```

5. Click **Save rules**.

> Total rules: 1 (SSH) + 15 (HTTP) + 15 (HTTPS) + optionally 14 (IPv6) = 31-45 rules.
> AWS allows up to 60 rules per security group by default, so this fits.

---

### PART C: EC2 Server Setup (SSH into EC2 — ~15 minutes)

SSH into your EC2 instance and run these commands in order.

#### C1. Install Nginx

```bash
sudo apt update && sudo apt install -y nginx
```

#### C2. Install SSL Certificates

```bash
# Create directory for certificates
sudo mkdir -p /etc/nginx/ssl

# Create the origin certificate file — paste the certificate from Step A5
sudo nano /etc/nginx/ssl/cloudflare-origin.pem
# Paste the ORIGIN CERTIFICATE text, save (Ctrl+X, Y, Enter)

# Create the private key file — paste the private key from Step A5
sudo nano /etc/nginx/ssl/cloudflare-origin-key.pem
# Paste the PRIVATE KEY text, save (Ctrl+X, Y, Enter)

# Secure the private key
sudo chmod 600 /etc/nginx/ssl/cloudflare-origin-key.pem

# Download Cloudflare's Authenticated Origin Pull CA certificate
sudo curl -o /etc/nginx/ssl/cloudflare-authenticated-origin-pull.pem \
    https://developers.cloudflare.com/ssl/static/authenticated_origin_pull_ca.pem
```

#### C3. Create Nginx Configuration

```bash
sudo nano /etc/nginx/sites-available/ffoverseas-api
```

Paste this entire configuration:

```nginx
# Rate limiting by real visitor IP (from Cloudflare header)
limit_req_zone $http_cf_connecting_ip zone=api_limit:10m rate=10r/s;

# HTTP → HTTPS redirect
server {
    listen 80;
    server_name api.ffoverseas.in;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name api.ffoverseas.in;

    # ─── Cloudflare Origin Certificate ───
    ssl_certificate     /etc/nginx/ssl/cloudflare-origin.pem;
    ssl_certificate_key /etc/nginx/ssl/cloudflare-origin-key.pem;

    # ─── Authenticated Origin Pulls (mutual TLS) ───
    ssl_client_certificate /etc/nginx/ssl/cloudflare-authenticated-origin-pull.pem;
    ssl_verify_client on;

    # ─── TLS hardening ───
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # ─── Security headers ───
    add_header X-Content-Type-Options    "nosniff" always;
    add_header X-Frame-Options           "DENY" always;
    add_header X-XSS-Protection          "1; mode=block" always;
    add_header Referrer-Policy           "strict-origin-when-cross-origin" always;
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;

    # ─── Reject direct access (no CF-Connecting-IP = not from Cloudflare) ───
    if ($http_cf_connecting_ip = "") {
        return 403;
    }

    # ─── Reverse proxy to FastAPI ───
    location / {
        limit_req zone=api_limit burst=20 nodelay;

        proxy_pass         http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $http_cf_connecting_ip;
        proxy_set_header   X-Forwarded-For   $http_cf_connecting_ip;
        proxy_set_header   X-Forwarded-Proto $scheme;

        proxy_connect_timeout 10s;
        proxy_read_timeout    30s;
        proxy_send_timeout    30s;
    }

    # ─── Health check ───
    location = /api/health {
        proxy_pass         http://127.0.0.1:8000;
        proxy_http_version 1.1;
        proxy_set_header   Host              $host;
        proxy_set_header   X-Real-IP         $http_cf_connecting_ip;
        proxy_set_header   X-Forwarded-For   $http_cf_connecting_ip;
        proxy_set_header   X-Forwarded-Proto $scheme;
    }

    # Block hidden files
    location ~ /\. {
        deny all;
        return 404;
    }
}
```

Save and exit (Ctrl+X, Y, Enter).

#### C4. Activate Nginx Configuration

```bash
# Enable the site
sudo ln -s /etc/nginx/sites-available/ffoverseas-api /etc/nginx/sites-enabled/

# Remove default site
sudo rm -f /etc/nginx/sites-enabled/default

# Test configuration for syntax errors
sudo nginx -t

# Enable Nginx to start on boot and restart it
sudo systemctl enable nginx
sudo systemctl restart nginx
```

If `nginx -t` shows errors, double-check that the certificate files exist and are not empty:
```bash
ls -la /etc/nginx/ssl/
```

#### C5. Set Up DNS Auto-Update Script

```bash
# Create the credentials file (restricted permissions)
sudo mkdir -p /etc/cloudflare
sudo nano /etc/cloudflare/credentials
```

Paste (replacing with your actual values from Step A6):
```bash
CF_API_TOKEN="your-cloudflare-api-token-here"
CF_ZONE_ID="your-zone-id-here"
```

Save, then secure:
```bash
sudo chmod 600 /etc/cloudflare/credentials
```

Now create the update script:
```bash
sudo nano /usr/local/bin/update-cloudflare-dns.sh
```

Paste:
```bash
#!/bin/bash
# Auto-update Cloudflare DNS A record with current EC2 public IP

source /etc/cloudflare/credentials

CF_RECORD_NAME="api.ffoverseas.in"
LOG="/var/log/cloudflare-dns-update.log"

# Get current public IP from EC2 metadata (IMDSv2)
IMDS_TOKEN=$(curl -s -X PUT "http://169.254.169.254/latest/api/token" \
    -H "X-aws-ec2-metadata-token-ttl-seconds: 21600")
CURRENT_IP=$(curl -s -H "X-aws-ec2-metadata-token: $IMDS_TOKEN" \
    http://169.254.169.254/latest/meta-data/public-ipv4)

if [ -z "$CURRENT_IP" ]; then
    echo "$(date): ERROR - Could not get public IP" >> "$LOG"
    exit 1
fi

# Get DNS record ID
RECORD_ID=$(curl -s -X GET \
    "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records?name=${CF_RECORD_NAME}&type=A" \
    -H "Authorization: Bearer ${CF_API_TOKEN}" \
    -H "Content-Type: application/json" | \
    python3 -c "import sys,json; print(json.load(sys.stdin)['result'][0]['id'])" 2>/dev/null)

if [ -z "$RECORD_ID" ]; then
    echo "$(date): ERROR - Could not find DNS record for ${CF_RECORD_NAME}" >> "$LOG"
    exit 1
fi

# Get current IP in Cloudflare
CF_IP=$(curl -s -X GET \
    "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records/${RECORD_ID}" \
    -H "Authorization: Bearer ${CF_API_TOKEN}" \
    -H "Content-Type: application/json" | \
    python3 -c "import sys,json; print(json.load(sys.stdin)['result']['content'])" 2>/dev/null)

if [ "$CURRENT_IP" = "$CF_IP" ]; then
    echo "$(date): IP unchanged ($CURRENT_IP)" >> "$LOG"
    exit 0
fi

# Update DNS record
RESPONSE=$(curl -s -X PUT \
    "https://api.cloudflare.com/client/v4/zones/${CF_ZONE_ID}/dns_records/${RECORD_ID}" \
    -H "Authorization: Bearer ${CF_API_TOKEN}" \
    -H "Content-Type: application/json" \
    --data "{\"type\":\"A\",\"name\":\"${CF_RECORD_NAME}\",\"content\":\"${CURRENT_IP}\",\"ttl\":60,\"proxied\":true}")

SUCCESS=$(echo "$RESPONSE" | python3 -c "import sys,json; print(json.load(sys.stdin)['success'])" 2>/dev/null)

if [ "$SUCCESS" = "True" ]; then
    echo "$(date): SUCCESS - Updated ${CF_RECORD_NAME} to ${CURRENT_IP}" >> "$LOG"
else
    echo "$(date): FAILED - ${RESPONSE}" >> "$LOG"
fi
```

Save and activate:
```bash
# Make executable
sudo chmod +x /usr/local/bin/update-cloudflare-dns.sh

# Add cron jobs: run on boot + every 5 minutes
(crontab -l 2>/dev/null; echo "@reboot sleep 30 && /usr/local/bin/update-cloudflare-dns.sh") | crontab -
(crontab -l 2>/dev/null; echo "*/5 * * * * /usr/local/bin/update-cloudflare-dns.sh") | crontab -

# Test it immediately
/usr/local/bin/update-cloudflare-dns.sh
cat /var/log/cloudflare-dns-update.log
```

You should see `SUCCESS - Updated api.ffoverseas.in to <your-ip>`.

#### C6. Generate API Key and Update Backend .env

```bash
# Generate a secure random API key
openssl rand -hex 32
```

Copy the output. Then edit your `.env`:
```bash
nano /path/to/your/backend/.env
```

Add/update these lines:
```ini
FRONTEND_API_KEY="paste-the-64-character-hex-key-here"
ALLOWED_ORIGINS="https://ffoverseas.in,https://www.ffoverseas.in"
```

#### C7. Deploy Updated Backend Code

Pull the latest code (with the fixes applied) and rebuild:
```bash
cd /path/to/your/backend
git pull origin main
docker compose down && docker compose up -d --build
```

#### C8. Verify Everything Works

```bash
# 1. Check Nginx is running
sudo systemctl status nginx

# 2. Check Docker container is running
docker ps

# 3. Test health check through Cloudflare (should return {"status":"healthy"})
curl https://api.ffoverseas.in/api/health

# 4. Test that direct access is blocked (should fail / timeout)
curl -k https://<your-ec2-ip>/api/health
# This should fail because your Security Group blocks non-Cloudflare IPs

# 5. Test that API key is required (should return 403)
curl -X POST https://api.ffoverseas.in/api/public-chat \
    -H "Content-Type: application/json" \
    -d '{"sessionId":"test","message":"hello"}'
# Expected: {"error":"Access forbidden: Invalid API Key."}

# 6. Test with correct API key (should work)
curl -X POST https://api.ffoverseas.in/api/public-chat \
    -H "Content-Type: application/json" \
    -H "X-ORBIT-API-KEY: your-api-key-here" \
    -d '{"sessionId":"test","message":"hello"}'
# Expected: A chat response from the AI
```

---

### PART D: Vercel Frontend Changes (~10 minutes)

#### D1. Set Environment Variables in Vercel

Go to **Vercel Dashboard** → your project → **Settings** → **Environment Variables**:

| Variable Name | Value | Notes |
|:---|:---|:---|
| `BACKEND_API_URL` | `https://api.ffoverseas.in` | **No** `NEXT_PUBLIC_` prefix — server-side only |
| `BACKEND_API_KEY` | The 64-char hex key from Step C6 | **No** `NEXT_PUBLIC_` prefix — server-side only |

Remove any old variables like `NEXT_PUBLIC_API_URL` or `NEXT_PUBLIC_API_KEY` if they exist.

#### D2. Create the Server-Side API Route Proxy

This is the code that keeps your API key hidden from the browser.

**If you use App Router** (`/app` directory), create this file:

`app/api/chat/route.ts`
```typescript
import { NextResponse } from 'next/server';

export async function POST(request: Request) {
  // Validate Content-Type
  const contentType = request.headers.get('content-type');
  if (!contentType || !contentType.includes('application/json')) {
    return NextResponse.json({ error: 'Invalid content type' }, { status: 415 });
  }

  try {
    const body = await request.json();
    const { sessionId, message } = body;

    // Input validation — only forward clean, validated fields
    if (!sessionId || typeof sessionId !== 'string' || sessionId.length > 100) {
      return NextResponse.json({ error: 'Invalid sessionId' }, { status: 400 });
    }
    if (!message || typeof message !== 'string' || message.length > 2000) {
      return NextResponse.json({ error: 'Invalid message' }, { status: 400 });
    }

    const backendUrl = process.env.BACKEND_API_URL;
    const apiKey = process.env.BACKEND_API_KEY;

    if (!backendUrl || !apiKey) {
      console.error('Missing BACKEND_API_URL or BACKEND_API_KEY env vars');
      return NextResponse.json({ error: 'Server configuration error' }, { status: 500 });
    }

    // Server-to-server call — API key never reaches the browser
    const response = await fetch(`${backendUrl}/api/public-chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-ORBIT-API-KEY': apiKey,
      },
      body: JSON.stringify({ sessionId, message }),
    });

    const data = await response.json();
    return NextResponse.json(data, { status: response.status });
  } catch (error) {
    console.error('Chat proxy error:', error);
    return NextResponse.json({ error: 'Internal Server Error' }, { status: 500 });
  }
}
```

**If you use Pages Router** (`/pages` directory), create this file instead:

`pages/api/chat.ts`
```typescript
import type { NextApiRequest, NextApiResponse } from 'next';

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== 'POST') {
    return res.status(405).json({ error: 'Method not allowed' });
  }

  try {
    const { sessionId, message } = req.body;

    // Input validation
    if (!sessionId || typeof sessionId !== 'string' || sessionId.length > 100) {
      return res.status(400).json({ error: 'Invalid sessionId' });
    }
    if (!message || typeof message !== 'string' || message.length > 2000) {
      return res.status(400).json({ error: 'Invalid message' });
    }

    const backendUrl = process.env.BACKEND_API_URL;
    const apiKey = process.env.BACKEND_API_KEY;

    if (!backendUrl || !apiKey) {
      console.error('Missing BACKEND_API_URL or BACKEND_API_KEY env vars');
      return res.status(500).json({ error: 'Server configuration error' });
    }

    const response = await fetch(`${backendUrl}/api/public-chat`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'X-ORBIT-API-KEY': apiKey,
      },
      body: JSON.stringify({ sessionId, message }),
    });

    const data = await response.json();
    return res.status(response.status).json(data);
  } catch (error) {
    console.error('Chat proxy error:', error);
    return res.status(500).json({ error: 'Internal Server Error' });
  }
}
```

#### D3. Update Your Frontend Chat Component

Find where your frontend currently calls the backend directly and change it:

**Before** (insecure — API key exposed to browser):
```javascript
const response = await fetch('http://<ec2-ip>:8000/api/public-chat', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    'X-ORBIT-API-KEY': 'your-key-here',  // ← exposed!
  },
  body: JSON.stringify({ sessionId, message }),
});
```

**After** (secure — calls your own Vercel server route):
```javascript
const response = await fetch('/api/chat', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({ sessionId, message }),
});
const data = await response.json();
```

Notice: **no API key, no backend URL** in the client-side code.

#### D4. Create University Proxy Routes (if needed)

If your frontend calls university endpoints, create similar proxy routes for them. For example:

`app/api/universities/route.ts`
```typescript
import { NextResponse } from 'next/server';

export async function GET(request: Request) {
  const backendUrl = process.env.BACKEND_API_URL;
  const apiKey = process.env.BACKEND_API_KEY;

  if (!backendUrl || !apiKey) {
    return NextResponse.json({ error: 'Server configuration error' }, { status: 500 });
  }

  // Forward the query string
  const { searchParams } = new URL(request.url);
  const queryString = searchParams.toString();
  const url = `${backendUrl}/api/universities${queryString ? '?' + queryString : ''}`;

  const response = await fetch(url, {
    headers: { 'X-ORBIT-API-KEY': apiKey },
  });

  const data = await response.json();
  return NextResponse.json(data, { status: response.status });
}
```

#### D5. Redeploy

Push your frontend changes and Vercel will auto-deploy. Or trigger a manual redeploy from the Vercel dashboard.

---

## Execution Order Checklist

Complete these in order. Check each off as you go.

| # | Step | Time | Done? |
|---|------|------|:---:|
| 1 | Sign up for Cloudflare, add `ffoverseas.in`, change nameservers | 5 min + wait | ☐ |
| 2 | Add `api` A record (Proxied) in Cloudflare DNS | 1 min | ☐ |
| 3 | Set SSL to Full (Strict), enable HTTPS, TLS 1.2 min | 2 min | ☐ |
| 4 | Enable Authenticated Origin Pulls + Bot Fight Mode | 1 min | ☐ |
| 5 | Generate Origin Certificate, save cert + key | 3 min | ☐ |
| 6 | Create Cloudflare API Token, note Zone ID | 2 min | ☐ |
| 7 | Lock down AWS Security Groups (Cloudflare IPs only) | 10 min | ☐ |
| 8 | SSH into EC2, install Nginx | 2 min | ☐ |
| 9 | Install origin cert + private key + CF CA cert | 3 min | ☐ |
| 10 | Create and activate Nginx config | 3 min | ☐ |
| 11 | Set up DNS auto-update script + cron | 5 min | ☐ |
| 12 | Generate API key, update `.env` | 2 min | ☐ |
| 13 | Pull updated backend code, rebuild Docker | 3 min | ☐ |
| 14 | Run verification tests (Step C8) | 3 min | ☐ |
| 15 | Set Vercel env vars (no `NEXT_PUBLIC_` prefix) | 2 min | ☐ |
| 16 | Create Next.js API route proxy + update frontend calls | 10 min | ☐ |
| 17 | Redeploy Vercel frontend | 2 min | ☐ |
| | **Total** | **~55 min** | |
