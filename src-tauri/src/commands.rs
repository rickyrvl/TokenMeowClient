use crate::AppState;
use log::{error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

static METRICS_RUNNING: AtomicBool = AtomicBool::new(false);
static STATUS_RUNNING: AtomicBool = AtomicBool::new(false);

const SUPABASE_URL: &str = "https://vbqnhltqslotsoalcmqc.supabase.co";
const SUPABASE_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InZicW5obHRxc2xvdHNvYWxjbXFjIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzQ0NzUxNjYsImV4cCI6MjA5MDA1MTE2Nn0.cuD0_s34ftrmH8a6W-1mX8jiTOP-j2Ysbm5OeLyax_g";

#[derive(Debug, Serialize, Deserialize)]
pub struct GpuInfo {
    pub available: bool,
    pub gpu_type: String,
    pub memory: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppStateInfo {
    pub machine_id: Option<String>,
    pub tailscale_ip: Option<String>,
    pub is_running: bool,
    pub tokens_per_second: f64,
    pub gpu_usage: f64,
    pub today_earnings: f64,
}

#[tauri::command]
pub async fn check_gpu() -> Result<GpuInfo, String> {
    info!("Checking GPU availability...");

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .map_err(|e| e.to_string())?;

        let cpu_brand = String::from_utf8_lossy(&output.stdout);
        let is_apple_silicon = cpu_brand.contains("Apple");

        if is_apple_silicon {
            Ok(GpuInfo {
                available: true,
                gpu_type: "Apple Silicon".to_string(),
                memory: 0,
            })
        } else {
            Ok(GpuInfo {
                available: false,
                gpu_type: "No GPU".to_string(),
                memory: 0,
            })
        }
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let nvidia_exists = Command::new("nvidia-smi")
            .arg("--query-gpu=name")
            .arg("--format=csv,noheader")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if nvidia_exists {
            let output = Command::new("nvidia-smi")
                .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
                .output()
                .map_err(|e| e.to_string())?;

            let memory = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u64>()
                .unwrap_or(0);

            Ok(GpuInfo {
                available: true,
                gpu_type: "NVIDIA GPU".to_string(),
                memory,
            })
        } else {
            Ok(GpuInfo {
                available: false,
                gpu_type: "No NVIDIA GPU".to_string(),
                memory: 0,
            })
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Ok(GpuInfo {
            available: false,
            gpu_type: "Unknown".to_string(),
            memory: 0,
        })
    }
}

#[tauri::command]
pub async fn install_docker() -> Result<String, String> {
    info!("Installing Docker (Windows/Linux only)...");

    #[cfg(target_os = "windows")]
    {
        let ps_script = r#"
            $ErrorActionPreference = 'Stop'
            if (Get-Command docker -ErrorAction SilentlyContinue) {
                Write-Output 'Docker already installed'
                exit 0
            }
            Invoke-WebRequest -Uri https://get.docker.com -OutFile C:\docker-install.ps1
            Start-Process -FilePath powershell.exe -ArgumentList '-ExecutionPolicy', 'Bypass', '-File', 'C:\docker-install.ps1' -Wait
            Start-Service docker
            Remove-Item C:\docker-install.ps1 -Force
            Write-Output 'Docker installed successfully'
        "#;

        let output = Command::new("powershell")
            .args(["-ExecutionPolicy", "Bypass", "-Command", ps_script])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok("Docker installed successfully".to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[cfg(target_os = "linux")]
    {
        let install_script = r#"
            if command -v docker &> /dev/null; then
                echo "Docker already installed"
                exit 0
            fi
            curl -fsSL https://get.docker.com -o /tmp/docker-install.sh
            chmod +x /tmp/docker-install.sh
            /tmp/docker-install.sh
            systemctl --now enable docker
            rm /tmp/docker-install.sh
            echo "Docker installed successfully"
        "#;

        let output = Command::new("sh")
            .args(["-c", install_script])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok("Docker installed successfully".to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[cfg(target_os = "macos")]
    {
        Ok("Docker not needed on macOS".to_string())
    }
}

#[tauri::command]
pub async fn start_tailscale(auth_key: String) -> Result<String, String> {
    info!("Starting Tailscale in headless mode...");

    #[cfg(target_os = "macos")]
    {
        let brew_tailscale = Command::new("brew")
            .args(["list", "tailscale"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !brew_tailscale {
            Command::new("brew")
                .args(["install", "tailscale"])
                .output()
                .map_err(|e| e.to_string())?;
        }

        let output = Command::new("/opt/homebrew/bin/tailscaled")
            .args(["--tun=utun"])
            .spawn();

        std::thread::sleep(Duration::from_secs(2));

        let login_output = Command::new("/opt/homebrew/bin/tailscale")
            .args(["login", "--authkey", &auth_key])
            .output()
            .map_err(|e| e.to_string())?;

        if login_output.status.success() {
            let ip_output = Command::new("/opt/homebrew/bin/tailscale")
                .args(["ip", "-4"])
                .output()
                .map_err(|e| e.to_string())?;

            let ip = String::from_utf8_lossy(&ip_output.stdout).trim().to_string();
            info!("Tailscale IP: {}", ip);
            Ok(ip)
        } else {
            Err(String::from_utf8_lossy(&login_output.stderr).to_string())
        }
    }

    #[cfg(target_os = "windows")]
    {
        let program_files = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
        let tailscale_path = format!("{}\\{}", program_files, "Tailscale\\tailscale.exe");

        if !std::path::Path::new(&tailscale_path).exists() {
            let ps_install = r#"
                $ErrorActionPreference = 'Stop'
                Invoke-WebRequest -Uri https://pkgs.tailscale.com/stable/tailscale-setup.exe -OutFile C:\tailscale-setup.exe
                Start-Process -FilePath C:\tailscale-setup.exe -ArgumentList '/S' -Wait
                Remove-Item C:\tailscale-setup.exe -Force
            "#;

            Command::new("powershell")
                .args(["-ExecutionPolicy", "Bypass", "-Command", ps_install])
                .output()
                .map_err(|e| e.to_string())?;
        }

        Command::new(&tailscale_path)
            .args(["socket"])
            .spawn()
            .map_err(|e| e.to_string())?;

        std::thread::sleep(Duration::from_secs(2));

        let output = Command::new(&tailscale_path)
            .args(["up", "--authkey", &auth_key])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let ip_output = Command::new(&tailscale_path)
                .args(["ip", "-4"])
                .output()
                .map_err(|e| e.to_string())?;

            let ip = String::from_utf8_lossy(&ip_output.stdout).trim().to_string();
            info!("Tailscale IP: {}", ip);
            Ok(ip)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[cfg(target_os = "linux")]
    {
        if !Command::new("which")
            .arg("tailscale")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            let output = Command::new("curl")
                .args([
                    "-fsSL",
                    "https://tailscale.com/install.sh",
                    "|",
                    "sh",
                ])
                .output()
                .map_err(|e| e.to_string())?;

            if !output.status.success() {
                return Err("Failed to install Tailscale".to_string());
            }
        }

        Command::new("tailscaled")
            .args(["--tun=utun"])
            .spawn()
            .map_err(|e| e.to_string())?;

        std::thread::sleep(Duration::from_secs(2));

        let output = Command::new("tailscale")
            .args(["up", "--authkey", &auth_key])
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            let ip_output = Command::new("tailscale")
                .args(["ip", "-4"])
                .output()
                .map_err(|e| e.to_string())?;

            let ip = String::from_utf8_lossy(&ip_output.stdout).trim().to_string();
            info!("Tailscale IP: {}", ip);
            Ok(ip)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("Unsupported platform".to_string())
    }
}

#[tauri::command]
pub async fn start_vllm_mlx(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<String, String> {
    info!("Starting vLLM-MLX on macOS...");

    #[cfg(not(target_os = "macos"))]
    {
        return Err("vLLM-MLX is only available on macOS".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        let models_dir = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("ComputeNode")
            .join("models");

        std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

        let pip_check = Command::new("pip")
            .args(["show", "vllm-mlx"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !pip_check {
            info!("Installing vllm-mlx...");
            let install_output = Command::new("pip")
                .args(["install", "vllm-mlx"])
                .output()
                .map_err(|e| e.to_string())?;

            if !install_output.status.success() {
                return Err(format!(
                    "Failed to install vllm-mlx: {}",
                    String::from_utf8_lossy(&install_output.stderr)
                ));
            }
        }

        let model_path = "mlx-community/Qwen3.5-9B-MLX-4bit";
        let serve_cmd = format!(
            "python -m vllm_mlx.serve --model {} --port 8000 --gpu-memory-utilization 0.9 --max-model-len 4096 --model-path {}",
            model_path,
            models_dir.to_string_lossy()
        );

        info!("Starting vLLM-MLX serve: {}", serve_cmd);

        let output = Command::new("sh")
            .args(["-c", &serve_cmd])
            .spawn()
            .map_err(|e| e.to_string())?;

        std::thread::sleep(Duration::from_secs(5));

        let http_client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;

        for i in 0..10 {
            match http_client.get("http://127.0.0.1:8000/v1/models").send() {
                Ok(resp) if resp.status().is_success() => {
                    info!("vLLM-MLX is running on port 8000");
                    *state.tailscale_ip.lock().unwrap() = Some("127.0.0.1".to_string());
                    return Ok("vLLM-MLX started successfully".to_string());
                }
                _ => {
                    if i < 9 {
                        std::thread::sleep(Duration::from_secs(2));
                    }
                }
            }
        }

        Err("vLLM-MLX failed to start".to_string())
    }
}

#[tauri::command]
pub async fn start_vllm_docker(
    state: State<'_, AppState>,
    _app: AppHandle,
) -> Result<String, String> {
    info!("Starting vLLM Docker container (Windows/Linux only)...");

    #[cfg(target_os = "macos")]
    {
        return Err("Docker vLLM is not needed on macOS".to_string());
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let docker_check = Command::new("docker")
            .args(["info"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !docker_check {
            return Err("Docker is not running. Please install Docker first.".to_string());
        }

        let models_dir = if cfg!(target_os = "windows") {
            std::path::PathBuf::from("C:\\ProgramData\\ComputeNode\\models")
        } else {
            std::path::PathBuf::from("/opt/compute/models")
        };

        std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

        let container_check = Command::new("docker")
            .args(["ps", "-a", "-f", "name=vllm-server", "-q"])
            .output()
            .map_err(|e| e.to_string())?;

        let container_id = String::from_utf8_lossy(&container_check.stdout).trim();
        if !container_id.is_empty() {
            Command::new("docker")
                .args(["rm", "-f", container_id])
                .output()
                .map_err(|e| e.to_string())?;
        }

        let volume_arg = format!("{}:/models", models_dir.to_string_lossy());

        let docker_cmd = if cfg!(target_os = "windows") {
            vec![
                "run",
                "-d",
                "--name",
                "vllm-server",
                "--gpus",
                "all",
                "-v",
                &volume_arg,
                "-p",
                "8000:8000",
                "vllm/vllm-openai:latest",
                "--model",
                "Qwen/Qwen3.5-9B-Instruct",
                "--port",
                "8000",
                "--gpu-memory-utilization",
                "0.9",
                "--max-model-len",
                "4096",
            ]
        } else {
            vec![
                "run",
                "-d",
                "--name",
                "vllm-server",
                "--gpus",
                "all",
                "-v",
                &volume_arg,
                "-p",
                "8000:8000",
                "--privileged",
                "vllm/vllm-openai:latest",
                "--model",
                "Qwen/Qwen3.5-9B-Instruct",
                "--port",
                "8000",
                "--gpu-memory-utilization",
                "0.9",
                "--max-model-len",
                "4096",
            ]
        };

        let output = Command::new("docker").args(&docker_cmd).output().map_err(|e| e.to_string())?;

        if !output.status.success() {
            return Err(format!(
                "Failed to start vLLM container: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        std::thread::sleep(Duration::from_secs(10));

        let http_client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;

        for i in 0..15 {
            match http_client.get("http://127.0.0.1:8000/v1/models").send() {
                Ok(resp) if resp.status().is_success() => {
                    info!("vLLM Docker is running on port 8000");
                    return Ok("vLLM Docker started successfully".to_string());
                }
                _ => {
                    if i < 14 {
                        std::thread::sleep(Duration::from_secs(2));
                    }
                }
            }
        }

        Err("vLLM Docker failed to start".to_string())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Err("Unsupported platform for Docker vLLM".to_string())
    }
}

#[tauri::command]
pub async fn get_metrics(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let tokens_per_sec = *state.tokens_per_second.lock().unwrap();
    let gpu_usage = *state.gpu_usage.lock().unwrap();
    let today_earnings = *state.today_earnings.lock().unwrap();

    #[cfg(target_os = "macos")]
    {
        let output = Command::new("curl")
            .args([
                "-s",
                "http://127.0.0.1:8000/v1/metrics",
            ])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let metrics_text = String::from_utf8_lossy(&output.stdout);
                let tokens = extract_vllm_metrics(&metrics_text);
                *state.tokens_per_second.lock().unwrap() = tokens.0;
                *state.gpu_usage.lock().unwrap() = tokens.1;
            }
        }

        let mlx_output = Command::new("python3")
            .args([
                "-c",
                "import mlx.core as mx; print(f'VRAM: {mx.metal.get_active_memory() / 1e9:.2f}GB')",
            ])
            .output();

        let mlx_memory = if let Ok(output) = mlx_output {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            } else {
                "N/A".to_string()
            }
        } else {
            "N/A".to_string()
        };

        Ok(serde_json::json!({
            "tokens_per_second": tokens_per_sec,
            "gpu_usage": gpu_usage,
            "today_earnings": today_earnings,
            "mlx_memory": mlx_memory,
            "platform": "macOS"
        }))
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let output = Command::new("docker")
            .args(["exec", "vllm-server", "nvidia-smi", "--query-gpu=utilization.gpu", "--format=csv,noheader"])
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                let usage = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse::<f64>()
                    .unwrap_or(0.0);
                *state.gpu_usage.lock().unwrap() = usage;
            }
        }

        let vllm_output = Command::new("curl")
            .args(["-s", "http://127.0.0.1:8000/v1/metrics"])
            .output();

        if let Ok(output) = vllm_output {
            if output.status.success() {
                let metrics_text = String::from_utf8_lossy(&output.stdout);
                let tokens = extract_vllm_metrics(&metrics_text);
                *state.tokens_per_second.lock().unwrap() = tokens.0;
            }
        }

        Ok(serde_json::json!({
            "tokens_per_second": tokens_per_sec,
            "gpu_usage": gpu_usage,
            "today_earnings": today_earnings,
            "platform": if cfg!(target_os = "windows") { "Windows" } else { "Linux" }
        }))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Ok(serde_json::json!({
            "tokens_per_second": 0.0,
            "gpu_usage": 0.0,
            "today_earnings": 0.0,
            "platform": "Unknown"
        }))
    }
}

fn extract_vllm_metrics(metrics_text: &str) -> (f64, f64) {
    let mut tokens_per_second = 0.0;
    let mut gpu_usage = 0.0;

    for line in metrics_text.lines() {
        if line.contains("tokens_per_second") {
            if let Some(val) = line.split_whitespace().last() {
                tokens_per_second = val.parse().unwrap_or(0.0);
            }
        }
        if line.contains("gpu_utilization") {
            if let Some(val) = line.split_whitespace().last() {
                gpu_usage = val.parse().unwrap_or(0.0);
            }
        }
    }

    (tokens_per_second, gpu_usage)
}

#[tauri::command]
pub async fn update_machine_status(
    state: State<'_, AppState>,
    machine_id: String,
    endpoint: String,
) -> Result<(), String> {
    let tailscale_ip = state.tailscale_ip.lock().unwrap().clone();

    if let Some(ip) = tailscale_ip {
        let client = Client::new();
        let url = format!("{}/rest/v1/machines", SUPABASE_URL);

        let body = serde_json::json!({
            "machine_id": machine_id,
            "endpoint": format!("http://{}:8000", ip),
            "status": "online",
            "last_seen": chrono::Utc::now().to_rfc3339()
        });

        client
            .patch(&url)
            .header("apikey", SUPABASE_ANON_KEY)
            .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
            .header("Prefer", "return=minimal")
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn report_metrics(
    state: State<'_, AppState>,
    machine_id: String,
) -> Result<(), String> {
    let tokens = *state.tokens_per_second.lock().unwrap();
    let gpu = *state.gpu_usage.lock().unwrap();

    let client = Client::new();
    let url = format!("{}/rest/v1/metrics_raw", SUPABASE_URL);

    let body = serde_json::json!({
        "machine_id": machine_id,
        "tokens_per_second": tokens,
        "gpu_usage": gpu,
        "recorded_at": chrono::Utc::now().to_rfc3339()
    });

    client
        .post(&url)
        .header("apikey", SUPABASE_ANON_KEY)
        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
        .header("Prefer", "return=minimal")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn set_machine_id(state: State<'_, AppState>, machine_id: String) -> Result<(), String> {
    *state.machine_id.lock().unwrap() = Some(machine_id);
    Ok(())
}

#[tauri::command]
pub async fn get_tailscale_ip(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(state.tailscale_ip.lock().unwrap().clone())
}

#[tauri::command]
pub async fn test_api_endpoint(endpoint: String) -> Result<serde_json::Value, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .post(&format!("{}/v1/chat/completions", endpoint))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": if cfg!(target_os = "macos") {
                "mlx-community/Qwen3.5-9B-MLX-4bit"
            } else {
                "Qwen/Qwen3.5-9B-Instruct"
            },
            "messages": [{"role": "user", "content": "Hello"}],
            "max_tokens": 10
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    Ok(serde_json::json!({
        "status": status.as_u16(),
        "response": text,
        "success": status.is_success()
    }))
}

#[tauri::command]
pub async fn get_app_state(state: State<'_, AppState>) -> Result<AppStateInfo, String> {
    Ok(AppStateInfo {
        machine_id: state.machine_id.lock().unwrap().clone(),
        tailscale_ip: state.tailscale_ip.lock().unwrap().clone(),
        is_running: *state.is_running.lock().unwrap(),
        tokens_per_second: *state.tokens_per_second.lock().unwrap(),
        gpu_usage: *state.gpu_usage.lock().unwrap(),
        today_earnings: *state.today_earnings.lock().unwrap(),
    })
}

pub fn start_background_tasks(app: AppHandle) -> Result<(), String> {
    let app_handle = app.clone();

    std::thread::spawn(move || {
        let client = Client::new();

        while STATUS_RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(30));

            let state = app_handle.state::<AppState>();
            if let Some(machine_id) = state.machine_id.lock().unwrap().clone() {
                if let Some(ip) = state.tailscale_ip.lock().unwrap().clone() {
                    let url = format!("{}/rest/v1/machines", SUPABASE_URL);
                    let body = serde_json::json!({
                        "machine_id": machine_id,
                        "endpoint": format!("http://{}:8000", ip),
                        "status": "online",
                        "last_seen": chrono::Utc::now().to_rfc3339()
                    });

                    let client = Client::new();
                    if let Err(e) = client
                        .patch(&url)
                        .header("apikey", SUPABASE_ANON_KEY)
                        .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
                        .header("Prefer", "return=minimal")
                        .json(&body)
                        .send()
                    {
                        error!("Failed to update machine status: {}", e);
                    }
                }
            }
        }
    });

    std::thread::spawn(move || {
        METRICS_RUNNING.store(true, Ordering::SeqCst);

        while METRICS_RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_secs(10));

            let state = app_handle.state::<AppState>();
            if let Some(machine_id) = state.machine_id.lock().unwrap().clone() {
                let tokens = *state.tokens_per_second.lock().unwrap();
                let gpu = *state.gpu_usage.lock().unwrap();

                let url = format!("{}/rest/v1/metrics_raw", SUPABASE_URL);
                let body = serde_json::json!({
                    "machine_id": machine_id,
                    "tokens_per_second": tokens,
                    "gpu_usage": gpu,
                    "recorded_at": chrono::Utc::now().to_rfc3339()
                });

                let client = Client::new();
                if let Err(e) = client
                    .post(&url)
                    .header("apikey", SUPABASE_ANON_KEY)
                    .header("Authorization", format!("Bearer {}", SUPABASE_ANON_KEY))
                    .header("Prefer", "return=minimal")
                    .json(&body)
                    .send()
                {
                    error!("Failed to report metrics: {}", e);
                }
            }
        }
    });

    Ok(())
}
