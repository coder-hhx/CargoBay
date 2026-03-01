// macOS hypervisor: Apple Virtualization.framework with Rosetta + VirtioFS support.
//
// Rosetta: On Apple Silicon, VZLinuxRosettaDirectoryShare provides x86_64 → arm64
// translation inside Linux VMs. The Rosetta binary is mounted and registered
// via binfmt_misc so x86_64 ELF binaries run transparently.
//
// VirtioFS: VZVirtioFileSystemDeviceConfiguration allows sharing host directories
// with near-native filesystem performance (faster than 9p/NFS).

use crate::hypervisor::{Hypervisor, HypervisorError, SharedDirectory, VmConfig, VmInfo, VmState};
use crate::store::{next_id_for_prefix, VmStore};
use std::collections::HashMap;
use std::sync::Mutex;
use tracing::warn;

/// macOS hypervisor backed by Apple Virtualization.framework.
pub struct MacOSHypervisor {
    vms: Mutex<HashMap<String, VmEntry>>,
    next_id: Mutex<u64>,
    store: VmStore,
}

struct VmEntry {
    info: VmInfo,
    /// VZ configuration parameters stored for lifecycle management.
    _rosetta_mounted: bool,
}

impl MacOSHypervisor {
    pub fn new() -> Self {
        let store = VmStore::new();
        let mut loaded = match store.load_vms() {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to load VM store ({}): {}", store.path().display(), e);
                vec![]
            }
        };

        for vm in &mut loaded {
            if vm.state != VmState::Stopped {
                vm.state = VmState::Stopped;
            }
        }

        let mut map: HashMap<String, VmEntry> = HashMap::new();
        for vm in loaded.iter().cloned() {
            map.insert(
                vm.id.clone(),
                VmEntry {
                    info: vm,
                    _rosetta_mounted: false,
                },
            );
        }

        let next_id = next_id_for_prefix(&loaded, "vz-");
        Self {
            vms: Mutex::new(map),
            next_id: Mutex::new(next_id),
            store,
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

    fn persist(&self) -> Result<(), HypervisorError> {
        let vms = self
            .vms
            .lock()
            .unwrap()
            .values()
            .map(|e| e.info.clone())
            .collect::<Vec<_>>();
        self.store.save_vms(&vms)
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

        {
            let vms = self.vms.lock().unwrap();
            if vms.values().any(|e| e.info.name == config.name) {
                return Err(HypervisorError::CreateFailed(format!(
                    "VM name already exists: {}",
                    config.name
                )));
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
            disk_gb: config.disk_gb,
            rosetta_enabled: config.rosetta,
            shared_dirs: config.shared_dirs,
        };

        let entry = VmEntry {
            info,
            _rosetta_mounted: false,
        };

        self.vms.lock().unwrap().insert(id.clone(), entry);
        if let Err(e) = self.persist() {
            self.vms.lock().unwrap().remove(&id);
            return Err(e);
        }

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
        let previous = {
            let mut vms = self.vms.lock().unwrap();
            let entry = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
            let prev = entry.info.state.clone();
            entry.info.state = VmState::Running;
            prev
        };
        if let Err(e) = self.persist() {
            let mut vms = self.vms.lock().unwrap();
            if let Some(entry) = vms.get_mut(id) {
                entry.info.state = previous;
            }
            return Err(e);
        }

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
        let (previous, rosetta_prev) = {
            let mut vms = self.vms.lock().unwrap();
            let entry = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
            let prev = entry.info.state.clone();
            let rosetta_prev = entry._rosetta_mounted;
            entry.info.state = VmState::Stopped;
            entry._rosetta_mounted = false;
            (prev, rosetta_prev)
        };
        if let Err(e) = self.persist() {
            let mut vms = self.vms.lock().unwrap();
            if let Some(entry) = vms.get_mut(id) {
                entry.info.state = previous;
                entry._rosetta_mounted = rosetta_prev;
            }
            return Err(e);
        }

        // TODO: virtualMachine.stop()

        Ok(())
    }

    fn delete_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let removed = self
            .vms
            .lock()
            .unwrap()
            .remove(id)
            .ok_or(HypervisorError::NotFound(id.into()))?;
        if let Err(e) = self.persist() {
            self.vms.lock().unwrap().insert(id.to_string(), removed);
            return Err(e);
        }
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
        drop(vms);
        if let Err(e) = self.persist() {
            let mut vms = self.vms.lock().unwrap();
            if let Some(entry) = vms.get_mut(vm_id) {
                entry.info.shared_dirs.retain(|d| d.tag != share.tag);
            }
            return Err(e);
        }

        // TODO: Real implementation using Virtualization.framework:
        // 1. Create VZSharedDirectory(url: hostPath, readOnly: readOnly)
        // 2. Create VZSingleDirectoryShare(directory: sharedDir)
        // 3. Create VZVirtioFileSystemDeviceConfiguration(tag: tag)
        // 4. Attach to running VM
        // 5. mount -t virtiofs <tag> <guest_path> inside VM via agent

        Ok(())
    }

    fn unmount_virtiofs(&self, vm_id: &str, tag: &str) -> Result<(), HypervisorError> {
        let previous = {
            let mut vms = self.vms.lock().unwrap();
            let entry = vms.get_mut(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
            let prev = entry.info.shared_dirs.clone();
            entry.info.shared_dirs.retain(|d| d.tag != tag);
            prev
        };
        if let Err(e) = self.persist() {
            let mut vms = self.vms.lock().unwrap();
            if let Some(entry) = vms.get_mut(vm_id) {
                entry.info.shared_dirs = previous;
            }
            return Err(e);
        }

        // TODO: umount <guest_path> inside VM, detach VZ device

        Ok(())
    }

    fn list_virtiofs_mounts(&self, vm_id: &str) -> Result<Vec<SharedDirectory>, HypervisorError> {
        let vms = self.vms.lock().unwrap();
        let entry = vms.get(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        Ok(entry.info.shared_dirs.clone())
    }
}
