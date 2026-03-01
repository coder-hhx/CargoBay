#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("cargobay-vz is only supported on macOS");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn main() {
    cargobay_core::logging::init();

    let args = match Args::parse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            eprintln!();
            eprintln!("{}", Args::usage());
            std::process::exit(2);
        }
    };

    if let Err(e) = run(args) {
        tracing::error!("{}", e);
        std::process::exit(1);
    }
}

#[cfg(target_os = "macos")]
#[link(name = "Virtualization", kind = "framework")]
extern "C" {}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone)]
struct Args {
    kernel: std::path::PathBuf,
    initrd: Option<std::path::PathBuf>,
    disk: std::path::PathBuf,
    cpus: usize,
    memory_mb: u64,
    cmdline: String,
    ready_file: Option<std::path::PathBuf>,
}

#[cfg(target_os = "macos")]
impl Args {
    fn usage() -> &'static str {
        "Usage:\n  cargobay-vz --kernel <path> --disk <path> --cpus <n> --memory-mb <n> [--initrd <path>] [--cmdline <str>] [--ready-file <path>]\n"
    }

    fn parse() -> Result<Self, String> {
        let mut kernel: Option<std::path::PathBuf> = None;
        let mut initrd: Option<std::path::PathBuf> = None;
        let mut disk: Option<std::path::PathBuf> = None;
        let mut cpus: Option<usize> = None;
        let mut memory_mb: Option<u64> = None;
        let mut cmdline: Option<String> = None;
        let mut ready_file: Option<std::path::PathBuf> = None;

        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    return Err(Self::usage().to_string());
                }
                "--kernel" => {
                    kernel = Some(
                        it.next()
                            .ok_or_else(|| "--kernel requires a value".to_string())?
                            .into(),
                    );
                }
                "--initrd" => {
                    initrd = Some(
                        it.next()
                            .ok_or_else(|| "--initrd requires a value".to_string())?
                            .into(),
                    );
                }
                "--disk" => {
                    disk = Some(
                        it.next()
                            .ok_or_else(|| "--disk requires a value".to_string())?
                            .into(),
                    );
                }
                "--cpus" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--cpus requires a value".to_string())?;
                    cpus = Some(
                        raw.parse::<usize>()
                            .map_err(|_| "Invalid --cpus".to_string())?,
                    );
                }
                "--memory-mb" => {
                    let raw = it
                        .next()
                        .ok_or_else(|| "--memory-mb requires a value".to_string())?;
                    memory_mb = Some(
                        raw.parse::<u64>()
                            .map_err(|_| "Invalid --memory-mb".to_string())?,
                    );
                }
                "--cmdline" => {
                    cmdline = Some(
                        it.next()
                            .ok_or_else(|| "--cmdline requires a value".to_string())?,
                    );
                }
                "--ready-file" => {
                    ready_file = Some(
                        it.next()
                            .ok_or_else(|| "--ready-file requires a value".to_string())?
                            .into(),
                    );
                }
                other => return Err(format!("Unknown argument: {}", other)),
            }
        }

        let kernel = kernel.ok_or_else(|| "Missing --kernel".to_string())?;
        let disk = disk.ok_or_else(|| "Missing --disk".to_string())?;
        let cpus = cpus.ok_or_else(|| "Missing --cpus".to_string())?;
        let memory_mb = memory_mb.ok_or_else(|| "Missing --memory-mb".to_string())?;
        let cmdline = cmdline.unwrap_or_else(|| "console=hvc0".to_string());

        Ok(Self {
            kernel,
            initrd,
            disk,
            cpus,
            memory_mb,
            cmdline,
            ready_file,
        })
    }
}

#[cfg(target_os = "macos")]
fn run(args: Args) -> Result<(), String> {
    use dispatch2::{DispatchQueue, DispatchQueueAttr};
    use std::sync::mpsc;
    use std::time::Duration;

    let ready_file = args.ready_file.clone();

    let queue = DispatchQueue::new("com.cargobay.vz.vm", DispatchQueueAttr::SERIAL);
    let queue_for_vm = queue.clone();

    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    queue.exec_async(move || {
        let tx_started = tx.clone();
        objc2::rc::autoreleasepool(|_| {
            if let Err(e) = start_vm_on_queue(args, &queue_for_vm, tx_started) {
                let _ = tx.send(Err(e));
            }
        });
    });

    rx.recv_timeout(Duration::from_secs(30))
        .map_err(|_| "Timed out waiting for VZ start completion".to_string())??;

    if let Some(path) = ready_file {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")));
        std::fs::write(&path, b"ready\n")
            .map_err(|e| format!("Failed to write ready file: {}", e))?;
    }

    tracing::info!("VZ VM started (pid {})", std::process::id());
    loop {
        std::thread::park();
    }
}

#[cfg(target_os = "macos")]
fn start_vm_on_queue(
    args: Args,
    vm_queue: &dispatch2::DispatchQueue,
    tx_started: std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    use block2::RcBlock;
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2_foundation::{NSArray, NSError, NSFileHandle, NSString, NSURL};
    use std::ptr;

    use objc2::extern_class;
    use objc2::runtime::NSObject;
    use objc2::{AnyThread, ClassType};

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZBootLoader"]
        struct VZBootLoader;
    );
    extern_class!(
        #[unsafe(super(VZBootLoader))]
        #[name = "VZLinuxBootLoader"]
        struct VZLinuxBootLoader;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZVirtualMachineConfiguration"]
        struct VZVirtualMachineConfiguration;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZStorageDeviceAttachment"]
        struct VZStorageDeviceAttachment;
    );
    extern_class!(
        #[unsafe(super(VZStorageDeviceAttachment))]
        #[name = "VZDiskImageStorageDeviceAttachment"]
        struct VZDiskImageStorageDeviceAttachment;
    );
    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZStorageDeviceConfiguration"]
        struct VZStorageDeviceConfiguration;
    );
    extern_class!(
        #[unsafe(super(VZStorageDeviceConfiguration))]
        #[name = "VZVirtioBlockDeviceConfiguration"]
        struct VZVirtioBlockDeviceConfiguration;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZNetworkDeviceAttachment"]
        struct VZNetworkDeviceAttachment;
    );
    extern_class!(
        #[unsafe(super(VZNetworkDeviceAttachment))]
        #[name = "VZNATNetworkDeviceAttachment"]
        struct VZNATNetworkDeviceAttachment;
    );
    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZNetworkDeviceConfiguration"]
        struct VZNetworkDeviceConfiguration;
    );
    extern_class!(
        #[unsafe(super(VZNetworkDeviceConfiguration))]
        #[name = "VZVirtioNetworkDeviceConfiguration"]
        struct VZVirtioNetworkDeviceConfiguration;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZEntropyDeviceConfiguration"]
        struct VZEntropyDeviceConfiguration;
    );
    extern_class!(
        #[unsafe(super(VZEntropyDeviceConfiguration))]
        #[name = "VZVirtioEntropyDeviceConfiguration"]
        struct VZVirtioEntropyDeviceConfiguration;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZSerialPortAttachment"]
        struct VZSerialPortAttachment;
    );
    extern_class!(
        #[unsafe(super(VZSerialPortAttachment))]
        #[name = "VZFileHandleSerialPortAttachment"]
        struct VZFileHandleSerialPortAttachment;
    );
    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZSerialPortConfiguration"]
        struct VZSerialPortConfiguration;
    );
    extern_class!(
        #[unsafe(super(VZSerialPortConfiguration))]
        #[name = "VZVirtioConsoleDeviceSerialPortConfiguration"]
        struct VZVirtioConsoleDeviceSerialPortConfiguration;
    );

    extern_class!(
        #[unsafe(super(NSObject))]
        #[name = "VZVirtualMachine"]
        struct VZVirtualMachine;
    );

    let kernel_path = args
        .kernel
        .to_str()
        .ok_or_else(|| "Kernel path is not valid UTF-8".to_string())?;
    let disk_path = args
        .disk
        .to_str()
        .ok_or_else(|| "Disk path is not valid UTF-8".to_string())?;

    let kernel_url = NSURL::fileURLWithPath(&NSString::from_str(kernel_path));
    let disk_url = NSURL::fileURLWithPath(&NSString::from_str(disk_path));

    let boot_loader: Retained<VZLinuxBootLoader> =
        unsafe { msg_send![VZLinuxBootLoader::alloc(), initWithKernelURL: &*kernel_url] };
    let cmdline = NSString::from_str(&args.cmdline);
    let _: () = unsafe { msg_send![&*boot_loader, setCommandLine: &*cmdline] };

    if let Some(initrd) = args.initrd.as_ref() {
        let initrd_path = initrd
            .to_str()
            .ok_or_else(|| "Initrd path is not valid UTF-8".to_string())?;
        let initrd_url = NSURL::fileURLWithPath(&NSString::from_str(initrd_path));
        let _: () = unsafe { msg_send![&*boot_loader, setInitialRamdiskURL: &*initrd_url] };
    }

    let mut disk_error: *mut NSError = ptr::null_mut();
    let attachment: Option<Retained<VZDiskImageStorageDeviceAttachment>> = unsafe {
        msg_send![
            VZDiskImageStorageDeviceAttachment::alloc(),
            initWithURL: &*disk_url,
            readOnly: false,
            error: &mut disk_error
        ]
    };
    let attachment = attachment.ok_or_else(|| {
        if disk_error.is_null() {
            "Failed to create disk attachment".to_string()
        } else {
            format!("Failed to create disk attachment: {}", unsafe {
                &*disk_error
            })
        }
    })?;

    let block_device: Retained<VZVirtioBlockDeviceConfiguration> = unsafe {
        msg_send![VZVirtioBlockDeviceConfiguration::alloc(), initWithAttachment: &*attachment]
    };
    let block_device_ref: &VZStorageDeviceConfiguration = &*block_device;
    let storage_devices = NSArray::from_slice(&[block_device_ref]);

    let network_device: Retained<VZVirtioNetworkDeviceConfiguration> =
        unsafe { msg_send![VZVirtioNetworkDeviceConfiguration::alloc(), init] };
    let nat_attachment: Retained<VZNATNetworkDeviceAttachment> =
        unsafe { msg_send![VZNATNetworkDeviceAttachment::alloc(), init] };
    let nat_attachment_ref: &VZNetworkDeviceAttachment = &*nat_attachment;
    let _: () = unsafe { msg_send![&*network_device, setAttachment: nat_attachment_ref] };
    let network_device_ref: &VZNetworkDeviceConfiguration = &*network_device;
    let network_devices = NSArray::from_slice(&[network_device_ref]);

    let entropy_device: Retained<VZVirtioEntropyDeviceConfiguration> =
        unsafe { msg_send![VZVirtioEntropyDeviceConfiguration::alloc(), init] };
    let entropy_device_ref: &VZEntropyDeviceConfiguration = &*entropy_device;
    let entropy_devices = NSArray::from_slice(&[entropy_device_ref]);

    let serial_port: Retained<VZVirtioConsoleDeviceSerialPortConfiguration> =
        unsafe { msg_send![VZVirtioConsoleDeviceSerialPortConfiguration::alloc(), init] };
    let stdout_handle = NSFileHandle::fileHandleWithStandardOutput();
    let serial_attachment: Retained<VZFileHandleSerialPortAttachment> = unsafe {
        let none_in: Option<&NSFileHandle> = None;
        msg_send![
            VZFileHandleSerialPortAttachment::alloc(),
            initWithFileHandleForReading: none_in,
            fileHandleForWriting: Some(&*stdout_handle)
        ]
    };
    let serial_attachment_ref: &VZSerialPortAttachment = &*serial_attachment;
    let _: () = unsafe { msg_send![&*serial_port, setAttachment: serial_attachment_ref] };
    let serial_port_ref: &VZSerialPortConfiguration = &*serial_port;
    let serial_ports = NSArray::from_slice(&[serial_port_ref]);

    let config: Retained<VZVirtualMachineConfiguration> =
        unsafe { msg_send![VZVirtualMachineConfiguration::class(), new] };
    let boot_loader_ref: &VZBootLoader = &*boot_loader;
    let _: () = unsafe { msg_send![&*config, setBootLoader: boot_loader_ref] };
    let _: () = unsafe { msg_send![&*config, setCPUCount: args.cpus] };

    let memory_bytes = args.memory_mb.saturating_mul(1024).saturating_mul(1024);
    let _: () = unsafe { msg_send![&*config, setMemorySize: memory_bytes] };
    let _: () = unsafe { msg_send![&*config, setStorageDevices: &*storage_devices] };
    let _: () = unsafe { msg_send![&*config, setNetworkDevices: &*network_devices] };
    let _: () = unsafe { msg_send![&*config, setEntropyDevices: &*entropy_devices] };
    let _: () = unsafe { msg_send![&*config, setSerialPorts: &*serial_ports] };

    let mut validate_error: *mut NSError = ptr::null_mut();
    let ok: bool = unsafe { msg_send![&*config, validateWithError: &mut validate_error] };
    if !ok {
        return Err(if validate_error.is_null() {
            "VZ configuration validation failed".to_string()
        } else {
            format!("VZ configuration validation failed: {}", unsafe {
                &*validate_error
            })
        });
    }

    let vm: Retained<VZVirtualMachine> = unsafe {
        msg_send![
            VZVirtualMachine::alloc(),
            initWithConfiguration: &*config,
            queue: vm_queue
        ]
    };

    let block = RcBlock::new(move |err: *mut NSError| {
        objc2::rc::autoreleasepool(|_| {
            if err.is_null() {
                let _ = tx_started.send(Ok(()));
            } else {
                let _ = tx_started.send(Err(format!("VZ start failed: {}", unsafe { &*err })));
            }
        });
    });

    let _: () = unsafe { msg_send![&*vm, startWithCompletionHandler: &*block] };

    std::mem::forget(vm);
    Ok(())
}
