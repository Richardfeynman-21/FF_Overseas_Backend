# AWS RDS Database Update Guide

Follow these steps to pull the latest updates from your repository and restore them to your AWS RDS database instance via your EC2 server.

## 🛠️ Prerequisites
* You need access to your private SSH key (`main.pem`) located in your `Downloads` folder.

## 🚀 Step-by-Step Update Commands

### 1. Connect to your EC2 Instance
Open your local terminal and run the following command to SSH into the server:
```bash
ssh -i ~/Downloads/main.pem ec2-user@ec2-13-234-116-233.ap-south-1.compute.amazonaws.com
```

### 2. Navigate to the App Repository Directory
Once logged into the EC2 instance, navigate to the folder where the backend code resides:
```bash
cd /home/ec2-user/app
```

### 3. Pull the Latest Database Dump from GitHub
Fetch the newly pushed dump file (`db_dumps/universities_db.dump`) from the repository:
```bash
git pull origin main
```

### 4. Copy the New Dump into the Running Docker Container
Copy the updated dump file from your EC2 host filesystem into the running backend container (`ffoverseas_backend`):
```bash
docker cp /home/ec2-user/app/db_dumps/universities_db.dump ffoverseas_backend:/app/db_dumps/universities_db.dump
```

### 5. Execute the Sync Script Inside the Container
Run the script inside the container to reset the database and restore the new dump structure directly to your Amazon RDS instance:
```bash
docker exec -it ffoverseas_backend ./sync_db_to_rds_container.sh
```

### 6. Restart the Backend Container
Restart the backend service to clear any stale database connection pools and cache:
```bash
docker restart ffoverseas_backend
```

---
*Note: The `sync_db_to_rds_container.sh` script automatically reads the RDS credentials and hostname from your EC2 container's `.env` configuration, drops the old database schema, recreates it, and performs a complete `pg_restore`.*
