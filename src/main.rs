use evdev::uinput::VirtualDevice;
use evdev::{AbsInfo, AbsoluteAxisCode, AttributeSet, Device, KeyCode, UinputAbsSetup};
use parking_lot::Mutex;
use std::env;
use std::{sync::Arc, thread, time::Duration};
use std::error::Error;

// --- CONFIGURATION ---
const PRIMARY_CONTROLLER_NAME: &str = "Xbox Wireless Controller";
const SECONDARY_CONTROLLER_NAME: &str = "RealityRunner Treadmill Sensor";
// ---------------------

fn find_device_by_name(name: &str) -> Result<Device, Box<dyn Error>> {
    for i in 0..32 {
        let path = format!("/dev/input/event{}", i);
        if let Ok(device) = Device::open(&path) 
            && device.name().unwrap_or_default().contains(name) {
            println!("Found '{}' at path: {}", name, path);
            return Ok(device);
        }
    }
    Err(format!("Could not find controller named '{}'. Check device name or ensure permissions are set.", name).into())
}

fn setup_virtual_device() -> Result<VirtualDevice, Box<dyn Error>> {
    println!("Creating virtual 'Muxed Controller'...");
    let stick_info = AbsInfo::new(0, -32768, 32767, 16, 128, 0);
    let trigger_info = AbsInfo::new(0, 0, 1023, 0, 0, 0);

    let mut builder = VirtualDevice::builder()?
        .name("Muxed Controller")
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, stick_info))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, stick_info))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_RX, stick_info))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_RY, stick_info))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Z, trigger_info))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_RZ, trigger_info))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0X, trigger_info))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_HAT0Y, trigger_info))?;

    let mut buttons: AttributeSet<KeyCode> = AttributeSet::default();
    buttons.insert(KeyCode::BTN_SOUTH);
    buttons.insert(KeyCode::BTN_NORTH);
    buttons.insert(KeyCode::BTN_EAST);
    buttons.insert(KeyCode::BTN_WEST);
    buttons.insert(KeyCode::BTN_SELECT);
    buttons.insert(KeyCode::BTN_START);
    buttons.insert(KeyCode::BTN_MODE);
    buttons.insert(KeyCode::BTN_TL);
    buttons.insert(KeyCode::BTN_TR);
    buttons.insert(KeyCode::BTN_THUMBL);
    buttons.insert(KeyCode::BTN_THUMBR);
    builder = builder.with_keys(&buttons)?;

    Ok(builder.build()?)
}

fn handle_controller(
    mut source_device: Device, 
    virt_device: Arc<Mutex<VirtualDevice>>
) -> Result<(), Box<dyn Error>> {
    
    let source_name = source_device.name().unwrap_or("Unknown").to_string();
    println!("Starting input stream for: {}", source_name);
    source_device.grab()?;

    loop {
        for event in source_device.fetch_events()? {
            let mut virt_dev = virt_device.lock();
            virt_dev.emit(&[event])?;
        }
        
        thread::sleep(Duration::from_millis(10));
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("---Controller Muxer Initialization ---");
    let args: Vec<String> = env::args().collect();
    let primary_name = args.get(1)
        .map(|s| s.to_owned())
        .unwrap_or_else(|| PRIMARY_CONTROLLER_NAME.to_string());

    let secondary_name = args.get(2)
        .map(|s| s.to_owned())
        .unwrap_or_else(|| SECONDARY_CONTROLLER_NAME.to_string());

    let virt_device = setup_virtual_device()?;
    let virt_device = Arc::new(Mutex::new(virt_device));

    println!("Using {primary_name} and {secondary_name} to mux. Start the target game/application and select 'Muxed Controller'.");
    println!("Press Ctrl+C to stop.");
    
    let connection_loop = |controller_name: String, virt_device: Arc<Mutex<VirtualDevice>>| {
        thread::spawn(move || {
            loop {
                match find_device_by_name(&controller_name) {
                    Ok(dev) => {
                        if let Err(e) = handle_controller(dev, Arc::clone(&virt_device)) {
                            eprintln!("[{}] Handler exited (reconnecting in 3s): {}", controller_name, e);
                        }
                    },
                    Err(_) => {
                        println!("[{}] Device not yet found. Searching in 3s...", controller_name);
                    }
                }
                thread::sleep(Duration::from_secs(3)); 
            }
        })
    };

    let _primary_handle = connection_loop(primary_name, Arc::clone(&virt_device));
    let _secondary_handle = connection_loop(secondary_name, Arc::clone(&virt_device));

    _primary_handle.join().unwrap();
    _secondary_handle.join().unwrap();
    Ok(())
}