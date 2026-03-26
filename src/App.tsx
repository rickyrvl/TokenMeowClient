import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { QRCodeSVG } from "qrcode.react";
import { createClient } from "@supabase/supabase-js";

const SUPABASE_URL = "https://vbqnhltqslotsoalcmqc.supabase.co";
const SUPABASE_ANON_KEY = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InZicW5obHRxc2xvdHNvYWxjbXFjIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzQ0NzUxNjYsImV4cCI6MjA5MDA1MTE2Nn0.cuD0_s34ftrmH8a6W-1mX8jiTOP-j2Ysbm5OeLyax_g";

const supabase = createClient(SUPABASE_URL, SUPABASE_ANON_KEY);

type AppView = "qr" | "register" | "dashboard" | "admin";

interface Machine {
  machine_id: string;
  endpoint: string;
  status: string;
  last_seen: string;
}

interface Metrics {
  tokens_per_second: number;
  gpu_usage: number;
  today_earnings: number;
  platform: string;
}

interface GpuInfo {
  available: boolean;
  gpu_type: string;
  memory: number;
}

interface AppState {
  machine_id: string | null;
  tailscale_ip: string | null;
  is_running: boolean;
  tokens_per_second: number;
  gpu_usage: number;
  today_earnings: number;
}

function App() {
  const [view, setView] = useState<AppView>("qr");
  const [machineName, setMachineName] = useState("");
  const [machines, setMachines] = useState<Machine[]>([]);
  const [metrics, setMetrics] = useState<Metrics | null>(null);
  const [gpuInfo, setGpuInfo] = useState<GpuInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [appState, setAppState] = useState<AppState | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [testResults, setTestResults] = useState<Record<string, boolean>>({});

  const addLog = useCallback((message: string, type: "info" | "error" | "success" = "info") => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [...prev.slice(-50), `[${timestamp}] ${message}`]);
  }, []);

  const checkGpu = useCallback(async () => {
    try {
      const info = await invoke<GpuInfo>("check_gpu");
      setGpuInfo(info);
      if (!info.available) {
        setError(`No GPU detected: ${info.gpu_type}. This application requires NVIDIA GPU or Apple Silicon.`);
      }
      return info.available;
    } catch (e) {
      setError(`Failed to check GPU: ${e}`);
      return false;
    }
  }, []);

  const checkAuth = useCallback(async () => {
    try {
      const { data: { session } } = await supabase.auth.getSession();
      if (session) {
        const machineId = session.user.id;
        await invoke("set_machine_id", { machineId });
        addLog("Authenticated successfully", "success");
        
        const gpuAvailable = await checkGpu();
        if (gpuAvailable) {
          await initializeServices();
        }
      }
    } catch (e) {
      addLog(`Auth check failed: ${e}`, "error");
    }
  }, [checkGpu, addLog]);

  const initializeServices = useCallback(async () => {
    setLoading(true);
    addLog("Initializing services...");
    
    try {
      const state = await invoke<AppState>("get_app_state");
      setAppState(state);

      if (state.machine_id) {
        setView("dashboard");
      } else {
        setView("register");
      }
    } catch (e) {
      addLog(`Failed to get app state: ${e}`, "error");
    }

    setLoading(false);
  }, [addLog]);

  const handleSSOLogin = async (provider: "google" | "facebook" | "apple") => {
    setLoading(true);
    setError(null);
    addLog(`Starting ${provider} SSO login...`);

    try {
      const { data, error } = await supabase.auth.signInWithOAuth({
        provider,
        options: {
          redirectTo: "tokenmeow://auth-callback",
        },
      });

      if (error) throw error;
      
      if (data.url) {
        addLog("OAuth URL generated, please complete login in browser", "success");
      }
    } catch (e) {
      setError(`Login failed: ${e}`);
      addLog(`Login failed: ${e}`, "error");
    }
    setLoading(false);
  };

  const handleEmailLogin = async () => {
    setLoading(true);
    setError(null);
    addLog("Email login not implemented - using QR code SSO");

    const qrUrl = `https://vbqnhltqslotsoalcmqc.supabase.co/auth/v1/authorize?provider=google`;
    window.open(qrUrl, "_blank");

    setLoading(false);
  };

  const handleRegister = async () => {
    if (!machineName.trim()) {
      setError("Please enter a machine name");
      return;
    }

    setLoading(true);
    setError(null);
    addLog(`Registering machine: ${machineName}...`);

    try {
      const gpuAvailable = await checkGpu();
      if (!gpuAvailable) {
        throw new Error("No GPU available");
      }

      addLog("Installing Docker (if needed)...");
      await invoke("install_docker");
      addLog("Docker installation complete", "success");

      const { data: { session } } = await supabase.auth.getSession();
      const authKey = session?.user.user_metadata?.tailscale_auth_key || "test-auth-key";

      addLog("Starting Tailscale...");
      const tailscaleIp = await invoke<string>("start_tailscale", { authKey });
      addLog(`Tailscale started with IP: ${tailscaleIp}`, "success");

      let vllmResult: string;
      if (navigator.platform.toLowerCase().includes("mac")) {
        addLog("Starting vLLM-MLX (macOS)...");
        vllmResult = await invoke<string>("start_vllm_mlx");
      } else {
        addLog("Starting vLLM Docker (Windows/Linux)...");
        vllmResult = await invoke<string>("start_vllm_docker");
      }
      addLog(vllmResult, "success");

      const machineId = session?.user.id || `local-${Date.now()}`;
      await invoke("set_machine_id", { machineId });

      addLog("Updating machine status in database...");
      await invoke("update_machine_status", {
        machineId,
        endpoint: `http://${tailscaleIp}:8000`,
      });
      addLog("Machine registered successfully!", "success");

      setView("dashboard");
      await fetchMachines();
    } catch (e) {
      setError(`Registration failed: ${e}`);
      addLog(`Registration failed: ${e}`, "error");
    }

    setLoading(false);
  };

  const fetchMachines = async () => {
    try {
      const { data, error } = await supabase
        .from("machines")
        .select("*")
        .order("last_seen", { ascending: false });

      if (error) throw error;
      setMachines(data || []);
    } catch (e) {
      addLog(`Failed to fetch machines: ${e}`, "error");
    }
  };

  const testApiEndpoint = async (endpoint: string) => {
    try {
      addLog(`Testing API: ${endpoint}...`);
      const result = await invoke<{ status: number; success: boolean }>("test_api_endpoint", { endpoint });
      
      if (result.success) {
        addLog(`API test successful (${result.status})`, "success");
        setTestResults((prev) => ({ ...prev, [endpoint]: true }));
      } else {
        addLog(`API test failed (${result.status})`, "error");
        setTestResults((prev) => ({ ...prev, [endpoint]: false }));
      }
    } catch (e) {
      addLog(`API test error: ${e}`, "error");
      setTestResults((prev) => ({ ...prev, [endpoint]: false }));
    }
  };

  useEffect(() => {
    checkAuth();

    const interval = setInterval(async () => {
      try {
        const state = await invoke<AppState>("get_app_state");
        setAppState(state);

        const metricsData = await invoke<Metrics>("get_metrics");
        setMetrics(metricsData);
      } catch (e) {
        console.error("Failed to fetch metrics:", e);
      }
    }, 5000);

    return () => clearInterval(interval);
  }, [checkAuth]);

  useEffect(() => {
    if (view === "admin") {
      fetchMachines();
    }
  }, [view]);

  const renderQRView = () => (
    <div className="card">
      <h1 className="title">TokenMeow</h1>
      <p className="subtitle">Scan QR code to login</p>

      <div className="qr-container">
        <div className="qr-box">
          <QRCodeSVG
            value={`https://vbqnhltqslotsoalcmqc.supabase.co/auth/v1/authorize?provider=google&redirect_to=tokenmeow://callback`}
            size={200}
            level="H"
          />
        </div>
      </div>

      <div className="divider">or continue with</div>

      <div className="sso-buttons">
        <button className="sso-btn" onClick={() => handleSSOLogin("google")}>
          <svg width="18" height="18" viewBox="0 0 24 24">
            <path fill="#4285F4" d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"/>
            <path fill="#34A853" d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"/>
            <path fill="#FBBC05" d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"/>
            <path fill="#EA4335" d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"/>
          </svg>
          Google
        </button>
        <button className="sso-btn" onClick={() => handleSSOLogin("facebook")}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="#1877F2">
            <path d="M24 12.073c0-6.627-5.373-12-12-12s-12 5.373-12 12c0 5.99 4.388 10.954 10.125 11.854v-8.385H7.078v-3.47h3.047V9.43c0-3.007 1.792-4.669 4.533-4.669 1.312 0 2.686.235 2.686.235v2.953H15.83c-1.491 0-1.956.925-1.956 1.874v2.25h3.328l-.532 3.47h-2.796v8.385C19.612 23.027 24 18.062 24 12.073z"/>
          </svg>
          Facebook
        </button>
        <button className="sso-btn" onClick={() => handleSSOLogin("apple")}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="#fff">
            <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z"/>
          </svg>
          Apple
        </button>
        <button className="sso-btn" onClick={handleEmailLogin}>
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"/>
            <polyline points="22,6 12,13 2,6"/>
          </svg>
          Email
        </button>
      </div>

      {error && <div className="error-message">{error}</div>}
    </div>
  );

  const renderRegisterView = () => (
    <div className="card">
      <h1 className="title">Register Machine</h1>
      <p className="subtitle">Enter a name for this compute node</p>

      <input
        type="text"
        className="input-field"
        placeholder="Machine name (e.g., GPU-Rig-01)"
        value={machineName}
        onChange={(e) => setMachineName(e.target.value)}
        disabled={loading}
      />

      <button
        className="btn btn-primary"
        onClick={handleRegister}
        disabled={loading || !machineName.trim()}
      >
        {loading ? "Setting up..." : "Confirm & Start"}
      </button>

      {gpuInfo && (
        <div className="status-bar" style={{ marginTop: 16 }}>
          <div className="status-item">
            <div className="status-value">{gpuInfo.gpu_type}</div>
            <div className="status-label">GPU</div>
          </div>
          <div className="status-item">
            <div className="status-value">{gpuInfo.memory > 0 ? `${gpuInfo.memory}MB` : "MLX"}</div>
            <div className="status-label">Memory</div>
          </div>
        </div>
      )}

      {error && <div className="error-message" style={{ marginTop: 16 }}>{error}</div>}

      {loading && (
        <div className="loading" style={{ marginTop: 16 }}>
          <div className="spinner"></div>
          <p style={{ marginTop: 8, fontSize: 12 }}>Initializing services...</p>
        </div>
      )}

      <div className="logs-container">
        {logs.map((log, i) => (
          <div key={i} className={`log-entry ${log.includes("error") ? "error" : log.includes("success") ? "success" : ""}`}>
            {log}
          </div>
        ))}
      </div>
    </div>
  );

  const renderDashboardView = () => (
    <div className="card">
      <h1 className="title">TokenMeow</h1>
      <p className="subtitle">Compute Node Dashboard</p>

      <div className="status-bar">
        <div className="status-item">
          <span className={`status-indicator ${appState?.is_running ? "status-online" : "status-offline"}`}></span>
          <span className="status-label">Status</span>
        </div>
        <div className="status-item">
          <div className="status-value">{metrics?.tokens_per_second.toFixed(1) || "0.0"}</div>
          <div className="status-label">Tokens/s</div>
        </div>
        <div className="status-item">
          <div className="status-value">{metrics?.gpu_usage.toFixed(0) || "0"}%</div>
          <div className="status-label">GPU</div>
        </div>
        <div className="status-item">
          <div className="status-value">${metrics?.today_earnings.toFixed(4) || "0.0000"}</div>
          <div className="status-label">Today</div>
        </div>
      </div>

      <div className="nav-tabs">
        <button className="nav-tab active">Dashboard</button>
        <button className="nav-tab" onClick={() => setView("admin")}>Admin</button>
      </div>

      <div className="status-bar" style={{ flexDirection: "column", alignItems: "flex-start", gap: 8 }}>
        <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
          <span className="status-label">Machine ID:</span>
          <span style={{ fontFamily: "monospace", fontSize: 12 }}>{appState?.machine_id || "N/A"}</span>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
          <span className="status-label">Tailscale IP:</span>
          <span style={{ fontFamily: "monospace", fontSize: 12 }}>{appState?.tailscale_ip || "N/A"}</span>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
          <span className="status-label">Platform:</span>
          <span style={{ fontFamily: "monospace", fontSize: 12 }}>{metrics?.platform || "N/A"}</span>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
          <span className="status-label">API Endpoint:</span>
          <span style={{ fontFamily: "monospace", fontSize: 12 }}>
            {appState?.tailscale_ip ? `http://${appState.tailscale_ip}:8000` : "N/A"}
          </span>
        </div>
      </div>

      <button className="btn btn-primary" style={{ marginTop: 16 }} onClick={() => window.close()}>
        Minimize to Tray
      </button>
    </div>
  );

  const renderAdminView = () => (
    <div className="card">
      <h1 className="title">Admin Panel</h1>
      <p className="subtitle">Manage compute nodes</p>

      <div className="nav-tabs">
        <button className="nav-tab" onClick={() => setView("dashboard")}>Dashboard</button>
        <button className="nav-tab active">Admin</button>
      </div>

      <div className="machine-list">
        {machines.length === 0 ? (
          <p style={{ textAlign: "center", color: "rgba(255,255,255,0.5)", padding: 20 }}>
            No machines found
          </p>
        ) : (
          machines.map((machine) => (
            <div key={machine.machine_id} className="machine-item">
              <div className="machine-info">
                <div className="machine-name">
                  <span className={`status-indicator ${machine.status === "online" ? "status-online" : "status-offline"}`}></span>
                  {machine.machine_id.slice(0, 8)}...
                </div>
                <div className="machine-endpoint">{machine.endpoint}</div>
              </div>
              <button
                className="test-btn"
                onClick={() => testApiEndpoint(machine.endpoint)}
              >
                {testResults[machine.endpoint] === true ? "✓" : testResults[machine.endpoint] === false ? "✗" : "Test"}
              </button>
            </div>
          ))
        )}
      </div>

      <button className="btn btn-primary" style={{ marginTop: 16 }} onClick={fetchMachines}>
        Refresh
      </button>

      <button className="btn btn-primary" style={{ marginTop: 8, background: "rgba(255,255,255,0.1)" }} onClick={() => setView("dashboard")}>
        Back to Dashboard
      </button>
    </div>
  );

  return (
    <div className="container">
      {view === "qr" && renderQRView()}
      {view === "register" && renderRegisterView()}
      {view === "dashboard" && renderDashboardView()}
      {view === "admin" && renderAdminView()}
    </div>
  );
}

export default App;
