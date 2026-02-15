use anyhow::{Context, Result};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use tracing::{error, trace};

// use std::io; // Unused
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use url::Url;

// Default credentials (mimicking gcloud for "out of the box" experience)
// Default credentials (mimicking gcloud for "out of the box" experience)
// Default credentials (mimicking user provided ID)
// Default credentials (mimicking user provided ID)
// Using option_env! to inject secrets at compile time for release builds
// Default credentials (mimicking user provided ID)
// Using option_env! to inject secrets at compile time for release builds
const DEFAULT_CLIENT_ID: &str = if let Some(id) = option_env!("PILOTEER_GOOGLE_CLIENT_ID") {
    id
} else {
    "88963378611-69o10smqtq35mbdnsl8rrl64sf2fi4qt.apps.googleusercontent.com"
};

const DEFAULT_CLIENT_SECRET: &str =
    if let Some(secret) = option_env!("PILOTEER_GOOGLE_CLIENT_SECRET") {
        secret
    } else {
        ""
    };

pub async fn login(client_id: Option<String>, client_secret: Option<String>) -> Result<String> {
    let (google_client_id, google_client_secret, is_custom_creds) = if let Some(id) = client_id {
        (
            ClientId::new(id),
            Some(ClientSecret::new(client_secret.unwrap_or_default())),
            true,
        )
    } else {
        // Use default credentials — no client_secret for PKCE-only flow
        let secret = if DEFAULT_CLIENT_SECRET.is_empty() {
            None // PKCE-only: omit client_secret entirely
        } else {
            Some(ClientSecret::new(DEFAULT_CLIENT_SECRET.to_string()))
        };
        (ClientId::new(DEFAULT_CLIENT_ID.to_string()), secret, false)
    };

    let auth_url = AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
        .context("Invalid authorization URL")?;
    let token_url = TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
        .context("Invalid token URL")?;

    // Bind to a local port to handle the redirect
    // Use port 0 to let OS assign a free port, but Google Cloud Console usually requires exact redirect URIs.
    // Assuming 8085 or similar is configured in Google Cloud Console.
    // For robust flexible implementation, one would ideally iterate ports or use localhost:0 if allowed.
    // Let's try 8085 as a default convention for this tools.
    // Use port 8085 as default convention.
    // Bind to 0.0.0.0 to allow connections from outside localhost (e.g. if running in container/VM)
    // The redirect URI sent to Google must still be localhost:8085 (or whatever is registered)
    // but the local listener needs to accept the connection.
    let listener = TcpListener::bind("0.0.0.0:8085")
        .await
        .context("Failed to bind to 0.0.0.0:8085 for callback. Is another instance running?")?;

    // IMPORTANT: The redirect URL must match EXACTLY what is registered in the Google Cloud Console.
    // For the default GCloud CLI ID, it expects "http://localhost:8085".
    // "127.0.0.1" will often cause a "redirect_uri_mismatch" error or token exchange failure.
    let redirect_url = RedirectUrl::new("http://localhost:8085/oauth2callback".to_string())
        .context("Invalid redirect URL")?;

    let client = BasicClient::new(
        google_client_id,
        google_client_secret,
        auth_url,
        Some(token_url),
    )
    .set_auth_type(oauth2::AuthType::RequestBody)
    .set_redirect_uri(redirect_url);

    // Generate the PKCE challenge and authorization URL.
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut auth_request = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/generative-language.retriever".to_string(),
        ));

    // Request 'cloud-platform' scope only if using custom credentials
    if is_custom_creds {
        auth_request = auth_request.add_scope(Scope::new(
            "https://www.googleapis.com/auth/cloud-platform".to_string(),
        ));
    }

    let (authorize_url, _csrf_state) = auth_request.set_pkce_challenge(pkce_challenge).url();

    println!("Opening browser to: {}", authorize_url);
    if webbrowser::open(authorize_url.as_str()).is_err() {
        println!("Failed to open browser. Please visit the URL manually.");
    }

    // Wait for the incoming request
    // Loop to handle potential spurious connections
    // Wait for the incoming request OR manual input
    println!("Waiting for authentication...");
    println!("1. If the browser redirected successfully, the login will complete automatically.");
    println!(
        "2. If the connection failed (e.g. WSL/Remote), COPY the 'code' parameter from the URL in your browser."
    );
    println!("   Paste it here and press Enter:");

    // We need to find the code.
    let mut code_str = String::new();

    // Stdin reader
    let mut stdin_reader = BufReader::new(tokio::io::stdin());
    let mut stdin_line = String::new();

    trace!("Entering event loop...");
    loop {
        tokio::select! {
            // Option A: HTTP Callback
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((mut stream, addr)) => {
                        trace!("Accepted connection from: {}", addr);
                        let mut reader = BufReader::new(&mut stream);
                        let mut request_line = String::new();
                        if reader.read_line(&mut request_line).await.is_err() {
                            trace!("Failed to read request line");
                           continue;
                        }
                        trace!("Request Line: {}", request_line.trim());

                        let parts: Vec<&str> = request_line.split_whitespace().collect();
                        if parts.len() < 2 {
                            trace!("Invalid request line parts: {:?}", parts);
                            continue;
                        }
                        let redirect_path = parts[1];
                        trace!("Path: {}", redirect_path);

                        if !redirect_path.contains("code=") {
                            trace!("Path does not contain code=");
                            continue;
                        }

                        // Parse code
                         match Url::parse(&format!("http://localhost{}", redirect_path)) {
                            Ok(url) => {
                                if let Some((_, code_val)) = url.query_pairs().find(|(k, _)| k == "code") {
                                    trace!("Found code in URL");
                                    code_str = code_val.into_owned();

                                    // Send Response
                                    let response = "HTTP/1.1 200 OK\r\n\r\n<html><body>Login successful! You can return to the terminal.</body></html>";
                                    let _ = stream.write_all(response.as_bytes()).await;
                                    let _ = stream.flush().await; // Ensure sent
                                    break;
                                } else {
                                    trace!("Code query param not found in URL");
                                }
                            }
                            Err(e) => {
                                trace!("Failed to parse URL: {}", e);
                                continue
                            },
                         }
                    }
                    Err(e) => error!("Connection error: {}", e),
                }
            }

            // Option B: Manual Input
            _ = stdin_reader.read_line(&mut stdin_line) => {
                let trimmed = stdin_line.trim();
                if !trimmed.is_empty() {
                    trace!("Reading manual input: {}", trimmed);
                    // Start of manual code handling
                    // It can be just the code, or the full URL.
                    // Let's assume user might paste full URL too.
                    if trimmed.contains("code=") {
                         match Url::parse(trimmed) {
                             Ok(url) => {
                                 if let Some((_, code_val)) = url.query_pairs().find(|(k, _)| k == "code") {
                                     code_str = code_val.into_owned();
                                     break;
                                 }
                             }
                             Err(_) => {
                                 // Maybe it's a partial query string? "?code=..."
                                 if let Ok(url) = Url::parse(&format!("http://localhost/{}", trimmed))
                                    && let Some((_, code_val)) = url.query_pairs().find(|(k, _)| k == "code")
                                 {
                                     code_str = code_val.into_owned();
                                     break;
                                 }
                             }
                         }
                    }

                    // If simple string, assume it is the code
                    if !code_str.is_empty() { break; } // already found above
                    code_str = trimmed.to_string();
                    trace!("Using manual input as code");
                    break;
                }
                stdin_line.clear();
            }
        }
    }

    println!("Authenticating with provider...");
    let code = oauth2::AuthorizationCode::new(code_str);

    // Exchange the code for a token
    let token_result = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_verifier)
        // Use async http client
        .request_async(oauth2::reqwest::async_http_client)
        .await
        .map_err(|e| {
            // Log detailed error information
            match &e {
                oauth2::RequestTokenError::ServerResponse(err) => {
                    let msg = format!(
                        "OAuth Server Error: error='{}', description='{:?}', uri='{:?}'",
                        err.error(),
                        err.error_description(),
                        err.error_uri()
                    );
                    error!("{}", msg);
                    anyhow::anyhow!("Failed to exchange code for token: {}", msg)
                }
                oauth2::RequestTokenError::Request(req_err) => {
                    error!("OAuth Request Error: {}", req_err);
                    anyhow::anyhow!("Failed to exchange code for token: {}", e)
                }
                oauth2::RequestTokenError::Parse(parse_err, body) => {
                    error!(
                        "OAuth Parse Error: {} Body: {:?}",
                        parse_err,
                        String::from_utf8_lossy(body)
                    );
                    anyhow::anyhow!("Failed to exchange code for token: {}", e)
                }
                oauth2::RequestTokenError::Other(msg) => {
                    error!("OAuth Other Error: {}", msg);
                    anyhow::anyhow!("Failed to exchange code for token: {}", e)
                }
            }
        })?;

    let access_token = token_result.access_token().secret().to_string();
    Ok(access_token)
}

/// Try to get an access token from gcloud CLI (Application Default Credentials).
/// This is a fallback when OAuth flow fails or when user has gcloud configured.
pub async fn get_gcloud_token() -> Result<String> {
    let output = tokio::process::Command::new("gcloud")
        .args(["auth", "application-default", "print-access-token"])
        .output()
        .await
        .context("Failed to run gcloud. Is Google Cloud SDK installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "gcloud auth failed: {}\n\nRun: gcloud auth application-default login",
            stderr.trim()
        );
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        anyhow::bail!("gcloud returned empty token. Run: gcloud auth application-default login");
    }
    Ok(token)
}

/// Try to get an access token by reading the Application Default Credentials file directly.
/// This is useful in IDE environments (like Antigravity) where gcloud CLI may not be on PATH
/// but ADC credentials have already been provisioned.
pub async fn get_adc_token() -> Result<String> {
    // Check GOOGLE_APPLICATION_CREDENTIALS first, then default ADC path
    let adc_path = if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
        std::path::PathBuf::from(path)
    } else {
        let home = std::env::var("HOME").context("HOME not set")?;
        std::path::PathBuf::from(home)
            .join(".config")
            .join("gcloud")
            .join("application_default_credentials.json")
    };

    if !adc_path.exists() {
        anyhow::bail!(
            "ADC file not found at {}. Run: gcloud auth application-default login",
            adc_path.display()
        );
    }

    let content = tokio::fs::read_to_string(&adc_path)
        .await
        .context("Failed to read ADC file")?;

    let creds: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse ADC JSON")?;

    // Service account key files have "type": "service_account" and need a JWT exchange.
    // Authorized user credentials have "type": "authorized_user" with a refresh token.
    let cred_type = creds
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");

    match cred_type {
        "authorized_user" => {
            // Exchange refresh token for access token
            let client_id = creds["client_id"]
                .as_str()
                .context("Missing client_id in ADC")?;
            let client_secret = creds["client_secret"]
                .as_str()
                .context("Missing client_secret in ADC")?;
            let refresh_token = creds["refresh_token"]
                .as_str()
                .context("Missing refresh_token in ADC")?;

            let http_client = reqwest::Client::new();
            let resp = http_client
                .post("https://oauth2.googleapis.com/token")
                .form(&[
                    ("client_id", client_id),
                    ("client_secret", client_secret),
                    ("refresh_token", refresh_token),
                    ("grant_type", "refresh_token"),
                ])
                .send()
                .await
                .context("Failed to exchange ADC refresh token")?;

            if !resp.status().is_success() {
                let err = resp.text().await.unwrap_or_default();
                anyhow::bail!("ADC token refresh failed: {}", err);
            }

            let body: serde_json::Value = resp.json().await?;
            let token = body["access_token"]
                .as_str()
                .context("No access_token in refresh response")?;
            Ok(token.to_string())
        }
        "service_account" => {
            // For service accounts, delegate to gcloud which handles JWT signing
            get_gcloud_token().await
        }
        _ => {
            anyhow::bail!("Unsupported ADC credential type: {}", cred_type);
        }
    }
}
