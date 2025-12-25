#[cfg(target_os = "hermit")]
use arceos_rust as _;

use std::io::{self, prelude::*};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

const LOCAL_IP: &str = "0.0.0.0";
const LOCAL_PORT: u16 = 5555;

struct HttpRequest {
    method: String,
    path: String,
    query: HashMap<String, String>,
    body: String,
}

#[derive(Clone)]
struct TodoItem {
    id: u32,
    title: String,
    description: String,
    completed: bool,
    priority: u8,
    created_at: u64,
    updated_at: u64,
}

#[derive(Clone)]
struct User {
    id: u32,
    username: String,
    email: String,
    name: String,
    created_at: u64,
}

#[derive(Clone)]
struct Post {
    id: u32,
    user_id: u32,
    title: String,
    content: String,
    published: bool,
    created_at: u64,
    updated_at: u64,
}

struct AppState {
    todos: Arc<Mutex<HashMap<u32, TodoItem>>>,
    users: Arc<Mutex<HashMap<u32, User>>>,
    posts: Arc<Mutex<HashMap<u32, Post>>>,
    next_todo_id: Arc<Mutex<u32>>,
    next_user_id: Arc<Mutex<u32>>,
    next_post_id: Arc<Mutex<u32>>,
}

fn get_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl TodoItem {
    fn to_json(&self) -> String {
        format!(
            r#"{{"id":{},"title":"{}","description":"{}","completed":{},"priority":{},"created_at":{},"updated_at":{}}}"#,
            self.id,
            escape_json(&self.title),
            escape_json(&self.description),
            self.completed,
            self.priority,
            self.created_at,
            self.updated_at
        )
    }

    fn from_json(json: &str) -> Option<Self> {
        let mut id = None;
        let mut title = None;
        let mut description = None;
        let mut completed = None;
        let mut priority = None;

        let cleaned = json.trim().trim_start_matches('{').trim_end_matches('}');
        for part in cleaned.split(',') {
            let part = part.trim();
            if let Some((key, value)) = parse_json_field(part) {
                match key {
                    "id" => {
                        if let Ok(val) = value.parse::<u32>() {
                            id = Some(val);
                        }
                    }
                    "title" => {
                        title = Some(unescape_json(value.trim_matches('"')));
                    }
                    "description" => {
                        description = Some(unescape_json(value.trim_matches('"')));
                    }
                    "completed" => {
                        completed = Some(value == "true");
                    }
                    "priority" => {
                        if let Ok(val) = value.parse::<u8>() {
                            priority = Some(val);
                        }
                    }
                    _ => {}
                }
            }
        }

        Some(TodoItem {
            id: id.unwrap_or(0),
            title: title.unwrap_or_default(),
            description: description.unwrap_or_default(),
            completed: completed.unwrap_or(false),
            priority: priority.unwrap_or(0),
            created_at: get_timestamp(),
            updated_at: get_timestamp(),
        })
    }
}

impl User {
    fn to_json(&self) -> String {
        format!(
            r#"{{"id":{},"username":"{}","email":"{}","name":"{}","created_at":{}}}"#,
            self.id,
            escape_json(&self.username),
            escape_json(&self.email),
            escape_json(&self.name),
            self.created_at
        )
    }

    fn from_json(json: &str) -> Option<Self> {
        let mut id = None;
        let mut username = None;
        let mut email = None;
        let mut name = None;

        let cleaned = json.trim().trim_start_matches('{').trim_end_matches('}');
        for part in cleaned.split(',') {
            let part = part.trim();
            if let Some((key, value)) = parse_json_field(part) {
                match key {
                    "id" => {
                        if let Ok(val) = value.parse::<u32>() {
                            id = Some(val);
                        }
                    }
                    "username" => {
                        username = Some(unescape_json(value.trim_matches('"')));
                    }
                    "email" => {
                        email = Some(unescape_json(value.trim_matches('"')));
                    }
                    "name" => {
                        name = Some(unescape_json(value.trim_matches('"')));
                    }
                    _ => {}
                }
            }
        }

        Some(User {
            id: id.unwrap_or(0),
            username: username.unwrap_or_default(),
            email: email.unwrap_or_default(),
            name: name.unwrap_or_default(),
            created_at: get_timestamp(),
        })
    }
}

impl Post {
    fn to_json(&self) -> String {
        format!(
            r#"{{"id":{},"user_id":{},"title":"{}","content":"{}","published":{},"created_at":{},"updated_at":{}}}"#,
            self.id,
            self.user_id,
            escape_json(&self.title),
            escape_json(&self.content),
            self.published,
            self.created_at,
            self.updated_at
        )
    }

    fn from_json(json: &str) -> Option<Self> {
        let mut id = None;
        let mut user_id = None;
        let mut title = None;
        let mut content = None;
        let mut published = None;

        let cleaned = json.trim().trim_start_matches('{').trim_end_matches('}');
        for part in cleaned.split(',') {
            let part = part.trim();
            if let Some((key, value)) = parse_json_field(part) {
                match key {
                    "id" => {
                        if let Ok(val) = value.parse::<u32>() {
                            id = Some(val);
                        }
                    }
                    "user_id" => {
                        if let Ok(val) = value.parse::<u32>() {
                            user_id = Some(val);
                        }
                    }
                    "title" => {
                        title = Some(unescape_json(value.trim_matches('"')));
                    }
                    "content" => {
                        content = Some(unescape_json(value.trim_matches('"')));
                    }
                    "published" => {
                        published = Some(value == "true");
                    }
                    _ => {}
                }
            }
        }

        Some(Post {
            id: id.unwrap_or(0),
            user_id: user_id.unwrap_or(0),
            title: title.unwrap_or_default(),
            content: content.unwrap_or_default(),
            published: published.unwrap_or(false),
            created_at: get_timestamp(),
            updated_at: get_timestamp(),
        })
    }
}

fn parse_json_field(part: &str) -> Option<(&str, &str)> {
    if let Some(colon_pos) = part.find(':') {
        let key = part[..colon_pos].trim().trim_matches('"');
        let value = part[colon_pos + 1..].trim();
        Some((key, value))
    } else {
        None
    }
}

fn escape_json(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            _ => c.to_string(),
        })
        .collect()
}

fn unescape_json(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    _ => {
                        result.push('\\');
                        result.push(next);
                    }
                }
            } else {
                result.push('\\');
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn parse_query_string(query: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for part in query.split('&') {
        if let Some(equal_pos) = part.find('=') {
            let key = part[..equal_pos].trim();
            let value = part[equal_pos + 1..].trim();
            params.insert(key.to_string(), value.to_string());
        }
    }
    params
}

fn parse_request(buf: &[u8]) -> Option<HttpRequest> {
    let request_str = match core::str::from_utf8(buf) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let parts: Vec<&str> = request_str.split("\r\n\r\n").collect();
    let header = parts.get(0)?;
    let body = parts.get(1).unwrap_or(&"");

    let lines: Vec<&str> = header.lines().collect();
    if lines.is_empty() {
        return None;
    }

    let first_line = lines[0];
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }

    let method = parts[0].to_string();
    let full_path = parts[1].to_string();
    let mut query = HashMap::new();
    let path = if let Some(q_pos) = full_path.find('?') {
        let query_str = &full_path[q_pos + 1..];
        query = parse_query_string(query_str);
        full_path[..q_pos].to_string()
    } else {
        full_path
    };

    Some(HttpRequest {
        method,
        path,
        query,
        body: body.to_string(),
    })
}

fn send_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) -> io::Result<()> {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, PUT, DELETE, PATCH, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()?;
    Ok(())
}

fn send_html(stream: &mut TcpStream, html: &str) -> io::Result<()> {
    send_response(stream, "200 OK", "text/html; charset=utf-8", html.as_bytes())
}

fn send_json(stream: &mut TcpStream, json: &str) -> io::Result<()> {
    send_response(stream, "200 OK", "application/json", json.as_bytes())
}

fn send_error(stream: &mut TcpStream, code: u16, message: &str) -> io::Result<()> {
    let json = format!(r#"{{"error":"{}","code":{},"message":"{}"}}"#, escape_json(message), code, escape_json(message));
    let status = format!("{} {}", code, message);
    send_response(stream, &status, "application/json", json.as_bytes())
}

fn send_created(stream: &mut TcpStream, json: &str) -> io::Result<()> {
    send_response(stream, "201 Created", "application/json", json.as_bytes())
}

fn send_no_content(stream: &mut TcpStream) -> io::Result<()> {
    send_response(stream, "204 No Content", "application/json", &[])?;
    Ok(())
}

fn extract_id(path: &str, prefix: &str) -> Option<u32> {
    if path.starts_with(prefix) {
        let rest = &path[prefix.len()..];
        if rest.starts_with('/') {
            let id_str = &rest[1..];
            if let Some(slash_pos) = id_str.find('/') {
                id_str[..slash_pos].parse::<u32>().ok()
            } else {
                id_str.parse::<u32>().ok()
            }
        } else {
            None
        }
    } else {
        None
    }
}

fn apply_pagination<T, F>(items: Vec<T>, query: &HashMap<String, String>, to_json: F) -> String
where
    F: Fn(&T) -> String,
{
    let page: usize = query.get("page").and_then(|s| s.parse().ok()).unwrap_or(1);
    let per_page: usize = query.get("per_page").and_then(|s| s.parse().ok()).unwrap_or(10);
    let start = (page - 1) * per_page;

    let total = items.len();
    let paginated: Vec<String> = items.into_iter().skip(start).take(per_page).map(|item| to_json(&item)).collect();
    let json_items = paginated.join(",");
    format!(
        r#"{{"data":[{}],"pagination":{{"page":{},"per_page":{},"total":{},"total_pages":{}}}}}"#,
        json_items,
        page,
        per_page,
        total,
        (total + per_page - 1) / per_page
    )
}

fn handle_get_todos(stream: &mut TcpStream, query: &HashMap<String, String>, state: &AppState) -> io::Result<()> {
    let todos = state.todos.lock().unwrap();
    let mut items: Vec<&TodoItem> = todos.values().collect();

    if let Some(completed_str) = query.get("completed") {
        let filter_completed = completed_str == "true";
        items.retain(|item| item.completed == filter_completed);
    }

    if let Some(priority_str) = query.get("priority") {
        if let Ok(priority) = priority_str.parse::<u8>() {
            items.retain(|item| item.priority == priority);
        }
    }

    if query.contains_key("page") || query.contains_key("per_page") {
        let items_vec: Vec<TodoItem> = items.iter().map(|&item| (*item).clone()).collect();
        let json = apply_pagination(items_vec, query, |item| item.to_json());
        send_json(stream, &json)
    } else {
        let json_items: Vec<String> = items.iter().map(|item| item.to_json()).collect();
        let json = format!("[{}]", json_items.join(","));
        send_json(stream, &json)
    }
}

fn handle_get_todo(stream: &mut TcpStream, id: u32, state: &AppState) -> io::Result<()> {
    let todos = state.todos.lock().unwrap();
    match todos.get(&id) {
        Some(item) => send_json(stream, &item.to_json()),
        None => send_error(stream, 404, "Todo not found"),
    }
}

fn handle_post_todo(stream: &mut TcpStream, body: &str, state: &AppState) -> io::Result<()> {
    if let Some(mut item) = TodoItem::from_json(body) {
        if item.title.is_empty() {
            return send_error(stream, 400, "Title is required");
        }
        let mut next_id = state.next_todo_id.lock().unwrap();
        item.id = *next_id;
        *next_id += 1;
        let json = item.to_json();
        {
            let mut todos = state.todos.lock().unwrap();
            todos.insert(item.id, item);
        }
        send_created(stream, &json)
    } else {
        send_error(stream, 400, "Invalid JSON")
    }
}

fn handle_put_todo(stream: &mut TcpStream, id: u32, body: &str, state: &AppState) -> io::Result<()> {
    let mut todos = state.todos.lock().unwrap();
    if let Some(existing) = todos.get(&id) {
        if let Some(mut item) = TodoItem::from_json(body) {
            item.id = id;
            item.created_at = existing.created_at;
            item.updated_at = get_timestamp();
            todos.insert(id, item.clone());
            drop(todos);
            send_json(stream, &item.to_json())
        } else {
            drop(todos);
            send_error(stream, 400, "Invalid JSON")
        }
    } else {
        drop(todos);
        send_error(stream, 404, "Todo not found")
    }
}

fn handle_patch_todo(stream: &mut TcpStream, id: u32, body: &str, state: &AppState) -> io::Result<()> {
    let mut todos = state.todos.lock().unwrap();
    if let Some(existing) = todos.get_mut(&id) {
        if let Some(updates) = TodoItem::from_json(body) {
            if !updates.title.is_empty() {
                existing.title = updates.title;
            }
            if !updates.description.is_empty() {
                existing.description = updates.description;
            }
            existing.completed = updates.completed;
            existing.priority = updates.priority;
            existing.updated_at = get_timestamp();
            let json = existing.to_json();
            drop(todos);
            send_json(stream, &json)
        } else {
            drop(todos);
            send_error(stream, 400, "Invalid JSON")
        }
    } else {
        drop(todos);
        send_error(stream, 404, "Todo not found")
    }
}

fn handle_delete_todo(stream: &mut TcpStream, id: u32, state: &AppState) -> io::Result<()> {
    let mut todos = state.todos.lock().unwrap();
    if todos.remove(&id).is_some() {
        drop(todos);
        send_no_content(stream)
    } else {
        drop(todos);
        send_error(stream, 404, "Todo not found")
    }
}

fn handle_get_users(stream: &mut TcpStream, query: &HashMap<String, String>, state: &AppState) -> io::Result<()> {
    let users = state.users.lock().unwrap();
    let items: Vec<&User> = users.values().collect();

    if query.contains_key("page") || query.contains_key("per_page") {
        let items_vec: Vec<User> = items.iter().map(|&item| (*item).clone()).collect();
        let json = apply_pagination(items_vec, query, |item| item.to_json());
        send_json(stream, &json)
    } else {
        let json_items: Vec<String> = items.iter().map(|item| item.to_json()).collect();
        let json = format!("[{}]", json_items.join(","));
        send_json(stream, &json)
    }
}

fn handle_get_user(stream: &mut TcpStream, id: u32, state: &AppState) -> io::Result<()> {
    let users = state.users.lock().unwrap();
    match users.get(&id) {
        Some(user) => send_json(stream, &user.to_json()),
        None => send_error(stream, 404, "User not found"),
    }
}

fn handle_post_user(stream: &mut TcpStream, body: &str, state: &AppState) -> io::Result<()> {
    if let Some(mut user) = User::from_json(body) {
        if user.username.is_empty() || user.email.is_empty() {
            return send_error(stream, 400, "Username and email are required");
        }
        let mut next_id = state.next_user_id.lock().unwrap();
        user.id = *next_id;
        *next_id += 1;
        let json = user.to_json();
        {
            let mut users = state.users.lock().unwrap();
            users.insert(user.id, user);
        }
        send_created(stream, &json)
    } else {
        send_error(stream, 400, "Invalid JSON")
    }
}

fn handle_put_user(stream: &mut TcpStream, id: u32, body: &str, state: &AppState) -> io::Result<()> {
    let mut users = state.users.lock().unwrap();
    if let Some(existing) = users.get(&id) {
        if let Some(mut user) = User::from_json(body) {
            user.id = id;
            user.created_at = existing.created_at;
            users.insert(id, user.clone());
            drop(users);
            send_json(stream, &user.to_json())
        } else {
            drop(users);
            send_error(stream, 400, "Invalid JSON")
        }
    } else {
        drop(users);
        send_error(stream, 404, "User not found")
    }
}

fn handle_delete_user(stream: &mut TcpStream, id: u32, state: &AppState) -> io::Result<()> {
    let mut users = state.users.lock().unwrap();
    if users.remove(&id).is_some() {
        drop(users);
        send_no_content(stream)
    } else {
        drop(users);
        send_error(stream, 404, "User not found")
    }
}

fn handle_get_posts(stream: &mut TcpStream, query: &HashMap<String, String>, state: &AppState) -> io::Result<()> {
    let posts = state.posts.lock().unwrap();
    let mut items: Vec<&Post> = posts.values().collect();

    if let Some(user_id_str) = query.get("user_id") {
        if let Ok(user_id) = user_id_str.parse::<u32>() {
            items.retain(|post| post.user_id == user_id);
        }
    }

    if let Some(published_str) = query.get("published") {
        let filter_published = published_str == "true";
        items.retain(|post| post.published == filter_published);
    }

    if query.contains_key("page") || query.contains_key("per_page") {
        let items_vec: Vec<Post> = items.iter().map(|&item| (*item).clone()).collect();
        let json = apply_pagination(items_vec, query, |item| item.to_json());
        send_json(stream, &json)
    } else {
        let json_items: Vec<String> = items.iter().map(|item| item.to_json()).collect();
        let json = format!("[{}]", json_items.join(","));
        send_json(stream, &json)
    }
}

fn handle_get_post(stream: &mut TcpStream, id: u32, state: &AppState) -> io::Result<()> {
    let posts = state.posts.lock().unwrap();
    match posts.get(&id) {
        Some(post) => send_json(stream, &post.to_json()),
        None => send_error(stream, 404, "Post not found"),
    }
}

fn handle_post_post(stream: &mut TcpStream, body: &str, state: &AppState) -> io::Result<()> {
    if let Some(mut post) = Post::from_json(body) {
        if post.title.is_empty() || post.user_id == 0 {
            return send_error(stream, 400, "Title and user_id are required");
        }
        let users = state.users.lock().unwrap();
        if !users.contains_key(&post.user_id) {
            drop(users);
            return send_error(stream, 400, "User not found");
        }
        drop(users);
        let mut next_id = state.next_post_id.lock().unwrap();
        post.id = *next_id;
        *next_id += 1;
        let json = post.to_json();
        {
            let mut posts = state.posts.lock().unwrap();
            posts.insert(post.id, post);
        }
        send_created(stream, &json)
    } else {
        send_error(stream, 400, "Invalid JSON")
    }
}

fn handle_put_post(stream: &mut TcpStream, id: u32, body: &str, state: &AppState) -> io::Result<()> {
    let mut posts = state.posts.lock().unwrap();
    if let Some(existing) = posts.get(&id) {
        if let Some(mut post) = Post::from_json(body) {
            post.id = id;
            post.created_at = existing.created_at;
            post.updated_at = get_timestamp();
            posts.insert(id, post.clone());
            drop(posts);
            send_json(stream, &post.to_json())
        } else {
            drop(posts);
            send_error(stream, 400, "Invalid JSON")
        }
    } else {
        drop(posts);
        send_error(stream, 404, "Post not found")
    }
}

fn handle_delete_post(stream: &mut TcpStream, id: u32, state: &AppState) -> io::Result<()> {
    let mut posts = state.posts.lock().unwrap();
    if posts.remove(&id).is_some() {
        drop(posts);
        send_no_content(stream)
    } else {
        drop(posts);
        send_error(stream, 404, "Post not found")
    }
}

fn handle_options(stream: &mut TcpStream) -> io::Result<()> {
    send_response(stream, "200 OK", "application/json", &[])?;
    Ok(())
}

fn handle_api_route(stream: &mut TcpStream, request: &HttpRequest, state: &AppState) -> io::Result<()> {
    if request.path == "/api/todos" {
        match request.method.as_str() {
            "GET" => handle_get_todos(stream, &request.query, state),
            "POST" => handle_post_todo(stream, &request.body, state),
            "OPTIONS" => handle_options(stream),
            _ => send_error(stream, 405, "Method Not Allowed"),
        }
    } else if request.path.starts_with("/api/todos/") {
        if let Some(id) = extract_id(&request.path, "/api/todos") {
            match request.method.as_str() {
                "GET" => handle_get_todo(stream, id, state),
                "PUT" => handle_put_todo(stream, id, &request.body, state),
                "PATCH" => handle_patch_todo(stream, id, &request.body, state),
                "DELETE" => handle_delete_todo(stream, id, state),
                "OPTIONS" => handle_options(stream),
                _ => send_error(stream, 405, "Method Not Allowed"),
            }
        } else {
            send_error(stream, 400, "Invalid ID")
        }
    } else if request.path == "/api/users" {
        match request.method.as_str() {
            "GET" => handle_get_users(stream, &request.query, state),
            "POST" => handle_post_user(stream, &request.body, state),
            "OPTIONS" => handle_options(stream),
            _ => send_error(stream, 405, "Method Not Allowed"),
        }
    } else if request.path.starts_with("/api/users/") {
        if let Some(id) = extract_id(&request.path, "/api/users") {
            match request.method.as_str() {
                "GET" => handle_get_user(stream, id, state),
                "PUT" => handle_put_user(stream, id, &request.body, state),
                "DELETE" => handle_delete_user(stream, id, state),
                "OPTIONS" => handle_options(stream),
                _ => send_error(stream, 405, "Method Not Allowed"),
            }
        } else {
            send_error(stream, 400, "Invalid ID")
        }
    } else if request.path == "/api/posts" {
        match request.method.as_str() {
            "GET" => handle_get_posts(stream, &request.query, state),
            "POST" => handle_post_post(stream, &request.body, state),
            "OPTIONS" => handle_options(stream),
            _ => send_error(stream, 405, "Method Not Allowed"),
        }
    } else if request.path.starts_with("/api/posts/") {
        if let Some(id) = extract_id(&request.path, "/api/posts") {
            match request.method.as_str() {
                "GET" => handle_get_post(stream, id, state),
                "PUT" => handle_put_post(stream, id, &request.body, state),
                "DELETE" => handle_delete_post(stream, id, state),
                "OPTIONS" => handle_options(stream),
                _ => send_error(stream, 405, "Method Not Allowed"),
            }
        } else {
            send_error(stream, 400, "Invalid ID")
        }
    } else if request.path == "/api" {
        let json = r#"{"resources":["/api/todos","/api/users","/api/posts"],"version":"1.0.0"}"#;
        send_json(stream, json)
    } else {
        send_error(stream, 404, "Not Found")
    }
}

fn get_content_type(path: &str) -> &str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".gif") {
        "image/gif"
    } else if path.ends_with(".txt") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    }
}

fn serve_file(stream: &mut TcpStream, file_path: &str) -> io::Result<()> {
    if !Path::new(file_path).exists() {
        return send_error(stream, 404, "Not Found");
    }

    match fs::read(file_path) {
        Ok(content) => {
            let content_type = get_content_type(file_path);
            send_response(stream, "200 OK", content_type, &content)
        }
        Err(_) => send_error(stream, 500, "Internal Server Error"),
    }
}

fn handle_get(stream: &mut TcpStream, request: &HttpRequest, state: &AppState) -> io::Result<()> {
    if request.path.starts_with("/api/") {
        handle_api_route(stream, request, state)
    } else if request.path == "/" {
        let html = r#"<html>
<head><title>ArceOS RESTful API</title></head>
<body>
  <h1>ArceOS RESTful API Server</h1>
  <h2>Resources:</h2>
  <ul>
    <li><strong>Todos:</strong> /api/todos</li>
    <li><strong>Users:</strong> /api/users</li>
    <li><strong>Posts:</strong> /api/posts</li>
  </ul>
  <h2>Endpoints:</h2>
  <h3>Todos:</h3>
  <ul>
    <li>GET /api/todos - List all todos (supports ?completed=true&priority=1&page=1&per_page=10)</li>
    <li>GET /api/todos/{id} - Get a todo by ID</li>
    <li>POST /api/todos - Create a new todo</li>
    <li>PUT /api/todos/{id} - Update a todo</li>
    <li>PATCH /api/todos/{id} - Partially update a todo</li>
    <li>DELETE /api/todos/{id} - Delete a todo</li>
  </ul>
  <h3>Users:</h3>
  <ul>
    <li>GET /api/users - List all users (supports ?page=1&per_page=10)</li>
    <li>GET /api/users/{id} - Get a user by ID</li>
    <li>POST /api/users - Create a new user</li>
    <li>PUT /api/users/{id} - Update a user</li>
    <li>DELETE /api/users/{id} - Delete a user</li>
  </ul>
  <h3>Posts:</h3>
  <ul>
    <li>GET /api/posts - List all posts (supports ?user_id=1&published=true&page=1&per_page=10)</li>
    <li>GET /api/posts/{id} - Get a post by ID</li>
    <li>POST /api/posts - Create a new post</li>
    <li>PUT /api/posts/{id} - Update a post</li>
    <li>DELETE /api/posts/{id} - Delete a post</li>
  </ul>
  <h2>Example:</h2>
  <pre>curl -X POST http://localhost:5555/api/todos \
  -H "Content-Type: application/json" \
  -d '{"title":"Test todo","description":"Test description","priority":1}'</pre>
</body>
</html>"#;
        send_html(stream, html)
    } else {
        let file_path = if request.path == "/index.html" {
            "index.html"
        } else if request.path.starts_with("/static/") {
            &request.path[8..]
        } else {
            &request.path[1..]
        };
        serve_file(stream, file_path)
    }
}

fn handle_route(stream: &mut TcpStream, request: &HttpRequest, state: &AppState) -> io::Result<()> {
    match request.method.as_str() {
        "GET" => handle_get(stream, request, state),
        "POST" | "PUT" | "PATCH" | "DELETE" => {
            if request.path.starts_with("/api/") {
                handle_api_route(stream, request, state)
            } else {
                send_error(stream, 405, "Method Not Allowed")
            }
        }
        "OPTIONS" => handle_options(stream),
        "HEAD" => {
            send_response(stream, "200 OK", "text/html", &[])?;
            Ok(())
        }
        _ => send_error(stream, 405, "Method Not Allowed"),
    }
}

fn http_server(mut stream: TcpStream, state: Arc<AppState>) -> io::Result<()> {
    let mut buf = [0u8; 8192];
    let len = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return Ok(()),
    };

    if len == 0 {
        return Ok(());
    }

    if let Some(request) = parse_request(&buf[..len]) {
        handle_route(&mut stream, &request, &state)?;
    } else {
        send_error(&mut stream, 400, "Bad Request")?;
    }

    Ok(())
}

fn accept_loop(state: Arc<AppState>) -> io::Result<()> {
    let listener = TcpListener::bind((LOCAL_IP, LOCAL_PORT))?;
    println!("ArceOS RESTful API Server listening on: http://{}/", listener.local_addr().unwrap());

    let mut client_id = 0;
    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let id = client_id;
                let state_clone = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(e) = http_server(stream, state_clone) {
                        eprintln!("Client {} error: {:?}", id, e);
                    }
                });
                client_id += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

fn main() {
    println!("Starting ArceOS RESTful API Server...");
    let state = Arc::new(AppState {
        todos: Arc::new(Mutex::new(HashMap::new())),
        users: Arc::new(Mutex::new(HashMap::new())),
        posts: Arc::new(Mutex::new(HashMap::new())),
        next_todo_id: Arc::new(Mutex::new(1)),
        next_user_id: Arc::new(Mutex::new(1)),
        next_post_id: Arc::new(Mutex::new(1)),
    });
    if let Err(e) = accept_loop(state) {
        eprintln!("Server error: {:?}", e);
    }
}
