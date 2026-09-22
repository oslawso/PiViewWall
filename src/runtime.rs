use crate::config::{Config, FeedConfig, WindowRect};
use crate::layout::RenderLayout;
use std::process::Child;
use std::time::Duration;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StreamStatus {
    #[default]
    Stopped,
    Starting,
    Running,
    Failed,
    Restarting,
}

#[derive(Debug)]
pub struct FeedRuntime {
    pub name: String,
    pub url: String,
    pub window: WindowRect,
    pub status: StreamStatus,
    pub restart_count: u32,
    pub last_error: Option<String>,
    pub last_command: Option<String>,
    pub process: Option<Child>,
}

impl FeedRuntime {
    pub fn from_config(feed: &FeedConfig) -> Self {
        Self {
            name: feed.name.clone(),
            url: feed.url.clone(),
            window: feed.window.clone(),
            status: StreamStatus::Stopped,
            restart_count: 0,
            last_error: None,
            last_command: None,
            process: None,
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.url.trim().is_empty() {
            return Err(format!("feed '{}' has no URL", self.name));
        }

        self.status = StreamStatus::Starting;
        self.last_error = None;
        Ok(())
    }

    pub fn mark_running(&mut self) {
        self.status = StreamStatus::Running;
        self.last_error = None;
    }

    pub fn mark_failed(&mut self, error: impl Into<String>) {
        self.status = StreamStatus::Failed;
        self.restart_count += 1;
        self.last_error = Some(error.into());
    }

    pub fn restart(&mut self) -> Result<(), String> {
        if self.url.trim().is_empty() {
            return Err(format!("feed '{}' cannot restart without a URL", self.name));
        }

        let command = match &self.last_command {
            Some(command) => command.clone(),
            None => {
                let x = self.window.left;
                let y = self.window.top;
                format!(
                    "gst-launch-1.0 rtspsrc location=\"{}\" latency=100 ! rtph264depay ! decodebin ! videoconvert ! videoscale ! waylandsink fullscreen=false window=x={x},y={y},width={},height={} display={}x{}",
                    self.url,
                    self.window.width(),
                    self.window.height(),
                    self.window.right.max(1),
                    self.window.bottom.max(1)
                )
            }
        };

        let child = ProcessRunner::spawn(&command)?;
        self.process = Some(child);
        self.last_command = Some(command.clone());
        self.status = StreamStatus::Restarting;
        self.last_error = None;
        self.mark_running();
        println!("Restarting {} via command: {}", self.name, command);
        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct StreamManager {
    pub feeds: Vec<FeedRuntime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLaunchPlan {
    pub backend: String,
    pub summary: String,
}

pub trait MediaBackend {
    fn launch_plan(&self, feed: &FeedRuntime, layout: &RenderLayout) -> Result<MediaLaunchPlan, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GStreamerBackend;

impl MediaBackend for GStreamerBackend {
    fn launch_plan(&self, feed: &FeedRuntime, layout: &RenderLayout) -> Result<MediaLaunchPlan, String> {
        if feed.url.trim().is_empty() {
            return Err(format!("feed '{}' has no RTSP URL to launch", feed.name));
        }

        let width = layout.width;
        let height = layout.height;
        let x = feed.window.left;
        let y = feed.window.top;
        let window = format!(
            "x={x},y={y},width={},height={}",
            feed.window.width(),
            feed.window.height()
        );

        let summary = format!(
            "gst-launch-1.0 rtspsrc location=\"{}\" latency=100 ! rtph264depay ! decodebin ! videoconvert ! videoscale ! waylandsink fullscreen=false window={window} display={}x{}",
            feed.url,
            width,
            height
        );

        Ok(MediaLaunchPlan {
            backend: "gstreamer".to_string(),
            summary,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessRunner;

impl ProcessRunner {
    pub fn spawn(command: &str) -> Result<Child, String> {
        let (shell, args) = if cfg!(windows) {
            ("cmd", vec!["/C", command])
        } else {
            ("sh", vec!["-c", command])
        };

        std::process::Command::new(shell)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|err| format!("failed to spawn process for command '{command}': {err}"))
    }

    pub fn run(command: &str) -> Result<(), String> {
        Self::spawn(command).map(|_| ())
    }
}

impl StreamManager {
    pub fn from_config(config: &Config) -> Self {
        let feeds = config
            .cameras
            .feed
            .iter()
            .map(FeedRuntime::from_config)
            .collect();

        Self { feeds }
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        for feed in &mut self.feeds {
            feed.start()?;
            feed.mark_running();
        }
        Ok(())
    }

    pub fn launch_with_backend(
        &mut self,
        backend: &impl MediaBackend,
        layout: &RenderLayout,
    ) -> Result<(), String> {
        for feed in &mut self.feeds {
            let launch_plan = backend.launch_plan(feed, layout)?;
            feed.start()?;

            match ProcessRunner::spawn(&launch_plan.summary) {
                Ok(child) => {
                    feed.process = Some(child);
                    feed.last_command = Some(launch_plan.summary.clone());
                    feed.mark_running();
                    println!(
                        "Launching {} via {}: {}",
                        feed.name, launch_plan.backend, launch_plan.summary
                    );
                }
                Err(err) => {
                    feed.mark_failed(err.clone());
                    println!("Failed to launch {} via {}: {}", feed.name, launch_plan.backend, err);
                }
            }
        }
        Ok(())
    }

    pub fn monitor_health(&mut self) -> Result<(), String> {
        for feed in &mut self.feeds {
            if let Some(child) = &mut feed.process {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        feed.status = StreamStatus::Failed;
                        feed.last_error = Some("process exited unexpectedly".to_string());
                    }
                    Ok(None) => {
                        if feed.status != StreamStatus::Running {
                            feed.mark_running();
                        }
                    }
                    Err(err) => {
                        feed.status = StreamStatus::Failed;
                        feed.last_error = Some(format!("failed to monitor process: {err}"));
                    }
                }
            }
        }

        Ok(())
    }

    pub fn monitor_and_restart(&mut self) -> Result<(), String> {
        for feed in &mut self.feeds {
            let exited = match &mut feed.process {
                Some(child) => match child.try_wait() {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(err) => {
                        feed.status = StreamStatus::Failed;
                        feed.last_error = Some(format!("failed to monitor process: {err}"));
                        continue;
                    }
                },
                None => false,
            };

            if exited {
                feed.status = StreamStatus::Failed;
                feed.last_error = Some("process exited unexpectedly".to_string());
                println!("Detected failed process for {}. Restarting...", feed.name);
                feed.restart()?;
            }
        }

        Ok(())
    }

    pub fn run_service_loop(&mut self, interval: Duration) -> Result<(), String> {
        loop {
            self.monitor_and_restart()?;
            std::thread::sleep(interval);
        }
    }

    pub fn restart_failed(&mut self, name: &str) -> Result<(), String> {
        let feed = self
            .feeds
            .iter_mut()
            .find(|feed| feed.name == name)
            .ok_or_else(|| format!("feed '{name}' was not found"))?;

        if feed.status != StreamStatus::Failed {
            return Err(format!("feed '{name}' is not in a failed state"));
        }

        feed.restart()?;
        Ok(())
    }

    pub fn print_status(&self) {
        println!("Stream status:");
        for feed in &self.feeds {
            println!(
                "- {}: {:?}{}",
                feed.name,
                feed.status,
                match &feed.last_error {
                    Some(error) => format!(" ({error})"),
                    None => String::new(),
                }
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn manages_stream_health_and_restarts() {
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
"#,
        )
        .expect("config should parse");

        let mut manager = StreamManager::from_config(&config);
        manager.start_all().expect("initial start should succeed");
        manager.feeds[0].mark_failed("timed out while connecting");

        assert_eq!(manager.feeds[0].status, StreamStatus::Failed);
        manager.restart_failed("FrontDoor").expect("restart should succeed");
        assert_eq!(manager.feeds[0].status, StreamStatus::Running);
        assert_eq!(manager.feeds[0].restart_count, 1);
    }

    #[test]
    fn builds_gstreamer_launch_plan_for_feed() {
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
"#,
        )
        .expect("config should parse");

        let layout = RenderLayout::from_config(&config).expect("layout should be valid");
        let feed = FeedRuntime::from_config(&config.cameras.feed[0]);
        let launch = GStreamerBackend.launch_plan(&feed, &layout).expect("launch plan should be built");

        assert!(launch.summary.contains("rtspsrc location=\"rtsp://camera.example/stream\""));
        assert!(launch.backend == "gstreamer");
    }

    #[test]
    fn detects_exited_processes() {
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
"#,
        )
        .expect("config should parse");

        let mut manager = StreamManager::from_config(&config);
        let mut child = ProcessRunner::spawn(if cfg!(windows) { "cmd /C exit 1" } else { "sh -c 'exit 1'" }).expect("process should spawn");
        let _ = child.wait();
        manager.feeds[0].last_command = Some(if cfg!(windows) { "cmd /C exit 1".to_string() } else { "sh -c 'exit 1'".to_string() });
        manager.feeds[0].process = Some(child);
        manager.feeds[0].status = StreamStatus::Running;

        manager.monitor_and_restart().expect("monitor should run");
        assert_eq!(manager.feeds[0].status, StreamStatus::Running);
    }
}
