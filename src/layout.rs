use crate::config::{Config, WindowRect};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedPlacement {
    pub name: String,
    pub window: WindowRect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderLayout {
    pub width: u32,
    pub height: u32,
    pub placements: Vec<FeedPlacement>,
}

impl WindowRect {
    pub fn width(&self) -> u32 {
        self.right.saturating_sub(self.left)
    }

    pub fn height(&self) -> u32 {
        self.bottom.saturating_sub(self.top)
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

impl RenderLayout {
    pub fn from_config(config: &Config) -> Result<Self, String> {
        let placements = config
            .cameras
            .feed
            .iter()
            .map(|feed| FeedPlacement {
                name: feed.name.clone(),
                window: feed.window.clone(),
            })
            .collect::<Vec<_>>();

        for placement in &placements {
            if placement.window.right > config.display.width
                || placement.window.bottom > config.display.height
            {
                return Err(format!(
                    "feed '{}' window exceeds the display bounds: {:?}",
                    placement.name, placement.window
                ));
            }
        }

        for (index, first) in placements.iter().enumerate() {
            for second in placements.iter().skip(index + 1) {
                if first.window.overlaps(&second.window) {
                    return Err(format!(
                        "feed '{}' overlaps with '{}' in the display layout",
                        first.name, second.name
                    ));
                }
            }
        }

        Ok(Self {
            width: config.display.width,
            height: config.display.height,
            placements,
        })
    }

    pub fn render_summary(&self) {
        println!("Render layout: {}x{}", self.width, self.height);
        for placement in &self.placements {
            println!(
                "- {}: {}x{} @ ({},{}) -> ({},{})",
                placement.name,
                placement.window.width(),
                placement.window.height(),
                placement.window.left,
                placement.window.top,
                placement.window.right,
                placement.window.bottom
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn creates_render_layout_for_non_overlapping_windows() {
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
url = "rtsp://camera.example/stream"
window = "0,0,960,540"

[[cameras.feed]]
name = "BackYard"
url = "rtsp://camera.example/stream2"
window = "960,0,1920,540"
"#,
        )
        .expect("config should parse");

        let layout = RenderLayout::from_config(&config).expect("layout should be valid");
        assert_eq!(layout.placements.len(), 2);
        assert_eq!(layout.placements[1].window.left, 960);
    }

    #[test]
    fn rejects_overlapping_render_windows() {
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
url = "rtsp://camera.example/stream"
window = "0,0,960,540"

[[cameras.feed]]
name = "BackYard"
url = "rtsp://camera.example/stream2"
window = "800,100,1600,600"
"#,
        )
        .expect("config should parse");

        assert!(RenderLayout::from_config(&config).is_err());
    }
}
