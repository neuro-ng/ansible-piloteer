use anyhow::{Context, Result};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use url::Url;

// Default credentials (mimicking gcloud for "out of the box" experience)
const DEFAULT_CLIENT_ID: &str = "32555940559.apps.googleusercontent.com"; // GCloud CLI ID
const DEFAULT_CLIENT_SECRET: &str = "ZbB1UIWNIVi8GigZb1lQCj_F"; // Known public secret for GCloud CLI

pub async fn login(client_id: Option<String>, client_secret: Option<String>) -> Result<String> {
    let (google_client_id, google_client_secret) = if let Some(id) = client_id {
        (
            ClientId::new(id),
            Some(ClientSecret::new(client_secret.unwrap_or_default())),
        )
    } else {
        // Use default GCloud credentials
        (
            ClientId::new(DEFAULT_CLIENT_ID.to_string()),
            Some(ClientSecret::new(DEFAULT_CLIENT_SECRET.to_string())),
        )
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
    let listener = TcpListener::bind("127.0.0.1:8085")
        .context("Failed to bind to localhost:8085 for callback. Is another instance running?")?;

    // IMPORTANT: The redirect URL must match EXACTLY what is registered in the Google Cloud Console.
    // For the default GCloud CLI ID, it expects "http://localhost:8085".
    // "127.0.0.1" will often cause a "redirect_uri_mismatch" error or token exchange failure.
    let redirect_url =
        RedirectUrl::new("http://localhost:8085".to_string()).context("Invalid redirect URL")?;

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

    let (authorize_url, _csrf_state) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/userinfo.email".to_string(),
        ))
        // Profile scope causes 403 with default client ID
        // .add_scope(Scope::new("https://www.googleapis.com/auth/userinfo.profile".to_string()))
        // Add scope for Google AI if needed, e.g. "https://www.googleapis.com/auth/generative-language.retriever.readonly"
        // For general "login", email/profile is enough. If we want to use Gemini API, we might need more.
        .set_pkce_challenge(pkce_challenge)
        .url();

    println!("Opening browser to: {}", authorize_url);
    if webbrowser::open(authorize_url.as_str()).is_err() {
        println!("Failed to open browser. Please visit the URL manually.");
    }

    // Wait for the incoming request
    if let Some(stream) = listener.incoming().next() {
        let mut stream = stream.context("Failed to accept connection")?;

        // Read the request line
        let mut reader = BufReader::new(&stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;

        // "GET /?code=...&state=... HTTP/1.1"
        let redirect_url = request_line
            .split_whitespace()
            .nth(1)
            .context("Invalid HTTP request")?;

        let url = Url::parse(&format!("http://localhost{}", redirect_url))
            .context("Failed to parse redirect URL")?;

        let code_pair = url
            .query_pairs()
            .find(|(key, _)| key == "code")
            .context("No code received in redirect")?;

        let code = oauth2::AuthorizationCode::new(code_pair.1.into_owned());

        // Exchange the code for a token
        // Exchange the code for a token
        let token_result = match client
            .exchange_code(code)
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
        {
            Ok(token) => token,
            Err(e) => {
                if let oauth2::RequestTokenError::ServerResponse(r) = &e {
                    println!("OAuth Server Error: {:?}", r);
                }
                return Err(e.into());
            }
        };

        // Respond to the browser
        let response = "HTTP/1.1 200 OK\r\n\r\n<html><body>Login successful! You can close this window.</body></html>";
        stream.write_all(response.as_bytes())?;

        // Extract access token
        let access_token = token_result.access_token().secret().to_string();
        return Ok(access_token);
    }

    Err(anyhow::anyhow!("No connection received"))
}
