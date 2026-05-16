//! M09 — spawn real Renode, monitor TCP, USART1 socket bridge.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream as AsyncTcpStream;
use tokio::sync::mpsc;
use tokio::time::sleep;
use z6ds_core::contracts::UartStreamChunk;
use z6ds_core::contracts::SCHEMA_VERSION_UART_CHUNK;

use crate::task::spawn_background;

const RENODE_PLATFORM: &str = "/opt/renode/platforms/cpus/stm32f429.repl";

/// Lab pins polled for `GpioStateMap` (PG13/14 LEDs, PA0 USER, PD14/15 HC-SR04).
const WATCH_PINS: &[(&str, &str, u32)] = &[
    ("gpioPortG", "PG", 13),
    ("gpioPortG", "PG", 14),
    ("gpioPortA", "PA", 0),
    ("gpioPortD", "PD", 14),
    ("gpioPortD", "PD", 15),
];

pub struct RenodeAdapter {
    session_id: String,
    child: Option<Child>,
    monitor_port: u16,
    uart_port: u16,
    work_dir: PathBuf,
    monitor: Arc<Mutex<Option<TcpStream>>>,
    virtual_time_ns: Arc<Mutex<u64>>,
}

impl RenodeAdapter {
    pub fn new(session_id: impl Into<String>, monitor_port: u16, uart_port: u16) -> Self {
        Self {
            session_id: session_id.into(),
            child: None,
            monitor_port,
            uart_port,
            work_dir: std::env::temp_dir().join(format!("z6ds-renode-{}", uuid::Uuid::new_v4())),
            monitor: Arc::new(Mutex::new(None)),
            virtual_time_ns: Arc::new(Mutex::new(0)),
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn uart_port(&self) -> u16 {
        self.uart_port
    }

    pub fn virtual_time_ns(&self) -> u64 {
        *self.virtual_time_ns.lock().unwrap()
    }

    pub fn tick_virtual_time(&self, delta_ns: u64) {
        let mut t = self.virtual_time_ns.lock().unwrap();
        *t = t.saturating_add(delta_ns);
    }

    /// Locate `renode` binary.
    pub fn find_renode() -> Result<PathBuf> {
        if let Ok(p) = std::env::var("RENODE_BIN") {
            let path = PathBuf::from(p);
            if path.is_file() {
                return Ok(path);
            }
        }
        let which = Command::new("which").arg("renode").output().ok();
        if let Some(out) = which {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !s.is_empty() {
                    return Ok(PathBuf::from(s));
                }
            }
        }
        for candidate in ["/usr/bin/renode", "/opt/renode/renode"] {
            let p = PathBuf::from(candidate);
            if p.is_file() {
                return Ok(p);
            }
        }
        Err(anyhow!("renode_missing: install Renode and ensure `renode` is on PATH"))
    }

    pub fn start(&mut self, elf_path: &Path) -> Result<()> {
        if !elf_path.is_file() {
            return Err(anyhow!("invalid_elf: {}", elf_path.display()));
        }
        if !Path::new(RENODE_PLATFORM).exists() {
            return Err(anyhow!(
                "unsupported_machine_model: platform file missing at {}",
                RENODE_PLATFORM
            ));
        }

        std::fs::create_dir_all(&self.work_dir)?;
        let resc = self.write_resc_script(elf_path)?;
        let renode = Self::find_renode()?;

        let child = Command::new(&renode)
            .arg("--disable-gui")
            .arg("--hide-monitor")
            .arg("--hide-log")
            .arg("-P")
            .arg(self.monitor_port.to_string())
            .arg(&resc)
            .current_dir(&self.work_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("engine_start_failed: failed to spawn renode")?;

        self.child = Some(child);
        self.wait_for_monitor()?;
        self.connect_monitor()?;
        Ok(())
    }

    fn write_resc_script(&self, elf_path: &Path) -> Result<PathBuf> {
        let elf = elf_path
            .canonicalize()
            .unwrap_or_else(|_| elf_path.to_path_buf());
        let resc_path = self.work_dir.join("z6ds.resc");
        let content = format!(
            r#"using sysbus
mach create "z6ds"
machine LoadPlatformDescription @{RENODE_PLATFORM}

emulation CreateServerSocketTerminal {uart_port} "uart_term" false
connector Connect sysbus.usart1 uart_term

macro reset
"""
    sysbus LoadELF @{elf}
"""
runMacro $reset
start
"#,
            RENODE_PLATFORM = RENODE_PLATFORM,
            uart_port = self.uart_port,
            elf = elf.display()
        );
        std::fs::write(&resc_path, content)?;
        Ok(resc_path)
    }

    fn wait_for_monitor(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.monitor_port);
        for _ in 0..80 {
            if TcpStream::connect(&addr).is_ok() {
                std::thread::sleep(Duration::from_millis(100));
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Err(anyhow!("engine_start_failed: monitor port {} not ready", self.monitor_port))
    }

    fn connect_monitor(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.monitor_port);
        let stream = TcpStream::connect(&addr).context("monitor connect")?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        *self.monitor.lock().unwrap() = Some(stream);
        let _ = self.monitor_command("help", 512);
        Ok(())
    }

    pub fn monitor_command(&self, cmd: &str, max_read: usize) -> Result<String> {
        let mut guard = self.monitor.lock().unwrap();
        let stream = guard.as_mut().ok_or_else(|| anyhow!("engine_process_lost"))?;
        let line = format!("{cmd}\n");
        stream
            .write_all(line.as_bytes())
            .context("monitor write")?;
        stream.flush().ok();
        let mut buf = vec![0u8; max_read];
        let mut out = String::new();
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    out.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if out.contains("(monitor)") || out.len() > max_read.saturating_sub(64) {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(out)
    }

    pub fn reset(&self, elf_path: &Path) -> Result<()> {
        let elf = elf_path
            .canonicalize()
            .unwrap_or_else(|_| elf_path.to_path_buf());
        let _ = self.monitor_command(&format!("sysbus LoadELF @{}", elf.display()), 4096)?;
        let _ = self.monitor_command("start", 1024)?;
        Ok(())
    }

    pub fn stop_emulation(&self) -> Result<()> {
        let _ = self.monitor_command("pause", 1024);
        Ok(())
    }

    pub fn inject_pin_level(&self, pin_id: &str, level: u8) -> Result<()> {
        let (port, bit) = parse_pin_id(pin_id)?;
        let pressed = level != 0;
        let cmd = format!("sysbus.{port} OnGPIO {bit} {pressed}", pressed = pressed);
        let _ = self.monitor_command(&cmd, 2048)?;
        Ok(())
    }

    pub fn read_gpio_map(&self) -> Result<std::collections::HashMap<String, u8>> {
        let mut pins = std::collections::HashMap::new();
        for (port, prefix, bit) in WATCH_PINS {
            let odr_addr = gpio_odr_address(port)?;
            let resp = self.monitor_command(&format!("sysbus ReadDoubleWord 0x{odr_addr:X}"), 512)?;
            let value = parse_hex_u32(&resp).unwrap_or(0);
            let pin_id = format!("{prefix}{bit}");
            let level = if (value >> bit) & 1 == 1 { 1 } else { 0 };
            pins.insert(pin_id, level);
        }
        self.tick_virtual_time(1_000_000);
        Ok(pins)
    }

    pub fn spawn_uart_reader(
        &self,
        tx: mpsc::UnboundedSender<UartStreamChunk>,
    ) -> tokio::task::JoinHandle<()> {
        let port = self.uart_port;
        let session_id = self.session_id.clone();
        let vt = Arc::clone(&self.virtual_time_ns);
        spawn_background(async move {
            let addr = format!("127.0.0.1:{port}");
            for attempt in 0..40 {
                match AsyncTcpStream::connect(&addr).await {
                    Ok(mut stream) => {
                        let mut buf = [0u8; 4096];
                        loop {
                            match stream.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    let bytes = buf[..n].to_vec();
                                    let mut t = vt.lock().unwrap();
                                    *t = t.saturating_add(1_000_000);
                                    let chunk = UartStreamChunk {
                                        schema_version: SCHEMA_VERSION_UART_CHUNK,
                                        session_id: session_id.clone(),
                                        port_id: "USART1".into(),
                                        direction: "tx".into(),
                                        bytes_base64: B64.encode(&bytes),
                                        timestamp: chrono_now(),
                                        virtual_time_ns: *t,
                                    };
                                    let _ = tx.send(chunk);
                                }
                                Err(_) => break,
                            }
                        }
                        break;
                    }
                    Err(_) => {
                        sleep(Duration::from_millis(250)).await;
                    }
                }
                if attempt == 39 {
                    eprintln!("uart: socket {addr} not available");
                }
            }
        })
    }

    pub async fn host_uart_tx(&self, bytes: &[u8]) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.uart_port);
        let mut stream = AsyncTcpStream::connect(&addr)
            .await
            .context("uart port connect")?;
        stream.write_all(bytes).await?;
        stream.flush().await?;
        Ok(())
    }

    /// Blocking UART TX for Tauri command handlers (no async runtime required).
    pub fn host_uart_tx_blocking(&self, bytes: &[u8]) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.uart_port);
        let mut stream =
            TcpStream::connect(&addr).context("uart port connect (blocking)")?;
        stream.write_all(bytes).context("uart write")?;
        stream.flush().ok();
        Ok(())
    }

    pub fn shutdown(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = self.monitor_command("quit", 256);
            let _ = child.kill();
            let _ = child.wait();
        }
        *self.monitor.lock().unwrap() = None;
        let _ = std::fs::remove_dir_all(&self.work_dir);
    }
}

impl Drop for RenodeAdapter {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn parse_pin_id(pin_id: &str) -> Result<(&'static str, u32)> {
    if pin_id.len() < 3 {
        return Err(anyhow!("invalid pin id: {pin_id}"));
    }
    let (port_letter, num_str) = pin_id.split_at(2);
    let bit: u32 = num_str.parse().context("pin bit")?;
    let port = match port_letter {
        "PA" => "gpioPortA",
        "PB" => "gpioPortB",
        "PC" => "gpioPortC",
        "PD" => "gpioPortD",
        "PE" => "gpioPortE",
        "PF" => "gpioPortF",
        "PG" => "gpioPortG",
        "PH" => "gpioPortH",
        _ => return Err(anyhow!("unsupported pin port: {port_letter}")),
    };
    Ok((port, bit))
}

fn gpio_odr_address(port: &str) -> Result<u32> {
    let base = match port {
        "gpioPortA" => 0x4002_0000,
        "gpioPortB" => 0x4002_0400,
        "gpioPortC" => 0x4002_0800,
        "gpioPortD" => 0x4002_0C00,
        "gpioPortE" => 0x4002_1000,
        "gpioPortF" => 0x4002_1400,
        "gpioPortG" => 0x4002_1800,
        "gpioPortH" => 0x4002_1C00,
        _ => return Err(anyhow!("unknown gpio port {port}")),
    };
    Ok(base + 0x14)
}

fn parse_hex_u32(text: &str) -> Option<u32> {
    for token in text.split_whitespace() {
        if let Some(hex) = token.strip_prefix("0x") {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                return Some(v);
            }
        }
    }
    None
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pin_pa0() {
        let (port, bit) = parse_pin_id("PA0").unwrap();
        assert_eq!(port, "gpioPortA");
        assert_eq!(bit, 0);
    }

    #[test]
    fn renode_binary_discoverable() {
        if std::env::var("Z6DS_SKIP_RENODE").is_ok() {
            return;
        }
        assert!(RenodeAdapter::find_renode().is_ok());
    }
}
