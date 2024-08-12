use core::time;
use reqwest::Client;
use serde_json::json;
use std::{thread, time::Instant};
use tokio;

const BASE_URL: &str = "http://localhost:8080";
const NUM_REQUESTS: usize = 1000;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    println!("Starting API endpoint tests...");

    // Test POST /notes
    let post_start = Instant::now();
    for i in 0..NUM_REQUESTS {
        let response = client
            .post(format!("{}/notes", BASE_URL))
            .json(&json!({
                "id": format!("test{}", i),
                "text": format!("Test note {}", i)
            }))
            .send()
            .await;
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    print!(".")
                } else {
                    print!("x")
                }
            }
            Err(..) => thread::sleep(time::Duration::from_secs(1)),
        }
    }
    println!("\nPOST /notes: {:?}", post_start.elapsed());

    // Test GET /notes
    let get_all_start = Instant::now();
    for _ in 0..NUM_REQUESTS {
        let response = client.get(format!("{}/notes", BASE_URL)).send().await?;

        if response.status().is_success() {
            print!(".");
        } else {
            print!("x");
        }
    }
    println!("\nGET /notes: {:?}", get_all_start.elapsed());

    // Test GET /notes/{id}
    let get_one_start = Instant::now();
    for i in 0..NUM_REQUESTS {
        let response = client
            .get(format!("{}/notes/test{}", BASE_URL, i % 100))
            .send()
            .await?;

        if response.status().is_success() {
            print!(".");
        } else {
            print!("x");
        }
    }
    println!("\nGET /notes/{{id}}: {:?}", get_one_start.elapsed());

    // Test DELETE /notes/{id}
    let delete_start = Instant::now();
    for i in 0..NUM_REQUESTS {
        let response = client
            .delete(format!("{}/notes/test{}", BASE_URL, i % 100))
            .send()
            .await?;

        if response.status().is_success() {
            print!(".");
        } else {
            print!("x");
        }
    }
    println!("\nDELETE /notes/{{id}}: {:?}", delete_start.elapsed());

    println!("API endpoint tests completed.");
    Ok(())
}
