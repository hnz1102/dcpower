// AP33772S USB PD SINK Controller driver wrapper for ESP32
// SPDX-License-Identifier: MIT
// Copyright (c) 2024 Hiroshi Nakajima

//! This module provides a wrapper around the ap33772s-driver crate
//! specifically for ESP32 platforms using esp-idf-hal.

#![allow(dead_code)]

use log::*;
use std::thread;
use std::time::Duration;
use esp_idf_hal::i2c;
use esp_idf_hal::delay::BLOCK;
use embedded_hal::i2c::{I2c, ErrorType};
use embedded_hal::delay::DelayNs;
use esp_idf_sys::EspError;

// Import the driver and types from the ap33772s-driver crate (note: hyphens become underscores)
use ap33772s_driver::AP33772S as GenericAP33772S;
pub use ap33772s_driver::{PDVoltage, PDOInfo, PDStatus};

// Error type wrapper for embedded-hal compatibility
#[derive(Debug)]
pub struct I2cError(pub EspError);

impl embedded_hal::i2c::Error for I2cError {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        embedded_hal::i2c::ErrorKind::Other
    }
}

// Delay implementation for ESP32 using std::thread::sleep
pub struct StdDelay;

impl DelayNs for StdDelay {
    fn delay_ns(&mut self, ns: u32) {
        if ns > 0 {
            let duration = Duration::from_nanos(ns as u64);
            thread::sleep(duration);
        }
    }
}

/// I2C wrapper to adapt esp-idf-hal I2cDriver to embedded-hal traits
struct I2cWrapper<'a, 'b> {
    driver: &'a mut i2c::I2cDriver<'b>,
}

impl<'a, 'b> I2cWrapper<'a, 'b> {
    fn new(driver: &'a mut i2c::I2cDriver<'b>) -> Self {
        Self { driver }
    }
}

impl<'a, 'b> ErrorType for I2cWrapper<'a, 'b> {
    type Error = I2cError;
}

impl<'a, 'b> I2c for I2cWrapper<'a, 'b> {
    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        self.driver.write(address, bytes, BLOCK)
            .map_err(I2cError)
    }

    fn read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.driver.read(address, buffer, BLOCK)
            .map_err(I2cError)
    }

    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        for operation in operations {
            match operation {
                embedded_hal::i2c::Operation::Write(bytes) => {
                    self.write(address, bytes)?;
                }
                embedded_hal::i2c::Operation::Read(buffer) => {
                    self.read(address, buffer)?;
                }
            }
        }
        Ok(())
    }
}

/// ESP32-specific wrapper for the AP33772S driver
/// 
/// This wrapper provides the same interface as the original usbpd.rs
/// but uses the generic ap33772s-driver crate internally.
pub struct AP33772S {
    driver: GenericAP33772S,
}

impl AP33772S {
    /// Create a new AP33772S driver instance
    pub fn new() -> Self {
        AP33772S {
            driver: GenericAP33772S::new(),
        }
    }

    /// Initialize the AP33772S controller
    pub fn init(&mut self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<()> {
        info!("Initializing AP33772S...");
        
        // Create wrapper for the I2C driver
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        
        match self.driver.init(&mut i2c_wrapper) {
            Ok(()) => {
                info!("AP33772S initialized successfully");
                // Log available PDOs
                for pdo in self.driver.get_pdo_list() {
                    let is_epr = pdo.pdo_index >= 8;
                    info!("PDO {}: {}mV, {}mA, {}mW, {}",
                        pdo.pdo_index,
                        pdo.voltage_mv,
                        pdo.current_ma,
                        pdo.max_power_mw,
                        if pdo.is_fixed { 
                            if is_epr { "Fixed EPR" } else { "Fixed SPR" }
                        } else { 
                            if is_epr { "AVS APDO" } else { "PPS APDO" }
                        }
                    );
                }
                info!("Found {} PDOs", self.driver.get_pdo_list().len());
                Ok(())
            },
            Err(e) => {
                error!("Failed to initialize AP33772S: {:?}", e);
                Err(anyhow::anyhow!("AP33772S initialization failed"))
            }
        }
    }
    
    /// Perform a hard reset of the PD connection
    pub fn hard_reset(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<()> {
        info!("Performing hard reset on AP33772S...");
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        
        match self.driver.hard_reset(&mut i2c_wrapper) {
            Ok(()) => {
                // Wait for reset to complete
                thread::sleep(Duration::from_millis(500));
                info!("AP33772S hard reset complete");
                Ok(())
            },
            Err(e) => {
                error!("Hard reset failed: {:?}", e);
                Err(anyhow::anyhow!("Hard reset failed"))
            }
        }
    }

    /// Request specific voltage from the USB PD source using predefined PDO index
    pub fn request_voltage(&self, i2cdrv: &mut i2c::I2cDriver, voltage: PDVoltage) -> anyhow::Result<()> {
        info!("Requesting voltage: {:?}", voltage);
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        let mut delay = StdDelay;
        
        match self.driver.request_voltage(&mut i2c_wrapper, &mut delay, voltage) {
            Ok(()) => {
                info!("Voltage request successful");
                Ok(())
            },
            Err(e) => {
                error!("Voltage request failed: {:?}", e);
                Err(anyhow::anyhow!("Voltage request failed"))
            }
        }
    }

    /// Request custom voltage and current from the USB PD source
    /// This maps to the nearest standard PDVoltage since the generic driver doesn't support arbitrary voltages
    pub fn request_custom_voltage(&self, i2cdrv: &mut i2c::I2cDriver, voltage_mv: u16, _current_ma: u16) -> anyhow::Result<()> {
        info!("Requesting custom voltage: {}mV (mapping to nearest standard voltage)", voltage_mv);
        
        // First, check available PDOs to see if the requested voltage is actually available
        let pdo_list = self.driver.get_pdo_list();
        info!("Available PDOs:");
        for pdo in pdo_list {
            info!("  PDO {}: {}mV, {}mA, {}mW, {}",
                pdo.pdo_index, pdo.voltage_mv, pdo.current_ma, pdo.max_power_mw,
                if pdo.is_fixed { "Fixed" } else { "Variable" });
        }
        
        // Find best matching PDO
        let mut best_pdo: Option<&ap33772s_driver::PDOInfo> = None;
        let mut best_diff = u32::MAX;
        
        for pdo in pdo_list {
            if pdo.voltage_mv >= voltage_mv {
                let diff = (pdo.voltage_mv as u32) - (voltage_mv as u32);
                if diff < best_diff {
                    best_diff = diff;
                    best_pdo = Some(pdo);
                }
            }
        }
        
        if let Some(best_pdo) = best_pdo {
            info!("Found suitable PDO {}: {}mV (requested {}mV)", 
                best_pdo.pdo_index, best_pdo.voltage_mv, voltage_mv);
            
            // Map to nearest standard PDVoltage
            let pd_voltage = if best_pdo.voltage_mv <= 6500 {
                PDVoltage::V5
            } else if best_pdo.voltage_mv <= 10500 {
                PDVoltage::V9
            } else if best_pdo.voltage_mv <= 13500 {
                PDVoltage::V12
            } else if best_pdo.voltage_mv <= 17500 {
                PDVoltage::V15
            } else if best_pdo.voltage_mv <= 24000 {
                PDVoltage::V20
            } else if best_pdo.voltage_mv <= 32000 {
                PDVoltage::V28
            } else if best_pdo.voltage_mv <= 38000 {
                PDVoltage::V36
            } else if best_pdo.voltage_mv <= 44000 {
                PDVoltage::V40
            } else {
                PDVoltage::V48
            };
            
            info!("Mapped {}mV to {:?}", best_pdo.voltage_mv, pd_voltage);
            self.request_voltage(i2cdrv, pd_voltage)
        } else {
            error!("No suitable PDO found for voltage {}mV", voltage_mv);
            Err(anyhow::anyhow!("No suitable PDO found for requested voltage"))
        }
    }

    /// Read the current status of the PD controller
    pub fn get_status(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<PDStatus> {
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        
        match self.driver.get_status(&mut i2c_wrapper) {
            Ok(status) => Ok(status),
            Err(e) => {
                error!("Get status failed: {:?}", e);
                Err(anyhow::anyhow!("Get status failed"))
            }
        }
    }

    /// Get current voltage in volts (convenience method)
    pub fn get_voltage_v(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<f32> {
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        match self.driver.get_status(&mut i2c_wrapper) {
            Ok(status) => Ok(status.voltage_mv as f32 / 1000.0),
            Err(_) => Err(anyhow::anyhow!("Failed to get voltage"))
        }
    }

    /// Get current in amperes (convenience method)
    pub fn get_current_a(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<f32> {
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        match self.driver.get_status(&mut i2c_wrapper) {
            Ok(status) => Ok(status.current_ma as f32 / 1000.0),
            Err(_) => Err(anyhow::anyhow!("Failed to get current"))
        }
    }

    /// Get power in watts (convenience method)
    pub fn get_power_w(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<f32> {
        let voltage = self.get_voltage_v(i2cdrv)?;
        let current = self.get_current_a(i2cdrv)?;
        Ok(voltage * current)
    }

    /// Set voltage using float value in volts (convenience method)
    pub fn set_voltage_v(&self, i2cdrv: &mut i2c::I2cDriver, voltage: f32) -> anyhow::Result<()> {
        let pd_voltage = match voltage {
            v if v <= 5.5 => PDVoltage::V5,
            v if v <= 9.5 => PDVoltage::V9,
            v if v <= 12.5 => PDVoltage::V12,
            v if v <= 15.5 => PDVoltage::V15,
            v if v <= 20.5 => PDVoltage::V20,
            v if v <= 28.5 => PDVoltage::V28,
            v if v <= 36.5 => PDVoltage::V36,
            v if v <= 40.5 => PDVoltage::V40,
            v if v <= 48.5 => PDVoltage::V48,
            _ => return Err(anyhow::anyhow!("Voltage {} V out of range", voltage)),
        };
        
        self.request_voltage(i2cdrv, pd_voltage)
    }

    /// Get maximum voltage available
    pub fn get_max_voltage(&self) -> u16 {
        self.driver.get_max_voltage()
    }

    /// Set custom voltage and current using float values (convenience method)
    pub fn set_custom_voltage_v(&self, i2cdrv: &mut i2c::I2cDriver, voltage: f32, current: f32) -> anyhow::Result<()> {
        let voltage_mv = (voltage * 1000.0) as u16;
        let current_ma = (current * 1000.0) as u16;
        self.request_custom_voltage(i2cdrv, voltage_mv, current_ma)
    }

    /// Get available PDO information
    pub fn get_pdo_list(&self) -> &[PDOInfo] {
        self.driver.get_pdo_list()
    }

    /// Get temperature in degrees Celsius
    pub fn get_temperature_c(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<i8> {
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        match self.driver.get_status(&mut i2c_wrapper) {
            Ok(status) => Ok(status.temperature),
            Err(_) => Err(anyhow::anyhow!("Failed to get temperature"))
        }
    }

    /// Configure the protection features
    pub fn configure_protections(
        &self,
        i2cdrv: &mut i2c::I2cDriver,
        enable_uvp: bool,
        enable_ovp: bool,
        enable_ocp: bool,
        enable_otp: bool,
        enable_dr: bool,
    ) -> anyhow::Result<()> {
        info!("Configuring protections: UVP={}, OVP={}, OCP={}, OTP={}, DR={}", 
            enable_uvp, enable_ovp, enable_ocp, enable_otp, enable_dr);
        
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        
        match self.driver.configure_protections(
            &mut i2c_wrapper,
            enable_uvp,
            enable_ovp,
            enable_ocp,
            enable_otp,
            enable_dr,
        ) {
            Ok(()) => {
                info!("Protection configuration successful");
                Ok(())
            },
            Err(e) => {
                error!("Configure protections failed: {:?}", e);
                Err(anyhow::anyhow!("Configure protections failed"))
            }
        }
    }

    /// Set VOUT to auto control
    pub fn set_vout_auto_control(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<()> {
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        
        match self.driver.set_vout_auto_control(&mut i2c_wrapper) {
            Ok(()) => {
                info!("VOUT auto control enabled");
                Ok(())
            },
            Err(e) => {
                error!("Set auto control failed: {:?}", e);
                Err(anyhow::anyhow!("Set auto control failed"))
            }
        }
    }
    
    /// Force VOUT OFF
    pub fn force_vout_off(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<()> {
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        
        match self.driver.force_vout_off(&mut i2c_wrapper) {
            Ok(()) => {
                info!("VOUT forced OFF");
                Ok(())
            },
            Err(e) => {
                error!("Force VOUT OFF failed: {:?}", e);
                Err(anyhow::anyhow!("Force VOUT OFF failed"))
            }
        }
    }
    
    /// Force VOUT ON
    pub fn force_vout_on(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<()> {
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        
        match self.driver.force_vout_on(&mut i2c_wrapper) {
            Ok(()) => {
                info!("VOUT forced ON");
                Ok(())
            },
            Err(e) => {
                error!("Force VOUT ON failed: {:?}", e);
                Err(anyhow::anyhow!("Force VOUT ON failed"))
            }
        }
    }

    /// Dump register values for debugging
    pub fn dump_registers(&self, i2cdrv: &mut i2c::I2cDriver) -> anyhow::Result<()> {
        info!("Register dump functionality moved to generic driver");
        // The generic driver doesn't expose individual register access
        // as it's abstracted away. For debugging, use get_status() instead.
        let mut i2c_wrapper = I2cWrapper::new(i2cdrv);
        match self.driver.get_status(&mut i2c_wrapper) {
            Ok(status) => {
                info!("Status dump - Voltage: {}mV, Current: {}mA, Temperature: {}°C", 
                    status.voltage_mv, status.current_ma, status.temperature);
                info!("Fault status: {:?}, Attached: {}, Busy: {}", 
                    status.fault_type, status.is_attached, status.is_busy);
                Ok(())
            },
            Err(_) => Err(anyhow::anyhow!("Failed to dump registers"))
        }
    }
}

impl Default for AP33772S {
    fn default() -> Self {
        Self::new()
    }
}
