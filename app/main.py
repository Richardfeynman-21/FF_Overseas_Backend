import os
import time
import asyncio
from collections import defaultdict
from contextlib import asynccontextmanager
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import BaseModel
import httpx

from app.config import settings

# Session store: sessionId -> {"messageCount": int, "history": list, "createdAt": float}
sessions = {}

# IP rate limiting store: ip -> list of timestamps
ip_requests = defaultdict(list)

async def cleanup_sessions_loop():
    while True:
        await asyncio.sleep(600)  # every 10 minutes
        now = time.time()
        ONE_HOUR = 3600
        expired_keys = [k for k, v in sessions.items() if now - v["createdAt"] > ONE_HOUR]
        for k in expired_keys:
            sessions.pop(k, None)

@asynccontextmanager
async def lifespan(app: FastAPI):
    # Start background cleanup task
    cleanup_task = asyncio.create_task(cleanup_sessions_loop())
    yield
    cleanup_task.cancel()

app = FastAPI(title="Orbit Chatbot Backend", lifespan=lifespan)

# Setup CORS
origins = ["*"]
if settings.ALLOWED_ORIGINS and settings.ALLOWED_ORIGINS != "*":
    origins = [o.strip() for o in settings.ALLOWED_ORIGINS.split(",") if o.strip()]

app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

class ChatRequest(BaseModel):
    sessionId: str
    message: str

def check_rate_limit(ip: str) -> bool:
    now = time.time()
    # filter out timestamps older than the window
    ip_requests[ip] = [t for t in ip_requests[ip] if now - t < settings.RATE_LIMIT_WINDOW]
    if len(ip_requests[ip]) >= settings.RATE_LIMIT_MAX:
        return False
    ip_requests[ip].append(now)
    return True

def build_system_prompt(limit_reached: bool) -> str:
    prompt = (
        "You are 'Fly & Flourish Assistant,' the official chat assistant for Fly & Flourish Overseas, an overseas education consultancy.\n\n"
        "About Fly & Flourish Overseas:\n"
        "- Destination Countries: UK, USA, Canada, Australia, Germany, Ireland, New Zealand, etc.\n"
        "- Core Services: Course & university shortlisting, application support, visa guidance, scholarship assistance, and pre-departure briefings.\n"
        "- Brand Tone: Warm, professional, informative, and encouraging.\n\n"
        "Scope — what you CAN answer:\n"
        "- General questions about destinations, courses, application process overview.\n"
        "- Costs and funding ranges.\n"
        "- Fly & Flourish Overseas's services and how the consultancy process works.\n\n"
        "Scope — what you MUST NOT do:\n"
        "- NO personalized eligibility verdicts.\n"
        "- NO university recommendations tailored to a specific profile.\n"
        "- NO visa-case-specific advice.\n"
        "- NO outcome guarantees (admissions, scholarships, visas).\n"
        "- For anything personal or requiring evaluation, redirect the user to \"register or log in so Fly & Flourish Overseas can access your profile and give tailored guidance.\"\n\n"
        "Style and Language rules:\n"
        "- Be concise (3-5 sentences) unless explicitly asked for more detail.\n"
        "- Maintain a warm, professional, and encouraging tone.\n"
        "- Do NOT use \"as an AI\" or similar disclaimers.\n"
        "- Language: You MUST respond ONLY in English. If the user asks a question in another language, politely reply in English stating that you can only assist in English."
    )
    if limit_reached:
        prompt += "\n\nIMPORTANT: This is the user's last free question before they must register. End your answer with a brief, friendly line inviting them to sign up or log in to keep chatting and get personalized guidance based on their profile."
    return prompt

async def call_gemini(system_prompt: str, history: list, user_message: str) -> str:
    api_key = settings.GEMINI_API_KEY
    if not api_key:
        raise ValueError("GEMINI_API_KEY is not configured.")

    # Convert history to Gemini format (user/model roles with parts)
    contents = []
    for msg in history:
        role = "model" if msg["role"] == "assistant" else msg["role"]
        contents.append({
            "role": role,
            "parts": [{"text": msg["content"]}]
        })
    # Add new user message
    contents.append({
        "role": "user",
        "parts": [{"text": user_message}]
    })

    url = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={api_key}"
    async with httpx.AsyncClient(timeout=8.0) as client:
        response = await client.post(
            url,
            headers={"Content-Type": "application/json"},
            json={
                "systemInstruction": {"parts": [{"text": system_prompt}]},
                "contents": contents,
                "generationConfig": {
                    "maxOutputTokens": 300,
                    "temperature": 0.7
                }
            }
        )

    if response.status_code != 200:
        raise Exception(f"Gemini API error: {response.status_code} {response.text}")

    data = response.json()
    try:
        reply = data["candidates"][0]["content"]["parts"][0]["text"]
        return reply
    except (KeyError, IndexError) as e:
        raise Exception(f"Failed to parse Gemini response: {e}. Response: {data}")

async def call_openrouter(system_prompt: str, history: list, user_message: str) -> str:
    api_key = settings.OPENROUTER_API_KEY
    if not api_key:
        raise ValueError("OPENROUTER_API_KEY is not configured.")

    # Format history for OpenRouter
    messages = [{"role": "system", "content": system_prompt}]
    for msg in history:
        role = "assistant" if msg["role"] == "model" else msg["role"]
        messages.append({"role": role, "content": msg["content"]})
    messages.append({"role": "user", "content": user_message})

    url = "https://openrouter.ai/api/v1/chat/completions"
    async with httpx.AsyncClient(timeout=8.0) as client:
        response = await client.post(
            url,
            headers={
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json"
            },
            json={
                "model": "openrouter/free",
                "messages": messages,
                "max_tokens": 300,
                "temperature": 0.7
            }
        )

    if response.status_code != 200:
        raise Exception(f"OpenRouter API error: {response.status_code} {response.text}")

    data = response.json()
    try:
        return data["choices"][0]["message"]["content"]
    except (KeyError, IndexError) as e:
        raise Exception(f"Failed to parse OpenRouter response: {e}. Response: {data}")

async def get_chat_reply(system_prompt: str, history: list, user_message: str) -> tuple[str, str]:
    try:
        reply = await call_gemini(system_prompt, history, user_message)
        return reply, "gemini"
    except Exception as e:
        print(f"Gemini failed, falling back to OpenRouter. Error: {e}")
        try:
            reply = await call_openrouter(system_prompt, history, user_message)
            return reply, "openrouter"
        except Exception as fallback_err:
            print(f"OpenRouter fallback failed. Error: {fallback_err}")
            raise Exception("Both Gemini and OpenRouter failed.")

@app.post("/api/public-chat")
async def chat_endpoint(request: ChatRequest, req: Request):
    # Verify API Key if configured
    if settings.FRONTEND_API_KEY:
        api_key_header = req.headers.get("X-ORBIT-API-KEY")
        if not api_key_header or api_key_header != settings.FRONTEND_API_KEY:
            return JSONResponse(
                status_code=403,
                content={"error": "Access forbidden: Invalid API Key."}
            )

    client_ip = req.client.host if req.client else "unknown"
    
    if not check_rate_limit(client_ip):
        return JSONResponse(
            status_code=429,
            content={"error": "Too many requests from this IP, please try again later."}
        )
    
    session_id = request.sessionId
    message = request.message
    
    if not session_id or not message:
        return JSONResponse(
            status_code=400,
            content={"error": "Missing sessionId or message."}
        )
        
    if session_id not in sessions:
        sessions[session_id] = {
            "messageCount": 0,
            "history": [],
            "createdAt": time.time()
        }
        
    session = sessions[session_id]
    
    if session["messageCount"] >= settings.FREE_QUESTION_LIMIT:
        return {
            "reply": "You've reached the free preview limit. Sign up or log in to keep chatting and get personalized guidance based on your profile!",
            "action": "show_auth_prompt"
        }
        
    is_last_free_question = session["messageCount"] == settings.FREE_QUESTION_LIMIT - 1
    system_prompt = build_system_prompt(is_last_free_question)
    
    try:
        reply, provider = await get_chat_reply(system_prompt, session["history"], message)
    except Exception as e:
        print(f"Error in chat reply generation: {e}")
        return JSONResponse(
            status_code=500,
            content={"error": "Something went wrong. Please try again."}
        )
        
    print(f"[PublicChat] Session: {session_id} | Provider: {provider} | Messages: {session['messageCount'] + 1}/{settings.FREE_QUESTION_LIMIT}")
    
    session["history"].append({"role": "user", "content": message})
    session["history"].append({"role": "assistant", "content": reply})
    
    if len(session["history"]) > 6:
        session["history"] = session["history"][-6:]
        
    session["messageCount"] += 1
    
    return {
        "reply": reply,
        "action": "prompt_signup_soon" if is_last_free_question else "continue",
        "remaining": max(0, settings.FREE_QUESTION_LIMIT - session["messageCount"])
    }

@app.get("/api/health")
async def health_check():
    return {
        "status": "healthy",
        "timestamp": time.time()
    }
