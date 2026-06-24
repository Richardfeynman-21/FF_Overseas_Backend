# Orbit Chatbot Python Backend (FastAPI)

This is the Python-based FastAPI backend for the Fly & Flourish (ORBIT) public preview chatbot. It runs as a database-free service, storing sessions in-memory and running rate limiters locally.

## 🚀 Features

- **FastAPI-powered REST API** endpoint: `POST /api/public-chat`.
- **IP-based Rate Limiting**: Limit of 20 requests per 10 minutes per IP.
- **In-memory Session Store**: Tracks preview question counts and history per anonymous visitor session, automatically clearing sessions older than 1 hour.
- **LLM Failover**: Tries Gemini API first, falling back to OpenRouter (Gemini 2.0 Flash Free) if needed.
- **Preview limits**: Configurable free questions (default: 5), after which it instructs the visitor to sign up or log in.

---

## 🛠️ Local Setup & Running

1. **Environment Variables**:
   Create a `.env` file in the root folder with:
   ```env
   GEMINI_API_KEY="your-gemini-key"
   OPENROUTER_API_KEY="your-openrouter-key"
   FREE_QUESTION_LIMIT=5
   RATE_LIMIT_MAX=20
   ```

2. **Install Dependencies**:
   ```bash
   python3 -m venv venv
   source venv/bin/activate
   pip install -r requirements.txt
   ```

3. **Run Server**:
   ```bash
   uvicorn main:app --host 0.0.0.0 --port 8000 --reload
   ```

4. **Run using Docker Compose**:
   ```bash
   docker compose up --build -d
   ```

---

## 🚀 Deployment (AWS EC2 CI/CD)

This project has an automated GitHub Actions deployment workflow configured in `.github/workflows/deploy.yml` that SSHes into your AWS EC2 instance, pulls the latest code, and runs `docker compose up --build -d`.

### 1. Set up your EC2 Instance (Amazon Linux)
SSH into your EC2 server (as `ec2-user`) and install Docker:
```bash
# Update package manager
sudo dnf update -y     # (Use 'sudo yum update -y' if on Amazon Linux 2)

# Install Docker
sudo dnf install -y docker   # (Use 'sudo amazon-linux-extras install docker -y' if on Amazon Linux 2)

# Start and enable Docker service
sudo systemctl start docker
sudo systemctl enable docker

# Install Docker Compose plugin
sudo dnf install -y docker-compose-plugin

# Add default 'ec2-user' to the docker group (so you don't need sudo)
sudo usermod -aG docker ec2-user

# Log out and log back in to apply group changes
```

### 2. Prepare the Deployment Directory
On your EC2 server, clone the repository inside `/home/ec2-user/app`:
```bash
git clone https://github.com/Richardfeynman-21/FF_Overseas_Backend.git /home/ec2-user/app
```
Create a `.env` file inside `/home/ec2-user/app/` with your API keys:
```bash
cd /home/ec2-user/app
nano .env
```
*(Paste your API keys and save).*

### 3. Configure GitHub Repository Secrets
In your GitHub repository, go to **Settings** -> **Secrets and variables** -> **Actions** -> **New repository secret** and add:
- `EC2_HOST`: The public IP / public DNS of your EC2 instance.
- `EC2_USERNAME`: `ec2-user`
- `EC2_SSH_KEY`: The contents of your private SSH key file (`.pem`).

