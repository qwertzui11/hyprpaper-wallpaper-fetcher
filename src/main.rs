use std::path::PathBuf;

use reqwest::header;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth = auth_key()?;
    let retry_count = 10;
    for _ in 0..retry_count {
        let download = download_photo(&auth).await;
        if download.is_ok() {
            return Ok(());
        }
        sleep(std::time::Duration::from_secs(60)).await;
    }
    // TODO: return error!
    Ok(())
}

async fn download_photo(auth: &str) -> Result<(), Box<dyn std::error::Error>> {
    // https://rust-lang-nursery.github.io/rust-cookbook/web/clients/apis.html
    let request_url =
        "https://api.unsplash.com/photos/random?topics=wallpapers&orientation=landscape";
    println!("getting {}", request_url);
    let mut headers = header::HeaderMap::new();
    headers.insert(header::AUTHORIZATION, auth.parse()?);
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    let random = do_request(&client, request_url).await?;
    let random = random.json::<RandomResponse>().await?;
    let photo = random.urls.full;
    println!("getting: {}", photo);
    let photo = do_request(&client, &photo).await?;
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

async fn do_request(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let response = client.get(url).send().await?;
    if response.status() != 200 {
        // TODO: do an error!
        println!("random.status(): {}", response.status());
    }
    Ok(response)
}

#[derive(Debug, serde::Deserialize)]
struct RandomResponse {
    #[allow(dead_code)]
    width: i32,
    #[allow(dead_code)]
    height: i32,
    urls: PhotoSrcResponse,
}

#[derive(Debug, serde::Deserialize)]
struct PhotoSrcResponse {
    full: String,
}

fn api_key() -> Result<String, Box<dyn std::error::Error>> {
    let config_dir = dirs::config_dir().ok_or("could not get config-dir")?;
    let api_file = config_dir.join("unsplash-key");
    let api_key = std::fs::read_to_string(api_file)?;
    let api_key = api_key.trim().to_owned();
    Ok(api_key)
}

fn auth_key() -> Result<String, Box<dyn std::error::Error>> {
    api_key().map(|k| format!("Client-ID {}", k))
}

fn photo_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let result = dirs::picture_dir().ok_or("could not get picture-dir")?;
    let result = result.join("hyprpaper-wallpaper.jpeg");
    let result = std::path::absolute(result).unwrap();
    Ok(result)
}
