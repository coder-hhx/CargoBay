// Linux hypervisor: KVM via rust-vmm with VirtioFS support.
//
// VirtioFS: Uses virtiofsd (or its Rust equivalent) to provide high-performance
// shared filesystem between host and guest. The virtiofsd daemon runs on the host
// and communicates with the guest kernel's virtiofs driver via VHOST-USER protocol.
//
// Rosetta: Not available on Linux (Apple-only technology). x86_64 containers
// on ARM Linux would use QEMU user-mode emulation instead.

use crate::hypervisor::{Hypervisor, HypervisorError, SharedDirectory, VmConfig, VmInfo, VmState};
use std::collections::HashMap;
use std::sync::Mutex;

/// Linux hypervisor backed by KVM (via rust-vmm).
pub struct LinuxHypervisor {
    vms: Mutex<HashMap<String, VmEntry>>,
    next_id: Mutex<u64>,
}

struct VmEntry {
    info: VmInfo,
    /// PIDs of virtiofsd processes for each mount tag.
    _virtiofsd_pids: HashMap<String, u32>,
}

impl LinuxHypervisor {
    pub fn new() -> Self {
        Self {
            vms: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
        }
    }

    /// Check if KVM is available on this system.
    pub fn kvm_available() -> bool {
        std::path::Path::new("/dev/kvm").exists()
    }
}

impl Hypervisor for LinuxHypervisor {
    fn create_vm(&self, config: VmConfig) -> Result<String, HypervisorError> {
        if !Self::kvm_available() {
            return Err(HypervisorError::CreateFailed(
                "KVM not available. Ensure /dev/kvm exists and you have permissions.".into(),
            ));
        }

        if config.rosetta {
            return Err(HypervisorError::RosettaUnavailable(
                "Rosetta is only available on macOS Apple Silicon. Use QEMU user-mode for x86_64 emulation on Linux.".into(),
            ));
        }

        for dir in &config.shared_dirs {
            if !std::path::Path::new(&dir.host_path).exists() {
                return Err(HypervisorError::VirtioFsError(
                    format!("Host path does not exist: {}", dir.host_path),
                ));
            }
        }

        let mut id_counter = self.next_id.lock().unwrap();
        let id = format!("kvm-{}", *id_counter);
        *id_counter += 1;

        let info = VmInfo {
            id: id.clone(),
            name: config.name,
            state: VmState::Stopped,
            cpus: config.cpus,
            memory_mb: config.memory_mb,
            rosetta_enabled: false,
            shared_dirs: config.shared_dirs,
        };

        let entry = VmEntry {
            info,
            _virtiofsd_pids: HashMap::new(),
        };

        self.vms.lock().unwrap().insert(id.clone(), entry);

        // TODO: Real implementation using rust-vmm crates:
        // 1. Open /dev/kvm, create VM fd (KVM_CREATE_VM)
        // 2. Configure memory regions (KVM_SET_USER_MEMORY_REGION)
        // 3. Create vCPUs (KVM_CREATE_VCPU)
        // 4. Load kernel + initrd into memory
        // 5. Set up virtio-net, virtio-blk devices
        // 6. For each shared_dir:
        //    - Spawn virtiofsd: virtiofsd --socket-path=/tmp/<tag>.sock --shared-dir=<host_path>
        //    - Configure vhost-user-fs device connected to the socket
        // 7. Set up boot parameters

        Ok(id)
    }

    fn start_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let entry = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
        entry.info.state = VmState::Running;

        // TODO: Real implementation:
        // 1. Run vCPU loop (KVM_RUN) in separate threads
        // 2. Start virtiofsd processes for VirtioFS mounts
        // 3. Handle VM exits and I/O

        Ok(())
    }

    fn stop_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let entry = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
        entry.info.state = VmState::Stopped;

        // TODO: Stop virtiofsd processes, clean up vCPU threads

        entry._virtiofsd_pids.clear();
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
        false // Rosetta is macOS-only
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

        if entry.info.shared_dirs.iter().any(|d| d.tag == share.tag) {
            return Err(HypervisorError::VirtioFsError(format!(
                "Mount tag already exists: {}",
                share.tag
            )));
        }

        entry.info.shared_dirs.push(share.clone());

        // TODO: Real implementation:
        // 1. Spawn virtiofsd --socket-path=/tmp/<tag>.sock --shared-dir=<host_path>
        //    [--sandbox=none if read_only is false]
        // 2. Connect vhost-user-fs device to the socket
        // 3. Inside VM: mount -t virtiofs <tag> <guest_path>
        // 4. Store virtiofsd PID for cleanup

        Ok(())
    }

    fn unmount_virtiofs(&self, vm_id: &str, tag: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let entry = vms.get_mut(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        entry.info.shared_dirs.retain(|d| d.tag != tag);

        // TODO: Kill virtiofsd process, umount inside VM

        entry._virtiofsd_pids.remove(tag);
        Ok(())
    }

    fn list_virtiofs_mounts(
        &self,
        vm_id: &str,
    ) -> Result<Vec<SharedDirectory>, HypervisorError> {
        let vms = self.vms.lock().unwrap();
        let entry = vms.get(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        Ok(entry.info.shared_dirs.clone())
    }
}
