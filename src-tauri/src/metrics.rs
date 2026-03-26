use std::process::Command;

pub struct MetricsCollector;

impl MetricsCollector {
    pub fn get_tokens_per_second() -> f64 {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("curl")
                .args(["-s", "http://127.0.0.1:8000/v1/metrics"])
                .output()
            {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines() {
                        if line.contains("tokens_per_second") {
                            if let Some(val) = line.split_whitespace().last() {
                                return val.parse().unwrap_or(0.0);
                            }
                        }
                    }
                }
            }
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            if let Ok(output) = Command::new("docker")
                .args(["exec", "vllm-server", "curl", "-s", "http://localhost:8000/v1/metrics"])
                .output()
            {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines() {
                        if line.contains("tokens_per_second") {
                            if let Some(val) = line.split_whitespace().last() {
                                return val.parse().unwrap_or(0.0);
                            }
                        }
                    }
                }
            }
        }

        0.0
    }

    pub fn get_gpu_usage() -> f64 {
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = Command::new("powermetrics")
                .args(["--samplers", "gpu", "-i", "1000", "-n", "1"])
                .output()
            {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    for line in text.lines() {
                        if line.contains("GPU Activity") {
                            if let Some(percent) = line.split(':').nth(1) {
                                let val = percent.trim().trim_end_matches('%');
                                return val.parse().unwrap_or(0.0);
                            }
                        }
                    }
                }
            }
            0.0
        }

        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = Command::new("nvidia-smi")
                .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
                .output()
            {
                if output.status.success() {
                    return String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .parse()
                        .unwrap_or(0.0);
                }
            }
            0.0
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(output) = Command::new("nvidia-smi")
                .args(["--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"])
                .output()
            {
                if output.status.success() {
                    return String::from_utf8_lossy(&output.stdout)
                        .trim()
                        .parse()
                        .unwrap_or(0.0);
                }
            }
            0.0
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            0.0
        }
    }
}
