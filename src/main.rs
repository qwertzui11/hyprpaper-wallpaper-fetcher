use std::{path::PathBuf, str::FromStr};

use rand::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = api_key()?;
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
    let request_url = format!(
        "https://api.pexels.com/v1/search?query={query}&size=large&orientation=landscape&per_page=80",
        query = "wallpaper"
    );
    println!("getting {}", request_url);
    let client = reqwest::Client::new();
    let search = client
        .get(request_url)
        .header(reqwest::header::AUTHORIZATION, api_key.clone())
        .send()
        .await?;
    let search = search.json::<SearchResponse>().await?;
    let photo = search
        .photos
        .choose(&mut thread_rng())
        .ok_or("no photots in search-response")?
        .src
        .original
        .clone();
    println!("getting: {}", photo);
    let photo = client
        .get(photo)
        .header(reqwest::header::AUTHORIZATION, api_key)
        .send()
        .await?;
    let photo = photo.bytes().await?;
    let photo_file = photo_file_path()?;
    std::fs::write(photo_file.clone(), photo)?;
    println!("setting: {:?}", photo_file);
    println!(
        "{:?}",
        std::process::Command::new("hyprctl")
            .arg("hyprpaper")
            .arg("unload")
            .arg(photo_file.clone())
            .output()?
    );
    println!(
        "{:?}",
        std::process::Command::new("hyprctl")
            .arg("hyprpaper")
            .arg("preload")
            .arg(photo_file.clone())
            .output()?
    );
    println!(
        "{:?}",
        std::process::Command::new("hyprctl")
            .arg("hyprpaper")
            .arg("wallpaper")
            .arg(format!(",{}", photo_file.to_str().unwrap()))
            .output()?
    );
    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    photos: Vec<PhotosResponse>,
}

#[derive(Debug, serde::Deserialize)]
struct PhotosResponse {
    #[allow(dead_code)]
    id: u64,
    #[allow(dead_code)]
    width: i32,
    #[allow(dead_code)]
    height: i32,
    #[allow(dead_code)]
    alt: String,
    src: PhotoSrcResponse,
}

#[derive(Debug, serde::Deserialize)]
struct PhotoSrcResponse {
    original: String,
}

fn api_key() -> Result<String, Box<dyn std::error::Error>> {
    let config_dir = dirs::config_dir().ok_or("could not get config-dir")?;
    let api_file = config_dir.join("pexel-hyprpaper-key");
    let api_key = std::fs::read_to_string(api_file)?;
    let api_key = api_key.trim().to_owned();
    println!("api_key: {api_key}");
    Ok(api_key)
}

fn photo_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let result = dirs::picture_dir().ok_or("could not get picture-dir")?;
    let result = result.join("hyprpaper-wallpaper.jpeg");
    let result = std::path::absolute(result).unwrap();
    Ok(result)
}
