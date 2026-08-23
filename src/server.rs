use crate::tools::sandbox;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

pub const PORT: u16 = 8888;

/// Serve the working directory read-only on localhost, in the background.
/// Bound to 127.0.0.1 on purpose: the served tree is the same one the tools
/// can write to, so it must not be reachable from the network.
pub fn spawn() -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", PORT))
        .map_err(|e| format!("cannot listen on 127.0.0.1:{PORT}: {e}"))?;

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let _ = handle(stream);
            });
        }
    });

    Ok(())
}

fn handle(mut stream: TcpStream) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));

    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    // Drain the headers so the client sees a clean response rather than a reset.
    let mut header = String::new();
    while reader.read_line(&mut header)? > 0 && header.trim() != "" {
        header.clear();
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");

    let head_only = match method {
        "GET" => false,
        "HEAD" => true,
        _ => return error(&mut stream, "405 Method Not Allowed", "read-only server", false),
    };

    let url_path = target.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode(url_path);
    let relative = decoded.trim_start_matches('/');
    let request = if relative.is_empty() { "." } else { relative };

    let path = match sandbox::confine(request) {
        Ok(path) => path,
        Err(_) => return error(&mut stream, "403 Forbidden", "outside the served directory", head_only),
    };

    if !path.exists() {
        return error(&mut stream, "404 Not Found", &decoded, head_only);
    }

    if path.is_dir() {
        // Relative links in a listing only resolve correctly under a trailing slash.
        if !url_path.ends_with('/') {
            return redirect(&mut stream, &format!("{url_path}/"));
        }
        let index = path.join("index.html");
        if index.is_file() {
            return send_file(&mut stream, &index, head_only);
        }
        let body = listing(&path, &decoded);
        return respond(&mut stream, "200 OK", "text/html; charset=utf-8", body.as_bytes(), head_only);
    }

    send_file(&mut stream, &path, head_only)
}

fn send_file(stream: &mut TcpStream, path: &Path, head_only: bool) -> std::io::Result<()> {
    match std::fs::read(path) {
        Ok(bytes) => respond(stream, "200 OK", content_type(path), &bytes, head_only),
        Err(e) => error(stream, "403 Forbidden", &e.to_string(), head_only),
    }
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    if !head_only {
        stream.write_all(body)?;
    }
    stream.flush()
}

fn redirect(stream: &mut TcpStream, location: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 301 Moved Permanently\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()
}

fn error(stream: &mut TcpStream, status: &str, detail: &str, head_only: bool) -> std::io::Result<()> {
    let body = format!("{status}\n{detail}\n");
    respond(stream, status, "text/plain; charset=utf-8", body.as_bytes(), head_only)
}

fn listing(dir: &Path, url_path: &str) -> String {
    let mut rows: Vec<(bool, String)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let is_dir = entry.path().is_dir();
            rows.push((is_dir, entry.file_name().to_string_lossy().into_owned()));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase())));

    let title = escape(url_path);
    let mut out = format!(
        "<!doctype html><meta charset=utf-8><title>{title}</title>\
         <style>body{{font:14px monospace;margin:2rem}}a{{text-decoration:none}}</style>\
         <h1>{title}</h1><ul>"
    );
    if url_path != "/" {
        out.push_str("<li><a href=\"..\">../</a></li>");
    }
    for (is_dir, name) in rows {
        let slash = if is_dir { "/" } else { "" };
        let safe = escape(&name);
        out.push_str(&format!("<li><a href=\"{safe}{slash}\">{safe}{slash}</a></li>"));
    }
    out.push_str("</ul>");
    out
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        "txt" | "md" | "toml" | "rs" | "log" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
