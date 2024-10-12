use serde_json::{json, Value};
use warp::{Filter, http::StatusCode, Rejection, Reply, reply};
use std::env;
use reqwest;
use log::{info, error};
use urlencoding::decode;
use std::borrow::Cow;

async fn send_to_discord(path: String, data: Value) -> Result<impl Reply, Rejection> {
    let discord_webhooks_env = env::var("DISCORD_WEBHOOKS").unwrap_or_else(|_| String::from("[]"));
    info!("DISCORD_WEBHOOKS (encoded): {}", discord_webhooks_env);

    let decoded_webhooks: Cow<str> = decode(&discord_webhooks_env).unwrap_or_else(|_| String::from("[]").into());
    info!("DISCORD_WEBHOOKS (decoded): {}", decoded_webhooks);

    // Ensure that the decoded JSON is properly formatted
    let webhooks: Value = serde_json::from_str(&decoded_webhooks)
        .unwrap_or_else(|err| {
            error!("Failed to parse DISCORD_WEBHOOKS: {}", err);
            error!("Decoded DISCORD_WEBHOOKS value: {}", decoded_webhooks);
            json!([])
        });
    
    info!("Parsed webhooks: {:?}", webhooks);

    let webhook_url = webhooks
        .as_array()
        .and_then(|arr| arr.iter().find(|obj| obj["path"] == path))
        .and_then(|obj| obj["url"].as_str());

    if let Some(url) = webhook_url {
        info!("Using webhook URL: {}", url);
        let client = reqwest::Client::new();

        info!("Received data: {}", data);

        let payload = if let Some(map) = data.as_object() {
            let exchange = map.get("exchange").and_then(Value::as_str).unwrap_or("");
            let ticker = map.get("ticker").and_then(Value::as_str).unwrap_or("");
            let close = map.get("close").and_then(Value::as_str).unwrap_or("");
            let open = map.get("open").and_then(Value::as_str).unwrap_or("");
            let volume = map.get("volume").and_then(Value::as_str).unwrap_or("");
            let event = map.get("event").and_then(Value::as_str).unwrap_or("");
            let interval = map.get("interval").and_then(Value::as_str).unwrap_or("");

            // Determine color based on closing and opening prices
            let color = if close < open {
                0xFF0000 // Red
            } else {
                0x00FF00 // Green
            };

            json!({
                "embeds": [{
                    "author": {
                        "name": format!("Whistle: {} {} at {}", ticker, event, exchange),
                        "url": "https://github.com/coinchimp/whistle",
                        "icon_url": "https://raw.githubusercontent.com/coinchimp/whistle/main/assets/images/whistle.png"
                    },
                    "description": format!("Open: {}\nClose: {}\nInterval: {}\nVolume: {}\n", open, close, interval, volume),
                    "color": color
                }]
            })
        } else {
            json!({
                "embeds": [{
                    "author": {
                        "name": "Whistle: Text Notification",
                        "url": "https://github.com/coinchimp/whistle",
                        "icon_url": "https://raw.githubusercontent.com/coinchimp/whistle/main/assets/images/whistle.png"
                    },
                    "description": format!("Event: {}", data),
                    "color": 0xFFC0CB // Pink
                }]
            })
        };

        // Send payload to Discord
        match client.post(url).json(&payload).send().await {
            Ok(_) => {
                info!("Message successfully sent to Discord.");
                Ok(reply::with_status("Content sent to Discord", StatusCode::OK))
            },
            Err(e) => {
                error!("Failed to send message to Discord: {:?}", e);
                Err(warp::reject::reject())
            }
        }
    } else {
        error!("No valid webhook URL found for path: {}", path);
        Err(warp::reject::reject())
    }
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    info!("Handling rejection: {:?}", err);
    Ok(reply::with_status("Not found", StatusCode::NOT_FOUND))
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let port: u16 = env::var("PORT").unwrap_or_else(|_| "8080".to_string()).parse().unwrap();

    // Define the webhook route
    let webhook_route = warp::post()
        .and(warp::path("webhook"))
        .and(warp::path::param())
        .and(warp::body::json::<Value>())
        .and_then(|path: String, data: Value| send_to_discord(path, data));

    // Define the health check route
    let health_route = warp::get()
        .and(warp::path::end())
        .map(|| "Healthy");

    // Combine the routes and set a rejection handler
    let routes = webhook_route.or(health_route)
        .recover(handle_rejection);

    // Start the server
    warp::serve(routes)
        .run(([0, 0, 0, 0], port))
        .await;
}
