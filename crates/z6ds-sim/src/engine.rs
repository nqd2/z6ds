//! M09 — emulation engine facade over Renode adapter.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde_json::json;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use z6ds_core::contracts::{
    event_types, AppEvent, BoardConfig, EngineHealth, GpioStateMap, SCHEMA_VERSION_GPIO,
    SCHEMA_VERSION_SIMULATOR,
};
use z6ds_core::EventBus;

use crate::peripherals::PeripheralHost;
use crate::renode_adapter::RenodeAdapter;
use crate::task::spawn_background;

pub struct EmulationEngine {
    session_id: String,
    adapter: Arc<Mutex<RenodeAdapter>>,
    elf_path: PathBuf,
    bus: EventBus,
    peripheral_host: Arc<Mutex<PeripheralHost>>,
    poll_handle: Option<JoinHandle<()>>,
    uart_handle: Option<JoinHandle<()>>,
}

impl EmulationEngine {
    pub fn new(session_id: impl Into<String>, bus: EventBus, monitor_port: u16, uart_port: u16) -> Self {
        let sid = session_id.into();
        Self {
            session_id: sid.clone(),
            adapter: Arc::new(Mutex::new(RenodeAdapter::new(sid, monitor_port, uart_port))),
            elf_path: PathBuf::new(),
            bus,
            peripheral_host: Arc::new(Mutex::new(PeripheralHost::new())),
            poll_handle: None,
            uart_handle: None,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn health(&self) -> EngineHealth {
        EngineHealth {
            schema_version: SCHEMA_VERSION_SIMULATOR,
            session_id: self.session_id.clone(),
            running: !self.elf_path.as_os_str().is_empty() && self.poll_handle.is_some(),
            pc: Some("0x08000000".into()),
            error: None,
        }
    }

    pub fn peripheral_host_mut(&mut self) -> std::sync::MutexGuard<'_, PeripheralHost> {
        self.peripheral_host.lock().unwrap()
    }

    pub fn peripheral_host_ref(&self) -> std::sync::MutexGuard<'_, PeripheralHost> {
        self.peripheral_host.lock().unwrap()
    }

    pub fn start(&mut self, elf_path: &Path, _board: &BoardConfig) -> Result<()> {
        self.elf_path = elf_path.to_path_buf();
        self.adapter.lock().unwrap().start(elf_path)?;
        self.start_background_tasks();
        Ok(())
    }

    fn start_background_tasks(&mut self) {
        let (uart_tx, mut uart_rx) = mpsc::unbounded_channel();
        self.uart_handle = Some(self.adapter.lock().unwrap().spawn_uart_reader(uart_tx));

        let bus_uart = self.bus.clone();
        spawn_background(async move {
            while let Some(chunk) = uart_rx.recv().await {
                let payload = serde_json::to_value(&chunk).unwrap_or(json!({}));
                bus_uart.publish(AppEvent::new(event_types::UART_ENGINE_TX, "M09", payload.clone()));
                let mut rx_chunk = chunk;
                rx_chunk.direction = "rx".into();
                bus_uart.publish(AppEvent::new(
                    event_types::UART_RX,
                    "M13",
                    serde_json::to_value(&rx_chunk).unwrap_or(payload),
                ));
            }
        });

        let bus_poll = self.bus.clone();
        let sid_poll = self.session_id.clone();
        let adapter_poll = Arc::clone(&self.adapter);
        let host_pins = Arc::clone(&self.peripheral_host);
        self.poll_handle = Some(spawn_background(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let pins = adapter_poll
                    .lock()
                    .ok()
                    .and_then(|a| a.read_gpio_map().ok())
                    .unwrap_or_default();
                if pins.is_empty() {
                    continue;
                }
                let vt = adapter_poll
                    .lock()
                    .map(|a| a.virtual_time_ns())
                    .unwrap_or(0);
                let map = GpioStateMap {
                    schema_version: SCHEMA_VERSION_GPIO,
                    session_id: sid_poll.clone(),
                    pins,
                    virtual_time_ns: vt,
                };
                let payload = serde_json::to_value(&map).unwrap_or(json!({}));
                bus_poll.publish(AppEvent::new(event_types::GPIO_CHANGED, "M09", payload.clone()));
                if let Ok(host) = host_pins.lock() {
                    host.on_gpio(&bus_poll, &map);
                }
            }
        }));
    }

    pub fn stop(&mut self) {
        if let Some(h) = self.poll_handle.take() {
            h.abort();
        }
        if let Some(h) = self.uart_handle.take() {
            h.abort();
        }
        if let Ok(mut a) = self.adapter.lock() {
            let _ = a.stop_emulation();
            a.shutdown();
        }
    }

    pub fn reset(&self) -> Result<()> {
        self.adapter.lock().unwrap().reset(&self.elf_path)
    }

    pub fn inject_pin(&self, pin_id: &str, level: u8) -> Result<()> {
        self.adapter.lock().unwrap().inject_pin_level(pin_id, level)
    }

    pub async fn host_uart_tx(&self, bytes: &[u8]) -> Result<()> {
        self.adapter.lock().unwrap().host_uart_tx(bytes).await
    }

    pub fn host_uart_tx_blocking(&self, bytes: &[u8]) -> Result<()> {
        self.adapter.lock().unwrap().host_uart_tx_blocking(bytes)
    }
}
