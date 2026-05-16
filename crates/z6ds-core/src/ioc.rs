//! M04 — parse STM32CubeIDE `.ioc` into `BoardConfig`.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use thiserror::Error;

use crate::contracts::{
    BoardConfig, ClockConfig, PinConfig, UartPortConfig, SCHEMA_VERSION_BOARD_CONFIG,
};

#[derive(Debug, Error)]
pub enum IocParseError {
    #[error("failed to read IOC file: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid IOC line: {line}")]
    InvalidLine { line: String },
}

/// Parse `.ioc` at `path`, merging lab defaults for DISC1 when fields are absent.
pub fn parse_ioc_file(path: impl AsRef<Path>) -> Result<BoardConfig> {
    let content = std::fs::read_to_string(path.as_ref()).context("read ioc")?;
    parse_ioc_content(&content)
}

/// Parse `.ioc` text content.
pub fn parse_ioc_content(content: &str) -> Result<BoardConfig> {
    let mut base = BoardConfig::lab_disc1_defaults();
    let kv = parse_ioc_key_values(content)?;

    if let Some(mcu) = kv.get("Mcu.Name").or_else(|| kv.get("Mcu.UserName")) {
        base.mcu_id = mcu.clone();
    }

    base.clock = extract_clock(&kv).unwrap_or(base.clock);
    merge_uart_from_ioc(&mut base, &kv);
    merge_pins_from_ioc(&mut base, &kv);

    base.schema_version = SCHEMA_VERSION_BOARD_CONFIG;
    Ok(base)
}

fn parse_ioc_key_values(content: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        map.insert(key.trim().to_string(), value.trim().to_string());
    }
    Ok(map)
}

fn extract_clock(kv: &HashMap<String, String>) -> Option<ClockConfig> {
    let sysclk = kv
        .get("RCC.SYSCLKFreq_VALUE")
        .or_else(|| kv.get("RCC.HCLKFreq_VALUE"))
        .and_then(|v| v.parse::<u64>().ok())?;
    let apb1 = kv
        .get("RCC.APB1Freq_Value")
        .or_else(|| kv.get("RCC.APB1CLKFreq_VALUE"))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(sysclk / 2);
    let apb2 = kv
        .get("RCC.APB2Freq_Value")
        .or_else(|| kv.get("RCC.APB2CLKFreq_VALUE"))
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(sysclk / 2);
    Some(ClockConfig {
        sysclk_hz: sysclk,
        apb1_hz: apb1,
        apb2_hz: apb2,
    })
}

fn merge_uart_from_ioc(config: &mut BoardConfig, kv: &HashMap<String, String>) {
    let peripherals: Vec<String> = kv
        .keys()
        .filter_map(|k| {
            let rest = k.strip_prefix("USART")?;
            let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if num.is_empty() {
                return None;
            }
            Some(format!("USART{num}"))
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    for usart in peripherals {
        let baud = kv
            .get(&format!("{usart}.BaudRate"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(115_200);
        let (tx_pin, rx_pin) = find_uart_pins(kv, &usart);
        if tx_pin.is_none() && rx_pin.is_none() {
            continue;
        }
        let profile = UartPortConfig {
            peripheral: usart.clone(),
            baud_rate: baud,
            data_bits: 8,
            parity: "none".into(),
            stop_bits: 1,
            tx_pin: tx_pin.unwrap_or_default(),
            rx_pin: rx_pin.unwrap_or_default(),
        };
        if let Some(existing) = config
            .uart_profiles
            .iter_mut()
            .find(|p| p.peripheral == usart)
        {
            *existing = profile;
        } else {
            config.uart_profiles.push(profile);
        }
    }
}

fn find_uart_pins(kv: &HashMap<String, String>, usart: &str) -> (Option<String>, Option<String>) {
    let mut tx = None;
    let mut rx = None;
    for (key, value) in kv {
        if !key.ends_with(".Signal") {
            continue;
        }
        let pin_id = key.trim_end_matches(".Signal");
        if value == &format!("{usart}_TX") {
            tx = Some(pin_id.to_string());
        } else if value == &format!("{usart}_RX") {
            rx = Some(pin_id.to_string());
        }
    }
    (tx, rx)
}

fn merge_pins_from_ioc(config: &mut BoardConfig, kv: &HashMap<String, String>) {
    let mut by_id: HashMap<String, PinConfig> = config
        .pins
        .drain(..)
        .map(|p| (p.pin_id(), p))
        .collect();

    for (key, signal) in kv {
        if !key.ends_with(".Signal") {
            continue;
        }
        let pin_id = key.trim_end_matches(".Signal");
        let Some((port, pin_num)) = split_pin_id(pin_id) else {
            continue;
        };
        let entry = by_id.entry(pin_id.to_string()).or_insert_with(|| PinConfig {
            port: port.clone(),
            pin: pin_num,
            label: None,
            signal: None,
        });
        entry.signal = Some(signal.clone());
        if entry.label.is_none() {
            entry.label = Some(signal.clone());
        }
    }

    config.pins = by_id.into_values().collect();
    config.pins.sort_by(|a, b| a.pin_id().cmp(&b.pin_id()));
}

fn split_pin_id(pin_id: &str) -> Option<(String, u8)> {
    if pin_id.len() < 3 {
        return None;
    }
    let (port, num) = pin_id.split_at(2);
    if !port.starts_with('P') {
        return None;
    }
    let pin = num.parse().ok()?;
    Some((port.to_string(), pin))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_IOC: &str = r#"
Mcu.Name=STM32F429ZITx
USART1.BaudRate=115200
PA9.Mode=Asynchronous
PA9.Signal=USART1_TX
PA10.Mode=Asynchronous
PA10.Signal=USART1_RX
RCC.SYSCLKFreq_VALUE=180000000
RCC.APB1Freq_Value=90000000
RCC.APB2Freq_Value=90000000
PG13.Signal=GPIO_Output
PG13.GPIO_Label=LD3 [Green Led]
"#;

    #[test]
    fn parses_usart1_and_clock_from_ioc() {
        let cfg = parse_ioc_content(SAMPLE_IOC).unwrap();
        assert_eq!(cfg.schema_version, 1);
        assert_eq!(cfg.clock.sysclk_hz, 180_000_000);
        let usart1 = cfg
            .uart_profiles
            .iter()
            .find(|p| p.peripheral == "USART1")
            .expect("USART1 profile");
        assert_eq!(usart1.baud_rate, 115_200);
        assert_eq!(usart1.tx_pin, "PA9");
        assert_eq!(usart1.rx_pin, "PA10");
    }

    #[test]
    fn parse_ioc_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.ioc");
        std::fs::write(&path, SAMPLE_IOC).unwrap();
        let cfg = parse_ioc_file(&path).unwrap();
        assert!(cfg.pins.iter().any(|p| p.pin_id() == "PG13"));
    }
}
