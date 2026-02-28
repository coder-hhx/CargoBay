// macOS hypervisor: Apple Virtualization.framework with Rosetta + VirtioFS support.
//
// Rosetta: On Apple Silicon, VZLinuxRosettaDirectoryShare provides x86_64 → arm64
// translation inside Linux VMs. The Rosetta binary is mounted and registered
// via binfmt_misc so x86_64 ELF binaries run transparently.
//
// VirtioFS: VZVirtioFileSystemDeviceConfiguration allows sharing host directories
// with near-native filesystem performance (faster than 9p/NFS).

use crate::hypervisor::{Hypervisor, HypervisorError, SharedDirectory, VmConfig, VmInfo, VmState};
use std::collections::HashMap;
use std::sync::Mutex;

/// macOS hypervisor backed by Apple Virtualization.framework.
pub struct MacOSHypervisor {
    vms: Mutex<HashMap<String, VmEntry>>,
    next_id: Mutex<u64>,
}

struct VmEntry {
    info: VmInfo,
    /// VZ configuration parameters stored for lifecycle management.
    _rosetta_mounted: bool,
}

impl MacOSHypervisor {
    pub fn new() -> Self {
        Self {
            vms: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
        }
    }

    /// Check if Rosetta is available on this Mac.
    /// Rosetta is only available on Apple Silicon (aarch64) running macOS 13+.
    fn check_rosetta_availability() -> bool {
        // Runtime check: arch must be aarch64
        #[cfg(target_arch = "aarch64")]
        {
            // Check if the Rosetta runtime exists
            std::path::Path::new("/Library/Apple/usr/libexec/oah/libRosettaRuntime").exists()
                || std::path::Path::new("/usr/libexec/rosetta").exists()
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            false
        }
    }
}

impl Hypervisor for MacOSHypervisor {
    fn create_vm(&self, config: VmConfig) -> Result<String, HypervisorError> {
        // Validate Rosetta request
        if config.rosetta && !Self::check_rosetta_availability() {
            return Err(HypervisorError::RosettaUnavailable(
                "Rosetta is only available on Apple Silicon Macs with macOS 13+".into(),
            ));
        }

        // Validate shared directory paths
        for dir in &config.shared_dirs {
            if !std::path::Path::new(&dir.host_path).exists() {
                return Err(HypervisorError::VirtioFsError(
                    format!("Host path does not exist: {}", dir.host_path),
                ));
            }
        }

        let mut id_counter = self.next_id.lock().unwrap();
        let id = format!("vz-{}", *id_counter);
        *id_counter += 1;

        let info = VmInfo {
            id: id.clone(),
            name: config.name,
            state: VmState::Stopped,
            cpus: config.cpus,
            memory_mb: config.memory_mb,
            rosetta_enabled: config.rosetta,
            shared_dirs: config.shared_dirs,
        };

        let entry = VmEntry {
            info,
            _rosetta_mounted: false,
        };

        self.vms.lock().unwrap().insert(id.clone(), entry);

        // TODO: Real implementation using Virtualization.framework FFI:
        // 1. Create VZVirtualMachineConfiguration
        // 2. Set VZLinuxBootLoader with kernel/initrd
        // 3. Configure VZVirtioNetworkDeviceConfiguration
        // 4. Configure VZVirtioBlockDeviceConfiguration for disk
        // 5. If rosetta: Add VZLinuxRosettaDirectoryShare
        // 6. For each shared_dir: Add VZVirtioFileSystemDeviceConfiguration
        //    with VZSharedDirectory → VZSingleDirectoryShare
        // 7. Validate configuration

        Ok(id)
    }

    fn start_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let entry = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
        entry.info.state = VmState::Running;

        // TODO: Real implementation:
        // 1. Create VZVirtualMachine from stored configuration
        // 2. Start the VM: virtualMachine.start()
        // 3. If rosetta enabled:
        //    - Mount Rosetta binary inside guest: mount -t virtiofs rosetta /mnt/rosetta
        //    - Register binfmt_misc: echo ':rosetta:M::\x7fELF\x02\x01\x01\x00...'
        //      > /proc/sys/fs/binfmt_misc/register
        // 4. For each VirtioFS share:
        //    - mount -t virtiofs <tag> <guest_path> inside the VM

        Ok(())
    }

    fn stop_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let entry = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
        entry.info.state = VmState::Stopped;
        entry._rosetta_mounted = false;

        // TODO: virtualMachine.stop()

        Ok(())
    }

    fn delete_vm(&self, id: &str) -> Result<(), HypervisorError> {
        self.vms
            .lock()
            .unwrap()
            .remove(id)
            .ok_or(HypervisorError::NotFound(id.into()))?;
        Ok(())
    }

    fn list_vms(&self) -> Result<Vec<VmInfo>, HypervisorError> {
        Ok(self
            .vms
            .lock()
            .unwrap()
            .values()
            .map(|e| e.info.clone())
            .collect())
    }

    fn rosetta_available(&self) -> bool {
        Self::check_rosetta_availability()
    }

    fn mount_virtiofs(
        &self,
        vm_id: &str,
        share: &SharedDirectory,
    ) -> Result<(), HypervisorError> {
        if !std::path::Path::new(&share.host_path).exists() {
            return Err(HypervisorError::VirtioFsError(format!(
                "Host path does not exist: {}",
                share.host_path
            )));
        }

        let mut vms = self.vms.lock().unwrap();
        let entry = vms.get_mut(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;

        // Check for duplicate tag
        if entry.info.shared_dirs.iter().any(|d| d.tag == share.tag) {
            return Err(HypervisorError::VirtioFsError(format!(
                "Mount tag already exists: {}",
                share.tag
            )));
        }

        entry.info.shared_dirs.push(share.clone());

        // TODO: Real implementation using Virtualization.framework:
        // 1. Create VZSharedDirectory(url: hostPath, readOnly: readOnly)
        // 2. Create VZSingleDirectoryShare(directory: sharedDir)
        // 3. Create VZVirtioFileSystemDeviceConfiguration(tag: tag)
        // 4. Attach to running VM
        // 5. mount -t virtiofs <tag> <guest_path> inside VM via agent

        Ok(())
    }

    fn unmount_virtiofs(&self, vm_id: &str, tag: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let entry = vms.get_mut(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        entry.info.shared_dirs.retain(|d| d.tag != tag);

        // TODO: umount <guest_path> inside VM, detach VZ device

        Ok(())
    }

    fn list_virtiofs_mounts(&self, vm_id: &str) -> Result<Vec<SharedDirectory>, HypervisorError> {
        let vms = self.vms.lock().unwrap();
        let entry = vms.get(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        Ok(entry.info.shared_dirs.clone())
    }
}
