use crate::config::{Config, FeedConfig, GStreamerConfig, WindowRect};
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
                let backend = GStreamerBackend::new(GStreamerConfig::default(), "wayland-0".to_string());
                backend.build_command(
                    &self.url,
                    self.window.left,
                    self.window.top,
                    self.window.width(),
                    self.window.height(),
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GStreamerBackend {
    pub config: GStreamerConfig,
    pub display_name: String,
}

impl GStreamerBackend {
    pub fn new(config: GStreamerConfig, display_name: String) -> Self {
        Self {
            config,
            display_name,
        }
    }

    fn build_command(
        &self,
        url: &str,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> String {
        let protocol = self.config.rtsp_protocol.as_str();
        let protocol_clause = match protocol {
            "udp" => " protocols=udp",
            _ => " protocols=tcp",
        };
        let tls_clause = if self.config.tls_validate { "" } else { " tls-validation-flags=0" };
        let render_rect = format!("{x},{y},{width},{height}");

        format!(
            "gst-launch-1.0 rtspsrc location=\"{}\"{}{} latency={} ! rtph264depay ! decodebin ! videoconvert ! videoscale ! waylandsink display={} fullscreen=false render-rectangle=\"{}\"",
            url,
            protocol_clause,
            tls_clause,
            self.config.latency,
            self.display_name,
            render_rect
        )
    }
}

impl MediaBackend for GStreamerBackend {
    fn launch_plan(&self, feed: &FeedRuntime, layout: &RenderLayout) -> Result<MediaLaunchPlan, String> {
        if feed.url.trim().is_empty() {
            return Err(format!("feed '{}' has no RTSP URL to launch", feed.name));
        }

        let x = feed.window.left;
        let y = feed.window.top;
        let width = feed.window.width();
        let height = feed.window.height();
        let summary = self.build_command(
            &feed.url,
            x,
            y,
            width,
            height,
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

[gstreamer]
rtsp_protocol = "tcp"
tls_validate = false
latency = 100

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
        let backend = GStreamerBackend::new(config.gstreamer.clone(), config.display.wayland_display.clone());
        let launch = backend.launch_plan(&feed, &layout).expect("launch plan should be built");

        assert!(launch.summary.contains("rtspsrc location=\"rtsp://camera.example/stream\""));
        assert!(launch.summary.contains("protocols=tcp"));
        assert!(launch.summary.contains("tls-validation-flags=0"));
        assert!(launch.summary.contains("latency=100"));
        assert!(launch.summary.contains("display=wayland-0"));
        assert!(launch.summary.contains("render-rectangle=\"0,0,960,540\""));
        assert!(!launch.summary.contains("window=x="));
        assert!(launch.backend == "gstreamer");
    }

    #[test]
    fn gstreamer_defaults_are_pi_safe() {
        let config = Config::from_str(
            r#"
[app]
name = "PiViewWall"
room = "main"

[display]
width = 1920
height = 1080
fullscreen = true

[gstreamer]
rtsp_protocol = "tcp"
tls_validate = false
latency = 100

[cameras]
[[cameras.feed]]
name = "FrontDoor"
url = "rtsp://camera.example/stream"
window = "0,0,960,540"
"#,
        )
        .expect("config should parse");

        assert_eq!(config.gstreamer.rtsp_protocol, "tcp");
        assert!(!config.gstreamer.tls_validate);
        assert_eq!(config.gstreamer.latency, 100);
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