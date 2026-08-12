//! Сводка о системе для стартовой страницы.
//!
//! Собирается точечно: полный опрос (`System::new_all`) перебирает ещё и все
//! процессы, а для сводки нужны только память, процессор и диски.

use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Serialize)]
pub struct DiskInfo {
    /// Буква тома или точка монтирования.
    pub mount: String,
    pub total: u64,
    pub free: u64,
}

#[derive(Serialize)]
pub struct SystemInfo {
    /// Название системы, например «Windows».
    pub os: String,
    /// Версия системы, например «11».
    pub os_version: String,
    /// Сборка ядра — то, что показывает winver.
    pub kernel: String,
    pub host: String,
    /// Сколько система работает без перезагрузки, секунды.
    pub uptime: u64,
    pub cpu: String,
    pub cpu_cores: usize,
    pub memory_total: u64,
    pub memory_used: u64,
    pub disks: Vec<DiskInfo>,
    /// Запущено ли приложение с правами администратора.
    pub admin: bool,
}

#[tauri::command]
pub async fn system_info() -> Result<SystemInfo, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu_all();

        let cpu = sys
            .cpus()
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default();

        let disks = Disks::new_with_refreshed_list()
            .iter()
            .map(|d| DiskInfo {
                mount: d.mount_point().to_string_lossy().to_string(),
                total: d.total_space(),
                free: d.available_space(),
            })
            .collect();

        SystemInfo {
            os: System::name().unwrap_or_else(|| "Windows".into()),
            os_version: System::os_version().unwrap_or_default(),
            kernel: System::kernel_version().unwrap_or_default(),
            host: System::host_name().unwrap_or_default(),
            uptime: System::uptime(),
            cpu,
            cpu_cores: sys.cpus().len(),
            memory_total: sys.total_memory(),
            memory_used: sys.used_memory(),
            disks,
            admin: crate::windows_cmd::windows_pars_admin(),
        }
    })
    .await
    .map_err(|e| format!("поток сбора сведений упал: {e}"))
}
