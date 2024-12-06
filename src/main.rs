use actix_web::{web, App, HttpServer, HttpResponse, get};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use serde_json::json;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

// Basic data structure for pool info
#[derive(Serialize, Deserialize)]
struct PoolInfo {
    address: String,
    message: String,
}

fn create_rpc_client() -> RpcClient {
    let url = "https://api.mainnet-beta.solana.com".to_string();
    RpcClient::new_with_commitment(url, CommitmentConfig::confirmed())
}

#[get("/solana/status")]
async fn get_solana_status() -> HttpResponse {
    let rpc_client = create_rpc_client();

    match tokio::task::spawn_blocking(move || rpc_client.get_slot()).await {
        Ok(Ok(slot)) => {
            HttpResponse::Ok().json(json!({
                "status": "connected",
                "current_slot": slot
            }))
        },
        Ok(Err(e)) => {
            eprintln!("Error with RPC: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to get slot: {}", e)
            }))
        },
        Err(e) => {
            eprintln!("Task error: {}", e);
            HttpResponse:: InternalServerError().json(json!({
                "error": format!("Task failed: {}", e)
            }))
        }
    }
}

#[get("/pool/{pool_id}")]
async fn get_pool_info(pool_id: web::Path<String>) -> HttpResponse {
    let rpc_client = create_rpc_client();

    let pubkey = match Pubkey::from_str(&pool_id){
        Ok(key) => key,
        Err(e) => {
            return HttpResponse::BadRequest().json(json!({
                "error": format!("Invalid pool ID: {}", e)
            }));
        }
    };

    match tokio::task::spawn_blocking(move || rpc_client.get_account(&pubkey)).await {
        Ok(Ok(account)) => {
            HttpResponse::Ok().json(json!({
                "pool_id": pool_id.to_string(),
                "lamports": account.lamports,
                "data_size": account.data.len(),
            }))
        },
        Ok(Err(e)) => {
            eprintln!("RPC error getting account: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to get account: {}", e)
            }))
        },
        Err(e) => {
            eprintln!("Task error: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Task failed: {}", e)
            }))
        }
    }
}

#[get("/token-pair/{token_a}/{token_b}")]
async fn get_token_pair_info(path: web::Path<(String, String)>) -> HttpResponse {
    let (token_a, token_b) = path.into_inner();
    println!("Analyzing token pair: {} and {}", token_a, token_b);
    
    let rpc_client = create_rpc_client();
    
    // Convert strings to pubkeys
    let token_a_pubkey = match Pubkey::from_str(&token_a) {
        Ok(key) => key,
        Err(e) => {
            return HttpResponse::BadRequest().json(json!({
                "error": format!("Invalid token A address: {}", e)
            }));
        }
    };

    let token_b_pubkey = match Pubkey::from_str(&token_b) {
        Ok(key) => key,
        Err(e) => {
            return HttpResponse::BadRequest().json(json!({
                "error": format!("Invalid token B address: {}", e)
            }));
        }
    };

    // Get token accounts info (wrapped in spawn_blocking)
    match tokio::task::spawn_blocking(move || {
        let token_a_info = rpc_client.get_account(&token_a_pubkey)?;
        let token_b_info = rpc_client.get_account(&token_b_pubkey)?;
        Ok::<_, solana_client::client_error::ClientError>((token_a_info, token_b_info))
    }).await {
        Ok(Ok((token_a_info, token_b_info))) => {
            HttpResponse::Ok().json(json!({
                "token_a": {
                    "address": token_a,
                    "data_size": token_a_info.data.len(),
                },
                "token_b": {
                    "address": token_b,
                    "data_size": token_b_info.data.len(),
                }
            }))
        },
        Ok(Err(e)) => {
            eprintln!("RPC error getting token info: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to get token info: {}", e)
            }))
        },
        Err(e) => {
            eprintln!("Task error: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Task failed: {}", e)
            }))
        }
    }
}

#[get("/transactions/{token}")]
async fn get_token_transactions(token: web::Path<String>) -> HttpResponse {
    println!("Fetching Solscan transactions for token: {}", token);
    
    let client = reqwest::Client::new();
    let url = format!("https://public-api.solscan.io/token/transfers?token={}&limit=50", token);
    
    match client.get(&url).send().await {
        Ok(response) => {
            match response.json::<serde_json::Value>().await {
                Ok(data) => {
                    println!("Successfully got transaction data");
                    HttpResponse::Ok().json(data)
                },
                Err(e) => {
                    eprintln!("Error parsing response: {}", e);
                    HttpResponse::InternalServerError().json(json!({
                        "error": format!("Failed to parse response: {}", e)
                    }))
                }
            }
        },
        Err(e) => {
            eprintln!("Error fetching from Solscan: {}", e);
            HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to fetch from Solscan: {}", e)
            }))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server at http://127.0.0.1:3000");

    HttpServer::new(|| {
        // Set up CORS to allow requests from your JavaScript frontend
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .service(get_pool_info)
            .service(get_solana_status)
            .service(get_token_pair_info)
            .service(get_token_transactions)
    })
    .bind(("127.0.0.1", 3000))?
    .run()
    .await
}