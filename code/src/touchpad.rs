use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime};
use std::sync::atomic::{AtomicBool, Ordering};
use std::ffi::c_void;
use log::*;

const MAX_TOUCHPADS: usize = 14;
const THRESHOLD_PERCENT: f32 = 0.011;

static TOUCH_ACTIVE_FLAG: AtomicBool = AtomicBool::new(false);

#[allow(dead_code)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone, Copy)]
pub enum KeyEvent {
    UpKeyDown,
    UpKeyUp,
    UpKeyDownLong,
    DownKeyDown,
    DownKeyUp,
    DownKeyDownLong,
    LeftKeyDown,
    LeftKeyUp,
    LeftKeyDownLong,
    RightKeyDown,
    RightKeyUp,
    RightKeyDownLong,
    CenterKeyDown,
    CenterKeyUp,
    CenterKeyDownLong,
    UpDownKeyCombinationDown,
}

#[derive(Debug, Clone)]
pub struct KeyInfo {
    active: bool,
    press_time: SystemTime,
    release_time: SystemTime,
    press_duration: u32,
    release_duration: u32,
    press_threshold: u32,
    press: bool,
    allow_repeat: bool,
    repeat_count: u32,
}

struct KeyState {
    up: KeyInfo,
    down: KeyInfo,
    left: KeyInfo,
    right: KeyInfo,
    center: KeyInfo,
    key_event: Vec<KeyEvent>,
}

#[derive(Debug)]
#[allow(unused)]
enum TouchPadChannel {
    TouchPad1 = 1,
    TouchPad2 = 2,
    TouchPad3 = 3,
    TouchPad4 = 4,
    TouchPad5 = 5,
    TouchPad6 = 6,
    TouchPad7 = 7,
    TouchPad8 = 8,
    TouchPad9 = 9,
    TouchPad10 = 10,
    TouchPad11 = 11,
    TouchPad12 = 12,
    TouchPad13 = 13,
    TouchPad14 = 14,
}

const USE_TOUCH_PAD_CHANNEL : [TouchPadChannel; 5] = [
    TouchPadChannel::TouchPad1,
    TouchPadChannel::TouchPad2,
    TouchPadChannel::TouchPad3,
    TouchPadChannel::TouchPad4,
    TouchPadChannel::TouchPad5,
];

// TouchPad channel mapping to key
const UP_KEY : usize = 3;
const DOWN_KEY : usize = 1;
const LEFT_KEY : usize = 4;
const RIGHT_KEY : usize = 2;
const CENTER_KEY : usize = 5;

struct TouchState {
    smooth_value: [u32; MAX_TOUCHPADS],
}

pub struct TouchPad {
    touch_state: Arc<Mutex<TouchState>>,
    key_state: Arc<Mutex<KeyState>>,
}

unsafe extern "C" fn touch_key_interrupt_handler(_arg: *mut c_void) {
    let intr = esp_idf_sys::touch_pad_read_intr_status_mask();
    if (intr & (esp_idf_sys::touch_pad_intr_mask_t_TOUCH_PAD_INTR_MASK_ACTIVE as u32 |
                esp_idf_sys::touch_pad_intr_mask_t_TOUCH_PAD_INTR_MASK_INACTIVE as u32)
    ) != 0 {
        TOUCH_ACTIVE_FLAG.store(true, Ordering::Relaxed);
    }
}

#[allow(dead_code)]
impl TouchPad {
    pub fn new() -> TouchPad {
        TouchPad { touch_state: Arc::new(Mutex::new(
            TouchState {
                            smooth_value: [0; MAX_TOUCHPADS],
            })),
            key_state: Arc::new(Mutex::new(
                KeyState {
                    up: KeyInfo { active: false, press_time: SystemTime::now(), release_time: SystemTime::now(), press_duration: 0, release_duration: 0, press_threshold: 0, press: false, allow_repeat: false, repeat_count: 0 },
                    down: KeyInfo { active: false, press_time: SystemTime::now(), release_time: SystemTime::now(), press_duration: 0, release_duration: 0, press_threshold: 0, press: false, allow_repeat: false, repeat_count: 0 },
                    left: KeyInfo { active: false, press_time: SystemTime::now(), release_time: SystemTime::now(), press_duration: 0, release_duration: 0, press_threshold: 0, press: false, allow_repeat: false, repeat_count: 0 },
                    right: KeyInfo { active: false, press_time: SystemTime::now(), release_time: SystemTime::now(), press_duration: 0, release_duration: 0, press_threshold: 0, press: false, allow_repeat: false, repeat_count: 0 },
                    center: KeyInfo { active: false, press_time: SystemTime::now(), release_time: SystemTime::now(), press_duration: 0, release_duration: 0, press_threshold: 0, press: false, allow_repeat: false, repeat_count: 0 },
                    key_event: Vec::new(),                         
                })),
        }
    }

    pub fn start(&mut self)
    {
        let touch_state = self.touch_state.clone();
        let key_state = self.key_state.clone();
        let _th = thread::spawn(move || {
            info!("Start TouchPad Read Thread.");
            unsafe {
                esp_idf_sys::touch_pad_init();
                for i in USE_TOUCH_PAD_CHANNEL.iter() {
                    match i {
                        TouchPadChannel::TouchPad1 => {
                            esp_idf_sys::touch_pad_config(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM1);
                        },
                        TouchPadChannel::TouchPad2 => {
                            esp_idf_sys::touch_pad_config(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM2);
                        },
                        TouchPadChannel::TouchPad3 => {
                            esp_idf_sys::touch_pad_config(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM3);
                        },
                        TouchPadChannel::TouchPad4 => {
                            esp_idf_sys::touch_pad_config(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM4);
                        },
                        TouchPadChannel::TouchPad5 => {
                            esp_idf_sys::touch_pad_config(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM5);
                        },
                        _ => {},
                    }
                }
                esp_idf_sys::touch_pad_isr_register(Some(touch_key_interrupt_handler), std::ptr::null_mut(),
                    esp_idf_sys::touch_pad_intr_mask_t_TOUCH_PAD_INTR_MASK_ACTIVE |
                    esp_idf_sys::touch_pad_intr_mask_t_TOUCH_PAD_INTR_MASK_INACTIVE);
                esp_idf_sys::touch_pad_intr_enable(
                    esp_idf_sys::touch_pad_intr_mask_t_TOUCH_PAD_INTR_MASK_ACTIVE |
                    esp_idf_sys::touch_pad_intr_mask_t_TOUCH_PAD_INTR_MASK_INACTIVE);
                esp_idf_sys::touch_pad_set_fsm_mode(esp_idf_sys::touch_fsm_mode_t_TOUCH_FSM_MODE_TIMER);
                esp_idf_sys::touch_pad_fsm_start();
                thread::sleep(Duration::from_millis(100));
                let mut lck = touch_state.lock().unwrap();
                for i in USE_TOUCH_PAD_CHANNEL.iter() {
                    match i {
                        TouchPadChannel::TouchPad1 => {
                            esp_idf_sys::touch_pad_filter_read_smooth(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM1, &mut lck.smooth_value[0]);
                            esp_idf_sys::touch_pad_set_thresh(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM1, (lck.smooth_value[0] as f32 * THRESHOLD_PERCENT) as u32);
                            info!("TouchPad1 threshold: {}", (lck.smooth_value[0] as f32 * THRESHOLD_PERCENT) as u32);
                        },
                        TouchPadChannel::TouchPad2 => {
                            esp_idf_sys::touch_pad_filter_read_smooth(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM2, &mut lck.smooth_value[1]);
                            esp_idf_sys::touch_pad_set_thresh(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM2, (lck.smooth_value[1] as f32 * THRESHOLD_PERCENT) as u32);
                            info!("TouchPad2 threshold: {}", (lck.smooth_value[1] as f32 * THRESHOLD_PERCENT) as u32);
                        },
                        TouchPadChannel::TouchPad3 => {
                            esp_idf_sys::touch_pad_filter_read_smooth(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM3, &mut lck.smooth_value[2]);
                            esp_idf_sys::touch_pad_set_thresh(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM3, (lck.smooth_value[2] as f32 * THRESHOLD_PERCENT) as u32);
                            info!("TouchPad5 threshold: {}", (lck.smooth_value[2] as f32 * THRESHOLD_PERCENT) as u32);
                        },
                        TouchPadChannel::TouchPad4 => {
                            esp_idf_sys::touch_pad_filter_read_smooth(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM4, &mut lck.smooth_value[3]);
                            esp_idf_sys::touch_pad_set_thresh(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM4, (lck.smooth_value[3] as f32 * THRESHOLD_PERCENT) as u32);
                            info!("TouchPad6 threshold: {}", (lck.smooth_value[3] as f32 * THRESHOLD_PERCENT) as u32);
                        },
                        TouchPadChannel::TouchPad5 => {
                            esp_idf_sys::touch_pad_filter_read_smooth(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM5, &mut lck.smooth_value[4]);
                            esp_idf_sys::touch_pad_set_thresh(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM5, (lck.smooth_value[4] as f32 * THRESHOLD_PERCENT) as u32);
                            info!("TouchPad7 threshold: {}", (lck.smooth_value[4] as f32 * THRESHOLD_PERCENT) as u32);
                        },
                        _ => {},
                    }
                }
            }

            loop {
                thread::sleep(Duration::from_millis(100));
                // raw data from touch pad
                // unsafe {
                    // let mut value = 0;
                    // esp_idf_sys::touch_pad_read_raw_data(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM1, &mut value);
                    // info!("TouchPad1 raw data: {}", value);
                    // esp_idf_sys::touch_pad_read_raw_data(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM2, &mut value);
                    // info!("TouchPad2 raw data: {}", value);
                    // esp_idf_sys::touch_pad_read_raw_data(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM5, &mut value);
                    // info!("TouchPad5 raw data: {}", value);
                    // esp_idf_sys::touch_pad_read_raw_data(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM6, &mut value);
                    // info!("TouchPad6 raw data: {}", value);
                    // esp_idf_sys::touch_pad_read_raw_data(esp_idf_sys::touch_pad_t_TOUCH_PAD_NUM7, &mut value);
                    // info!("TouchPad7 raw data: {}", value);
                // }

                if TOUCH_ACTIVE_FLAG.load(Ordering::Relaxed) {
                    let mut keylck = key_state.lock().unwrap();
                    unsafe {
                        let touch_status = esp_idf_sys::touch_pad_get_status();
                        for i in 0..MAX_TOUCHPADS {
                            if touch_status & (1 << i) != 0 {
                                info!("TouchPad{} touched.", i);
                                match i {
                                    UP_KEY => {
                                        keylck.up.active = true;
                                    },
                                    DOWN_KEY => {
                                        keylck.down.active = true;
                                    },
                                    LEFT_KEY => {
                                        keylck.left.active = true;
                                    },
                                    RIGHT_KEY => {
                                        keylck.right.active = true;
                                    },
                                    CENTER_KEY => {
                                        keylck.center.active = true;
                                    },
                                    _ => {},
                                }
                            }
                            else {
                                match i {
                                    UP_KEY => {
                                        keylck.up.active = false;
                                    },
                                    DOWN_KEY => {
                                        keylck.down.active = false;
                                    },
                                    LEFT_KEY => {
                                        keylck.left.active = false;
                                    },
                                    RIGHT_KEY => {
                                        keylck.right.active = false;
                                    },
                                    CENTER_KEY => {
                                        keylck.center.active = false;
                                    },
                                    _ => {},
                                }
                            }
                        }
                    }
                    TOUCH_ACTIVE_FLAG.store(false, Ordering::Relaxed);

                    // check combination of touch pad
                    if keylck.up.active && keylck.down.active {
                        keylck.key_event.push(KeyEvent::UpDownKeyCombinationDown);
                        info!("UpDownKeyCombinationDown");
                    }
                    else {
                        if keylck.up.active {
                            if ! keylck.up.press {
                                keylck.up.press = true;
                                keylck.up.press_time = SystemTime::now();
                                keylck.up.press_duration = 0;
                                keylck.up.release_duration = keylck.up.release_time.elapsed().unwrap().as_millis() as u32;
                                keylck.key_event.push(KeyEvent::UpKeyDown);
                                info!("UpKeyDown");
                            }
                        }
                        else {
                            if keylck.up.press {
                                keylck.up.press = false;
                                keylck.up.press_duration = keylck.up.press_time.elapsed().unwrap().as_millis() as u32;
                                keylck.up.release_time = SystemTime::now();
                                keylck.up.release_duration = 0;
                                keylck.up.repeat_count = 0;
                                keylck.key_event.push(KeyEvent::UpKeyUp);
                                info!("UpKeyUp");
                            }
                        }
                        if keylck.down.active {
                            if ! keylck.down.press {
                                keylck.down.press = true;
                                keylck.down.press_time = SystemTime::now();
                                keylck.down.press_duration = 0;
                                keylck.down.release_duration = keylck.down.release_time.elapsed().unwrap().as_millis() as u32;
                                keylck.key_event.push(KeyEvent::DownKeyDown);
                                info!("DownKeyDown");
                            }
                        }
                        else {
                            if keylck.down.press {
                                keylck.down.press = false;
                                keylck.down.press_duration = keylck.down.press_time.elapsed().unwrap().as_millis() as u32;
                                keylck.down.release_time = SystemTime::now();
                                keylck.down.release_duration = 0;
                                keylck.down.repeat_count = 0;
                                keylck.key_event.push(KeyEvent::DownKeyUp);
                                info!("DownKeyUp");
                            }
                        }
                        if keylck.left.active {
                            if ! keylck.left.press {
                                keylck.left.press = true;
                                keylck.left.press_time = SystemTime::now();
                                keylck.left.press_duration = 0;
                                keylck.left.release_duration = keylck.left.release_time.elapsed().unwrap().as_millis() as u32;
                                keylck.key_event.push(KeyEvent::LeftKeyDown);
                                info!("LeftKeyDown");
                            }
                        }
                        else {
                            if keylck.left.press {
                                keylck.left.press = false;
                                keylck.left.press_duration = keylck.left.press_time.elapsed().unwrap().as_millis() as u32;
                                keylck.left.release_time = SystemTime::now();
                                keylck.left.release_duration = 0;
                                keylck.left.repeat_count = 0;
                                keylck.key_event.push(KeyEvent::LeftKeyUp);
                                info!("LeftUpKeyUp");
                            }
                        }
                        if keylck.right.active {
                            if ! keylck.right.press {
                                keylck.right.press = true;
                                keylck.right.press_time = SystemTime::now();
                                keylck.right.press_duration = 0;
                                keylck.right.release_duration = keylck.right.release_time.elapsed().unwrap().as_millis() as u32;
                                keylck.key_event.push(KeyEvent::RightKeyDown);
                                info!("RightKeyDown");
                            }
                        }
                        else {
                            if keylck.right.press {
                                keylck.right.press = false;
                                keylck.right.press_duration = keylck.right.press_time.elapsed().unwrap().as_millis() as u32;
                                keylck.right.release_time = SystemTime::now();
                                keylck.right.release_duration = 0;
                                keylck.right.repeat_count = 0;
                                keylck.key_event.push(KeyEvent::RightKeyUp);
                                info!("RightKeyUp");
                            }
                        }
                        if keylck.center.active {
                            if ! keylck.center.press {
                                keylck.center.press = true;
                                keylck.center.press_time = SystemTime::now();
                                keylck.center.press_duration = 0;
                                keylck.center.release_duration = keylck.center.release_time.elapsed().unwrap().as_millis() as u32;
                                keylck.key_event.push(KeyEvent::CenterKeyDown);
                                info!("CenterKeyDown");
                            }
                        }
                        else {
                            if keylck.center.press {
                                keylck.center.press = false;
                                keylck.center.press_duration = keylck.center.press_time.elapsed().unwrap().as_millis() as u32;
                                keylck.center.release_time = SystemTime::now();
                                keylck.center.release_duration = 0;
                                keylck.center.repeat_count = 0;
                                keylck.key_event.push(KeyEvent::CenterKeyUp);
                                info!("CenterKeyUp");
                            }
                        }
                    }
                }
                // check press time and generate long press event
                let mut keylck = key_state.lock().unwrap();
                if keylck.up.press_threshold > 0 {
                    if keylck.up.press &&
                        (keylck.up.repeat_count == 0 || (keylck.up.allow_repeat && keylck.up.repeat_count > 0)) {                        
                        let duration = keylck.up.press_time.elapsed().unwrap().as_millis() as u32;
                        if duration > keylck.up.press_threshold {
                            keylck.key_event.push(KeyEvent::UpKeyDownLong);
                            keylck.up.press_time = SystemTime::now();
                            keylck.up.repeat_count += 1;
                            info!("UpKeyDownLong");
                        }
                    }
                }
                if keylck.down.press_threshold > 0 {
                    if keylck.down.press &&
                        (keylck.down.repeat_count == 0 || (keylck.down.allow_repeat && keylck.down.repeat_count > 0)) {
                        let duration = keylck.down.press_time.elapsed().unwrap().as_millis() as u32;
                        if duration > keylck.down.press_threshold {
                            keylck.key_event.push(KeyEvent::DownKeyDownLong);
                            keylck.down.press_time = SystemTime::now();
                            keylck.down.repeat_count += 1;
                            info!("DownKeyDownLong");
                        }
                    }
                }
                if keylck.left.press_threshold > 0 {
                    if keylck.left.press &&
                        (keylck.left.repeat_count == 0 || (keylck.left.allow_repeat && keylck.left.repeat_count > 0)) {
                        let duration = keylck.left.press_time.elapsed().unwrap().as_millis() as u32;
                        if duration > keylck.left.press_threshold {
                            keylck.key_event.push(KeyEvent::LeftKeyDownLong);
                            keylck.left.press_time = SystemTime::now();
                            keylck.left.repeat_count += 1;
                            info!("LeftKeyDownLong");
                        }
                    }
                }
                if keylck.right.press_threshold > 0 {
                    if keylck.right.press &&
                        (keylck.right.repeat_count == 0 || (keylck.right.allow_repeat && keylck.right.repeat_count > 0)) {
                        let duration = keylck.right.press_time.elapsed().unwrap().as_millis() as u32;
                        if duration > keylck.right.press_threshold {
                            keylck.key_event.push(KeyEvent::RightKeyDownLong);
                            keylck.right.press_time = SystemTime::now();
                            keylck.right.repeat_count += 1;
                            info!("RightKeyDownLong");
                        }
                    }
                }
                if keylck.center.press_threshold > 0 {
                    if keylck.center.press &&
                        (keylck.center.repeat_count == 0 || (keylck.center.allow_repeat && keylck.center.repeat_count > 0)) {
                        let duration = keylck.center.press_time.elapsed().unwrap().as_millis() as u32;
                        if duration > keylck.center.press_threshold {
                            keylck.key_event.push(KeyEvent::CenterKeyDownLong);
                            keylck.center.press_time = SystemTime::now();
                            keylck.center.repeat_count += 1;
                            info!("CenterKeyDownLong");
                        }
                    }
                }
                drop(keylck);
            }
        });
    }

    pub fn get_touchpad_status(&mut self, key: Key) -> bool
    {
        let lck = self.key_state.lock().unwrap();
        match key {
            Key::Up => {
                lck.up.press
            },
            Key::Down => {
                lck.down.press
            },
            Key::Left => {
                lck.left.press
            },
            Key::Right => {
                lck.right.press
            },
            Key::Center => {
                lck.center.press
            },
        }
    }
    
    pub fn get_button_press_time(&mut self, key: Key) -> u32
    {
        let lck = self.key_state.lock().unwrap();
        match key {
            Key::Up => {
                lck.up.press_duration
            },
            Key::Down => {
                lck.down.press_duration
            },
            Key::Left => {
                lck.left.press_duration
            },
            Key::Right => {
                lck.right.press_duration
            },
            Key::Center => {
                lck.center.press_duration
            },
        }
    }

    pub fn clear_all_button_event(&mut self)
    {
        let mut lck = self.key_state.lock().unwrap();
        lck.key_event.clear();
    }

    pub fn get_key_event_and_clear(&mut self) -> Vec<KeyEvent>
    {
        let mut lck = self.key_state.lock().unwrap();
        let ret = lck.key_event.clone();
        lck.key_event.clear();
        ret
    }

    pub fn set_press_threshold(&mut self, key: Key, threshold: u32, allow_repeat: bool)
    {
        let mut lck = self.key_state.lock().unwrap();
        match key {
            Key::Up => {
                lck.up.press_threshold = threshold;
                lck.up.allow_repeat = allow_repeat;
            },
            Key::Down => {
                lck.down.press_threshold = threshold;
                lck.down.allow_repeat = allow_repeat;
            },
            Key::Left => {
                lck.left.press_threshold = threshold;
                lck.left.allow_repeat = allow_repeat;
            },
            Key::Right => {
                lck.right.press_threshold = threshold;
                lck.right.allow_repeat = allow_repeat;
            },
            Key::Center => {
                lck.center.press_threshold = threshold;
                lck.center.allow_repeat = allow_repeat;
            },
        }
    }
}