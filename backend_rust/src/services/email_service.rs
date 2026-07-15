use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};
use crate::config::Config;
use crate::errors::AppError;

#[derive(Clone)]
pub struct EmailService {
    transport: Option<SmtpTransport>,
    from_email: String,
}

impl std::fmt::Debug for EmailService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailService")
            .field("transport_enabled", &self.transport.is_some())
            .field("from_email", &self.from_email)
            .finish()
    }
}

impl EmailService {
    pub fn new(config: &Config) -> Self {
        let from_email = config
            .smtp_user
            .clone()
            .unwrap_or_else(|| config.admin_email.clone());

        let smtp_host = config.smtp_host.as_ref().filter(|s| !s.trim().is_empty());
        let smtp_port = config.smtp_port;
        let smtp_user = config.smtp_user.as_ref().filter(|s| !s.trim().is_empty());
        let smtp_pass = config.smtp_pass.as_ref().filter(|s| !s.trim().is_empty());

        if let (Some(host), Some(port)) = (smtp_host, smtp_port) {
            tracing::info!("Initializing SMTP transport for {}:{}", host, port);
            let builder = if smtp_user.is_some() && smtp_pass.is_some() {
                SmtpTransport::relay(host)
            } else {
                Ok(SmtpTransport::builder_dangerous(host))
            };

            match builder {
                Ok(mut b) => {
                    b = b.port(port);
                    if let (Some(user), Some(pass)) = (smtp_user, smtp_pass) {
                        let creds = Credentials::new(user.to_string(), pass.to_string());
                        b = b.credentials(creds);
                    }
                    Self {
                        transport: Some(b.build()),
                        from_email,
                    }
                }
                Err(err) => {
                    tracing::error!(
                        "Failed to create SMTP transport builder: {}. Email sending will run in log-only mode.",
                        err
                    );
                    Self {
                        transport: None,
                        from_email,
                    }
                }
            }
        } else {
            tracing::info!("SMTP configuration is incomplete. Email service running in log-only mode.");
            Self {
                transport: None,
                from_email,
            }
        }
    }

    fn base_template(title: &str, content: &str) -> String {
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            background-color: #f4f6f8;
            margin: 0;
            padding: 0;
            color: #333333;
        }}
        .wrapper {{
            width: 100%;
            background-color: #f4f6f8;
            padding: 20px 0;
        }}
        .container {{
            max-width: 600px;
            margin: 0 auto;
            background-color: #ffffff;
            border-radius: 8px;
            overflow: hidden;
            box-shadow: 0 4px 6px rgba(0,0,0,0.05);
        }}
        .header {{
            background-color: #1e3a8a;
            color: #ffffff;
            padding: 30px 20px;
            text-align: center;
        }}
        .header h1 {{
            margin: 0;
            font-size: 24px;
            font-weight: 600;
        }}
        .content {{
            padding: 30px 20px;
            line-height: 1.6;
        }}
        .footer {{
            background-color: #f8fafc;
            padding: 20px;
            text-align: center;
            font-size: 12px;
            color: #64748b;
            border-top: 1px solid #e2e8f0;
        }}
        .button {{
            display: inline-block;
            background-color: #2563eb;
            color: #ffffff !important;
            padding: 12px 24px;
            text-decoration: none;
            border-radius: 4px;
            font-weight: 600;
            margin-top: 20px;
            text-align: center;
        }}
        .status-badge {{
            display: inline-block;
            padding: 6px 12px;
            background-color: #dbeafe;
            color: #1e40af;
            border-radius: 9999px;
            font-weight: 600;
            font-size: 14px;
        }}
    </style>
</head>
<body>
    <div class="wrapper">
        <div class="container">
            <div class="header">
                <h1>Fly & Flourish Overseas</h1>
            </div>
            <div class="content">
                {}
            </div>
            <div class="footer">
                <p>&copy; 2026 Fly & Flourish Overseas. All rights reserved.</p>
                <p>If you have any questions, reply to this email or contact support.</p>
            </div>
        </div>
    </div>
</body>
</html>"#,
            title, content
        )
    }

    pub async fn send_email(&self, to_email: &str, subject: &str, body: String) -> Result<(), AppError> {
        let from_mailbox: Mailbox = match self.from_email.parse() {
            Ok(m) => m,
            Err(e) => {
                return Err(AppError::Internal(anyhow::anyhow!("Invalid from email address: {}", e)));
            }
        };

        let to_mailbox: Mailbox = match to_email.parse() {
            Ok(m) => m,
            Err(e) => {
                return Err(AppError::BadRequest(format!("Invalid recipient email address '{}': {}", to_email, e)));
            }
        };

        let message = match Message::builder()
            .from(from_mailbox)
            .to(to_mailbox)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body.clone())
        {
            Ok(msg) => msg,
            Err(e) => {
                return Err(AppError::Internal(anyhow::anyhow!("Failed to build email message: {}", e)));
            }
        };

        if let Some(ref transport) = self.transport {
            let transport = transport.clone();
            tokio::task::spawn_blocking(move || {
                transport.send(&message)
            })
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Spawn blocking join error: {}", e)))?
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to send email via SMTP: {}", e)))?;
            
            tracing::info!("Email successfully sent to {}", to_email);
        } else {
            tracing::info!(
                "LOG-ONLY EMAIL SENT:\nTo: {}\nSubject: {}\nBody Preview: {}...",
                to_email,
                subject,
                if body.len() > 200 { &body[..200] } else { &body }
            );
        }

        Ok(())
    }

    pub async fn send_welcome_email(&self, to_email: &str, name: &str) -> Result<(), AppError> {
        let body_html = Self::base_template(
            "Welcome to Fly & Flourish!",
            &format!(
                r#"<h2>Welcome to Fly & Flourish Overseas, {}!</h2>
<p>We are thrilled to welcome you to our platform. Your journey to study abroad starts here!</p>
<p>Our dedicated agents are ready to assist you in finding the perfect university and course, managing your applications, and guiding you through every step of the visa process.</p>
<p>Get started by logging into your portal to complete your profile and upload your documents.</p>
<div style="text-align: center;">
    <a href="https://ffoverseas.com/login" class="button">Access Your Portal</a>
</div>
<p style="margin-top: 30px;">Best regards,<br>The Fly & Flourish Team</p>"#,
                name
            ),
        );

        self.send_email(to_email, "Welcome to Fly & Flourish Overseas!", body_html).await
    }

    pub async fn send_status_change_email(
        &self,
        to_email: &str,
        name: &str,
        university_name: &str,
        course_name: &str,
        new_status: &str,
    ) -> Result<(), AppError> {
        let body_html = Self::base_template(
            "Application Status Update",
            &format!(
                r#"<h2>Hello, {}!</h2>
<p>There has been an update to one of your applications.</p>
<div style="background-color: #f8fafc; border-left: 4px solid #2563eb; padding: 15px; margin: 20px 0;">
    <p style="margin: 5px 0;"><strong>University:</strong> {}</p>
    <p style="margin: 5px 0;"><strong>Course:</strong> {}</p>
    <p style="margin: 15px 0 5px 0;"><strong>New Status:</strong> <span class="status-badge">{}</span></p>
</div>
<p>Please log in to your portal for detailed information regarding this update or to take any required next steps.</p>
<div style="text-align: center;">
    <a href="https://ffoverseas.com/login" class="button">View Application</a>
</div>
<p style="margin-top: 30px;">Best regards,<br>The Fly & Flourish Team</p>"#,
                name, university_name, course_name, new_status
            ),
        );

        self.send_email(
            to_email,
            &format!("Update on your application: {} - {}", university_name, course_name),
            body_html,
        )
        .await
    }

    pub async fn send_password_reset_email(&self, to_email: &str, name: &str, token: &str) -> Result<(), AppError> {
        let body_html = Self::base_template(
            "Reset Your Password",
            &format!(
                r#"<h2>Hello, {}!</h2>
<p>We received a request to reset your password. If you did not make this request, you can safely ignore this email.</p>
<p>To reset your password, please click the button below. This link will expire shortly.</p>
<div style="text-align: center;">
    <a href="https://ffoverseas.com/reset-password?token={}" class="button">Reset Password</a>
</div>
<p style="margin-top: 20px; font-size: 12px; color: #64748b;">If the button above does not work, copy and paste this URL into your browser:<br>
https://ffoverseas.com/reset-password?token={}</p>
<p style="margin-top: 30px;">Best regards,<br>The Fly & Flourish Team</p>"#,
                name, token, token
            ),
        );

        self.send_email(to_email, "Reset your Fly & Flourish password", body_html).await
    }
}
