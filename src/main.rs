use anyhow::{Context, Result};
use dotenvy::dotenv;
use serde_json::Value;
use std::env;
use std::thread;

#[derive(Copy, Clone)]
struct Brand {
    id: &'static str,
    env: &'static str,
}

static BRANDS: &[Brand] = &[
    Brand {
        id: "mcom",
        env: "MCOM",
    },
    Brand {
        id: "bcom",
        env: "BCOM",
    },
];

fn brand_url(brand: &Brand) -> String {
    env::var(brand.env).expect("URL is not set (check .env)")
}

fn filter_response(arr: &Vec<Value>) -> Vec<Value> {
    arr.iter()
        .filter(|item| {
            item.get("affectedPageType")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    let is_list = obj
                        .get("type")
                        .and_then(|t| t.as_str())
                        .map(|s| s == "list")
                        .unwrap_or(false);

                    let has_pdp = obj
                        .get("value")
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().any(|x| x.as_str() == Some("PDP")))
                        .unwrap_or(false);

                    is_list && has_pdp
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn fetch(brand: &Brand) -> Result<Vec<Value>> {
    let url = brand_url(&brand);

    let text = reqwest::blocking::get(&url)
        .with_context(|| format!("GET {url} failed"))?
        .text()
        .context("reading response text failed")?;

    let json: Value = serde_json::from_str(&text).context("JSON parse failed")?;
    let arr = json
        .as_array()
        .with_context(|| "top-level JSON is not an array")?;

    let filtered = filter_response(&arr);
    Ok(filtered)
}

fn print_campaign(brand: &Brand, item: &Value) {
    let start = item
        .get("startDate")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let end = item.get("endDate").and_then(|v| v.as_str()).unwrap_or("-");
    let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("-");
    let desc = item.get("desc").and_then(|v| v.as_str()).unwrap_or("-");
    let created_by = item
        .get("createdBy")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    println!("{} -> {} - {}", brand.id, start, end);
    println!("{} - {}", name, created_by);
    println!("Description: {}", desc);

    println!("Recipes:");

    if let Some(recipes) = item.get("recipes").and_then(|r| r.as_array()) {
        for r in recipes {
            let id = r.get("id").and_then(|s| s.as_str()).unwrap_or("-");
            let weight = r.get("weight").and_then(|n| n.as_i64()).unwrap_or(0);
            println!("  {} - {}", id, weight);
        }
    } else {
        println!(" None")
    }
    println!();
}

fn main() -> Result<()> {
    dotenv().ok();

    let handles: Vec<(Brand, std::thread::JoinHandle<Result<Vec<Value>>>)> = BRANDS
        .iter()
        .copied()
        .map(|brand| (brand, thread::spawn(move || fetch(&brand))))
        .collect();

for (brand, handle) in handles {
    let items = handle
        .join()
        .expect("worker thread panicked")
        .unwrap_or_else(|e| {
            eprintln!("[{}] ERROR: {}", brand.id, e);
            Vec::new()
        });

    println!("=== {} === ({} items)\n", brand.id, items.len());
    for item in &items {
        print_campaign(&brand, item);
    }
}

    Ok(())
}
