use crate::hypervisor::{Hypervisor, HypervisorError, SharedDirectory, VmConfig, VmInfo, VmState};
use std::collections::HashMap;
use std::sync::Mutex;

/// Stub hypervisor for development/testing on unsupported platforms.
pub struct StubHypervisor {
    vms: Mutex<HashMap<String, VmInfo>>,
    next_id: Mutex<u64>,
}

impl StubHypervisor {
    pub fn new() -> Self {
        Self {
            vms: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
        }
    }
}

impl Hypervisor for StubHypervisor {
    fn create_vm(&self, config: VmConfig) -> Result<String, HypervisorError> {
        let mut id_counter = self.next_id.lock().unwrap();
        let id = format!("stub-{}", *id_counter);
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
        self.vms.lock().unwrap().insert(id.clone(), info);
        Ok(id)
    }

    fn start_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let vm = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
        vm.state = VmState::Running;
        Ok(())
    }

    fn stop_vm(&self, id: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let vm = vms.get_mut(id).ok_or(HypervisorError::NotFound(id.into()))?;
        vm.state = VmState::Stopped;
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
        Ok(self.vms.lock().unwrap().values().cloned().collect())
    }

    fn mount_virtiofs(&self, vm_id: &str, share: &SharedDirectory) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let vm = vms.get_mut(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        vm.shared_dirs.push(share.clone());
        Ok(())
    }

    fn unmount_virtiofs(&self, vm_id: &str, tag: &str) -> Result<(), HypervisorError> {
        let mut vms = self.vms.lock().unwrap();
        let vm = vms.get_mut(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        vm.shared_dirs.retain(|d| d.tag != tag);
        Ok(())
    }

    fn list_virtiofs_mounts(&self, vm_id: &str) -> Result<Vec<SharedDirectory>, HypervisorError> {
        let vms = self.vms.lock().unwrap();
        let vm = vms.get(vm_id).ok_or(HypervisorError::NotFound(vm_id.into()))?;
        Ok(vm.shared_dirs.clone())
    }
}
