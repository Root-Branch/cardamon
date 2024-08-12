use reqwest::Client;
use serde_json::json;
use std::time::{Duration, Instant};
use tokio;
use tokio::time::sleep;

const BASE_URL: &str = "http://localhost:8080";
const NUM_REQUESTS: usize = 100;
const MAX_RETRIES: u32 = 10;
const RETRY_DELAY: Duration = Duration::from_secs(5);

async fn retry_request<F, Fut, T>(mut f: F) -> Result<T, Box<dyn std::error::Error>>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, Box<dyn std::error::Error>>>,
{
    let mut retries = 0;
    loop {
        match f().await {
            Ok(result) => return Ok(result),
            Err(..) if retries < MAX_RETRIES => {
                println!(
                    "Request failed. Retrying in 5 seconds... (Attempt {} of {})",
                    retries + 1,
                    MAX_RETRIES
                );
                sleep(RETRY_DELAY).await;
                retries += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    println!("Starting API endpoint tests...");

    // Test POST /notes
    let post_start = Instant::now();
    for i in 0..NUM_REQUESTS {
        let result = retry_request(|| async {
            let response = client
                .post(format!("{}/notes", BASE_URL))
                .json(&json!({
                    "id": format!("test{}", i),
                    "text": format!("Test note {}", i)
                }))
                .send()
                .await?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("Request failed with status: {}", response.status()).into())
            }
        })
        .await;

        match result {
            Ok(_) => print!("."),
            Err(_) => print!("x"),
        }
    }
    println!("\nPOST /notes: {:?}", post_start.elapsed());

    // Test GET /notes
    let get_all_start = Instant::now();
    for _ in 0..NUM_REQUESTS {
        let result = retry_request(|| async {
            let response = client.get(format!("{}/notes", BASE_URL)).send().await?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("Request failed with status: {}", response.status()).into())
            }
        })
        .await;

        match result {
            Ok(_) => print!("."),
            Err(_) => print!("x"),
        }
    }
    println!("\nGET /notes: {:?}", get_all_start.elapsed());

    // Test GET /notes/{id}
    let get_one_start = Instant::now();
    for i in 0..NUM_REQUESTS {
        let result = retry_request(|| async {
            let response = client
                .get(format!("{}/notes/test{}", BASE_URL, i % 100))
                .send()
                .await?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("Request failed with status: {}", response.status()).into())
            }
        })
        .await;

        match result {
            Ok(_) => print!("."),
            Err(_) => print!("x"),
        }
    }
    println!("\nGET /notes/{{id}}: {:?}", get_one_start.elapsed());

    // Test DELETE /notes/{id}
    let delete_start = Instant::now();
    for i in 0..NUM_REQUESTS {
        let result = retry_request(|| async {
            let response = client
                .delete(format!("{}/notes/test{}", BASE_URL, i % 100))
                .send()
                .await?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(format!("Request failed with status: {}", response.status()).into())
            }
        })
        .await;

        match result {
            Ok(_) => print!("."),
            Err(_) => print!("x"),
        }
    }
    println!("\nDELETE /notes/{{id}}: {:?}", delete_start.elapsed());

    println!("API endpoint tests completed.");
    Ok(())
}

