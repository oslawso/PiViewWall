use serde::Deserialize;
use std::error::Error;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRect {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl WindowRect {
    pub fn from_string(value: &str) -> Result<Self, String> {
        let parts: Vec<&str> = value.split(',').map(str::trim).collect();
        if parts.len() != 4 {
            return Err(format!(
                "window must be in the format left,top,right,bottom: got '{value}'"
            ));
        }

        let left = parse_u32(parts[0], "left")?;
        let top = parse_u32(parts[1], "top")?;
        let right = parse_u32(parts[2], "right")?;
        let bottom = parse_u32(parts[3], "bottom")?;

        Ok(Self {
            left,
            top,
            right,
            bottom,
        })
    }
}

impl<'de> Deserialize<'de> for WindowRect {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        WindowRect::from_string(&value).map_err(serde::de::Error::custom)
    }
}

fn parse_u32(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("window {label} is not a valid u32: '{value}'"))
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppConfig {
    pub name: String,
    pub room: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DisplayConfig {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct FeedConfig {
    pub name: String,
    pub url: String,
    pub window: WindowRect,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CameraConfig {
    pub feed: Vec<FeedConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    pub app: AppConfig,
    pub display: DisplayConfig,
    pub cameras: CameraConfig,
}

impl Config {
    pub fn from_str(contents: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(contents)
    }

    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, Box<dyn Error>> {
        let contents = fs::read_to_string(path)?;
        Ok(Self::from_str(&contents)?)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.app.name.trim().is_empty() {
            return Err("app.name cannot be empty".to_string());
        }

        if self.app.room.trim().is_empty() {
            return Err("app.room cannot be empty".to_string());
        }

        if self.display.width == 0 || self.display.height == 0 {
            return Err("display width and height must be greater than zero".to_string());
        }

        if self.cameras.feed.is_empty() {
            return Err("at least one feed must be configured".to_string());
        }

        for feed in &self.cameras.feed {
            if feed.name.trim().is_empty() {
                return Err("feed.name cannot be empty".to_string());
            }

            if feed.url.trim().is_empty() {
                return Err(format!("feed '{}' has an empty URL", feed.name));
            }

            if feed.window.left >= feed.window.right || feed.window.top >= feed.window.bottom {
                return Err(format!(
                    "feed '{}' window must be a valid rectangle: {:?}",
                    feed.name, feed.window
                ));
            }

            if feed.window.right > self.display.width || feed.window.bottom > self.display.height {
                return Err(format!(
                    "feed '{}' window exceeds display bounds ({}x{}): {:?}",
                    feed.name,
                    self.display.width,
                    self.display.height,
                    feed.window
                ));
            }
        }

        Ok(())
    }

    pub fn print_summary(&self) {
        println!("Camera wall: {} ({})", self.app.name, self.app.room);
        println!(
            "Display: {}x{} (fullscreen={})",
            self.display.width, self.display.height, self.display.fullscreen
        );
        println!("Configured feeds: {}", self.cameras.feed.len());

        for feed in &self.cameras.feed {
            println!(
                "- {}: {} | window {:?}",
                feed.name, feed.url, feed.window
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_camera_wall_config() {
        let config = Config::from_str(
            r#"
[app]
name = "PiViewWall"
room = "main"

[display]
width = 1920
height = 1080
fullscreen = true

[cameras]
[[cameras.feed]]
name = "FrontDoor"
url = "rtsp://10.0.0.10:554/stream1"
window = "0,0,960,540"
"#,
        )
        .expect("config should parse");

        assert_eq!(config.app.name, "PiViewWall");
        assert_eq!(config.display.width, 1920);
        assert_eq!(config.cameras.feed.len(), 1);
        assert_eq!(config.cameras.feed[0].window.right, 960);
        assert_eq!(config.cameras.feed[0].window.bottom, 540);
    }

    #[test]
    fn validates_window_bounds() {
        let config = Config::from_str(
            r#"
[app]
name = "PiViewWall"
room = "main"

[display]
width = 1920
height = 1080
fullscreen = false

[cameras]
[[cameras.feed]]
name = "FrontDoor"
url = "rtsp://camera.example/stream"
window = "0,0,960,540"
"#,
        )
        .expect("config should parse");

        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_window_shape() {
        let config = Config::from_str(
            r#"
[app]
name = "PiViewWall"
room = "main"

[display]
width = 1920
height = 1080
fullscreen = true

[cameras]
[[cameras.feed]]
name = "Broken"
url = "rtsp://camera.example/stream"
window = "50,60,60,50"
"#,
        )
        .expect("config should parse");

        assert!(config.validate().is_err());
    }
}
