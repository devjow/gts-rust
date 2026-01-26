use axum::{body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response};
use chrono::Local;
use std::time::Instant;

// ANSI color codes
struct Colors {
    reset: &'static str,
    dim: &'static str,
    green: &'static str,
    yellow: &'static str,
    red: &'static str,
    cyan: &'static str,
    blue: &'static str,
    magenta: &'static str,
    gray: &'static str,
}

impl Colors {
    fn new() -> Self {
        // Check if stderr is a TTY (terminal)
        let use_colors = atty::is(atty::Stream::Stderr);

        if use_colors {
            Self {
                reset: "\x1b[0m",
                dim: "\x1b[2m",
                green: "\x1b[92m",   // 2xx success
                yellow: "\x1b[93m",  // 3xx redirect
                red: "\x1b[91m",     // 4xx, 5xx errors
                cyan: "\x1b[96m",    // Method
                blue: "\x1b[94m",    // Path
                magenta: "\x1b[95m", // Duration
                gray: "\x1b[90m",    // DEBUG content
            }
        } else {
            Self {
                reset: "",
                dim: "",
                green: "",
                yellow: "",
                red: "",
                cyan: "",
                blue: "",
                magenta: "",
                gray: "",
            }
        }
    }

    fn status_color(&self, status: StatusCode) -> &'static str {
        let code = status.as_u16();
        if (200..300).contains(&code) {
            self.green
        } else if (300..400).contains(&code) {
            self.yellow
        } else {
            self.red
        }
    }
}

#[derive(Clone)]
pub struct LoggingMiddleware {
    pub verbose: u8,
}

impl LoggingMiddleware {
    pub fn new(verbose: u8) -> Self {
        Self { verbose }
    }

    #[allow(clippy::too_many_lines)]
    pub async fn handle(&self, request: Request, next: Next) -> Response {
        if self.verbose == 0 {
            return next.run(request).await;
        }

        let colors = Colors::new();
        let method = request.method().clone();
        let uri = request.uri().clone();
        let start = Instant::now();

        // For verbose == 1 (INFO level), we only log the request/response summary
        // We don't need to consume the body, so pass the request through as-is
        let response = if self.verbose >= 2 {
            // Cache request body for DEBUG logging (verbose >= 2)
            let (parts, body) = request.into_parts();
            let body_bytes = axum::body::to_bytes(body, usize::MAX).await.ok();

            // Reconstruct request with the cached body
            let request = if let Some(ref bytes) = body_bytes {
                Request::from_parts(parts, Body::from(bytes.clone()))
            } else {
                Request::from_parts(parts, Body::empty())
            };

            // Log request body at DEBUG level
            if let Some(ref bytes) = body_bytes
                && !bytes.is_empty()
            {
                let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S,%3f");
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(bytes) {
                    let body_str = serde_json::to_string_pretty(&json).unwrap_or_default();
                    eprintln!(
                        "{} - DEBUG - {}Request body:{}\n{}{}{}",
                        timestamp, colors.dim, colors.reset, colors.gray, body_str, colors.reset
                    );
                } else {
                    let body_str = String::from_utf8_lossy(bytes);
                    eprintln!(
                        "{} - DEBUG - {}Request body (raw):{}\n{}{}{}",
                        timestamp, colors.dim, colors.reset, colors.gray, body_str, colors.reset
                    );
                }
            }

            next.run(request).await
        } else {
            // verbose == 1: just pass through without consuming the body
            next.run(request).await
        };

        let duration = start.elapsed();
        let status = response.status();
        let duration_ms = duration.as_secs_f64() * 1000.0;

        // Log response at INFO level (verbose >= 1)
        // Use eprintln! directly to avoid tracing's escaping of ANSI codes
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S,%3f");
        eprintln!(
            "{} - INFO - {}{}{} {}{}{} -> {}{}{} in {}{:.1}ms{}",
            timestamp,
            colors.cyan,
            method,
            colors.reset,
            colors.blue,
            uri.path(),
            colors.reset,
            colors.status_color(status),
            status.as_u16(),
            colors.reset,
            colors.magenta,
            duration_ms,
            colors.reset
        );

        // Log response body at DEBUG level (verbose >= 2)
        if self.verbose >= 2 {
            // Extract response body
            let (parts, body) = response.into_parts();
            if let Ok(bytes) = axum::body::to_bytes(body, usize::MAX).await {
                if !bytes.is_empty() {
                    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S,%3f");
                    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        let body_str = serde_json::to_string_pretty(&json).unwrap_or_default();
                        eprintln!(
                            "{} - DEBUG - {}Response body:{}\n{}{}{}",
                            timestamp,
                            colors.dim,
                            colors.reset,
                            colors.gray,
                            body_str,
                            colors.reset
                        );
                    } else {
                        let body_str = String::from_utf8_lossy(&bytes);
                        eprintln!(
                            "{} - DEBUG - {}Response body (raw):{}\n{}{}{}",
                            timestamp,
                            colors.dim,
                            colors.reset,
                            colors.gray,
                            body_str,
                            colors.reset
                        );
                    }
                }
                return Response::from_parts(parts, Body::from(bytes));
            }
            return Response::from_parts(parts, Body::empty());
        }

        response
    }
}
