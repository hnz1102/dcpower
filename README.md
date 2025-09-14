<div align="center">
  <h1><code>DC Power Unit for Breadboard</code></h1>
  <p>
    <img src="doc/dcpowerstation.png"/>
  </p>
</div>

# DC Power Unit for Breadboard

This is a DC Power Unit for Breadboard based on ESP32-S3 microcontroller and AP33772S USB Power Delivery (PD) Sink Controller. It can supply power to the breadboard from USB Power Delivery (PD) Charger with support for both Standard Power Range (SPR) and Extended Power Range (EPR) modes. The output voltage can be controlled from 0v to 20V in SPR mode and up to 48V in EPR mode with step of 10mV.   

## Key Features

- **High Power Delivery**: Up to 100W from USB PD Charger
- **Multiple Voltage Outputs**: 0V to 20V in SPR mode, 0V to 48V in EPR mode (10mV steps)
- **High Current Support**: Up to 5A when PD Charger supports EPR
- **Touch Control Interface**: Front panel touch switch for voltage selection and output control
- **Precise Control**: PID controller maintains constant output voltage
- **High Accuracy Monitoring and Control**: 10mV voltage accuracy using shunt resistor and INA228 current sensor, with 1mV resolution.
- **Real-time Display**: OLED display shows voltage, current, power consumption, and temperature
- **Data Logging**: WiFi connectivity for sending measurement data to InfluxDB server
- **Web Dashboard**: Data visualization using InfluxDB dashboard

**Note**: Some PD chargers may not support EPR mode or only support up to 28V/5A. Please check your PD charger specifications.

## Hardware Components

- **ESP32-S3-WROOM-1-N16R8**: Main microcontroller
- **AP33772S**: USB Power Delivery Sink Controller with EPR support
- **INA228**: High-precision current/voltage sensor
- **SSD1331**: Color OLED display (96x64 pixels)
- **Touch Interface**: Capacitive touch sensors for user interaction
- **MOSFET Load Circuit**: Electronic load for testing and regulation

## Software Architecture

The firmware is written in Rust using the ESP-IDF framework and consists of several modules:

- `main.rs`: Main application logic and system initialization
- `usbpd.rs`: AP33772S USB-PD driver interface using the ap33772s-driver crate
- `displayctl.rs`: OLED display control and user interface
- `currentlogs.rs`: Current and voltage measurement using INA228
- `touchpad.rs`: Touch sensor interface and user input handling
- `pidcont.rs`: PID controller for voltage regulation
- `wifi.rs`: WiFi connectivity and network management
- `transfer.rs`: Data transmission to InfluxDB server
- `syslogger.rs`: System logging functionality


## How to Use the Unit

### Basic Operation

1. **Connection**: Connect a USB-C PD charger to the input port
2. **Power On**: The unit automatically detects the PD source and displays available power profiles
3. **Voltage Selection**: Use the touch interface to select desired output voltage
4. **Output Control**: Press the center touch position to enable/disable output
5. **Monitoring**: View real-time measurements on the OLED display

### Touch Interface Controls

- **Up/Down Touch**: Increase or decrease output voltage in 100mV steps, long press for 1V steps
- **Left/Right Touch**: Increase or decrease output voltage in 10mV steps
- **Center Touch**: Long press to toggle output ON/OFF
- **Display Information**: Shows current voltage, current, power, unit temperature, and WiFi status

### Safety Features

- Under Voltage Protection (UVP)
- Over Voltage Protection (OVP) 
- Over Current Protection (OCP)
- Over Temperature Protection (OTP)
- Automatic fault detection and shutdown

These protections are implemented by the AP33772S.

## Dependencies and Crates

The project uses the custom `ap33772s-driver` crate for USB-PD communication:

```toml
[dependencies]
ap33772s-driver = { version = "0.1.1", features = ["std"] }
```

Other key dependencies include:
- `esp-idf-hal` v0.45.2: ESP32 hardware abstraction
- `embedded-hal` v1.0.0: Platform-agnostic hardware interfaces
- `ssd1331` v0.3.0: OLED display driver
- `embedded-graphics` v0.7: Graphics primitives for display
- `chrono` v0.4.38: Date and time handling
- `toml-cfg` v0.1.3: Configuration file parsing


# How to build from code and Install to the unit.

Using Ubuntu 22.04.3 LTS and ESP-IDF V5.4.2

## Prerequisites
Ensure that your system meets the following requirements before proceeding with the installation:
- Operating System: Linux-based distribution
- Required Packages: git, python3, python3-pip, gcc, build-essential, curl, pkg-config, libudev-dev, libtinfo5, clang, libclang-dev, llvm-dev, udev, libssl-dev, python3.10-venv

## Installation Steps

### 1. System Update and Package Installation
Update your system and install the necessary packages using:
```bash
sudo apt update && sudo apt -y install git python3 python3-pip gcc build-essential curl pkg-config libudev-dev libtinfo5 clang libclang-dev llvm-dev udev libssl-dev python3.10-venv
```

### 2. Rust Installation
Install Rust programming language and Cargo package manager:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
After installation, activate Rust by sourcing the environment:
```bash
. "$HOME/.cargo/env"
```

### 3. Additional Tools Installation
Install the following Rust tools:
- ldproxy
- espup
- cargo-espflash

Use the following commands:
```bash
cargo install ldproxy
cargo install espup
cargo install cargo-espflash
```

At this time (2025-07-25), espup cannot be compiled. If you get an error, please use the following command to install the toolchain.
```bash
cargo install cargo-binstall
cargo binstall espup
```

### 4. ESP Environment Setup
Run the following command to install and update the Espressif Rust ecosystem:
```bash
espup install
espup update
```
Set up environment variables:
```bash
. ./export-esp.sh
```

### 5. Udev Rules Configuration
Configure udev rules for device permissions:
```bash
sudo sh -c 'echo "SUBSYSTEMS==\"usb\", ATTRS{idVendor}==\"303a\", ATTRS{idProduct}==\"1001\", MODE=\"0666\"" > /etc/udev/rules.d/99-esp32.rules'
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### 6. Clone Repository
Clone the DC Power Unit repository:
```bash
git clone https://github.com/hnz1102/dcpower.git
cd dcpower/code
```

### 7. Setting WiFi SSID, Password, etc.
Change the following configuration file: `cfg.toml`
You have to set the following parameters: WiFi SSID, Password, InfluxDB Server IP Address, InfluxDB API Key, and InfluxDB API with your ORG.
You can get the API Key from the InfluxDB Web Console. Please see the 'How to Install the InfluxDB and Configure the Dashboard' section No.3.

```toml
[dcpowerunit]
wifi_ssid = "<Your AP SSID>"  # Set your AP SSID
wifi_psk = "<Your AP Password>" # Set your AP Password
influxdb_server = "<InfluxDB Server IP Address:Port>" # Set your InfluxDB Server IP Address and Port ex. 192.168.1.100:8086
pid_kp = "0.0000005"
pid_ki = "0.00002"
pid_kd = "0.1"
pwm_offset = "0"
pd_config_offset = "2.5"
shunt_resistance = "0.005"
shunt_temp_coefficient = "50"
max_current_limit = "5.2"
max_power_limit = "100.0"
influxdb_api_key = "<InfluxDB API KEY>" # Set your InfluxDB API Key
influxdb_api = "/api/v2/write?org=<ORG>&bucket=LOGGER&precision=ns" # Set your InfluxDB API with your ORG and BUCKET
influxdb_tag = "dcpowerunit"  # Tag for InfluxDB measurements
influxdb_measurement = "dcpowerunit" # Measurement name for InfluxDB
syslog_server = "<Syslog Server IP Address:Port>" # Set your Syslog Server IP Address and Port ex. 192.168.2.140:514
syslog_enable = "false" # Set to "true" to enable syslog
```

### 8. Build and Flash
Build the project:
```bash
cargo build --release
```

### 9. Flash the Firmware
Connect the DC Power Unit to your PC using a USB cable. Then, flash the firmware:
```bash
cargo espflash flash --release --monitor
```

If your device is not detected, power on the device and during the boot, press the `BOOT` button.
Then, run the flash command again.

### 10. Monitor the Output
After flashing the firmware, the console shows the booting messages and system initialization including:
- WiFi connection status
- AP33772S USB-PD controller initialization
- Available PDO (Power Data Object) detection
- Touch interface activation
- OLED display initialization
```bash
[2024-11-24T08:43:44Z INFO ] 🚀 A new version of cargo-espflash is available: v3.2.0
[2024-11-24T08:43:44Z INFO ] Serial port: '/dev/ttyACM0'
[2024-11-24T08:43:44Z INFO ] Connecting...
[2024-11-24T08:43:44Z INFO ] Using flash stub
Finished `release` profile [optimized] target(s) in 0.19s
Chip type:         esp32s3 (revision v0.1)
Crystal frequency: 40 MHz
Flash size:        16MB
Features:          WiFi, BLE
MAC address:       xx:xx:xx:xx:xx:xx
Bootloader:        /esp32/electricload/code/target/xtensa-esp32s3-espidf/release/build/esp-idf-sys-37f4c3bc37bda4bb/out/build/bootloader/bootloader.bin
Partition table:   partitions.csv
App/part. size:    1,358,304/15,728,640 bytes, 8.64%
[2024-11-24T08:43:45Z INFO ] Segment at address '0x0' has not changed, skipping write
[2024-11-24T08:43:45Z INFO ] Segment at address '0x8000' has not changed, skipping write
[2024-11-24T08:43:46Z INFO ] Segment at address '0x10000' has not changed, skipping write
[2024-11-24T08:43:46Z INFO ] Flashing has completed!
Commands:
    CTRL+R    Reset chip
    CTRL+C    Exit

ESP-ROM:esp32s3-20210327
Build:Mar 27 2021
rst:0x15 (USB_UART_CHIP_RESET),boot:0x8 (SPI_FAST_FLASH_BOOT)
Saved PC:0x40378bb6
0x40378bb6 - rtc_isr
    at ??:??
SPIWP:0xee
mode:DIO, clock div:2
load:0x3fce3810,len:0x178c
load:0x403c9700,len:0x4
load:0x403c9704,len:0xcbc
load:0x403cc700,len:0x2d9c
entry 0x403c9914
I (27) boot: ESP-IDF v5.2.2 2nd stage bootloader
I (27) boot: compile time Nov 22 2024 19:30:56
I (27) boot: Multicore bootloader
I (30) boot: chip revision: v0.1
I (34) boot.esp32s3: Boot SPI Speed : 40MHz
I (39) boot.esp32s3: SPI Mode       : DIO
I (44) boot.esp32s3: SPI Flash Size : 16MB
I (49) boot: Enabling RNG early entropy source...
I (54) boot: Partition Table:
I (58) boot: ## Label            Usage          Type ST Offset   Length
I (65) boot:  0 nvs              WiFi data        01 02 00009000 00006000
I (72) boot:  1 phy_init         RF data          01 01 0000f000 00001000
I (80) boot:  2 factory          factory app      00 00 00010000 00f00000
I (87) boot: End of partition table
I (91) esp_image: segment 0: paddr=00010020 vaddr=3c0e0020 size=52034h (335924) map
I (183) esp_image: segment 1: paddr=0006205c vaddr=3fc9ae00 size=04ce8h ( 19688) load
I (189) esp_image: segment 2: paddr=00066d4c vaddr=40374000 size=092cch ( 37580) load
I (200) esp_image: segment 3: paddr=00070020 vaddr=42000020 size=ddea0h (908960) map
I (427) esp_image: segment 4: paddr=0014dec8 vaddr=4037d2cc size=0dae8h ( 56040) load
I (453) boot: Loaded app from partition at offset 0x10000
I (453) boot: Disabling RNG early entropy source...
I (464) cpu_start: Multicore app
I (465) octal_psram: vendor id    : 0x0d (AP)
I (465) octal_psram: dev id       : 0x02 (generation 3)
I (468) octal_psram: density      : 0x03 (64 Mbit)
I (473) octal_psram: good-die     : 0x01 (Pass)
I (478) octal_psram: Latency      : 0x01 (Fixed)
I (484) octal_psram: VCC          : 0x01 (3V)
I (489) octal_psram: SRF          : 0x01 (Fast Refresh)
I (495) octal_psram: BurstType    : 0x01 (Hybrid Wrap)
I (500) octal_psram: BurstLen     : 0x01 (32 Byte)
I (506) octal_psram: Readlatency  : 0x02 (10 cycles@Fixed)
I (512) octal_psram: DriveStrength: 0x00 (1/1)
I (518) MSPI Timing: PSRAM timing tuning index: 4
I (523) esp_psram: Found 8MB PSRAM device
I (527) esp_psram: Speed: 80MHz
I (543) cpu_start: Pro cpu start user code
I (543) cpu_start: cpu freq: 160000000 Hz
I (543) cpu_start: Application information:
I (546) cpu_start: Project name:     libespidf
I (551) cpu_start: App version:      9e8d2c8-dirty
I (557) cpu_start: Compile time:     Nov 22 2024 19:30:46
I (563) cpu_start: ELF file SHA256:  000000000...
I (568) cpu_start: ESP-IDF:          v5.2.2
I (573) cpu_start: Min chip rev:     v0.0
I (578) cpu_start: Max chip rev:     v0.99 
I (582) cpu_start: Chip rev:         v0.1
I (587) heap_init: Initializing. RAM available for dynamic allocation:
I (594) heap_init: At 3FCA4390 len 00045380 (276 KiB): RAM
I (600) heap_init: At 3FCE9710 len 00005724 (21 KiB): RAM
I (607) heap_init: At 3FCF0000 len 00008000 (32 KiB): DRAM
I (613) heap_init: At 600FE010 len 00001FD8 (7 KiB): RTCRAM
I (619) esp_psram: Adding pool of 8192K of PSRAM memory to heap allocator
I (627) spi_flash: detected chip: gd
I (631) spi_flash: flash io: dio
W (635) pcnt(legacy): legacy driver is deprecated, please migrate to `driver/pulse_cnt.h`
W (643) i2c: This driver is an old driver, please migrate your application code to adapt `driver/i2c_master.h`
W (654) timer_group: legacy driver is deprecated, please migrate to `driver/gptimer.h`
W (663) ADC: legacy driver is deprecated, please migrate to `esp_adc/adc_oneshot.h`
I (671) sleep: Configure to isolate all GPIO pins in sleep state
I (678) sleep: Enable automatic switching of GPIO sleep configuration
I (685) main_task: Started on CPU0
I (695) esp_psram: Reserving pool of 32K of internal memory for DMA/internal allocations
I (695) main_task: Calling app_main()
I (715) electricload: [Limit] Current: 15A  Power: 105W
I (715) gpio: GPIO[15]| InputEn: 0| OutputEn: 0| OpenDrain: 0| Pullup: 0| Pulldown: 0| Intr:0 
I (725) gpio: GPIO[16]| InputEn: 0| OutputEn: 0| OpenDrain: 0| Pullup: 0| Pulldown: 0| Intr:0 
I (735) electricload::displayctl: Start Display Thread.
I (735) electricload: INA228 Config: FB6A
I (735) electricload: current_lsb=3.125e-5 shunt_cal_val=2048.0 shunt_cal=2048
I (745) electricload: Max duty: 16383
I (755) electricload: Max duty: 255
I (755) gpio: GPIO[42]| InputEn: 0| OutputEn: 0| OpenDrain: 0| Pullup: 0| Pulldown: 0| Intr:0 
I (765) gpio: GPIO[41]| InputEn: 0| OutputEn: 0| OpenDrain: 0| Pullup: 0| Pulldown: 0| Intr:0 
I (775) electricload::pulscount: Start puls count thread.
I (785) pp: pp rom version: e7ae62f
I (785) net80211: net80211 rom version: e7ae62f
I (805) wifi:wifi driver task: 3fcc2b98, prio:23, stack:6656, core=0
I (805) wifi:wifi firmware version: 3e0076f
I (805) wifi:wifi certification version: v7.0
I (805) wifi:config NVS flash: disabled
I (805) wifi:config nano formating: disabled
I (815) wifi:Init data frame dynamic rx buffer num: 32
I (815) wifi:Init static rx mgmt buffer num: 10
I (825) wifi:Init management short buffer num: 32
I (825) wifi:Init static tx buffer num: 16
I (825) wifi:Init tx cache buffer num: 32
I (835) wifi:Init static tx FG buffer num: 2
I (835) wifi:Init static rx buffer size: 1600
I (845) wifi:Init static rx buffer num: 10
I (845) wifi:Init dynamic rx buffer num: 32
I (845) wifi_init: rx ba win: 6
I (965) wifi_init: tcpip mbox: 32
I (965) wifi_init: udp mbox: 6
I (965) wifi_init: tcp mbox: 6
I (965) wifi_init: tcp tx win: 5760
I (975) wifi_init: tcp rx win: 5760
I (975) wifi_init: tcp mss: 1440
I (985) wifi_init: WiFi IRAM OP enabled
I (985) wifi_init: WiFi RX IRAM OP enabled
I (995) phy_init: phy_version 670,b7bc9b9,Apr 30 2024,10:54:13
I (1035) wifi:mode : sta ()
I (1035) wifi:enable tsf
I (3985) wifi:new:<4,0>, old:<1,0>, ap:<255,255>, sta:<4,0>, prof:1
I (3985) wifi:state: init -> auth (b0)
I (3985) wifi:state: auth -> assoc (0)
I (3995) wifi:state: assoc -> run (10)
I (4005) wifi:connected with xxxxxxxx, aid = 7, channel 4, BW20, bssid = xx:xx:xx:xx:xx:xx
I (4005) wifi:security: WPA2-PSK, phy: bgn, rssi: -36
I (4005) wifi:pm start, type: 1
I (4005) wifi:dp: 1, bi: 102400, li: 3, scale listen interval from 307200 us to 307200 us
I (4015) wifi:set rx beacon pti, rx_bcn_pti: 0, bcn_timeout: 25000, mt_pti: 0, mt_time: 10000
I (4025) wifi:AP's beacon interval = 102400 us, DTIM period = 1
I (4065) dcpowerunit::wifi: Wifi connected
I (4065) esp_idf_svc::sntp: Initializing
I (4065) esp_idf_svc::sntp: Initialization complete
I (4065) dcpowerunit: NTP Sync Start..
I (7475) dcpowerunit: NTP Sync Completed: 2024-11-24 08:43:53
I (7475) dcpowerunit::transfer: Start transfer thread.
I (7475) dcpowerunit::touchpad: Start TouchPad Read Thread.
I (7485) gpio: GPIO[18]| InputEn: 0| OutputEn: 0| OpenDrain: 0| Pullup: 0| Pulldown: 0| Intr:0 
I (7495) dcpowerunit: PID Controller: KP=0.001 KI=0.022 KD=0.00001
I (7495) dcpowerunit::usbpd: Initializing AP33772S USB-PD controller
I (7505) dcpowerunit::usbpd: USB-PD controller initialized successfully
I (7515) dcpowerunit::usbpd: Available PDOs:
I (7515) dcpowerunit::usbpd:   PDO 1: 5000mV, 3000mA, 15000mW, Fixed
I (7525) dcpowerunit::usbpd:   PDO 2: 9000mV, 3000mA, 27000mW, Fixed
I (7535) dcpowerunit::usbpd:   PDO 3: 12000mV, 3000mA, 36000mW, Fixed
I (7545) dcpowerunit::usbpd:   PDO 4: 15000mV, 3000mA, 45000mW, Fixed
I (7555) dcpowerunit::usbpd:   PDO 5: 20000mV, 5000mA, 100000mW, Fixed
I (7565) dcpowerunit::usbpd:   PDO 8: 28000mV, 5000mA, 140000mW, Fixed
I (7585) dcpowerunit::touchpad: TouchPad4 threshold: 1529
I (7585) electricload::touchpad: TouchPad5 threshold: 1519
I (7585) electricload::touchpad: TouchPad6 threshold: 1433
I (7585) electricload::touchpad: TouchPad7 threshold: 1523
I (7595) electricload::touchpad: TouchPad8 threshold: 297
I (7605) electricload::touchpad: TouchPad3 threshold: 1436
I (7605) electricload::touchpad: TouchPad9 threshold: 291
I (7625) electricload::touchpad: TouchPad1 threshold: 325
I (7625) electricload::touchpad: TouchPad2 threshold: 1619
I (7625) electricload::touchpad: TouchPad14 threshold: 313
I (7645) electricload::touchpad: TouchPad13 threshold: 307
I (7645) electricload::touchpad: TouchPad12 threshold: 302
I (7645) electricload::touchpad: TouchPad11 threshold: 287
I (7645) electricload::touchpad: TouchPad10 threshold: 281
I (7665) electricload::touchpad: TouchPad charge discharge times: 500 -> 1000
```

## How to Install the influxDB and Configure the Dashboard

### 1. Download [influxDB](https://docs.influxdata.com/influxdb/v2.7/install/?t=Linux) and Install.
```bash
$ wget https://dl.influxdata.com/influxdb/releases/influxdb2-2.7.0-amd64.deb
$ sudo dpkg -i influxdb2-2.7.0-amd64.deb
$ sudo service influxdb start
```

### 2. Configure the influxDB

```
Connect to the 'http://<influxDB installed PC Address>:8086'
```
Click `GET STARTED` and set `Username`, `Password`, `Initial Organization Name`, and `Initial Bucket Name`
|Term|Value|
|---|---|
|Username|Set login username as influxDB administrator web console|
|Password|Set login password as influxDB administrator web console|
|Initial Organization Name| Organization Name ex. ORG|
|Initial Bucket Name| LOGGER |

After set them, click `CONTINUE`.

### 3. Copy the operator API token.

You can see the operator API token on the browser. YOU WON'T BE ABLE TO SEE IT AGAIN!
If you want to get new API token, click `API Tokens` menu form `Sources` Icon, then click `GENERATE API TOKEN` and select `All access token`, click `Save`.
You can see a new API token and get it.
After copy the token, click `CONFIGURE LATER`.

### 4. Import the Dashboard Template.

Click the `Dashboard` icon, and select `Import Dashboard` from the `CREATE DASHBOARD` menu.

Drop the `influxdb/electric_load.json` file to `Drop a file here`, then click `IMPORT JSON AS DASHBOARD`.

You can see the `ELECTRIC LOAD` pannel on the Dashboards page.

Click this panel, and You can see the dashboard.

If you want to customize the dashboard design, click configure mark. You can change the graph design.

## Schematic, PCB Gabar Data

There is a Schematic data in hardware directory. 
If you want to make the PCB, you can order the [PCBway](https://www.pcbway.com/project/shareproject/Digitally_Controlled_Electric_Load_504eb052.html) this link.
The heat sink is not included in the schematic data. You can use the heat sink with the fan for LGA115x CPU Cooler. 

I used this [heat sink](https://www.ainex.jp/products/cc-06b/)

I guess another [heat sink](https://www.tronwire.com/collections/tronwire-cpu-coolers/products/tw-10) is also good.

This PCB is designed by [Kicad](https://www.kicad.org/). This board image photo is shown the jumper wire. But, the PCB data is already fixed the error.

UPDATE 2024-12-29: I changed the OpAmp from TSB6111ILT to OPA187IDBVR. OPA187IDBVR is a low noise and low offset voltage OpAmp. It can be used for the high precision control.

## LICENSE
This source code is licensed under MIT. Other Hardware Schematic documents are licensed under CC-BY-SA V4.0.
