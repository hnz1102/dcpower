// This is electric-load main program for ESP32-S3-WROOM-1-N16R8.
// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Hiroshi Nakajima

use std::{thread, time::Duration};
use esp_idf_hal::{gpio::*, prelude::*, spi, i2c};
use esp_idf_hal::delay::BLOCK;
use esp_idf_hal::peripherals::Peripherals;
use embedded_hal::spi::MODE_0;
use log::*;
use std::time::SystemTime;
use esp_idf_hal::adc::oneshot::config::AdcChannelConfig as AdcConfig;
use esp_idf_hal::adc::oneshot::config::Calibration;
use esp_idf_hal::adc::oneshot::*;
use esp_idf_hal::adc::attenuation::DB_11;
use esp_idf_hal::ledc::config::TimerConfig;
use esp_idf_hal::ledc::LedcTimerDriver;
use esp_idf_hal::ledc::LedcDriver;
use esp_idf_svc::sntp::{EspSntp, SyncStatus, SntpConf, OperatingMode, SyncMode};
use esp_idf_svc::wifi::EspWifi;
use chrono::{DateTime, Utc};

mod displayctl;
mod currentlogs;
mod wifi;
mod transfer;
mod touchpad;
mod pidcont;
mod usbpd;
mod syslogger;  // Add the syslogger module

use displayctl::{DisplayPanel, LoggingStatus, WifiStatus};
use currentlogs::{CurrentRecord, CurrentLog};
use transfer::{Transfer, ServerInfo};
use touchpad::{TouchPad, KeyEvent, Key};
use pidcont::PIDController;
use usbpd::{AP33772S, PDVoltage};

const ADCRANGE : bool = true; // true: 40.96mV, false: 163.84mV

#[toml_cfg::toml_config]
pub struct Config {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
    #[default("")]
    influxdb_server: &'static str,
    #[default("0.00001")]
    pid_kp: &'static str,
    #[default("0.05")]
    pid_ki: &'static str,
    #[default("0.000001")]
    pid_kd: &'static str,
    #[default("4500")]
    pwm_offset: &'static str,
    #[default("0.0")]
    pd_config_offset: &'static str,
    #[default("0.0")]
    shunt_resistance: &'static str,
    #[default("50")]
    shunt_temp_coefficient: &'static str,
    #[default("11.0")]
    max_current_limit: &'static str,
    #[default("110.0")]
    max_power_limit: &'static str,
    #[default("75.0")]
    max_temperature: &'static str,
    #[default("")]
    influxdb_api_key: &'static str,
    #[default("")]
    influxdb_api: &'static str,
    #[default("")]
    influxdb_measurement: &'static str,
    #[default("")]
    influxdb_tag: &'static str,
    #[default("")]
    syslog_server: &'static str,
    #[default("")]
    syslog_enable: &'static str,
}

fn main() -> anyhow::Result<()> {
    esp_idf_sys::link_patches();
    
    // Initialize the default ESP logger only if syslog is disabled
    // If syslog is enabled, we'll initialize the syslog logger later
    if CONFIG.syslog_enable != "true" {
        esp_idf_svc::log::EspLogger::initialize_default();
        // Set log level to INFO to ensure info!() messages are displayed
        log::set_max_level(log::LevelFilter::Info);
    }
    
    // Peripherals Initialize
    let peripherals = Peripherals::take().unwrap();
    // Initialize nvs
    unsafe {
        esp_idf_sys::nvs_flash_init();
    }

    // Log startup message
    println!("DCPowerUnit2 application started (println)");
    info!("DCPowerUnit2 application started (info)");
    
    // Load Config
    let max_current_limit = CONFIG.max_current_limit.parse::<f32>().unwrap();
    let max_power_limit = CONFIG.max_power_limit.parse::<f32>().unwrap();
    let max_temperature = CONFIG.max_temperature.parse::<f32>().unwrap();
    println!("[Config Limit] Current: {}A  Power: {}W  Temperature: {}°C", max_current_limit, max_power_limit, max_temperature);
    info!("[Config Limit] Current: {}A  Power: {}W  Temperature: {}°C", max_current_limit, max_power_limit, max_temperature);
    let server_info = ServerInfo::new(CONFIG.influxdb_server.to_string(), 
        CONFIG.influxdb_api_key.to_string(),
        CONFIG.influxdb_api.to_string(),
        CONFIG.influxdb_measurement.to_string(),
        CONFIG.influxdb_tag.to_string());

    // Display SPI
    let spi = peripherals.spi2;
    let sclk = peripherals.pins.gpio45;
    let sdo  = peripherals.pins.gpio17;
    let sdi_not_used : Option<Gpio2> = None;
    let cs_not_used : Option<Gpio2> = None;
    let dc = PinDriver::output(peripherals.pins.gpio15)?;
    let rst = PinDriver::output(peripherals.pins.gpio16)?;
    let spi_config = spi::SpiConfig::new().baudrate(1.MHz().into()).data_mode(MODE_0);
    let spi_driver_config = spi::config::DriverConfig::new();

    let spi_driver = spi::SpiDriver::new(
        spi,
        sclk,
        sdo,
        sdi_not_used,
        &spi_driver_config
    ).unwrap();
    
    let spi_device = spi::SpiDeviceDriver::new(spi_driver, cs_not_used, &spi_config)?;
    let mut dp = DisplayPanel::new();
    dp.start(spi_device, dc, rst);

    // Current/Voltage
    let i2c = peripherals.i2c0;
    let scl = peripherals.pins.gpio47;
    let sda = peripherals.pins.gpio21;
    let config = i2c::I2cConfig::new().baudrate(400.kHz().into());
    let mut i2cdrv = i2c::I2cDriver::new(i2c, sda, scl, &config)?;

    // read config
    let mut i2c_sel = PinDriver::output(peripherals.pins.gpio46).unwrap();
    i2c_sel.set_high().unwrap(); // Enable USB PD
    let mut ap33772s = AP33772S::new();
    match ap33772s.init(&mut i2cdrv) {
        Ok(()) => {
            info!("AP33772S initialized successfully");
        },
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to initialize AP33772S: {:?}", e));
        }
    }

    // Configure protection features: UVP=true, OVP=true, OCP=true, OTP=false, DR=false
    match ap33772s.configure_protections(&mut i2cdrv, true, true, true, false, false) {
        Ok(()) => {
            info!("AP33772S protections configured successfully");
        },
        Err(e) => {
            warn!("Failed to configure AP33772S protections: {:?}", e);
        }
    }
    match ap33772s.get_status(&mut i2cdrv) {
        Ok(status) => {
            // For debugging purposes, log status occasionally
            // Not implemented: NTC thermistor
            info!(
                "PD Status: Voltage={}mV, Current={}mA, Temp={}°C, PDP={}W",
                status.voltage_mv,
                status.current_ma,
                status.temperature,
                status.pdp_limit_w
            );
        },
        Err(e) => {
            info!("Failed to read AP33772S status: {:?}", e);
        }
    }
    let _ = ap33772s.request_voltage(&mut i2cdrv, PDVoltage::V5);
    // ap33772s.force_vout_off(&mut i2cdrv).unwrap();

    // Get PDO limits from connected source
    i2c_sel.set_high().unwrap(); // Enable USB PD for PDO query
    let (pdo_max_voltage, pdo_max_current) = ap33772s.get_pdo_limits();
    info!("PDO Limits: Max Voltage = {:.2}V, Max Current = {:.3}A", pdo_max_voltage, pdo_max_current);
    
    // Apply the more restrictive limit between config and PDO
    let effective_max_current = if pdo_max_current < max_current_limit { pdo_max_current } else { max_current_limit };
    info!("Effective Current Limit: {:.3}A (Config: {:.3}A, PDO: {:.3}A)", 
          effective_max_current, max_current_limit, pdo_max_current);
    println!("[Effective Limits] Voltage: {:.2}V  Current: {:.3}A", pdo_max_voltage, effective_max_current);

    // Select INA228
    i2c_sel.set_low().unwrap(); // Select INA228

    // Initialize INA228 sensor
    match ADCRANGE {
        true => write_ina228_reg16(&mut i2cdrv, 0x00, 0x0030)?, // Bit4: ADCRANGE=1(40.96mV), Bit5 Enables temperature compensation
        false => write_ina228_reg16(&mut i2cdrv, 0x00, 0x0020)?, // Bit4: ADCRANGE=0(163.84mV), Bit5 Enables temperature compensation
    }
    let read_value = read_ina228_reg16(&mut i2cdrv, 0x00)?;
    info!("INA228 Config Set to: {:04x}", read_value);

    // INA228 ADC Config
    let read_adc_config = read_ina228_reg16(&mut i2cdrv, 0x01)?;
    info!("INA228 ADC Config Read: {:04x}", read_adc_config);
    let write_adc_config : u16 = (read_adc_config & 0xFFF8) | 0x04; // Clear bits 0-2, 0x00: 1avg, 0x02: 16avg, 0x03: 64avg
    write_ina228_reg16(&mut i2cdrv, 0x01, write_adc_config)?;
    let read_adc_config = read_ina228_reg16(&mut i2cdrv, 0x01)?;
    info!("INA228 ADC Config Set to: {:04x}", read_adc_config);


    // SHUNT_CAL
    let shunt_resistance = CONFIG.shunt_resistance.parse::<f32>().unwrap();
    let current_lsb = match ADCRANGE {
        true => {
            // 40.96mV range
            40.96 / 524_288.0
        },
        false => {
            // 163.84mV range
            163.84 / 524_288.0
        }
    };
    let shunt_cal_val = match ADCRANGE {
        true => 13107.2 * current_lsb * 1000_000.0 * shunt_resistance * 4.0, // 40.96mV range
        false => 13107.2 * current_lsb * 1000_000.0 * shunt_resistance, // 163.84mV range
    };
    let shunt_cal = shunt_cal_val as u16;
    info!("current_lsb={:?} shunt_cal_val={:?} shunt_cal={:?}", current_lsb, shunt_cal_val, shunt_cal);
    write_ina228_reg16(&mut i2cdrv, 0x02, shunt_cal)?;
    let read_shunt_cal = read_ina228_reg16(&mut i2cdrv, 0x02)?;
    info!("INA228 SHUNT_CAL Set to: {:04x}", read_shunt_cal);
    // Shunt Temperature Coefficient
    let shunt_temp_coefficient = CONFIG.shunt_temp_coefficient.parse::<u16>().unwrap();
    info!("Shunt Temperature Coefficient: {:?}", shunt_temp_coefficient);
    write_ina228_reg16(&mut i2cdrv, 0x03, shunt_temp_coefficient)?;
    let read_shunt_temp_coefficient = read_ina228_reg16(&mut i2cdrv, 0x03)?;
    info!("INA228 SHUNT_TEMP_COEFFICIENT Set to: {:04x}", read_shunt_temp_coefficient);

    // Temperature Measurement
    let temperature: f32 = read_ina228_reg16(&mut i2cdrv, 0x06)? as f32 * 7.8125;
    info!("Initial Temperature Read: {:.2}°C", temperature / 1000.0);

    // calibration read
    let mut average_current_offset :f32 = 0.0;
    let mut average_voltage_offset :f32 = 0.0;
    // let (current_offset, voltage_offset) = calibration(&mut i2cdrv, current_lsb)?;
    // average_current_offset = current_offset;

    // PWM
    let timer_config_out_current = TimerConfig::default().frequency(4.kHz().into())
        .resolution(esp_idf_hal::ledc::config::Resolution::Bits14);
    let timer_driver_0 = LedcTimerDriver::new(peripherals.ledc.timer0, &timer_config_out_current).unwrap();
    let mut pwm_driver = LedcDriver::new(peripherals.ledc.channel0, &timer_driver_0, peripherals.pins.gpio38).unwrap();
    pwm_driver.set_duty(0).expect("Set duty failure");
    let max_duty = pwm_driver.get_max_duty();
    info!("Max duty: {}", max_duty);

    let pd_config_offset = CONFIG.pd_config_offset.parse::<f32>().unwrap();    

    // Temperature Logs
    let mut clogs = CurrentRecord::new();

    // Initialize logging for early debugging
    let mut wifi_enable : bool;
    let mut wifi_dev = wifi::wifi_connect(peripherals.modem, CONFIG.wifi_ssid, CONFIG.wifi_psk);

    if CONFIG.syslog_enable == "true" {
        // Initialize syslog logger to replace the default ESP logger
        println!("Initializing syslog logger...");
        thread::sleep(Duration::from_secs(5));
        
        match syslogger::init_logger(CONFIG.syslog_server, CONFIG.syslog_enable) {
            Ok(_) => {
                // Set log level for syslog
                log::set_max_level(log::LevelFilter::Info);
                println!("Syslog logger initialized successfully");
                info!("Syslog logger initialized successfully");
            },
            Err(e) => {
                // Fallback to ESP logger if syslog fails
                println!("Failed to initialize syslog logger: {:?}, using ESP logger instead", e);
                esp_idf_svc::log::EspLogger::initialize_default();
                log::set_max_level(log::LevelFilter::Info);
                info!("Failed to initialize syslog logger: {:?}, using ESP logger instead", e);
            }
        }
    } else {
        // syslog_enable is false, continue using default ESP console logger
        info!("Using default ESP console logger (syslog disabled)");
    }
    
    // NTP Server
    let sntp_conf = SntpConf {
        servers: ["time.aws.com",
                    "time.google.com",
                    "time.cloudflare.com",
                    "ntp.nict.jp"],
        operating_mode: OperatingMode::Poll,
        sync_mode: SyncMode::Immediate,
    };
    let ntp = EspSntp::new(&sntp_conf).unwrap();

    // NTP Sync
    // let now = SystemTime::now();
    // if now.duration_since(UNIX_EPOCH).unwrap().as_millis() < 1700000000 {
    info!("NTP Sync Start..");

    // wait for sync
    let mut sync_count = 0;
    while ntp.get_sync_status() != SyncStatus::Completed {
        sync_count += 1;
        if sync_count > 1000 {
            info!("NTP Sync Timeout");
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let now = SystemTime::now();
    let dt_now : DateTime<Utc> = now.into();
    let formatted = format!("{}", dt_now.format("%Y-%m-%d %H:%M:%S"));
    info!("NTP Sync Completed: {}", formatted);
        
    let mut txd =  Transfer::new(server_info);
    txd.start()?;

    // TouchPad
    let mut touchpad = TouchPad::new();
    touchpad.start();
    
    // ADC2-CH7 GPIO18 for Temperature
    let mut adc_temp = AdcDriver::new(peripherals.adc2)?;
    let mut adc_temp_config = AdcConfig {
        attenuation: DB_11,
        calibration: Calibration::Curve,
        .. AdcConfig::default()
    };
    let mut temp_pin = AdcChannelDriver::new(&mut adc_temp, peripherals.pins.gpio18, &mut adc_temp_config)?;

    // ADC1-CH8 GPIO9 for USB PD Voltage
    let mut adc_pd_voltage = AdcDriver::new(peripherals.adc1)?;
    let mut adc_pd_voltage_config = AdcConfig {
        attenuation: DB_11,
        calibration: Calibration::Curve,
        .. AdcConfig::default()
    };
    let mut usb_pd_pin = AdcChannelDriver::new(&mut adc_pd_voltage, peripherals.pins.gpio9, &mut adc_pd_voltage_config)?;
    
    // PID Controller
    let pid_kp = CONFIG.pid_kp.parse::<f32>().unwrap();
    let pid_ki = CONFIG.pid_ki.parse::<f32>().unwrap();
    let pid_kd = CONFIG.pid_kd.parse::<f32>().unwrap();
    let pwm_offset = CONFIG.pwm_offset.parse::<u32>().unwrap();
    info!("PID Controller: KP={} KI={} KD={}", pid_kp, pid_ki, pid_kd);
    let mut pid = PIDController::new(pid_kp, pid_ki, pid_kd, 0.0);

    // Start Display
    dp.enable_display(true);

    // TouchPad Long Press
    touchpad.set_press_threshold(Key::Center, 1000, false);
    touchpad.set_press_threshold(Key::Up, 300, true);
    touchpad.set_press_threshold(Key::Down, 300, true);

    // loop
    let mut measurement_count : u32 = 0;
    let mut logging_start = false;
    let mut load_start = false;
    let mut calibration_start = false;
    let mut set_output_voltage = 0.0;
    let mut previous_set_output_voltage = 0.0;
    let mut pwm_duty : u32;
    loop {
        thread::sleep(Duration::from_millis(10));

        let mut start_stop_btn = false;
        measurement_count += 1;
        if measurement_count % 10 == 0 {
            let key_event = touchpad.get_key_event_and_clear();
            for key in &key_event {
                match key {
                    KeyEvent::CenterKeyDown => {
                        // Clear error messages when center key is pressed
                        dp.set_message("".to_string(), false, 0);
                        info!("Error message cleared by center key press");
                    },
                    KeyEvent::CenterKeyDownLong => {
                        if start_stop_btn == false {
                            start_stop_btn = true;
                        }
                        else {
                            start_stop_btn = false;
                        } 
                    },
                    KeyEvent::UpKeyDown => {
                        set_output_voltage += 0.1;
                        if set_output_voltage > pdo_max_voltage {
                            set_output_voltage = pdo_max_voltage;
                        }
                        dp.set_output_voltage(set_output_voltage);
                    },
                    KeyEvent::RightKeyDown => {
                        set_output_voltage += 0.01;
                        if set_output_voltage > pdo_max_voltage {
                            set_output_voltage = pdo_max_voltage;
                        }
                        dp.set_output_voltage(set_output_voltage);
                    },
                    KeyEvent::UpKeyDownLong => {
                        set_output_voltage = ((set_output_voltage + 1.0) as u32) as f32;
                        if set_output_voltage > pdo_max_voltage {
                            set_output_voltage = pdo_max_voltage;
                        }
                        dp.set_output_voltage(set_output_voltage);
                    },
                    KeyEvent::DownKeyDown => {
                        set_output_voltage -= 0.1;
                        if set_output_voltage < 0.0 {
                            set_output_voltage = 0.0;
                        }
                        dp.set_output_voltage(set_output_voltage);
                    },
                    KeyEvent::LeftKeyDown => {
                        set_output_voltage -= 0.01;
                        if set_output_voltage < 0.0 {
                            set_output_voltage = 0.0;
                        }
                        dp.set_output_voltage(set_output_voltage);
                    },
                    KeyEvent::DownKeyDownLong => {
                        set_output_voltage = ((set_output_voltage - 1.0) as u32) as f32;
                        if set_output_voltage < 0.0 {
                            set_output_voltage = 0.0;
                        }
                        dp.set_output_voltage(set_output_voltage);
                    },
                    KeyEvent::UpDownKeyCombinationDown => {
                        // Calibration
                        calibration_start = true;
                    },
                    _ => {},
                }
            }
            // if key_event.len() > 0 {
            //     dp.set_message("".to_string(), false);
            // }
        }
        if start_stop_btn == true {
            if load_start == true {
                // to Stop
                logging_start = false;
                load_start = false;
                usbpd_control(&mut i2c_sel, &mut ap33772s, &mut i2cdrv, 0.0, pd_config_offset);
                // clogs.dump();
                // clogs.clear();
            }
            else {
                // to Start
                logging_start = true;
                load_start = true;
                measurement_count = 0;
                previous_set_output_voltage = 0.0;
                info!("Logging and Sending Start..");
                pid.reset();
                clogs.clear();
                dp.enable_display(true);
            }
        }

        let rssi = wifi::get_rssi();
        if rssi == 0 {
            wifi_enable = false;
            if measurement_count % 1000 == 0 {
                wifi_reconnect(&mut wifi_dev.as_mut().unwrap());
            }
        }
        else {
            wifi_enable = true;
        }

        if wifi_enable == false {
            dp.set_wifi_status(WifiStatus::Disconnected);
        }
        else {
            dp.set_wifi_status(WifiStatus::Connected);
        }

        if calibration_start == true {
            dp.set_message("Calibration..".to_string(), true, 0);
            let (current_offset, voltage_offset) = calibration(&mut i2cdrv, current_lsb)?;
            average_current_offset = current_offset;
            average_voltage_offset = voltage_offset;
            dp.set_message("".to_string(), false, 0);
            calibration_start = false;
        }

        if load_start == true {
            pid.set_setpoint(set_output_voltage);
            let diff_setpoint = set_output_voltage - previous_set_output_voltage;
            if diff_setpoint >= 0.1 || diff_setpoint <= -0.1 {
                // Set USB PD Voltage
                info!("Changing USB PD Voltage to {:.2}V from {:.2}V", set_output_voltage, previous_set_output_voltage);
                usbpd_control(&mut i2c_sel, &mut ap33772s, &mut i2cdrv, set_output_voltage, pd_config_offset);
                previous_set_output_voltage = set_output_voltage;
            }
            dp.set_current_status(LoggingStatus::Start);
        }
        else {
            dp.set_current_status(LoggingStatus::Stop);
        }

        // Read Current/Voltage
        let mut data = CurrentLog::default();
        // Timestamp
        let now = SystemTime::now();
        // set clock in ns
        data.clock = now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        // Voltage
        match voltage_read(&mut i2cdrv) {
            Ok(vbus) => {
                data.voltage = vbus - average_voltage_offset;
                // info!("vbus={:?} {:?}V", vbus_buf, data.voltage);
            },
            Err(e) => {
                info!("{:?}", e);
                dp.set_message(format!("{:?}", e), true, 1000);
            }
        }
        // Current
        match current_read(&mut i2cdrv, current_lsb) {
            Ok(current) => {
                data.current = current - average_current_offset;
            },
            Err(e) => {
                info!("{:?}", e);
                dp.set_message(format!("{:?}", e), true, 1000);
            }
        }
        // Power
        match power_read(&mut i2cdrv, current_lsb) {
            Ok(power) => {
                data.power = power;
            },
            Err(e) => {
                info!("{:?}", e);
                dp.set_message(format!("{:?}", e), true, 1000);
            }
        }
        // Current and Power Limit
        if data.current > effective_max_current && load_start == true {
            info!("Current Limit Over: {:.3}A (PDO Limited)", data.current);
            dp.set_message(format!("Current OV {:.3}A", data.current), true, 3000);
            load_start = false;
        }
        if data.power > max_power_limit && load_start == true {
            info!("Power Limit Over: {:.1}W", data.power);
            dp.set_message(format!("Power OV {:.1}W", data.power), true, 3000);
            load_start = false;
        }

        // Temperature
        let temp = temp_pin.read().unwrap() as f32 * 0.05;
        data.temp = temp;
        // Temperature Safety Check
        if temp > max_temperature && load_start == true {
            info!("Temperature Limit Over: {:.1}°C", temp);
            dp.set_message(format!("Temp OV {:.1}°C", temp), true, 3000);
            load_start = false;
        }
        // info!("Temperature: {:.2}°C", temp);
        dp.set_temperature(temp);
        // USB PD Voltage
        let pd_voltage = usb_pd_pin.read().unwrap() as f32 * 0.01125; // (47K + 4.7K) / 4.7K / 1000
        dp.set_usb_pd_voltage(pd_voltage);
        // info!("USB PD Voltage: {:.2}V", pd_voltage);
        dp.set_voltage(data.voltage, data.current, data.power);
        if load_start == false {
            pid.reset();
            pwm_duty = 0;
        }
        else if data.current > effective_max_current {
            // no voltage, over current
            info!("Voltage Off due to over current or load stop {}", data.current);
            pid.reset();
            pwm_duty = 0;
        }
        else {
            // Check voltage overshoot (>110% of setpoint)
            let voltage_overshoot_threshold = set_output_voltage * 1.10;
            if data.voltage > voltage_overshoot_threshold && set_output_voltage > 0.0 {
                info!("Voltage overshoot detected: {:.3}V > {:.3}V (110% of {:.3}V) - Resetting PID", 
                      data.voltage, voltage_overshoot_threshold, set_output_voltage);
                pid.reset();
                // Continue with PID control after reset
            }
            
            // PID Control
            let pid_out = pid.update(data.voltage);
            pwm_duty = (pid_out * (max_duty as f32)) as u32 + pwm_offset;
            if pwm_duty > max_duty {
                pwm_duty = max_duty;
            }
        }
        pwm_driver.set_duty(pwm_duty).expect("Set duty failure");
        // info!("Duty: {} Setpoint: {:.6}V Current Voltage: {:.6}V Diff: {:.6}V", pwm_duty, set_output_voltage, data.voltage, set_output_voltage - data.voltage);
        // PID Control
        dp.set_pwm_duty(pwm_duty);
        data.pwm = pwm_duty;
        if logging_start {
            clogs.record(data);
        }
        let current_record = clogs.get_size();
        if current_record >= 4095 {
            logging_start = false;  // Auto stop logging if buffer is full.
        }
        dp.set_buffer_watermark((current_record as u32) * 100 / 4095);

        if wifi_enable == true && current_record > 0 {
            let logs = clogs.get_all_data();
            let txcount = txd.set_transfer_data(logs);
            if txcount > 0 {
                clogs.remove_data(txcount);
            }
        }
    }
}

fn current_read(i2cdrv: &mut i2c::I2cDriver, current_lsb: f32) -> anyhow::Result<f32> {
    let mut curt_buf  = [0u8; 3];
    i2cdrv.write(0x40, &[0x07u8; 1], BLOCK)?;
    match i2cdrv.read(0x40, &mut curt_buf, BLOCK) {
        Ok(_v) => {
            let current_reg : f32;
            if curt_buf[0] & 0x80 == 0x80 {
                current_reg = (0x100000 - (((curt_buf[0] as u32) << 16 | (curt_buf[1] as u32) << 8 | (curt_buf[2] as u32)) >> 4)) as f32 * -1.0;
            }
            else {
                current_reg = (((curt_buf[0] as u32) << 16 | (curt_buf[1] as u32) << 8 | (curt_buf[2] as u32)) >> 4) as f32;
            }
            return Ok(current_lsb * current_reg);
        },
        Err(e) => {
            info!("{:?}", e);
            return Err(anyhow::anyhow!("Current Read Error"));
        }
    }
}

fn voltage_read(i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<f32> {
    let mut vbus_buf  = [0u8; 3];
    i2cdrv.write(0x40, &[0x05u8; 1], BLOCK)?;
    match i2cdrv.read(0x40, &mut vbus_buf, BLOCK){
        Ok(_v) => {
            let vbus = ((((vbus_buf[0] as u32) << 16 | (vbus_buf[1] as u32) << 8 | (vbus_buf[2] as u32)) >> 4) as f32 * 195.3125) / 1000_000.0;
            // info!("vbus_buf={:?} vbus={:?}", vbus_buf, vbus);
            return Ok(vbus);
        },
        Err(e) => {
            info!("{:?}", e);
            return Err(anyhow::anyhow!("Voltage Read Error"));
        }
    }
}

fn power_read(i2cdrv: &mut i2c::I2cDriver, current_lsb: f32) -> anyhow::Result<f32> {
    let mut power_buf = [0u8; 3];
    i2cdrv.write(0x40, &[0x08u8; 1], BLOCK)?;
    match i2cdrv.read(0x40, &mut power_buf, BLOCK) {
        Ok(_v) => {
            let power_reg = ((power_buf[0] as u32) << 16 | (power_buf[1] as u32) << 8 | (power_buf[2] as u32)) as f32;
            let power = 3.2 * current_lsb * power_reg;
            return Ok(power);
        },
        Err(e) => {
            info!("{:?}", e);
            return Err(anyhow::anyhow!("Power Read Error"));
        }
    }
}

fn write_ina228_reg16(i2cdrv: &mut i2c::I2cDriver, reg: u8, value: u16) -> anyhow::Result<()> {
    let mut config = [0u8; 3];
    config[0] = reg;
    config[1] = (value >> 8) as u8;
    config[2] = value as u8;
    i2cdrv.write(0x40, &config, BLOCK)?;
    Ok(())
}

fn read_ina228_reg16(i2cdrv: &mut i2c::I2cDriver, reg: u8) -> anyhow::Result<u16> {
    let mut data = [0u8; 2];
    i2cdrv.write(0x40, &[reg; 1], BLOCK)?;
    i2cdrv.read(0x40, &mut data, BLOCK)?;
    // info!("INA228 Reg {:02x} Read: {:02x} {:02x}", reg, data[0], data[1]);
    Ok(((data[0] as u16) << 8) | (data[1] as u16))
}


fn usbpd_control(i2c_sel: &mut PinDriver<Gpio46, Output>,
    ap33772s: &mut AP33772S,
    i2cdrv: &mut i2c::I2cDriver,
    voltage: f32,
    pd_config_offset: f32) {

    i2c_sel.set_high().unwrap(); // Enable USB PD
    // USB PD Control
    ap33772_usbpd_control(ap33772s, i2cdrv, voltage, pd_config_offset);
    i2c_sel.set_low().unwrap(); // Disable USB PD

} 

// if output_control is used, USB current will be unstable. 
// fn output_control(i2c_sel: &mut PinDriver<Gpio46, Output>,
//     ap33772s: &mut AP33772S,
//     i2cdrv: &mut i2c::I2cDriver,
//     out: bool) {
//     i2c_sel.set_high().unwrap(); // Enable USB PD
//     match out {
//         true => {
//             // Enable Output
//             match ap33772s.set_vout_auto_control(i2cdrv) {
//                 Ok(()) => {
//                     info!("Set VOUT Auto Control");
//                 },
//                 Err(e) => {
//                     info!("Failed to set VOUT Auto Control: {:?}", e);
//                 }
//             }
//         },
//         false => {
//             // Disable Output
//             match ap33772s.force_vout_off(i2cdrv) {
//                 Ok(()) => {
//                     info!("Forced VOUT Off");
//                 },
//                 Err(e) => {
//                     info!("Failed to force VOUT Off: {:?}", e);
//                 }
//             }
//         }
//     }
//     i2c_sel.set_low().unwrap(); // Disable USB PD
// }

fn ap33772_usbpd_control(ap33772s: &mut AP33772S, i2cdrv: &mut i2c::I2cDriver, voltage: f32, pd_config_offset: f32) {
    // USB PD Control
    // Set voltage
    if voltage <= 0.0 {
        // Disable Output
        let _ = ap33772s.request_voltage(i2cdrv, PDVoltage::V5);
        // ap33772s.force_vout_off(i2cdrv).unwrap();
        return;
    }
    // ap33772s.set_vout_auto_control(i2cdrv).unwrap();
    let mut max_current_limit = 5000; // 5A
    let mut req_voltage = voltage + pd_config_offset;
    let available_voltage = ap33772s.get_max_voltage() as f32 / 1000.0;
    if req_voltage > available_voltage {
        info!("Requested voltage exceeds available voltage: {} > {}", req_voltage, available_voltage);
        req_voltage = available_voltage;
    }
    let pd_voltage = (req_voltage * 1000.0) as u16;
    if req_voltage >= 5.0 {
        // Try to request custom voltage PPS APDO
        match ap33772s.request_custom_voltage(i2cdrv, pd_voltage, max_current_limit) {
            Ok(()) => {
                return;
            },
            Err(e) => {
                info!("Failed to request voltage: {:?}", e);
            }
        }
        // try to request maximum current to be 3A
        max_current_limit = 3000;
        // try to request custom voltage PPS APDO
        match ap33772s.request_custom_voltage(i2cdrv, pd_voltage, max_current_limit) {
            Ok(()) => {
                return;
            },
            Err(e) => {
                info!("Failed to request voltage: {:?}", e);
            }
        }
    }
    else {
        // 5V Fixed PDO
        // This unit needs to power on 5V.
        match ap33772s.request_voltage(i2cdrv, PDVoltage::V5) {
            Ok(()) => {
                return;
            },
            Err(e) => {
                info!("Failed to request 5V: {:?}", e);
            }
        }
    }
}

fn wifi_reconnect(wifi_dev: &mut EspWifi) -> bool{
    unsafe {
        esp_idf_sys::esp_wifi_start();
    }
    match wifi_dev.connect() {
        Ok(_) => { info!("Wifi connecting requested."); true},
        Err(ref e) => { info!("{:?}", e); false }
    }
}

fn calibration(i2cdrv: &mut i2c::I2cDriver, current_lsb: f32) -> anyhow::Result<(f32, f32)> {
    // INA228 Calibration
    // calibration read
    let mut average_current_offset = 0.0;
    let mut voltage_offset = 0.0;
    for _ in 0..300 {
        let read_current = current_read(i2cdrv, current_lsb)?;
        average_current_offset += read_current;
        let read_voltage = voltage_read(i2cdrv)?;
        voltage_offset += read_voltage;
        thread::sleep(Duration::from_millis(10));
    }
    average_current_offset /= 300.0;
    voltage_offset /= 300.0;
    info!("Average Current Offset: {:.3}A Voltage Offset: {:.3}V", average_current_offset, voltage_offset); 
    Ok((average_current_offset, voltage_offset))
}