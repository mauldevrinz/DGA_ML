import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Play, Square, Activity, Cpu, Upload, Wifi, WifiOff, Zap, Clock, Wind, Flame, Droplets } from 'lucide-react';
import './Classification.css';

// Gas composition labels — matches new training model (C2H2, C2H4, CH4, H2, OTHER)
const GAS_NAMES   = ['C₂H₂', 'C₂H₄', 'CH₄', 'H₂', 'OTHER'];
const GAS_KEYS    = ['C2H2', 'C2H4', 'CH4', 'H2', 'OTHER'];   // keys from backend JSON
const GAS_LABELS  = ['Acetylene', 'Ethylene', 'Methane', 'Hydrogen', 'Other Gases'];
const GAS_COLORS  = ['#ef4444', '#f97316', '#eab308', '#8b5cf6', '#10b981'];
const GAS_ICONS   = [Zap, Flame, Activity, Droplets, Wind];

// IEC severity → color mapping (diagnosis comes from backend now)
const SEVERITY_STYLE = {
  normal:   { bg: '#10b98130', color: '#6ee7b7', label: 'NORMAL' },
  caution:  { bg: '#eab30830', color: '#fde047', label: 'CAUTION' },
  warning:  { bg: '#f9731630', color: '#fdba74', label: 'WARNING' },
  critical: { bg: '#ef444430', color: '#fca5a5', label: 'CRITICAL' },
};

const Classification = () => {
  const [isRunning, setIsRunning] = useState(false);
  const [wsConnected, setWsConnected] = useState(false);
  const wsRef = useRef(null);
  const csvModeRef = useRef(false);
  const fileInputRef = useRef(null);

  // SNN inference results
  const [snnResult, setSnnResult] = useState(null);
  const [latency, setLatency] = useState(null);
  const [inferenceCount, setInferenceCount] = useState(0);
  const [history, setHistory] = useState([]);
  const [fpgaStatus, setFpgaStatus] = useState('idle'); // 'idle' | 'running' | 'error'
  const [errorMsg, setErrorMsg] = useState('');
  const [csvProgress, setCsvProgress] = useState(null);
  const [csvDialog, setCsvDialog] = useState(null);
  const [kriaPower, setKriaPower] = useState(null); // { v, i, p }

  const showDialog = useCallback((message, title = 'CSV Prediction') => {
    setCsvDialog({ title, message });
  }, []);

  // Connect to WebSocket
  const connectWs = useCallback(() => {
    if (wsRef.current && [WebSocket.CONNECTING, WebSocket.OPEN].includes(wsRef.current.readyState)) return;

    const host = window.location.hostname || '127.0.0.1';
    const ws = new WebSocket(`ws://${host}:8080`);
    wsRef.current = ws;

    ws.onopen = () => {
      setWsConnected(true);
      setErrorMsg('');
      ws.send(JSON.stringify({ type: 'GET_CYCLE_INFERENCE' }));
    };

    ws.onmessage = (event) => {
      const line = event.data;

      // Parse JSON responses
      if (typeof line === 'string' && line.trimStart().startsWith('{')) {
        try {
          const json = JSON.parse(line);
          if (json.type === 'CSV_PROGRESS') {
            setCsvProgress(json);
            return;
          }
          if (json.type === 'CYCLE_INFERENCE_ACK') {
            setIsRunning(Boolean(json.enabled));
            if (!json.enabled) setFpgaStatus('idle');
            return;
          }
          if (json.type === 'SNN_RESULT') {
            if (csvModeRef.current && json.inference_mode !== 'cycle') {
              csvModeRef.current = false;
              if (json.ok) {
                setCsvProgress({ progress: 100, message: 'CSV prediction complete' });
                setCsvDialog({
                  title: 'Transformer Oil Prediction Result',
                  data: json,
                });
              } else {
                setCsvDialog({
                  title: 'CSV Prediction Failed',
                  error: json.error || 'Unknown FPGA error',
                });
              }
              return;
            }
            if (json.ok) {
              setErrorMsg('');
              setSnnResult(json);
              setLatency(json.latency_ms);
              setFpgaStatus('running');
              setInferenceCount(prev => prev + 1);
              setHistory(prev => [{
                time: new Date().toLocaleTimeString(),
                  cycle: json.cycle,
                predicted_gas: json.predicted_gas,
                predicted_idx: json.predicted_idx,
                confidence: json.confidence,
                predicted_fault: json.predicted_fault,
                latency_ms: json.latency_ms,
                cycle_sample_count: json.cycle_sample_count,
                outputs: json.outputs,
                gas_confidences: json.gas_confidences,
                iec_diagnosis: json.iec_diagnosis,
              }, ...prev].slice(0, 10));
            } else {
              setFpgaStatus('error');
              const message = json.error || json.message || 'Unknown FPGA error';
              setErrorMsg(message);
              if (json.inference_mode === 'cycle') {
                setInferenceCount(prev => prev + 1);
                setHistory(prev => [{
                  time: new Date().toLocaleTimeString(),
                  cycle: json.cycle,
                  cycle_sample_count: json.cycle_sample_count,
                  error: message,
                }, ...prev].slice(0, 10));
              }
            }
          }
        } catch { /* ignore non-JSON */ }
        return;
      }

      // Parse KRIA power monitoring telemetry
      if (typeof line === 'string' && line.startsWith('KRIA Vbus=')) {
        const m = line.match(/KRIA Vbus=(-?[0-9.]+)V I=(-?[0-9.]+)A P=(-?[0-9.]+)W/);
        if (m) {
          setKriaPower({ v: m[1], i: m[2], p: m[3] });
        }
        return;
      }

    };

    ws.onclose = () => {
      // Ignore close events from an older socket after a replacement connected.
      if (wsRef.current === ws) {
        wsRef.current = null;
        setWsConnected(false);
        setIsRunning(false);
        setFpgaStatus('idle');
      }
    };

    ws.onerror = () => {
      setErrorMsg('WebSocket connection failed. Is serial_ws.py running?');
    };
  }, []);

  // Disconnect
  const disconnectWs = useCallback(() => {
    const ws = wsRef.current;
    wsRef.current = null;
    if (ws && ws.readyState !== WebSocket.CLOSED && ws.readyState !== WebSocket.CLOSING) {
      ws.close();
    }
    setWsConnected(false);
  }, []);

  const setCycleInference = useCallback((enabled) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    setIsRunning(enabled);
    setFpgaStatus(enabled ? 'running' : 'idle');
    ws.send(JSON.stringify({ type: 'SET_CYCLE_INFERENCE', enabled }));
  }, []);

  // Enable or disable one inference after each completed acquisition cycle.
  const toggleInference = useCallback(() => {
    if (isRunning) {
      setCycleInference(false);
      setIsRunning(false);
      setFpgaStatus('idle');
    } else {
      if (!wsConnected) {
        connectWs();
        setErrorMsg('Backend is reconnecting. Press Start FPGA Inference again when connected.');
        return;
      }
      setCycleInference(true);
      setIsRunning(true);
      setFpgaStatus('running');
    }
  }, [isRunning, wsConnected, connectWs, setCycleInference]);

  // Auto-connect on mount
  useEffect(() => {
    connectWs();
    return () => {
      disconnectWs();
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // CSV upload handler
  const handleCsvUpload = (e) => {
    const file = e.target.files[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const lines = reader.result.split('\n').filter(l => l.trim());
      if (lines.length < 2) { showDialog('CSV is empty.'); return; }
      // Parse header to find sensor columns
      const header = lines[0].split(',').map(h => h.trim());
      const dataLines = lines.slice(1);
      const rows = dataLines.map(line => {
        const cols = line.split(',');
        return Object.fromEntries(header.map((name, i) => [name, parseFloat(cols[i]) || 0]));
      });
      const sendCsv = (attempt = 0) => {
        const csvWs = wsRef.current;
        if (csvWs && csvWs.readyState === WebSocket.OPEN) {
          csvModeRef.current = true;
          csvWs.send(JSON.stringify({ type: 'SNN_CSV_INFER', rows }));
          showDialog(`CSV submitted for inference: ${rows.length} rows.`);
          return;
        }
        if (attempt < 10) {
          if (!csvWs || csvWs.readyState === WebSocket.CLOSED) connectWs();
          setTimeout(() => sendCsv(attempt + 1), 500);
        } else {
          showDialog('Backend WebSocket did not connect after 5 seconds. Check port 8080.');
        }
      };
      sendCsv();
      return;

    };
    reader.readAsText(file);
    e.target.value = '';
  };

  // Current diagnosis from backend IEC 60599 analysis
  const currentDiag = snnResult?.iec_diagnosis || null;
  const activeSeverity = SEVERITY_STYLE[currentDiag?.severity] || SEVERITY_STYLE.normal;
  const activeSeveritySolid = {
    normal: { background: '#10b981', foreground: '#064e3b' },
    caution: { background: '#eab308', foreground: '#ffffff' },
    warning: { background: '#f97316', foreground: '#431407' },
    critical: { background: '#ef4444', foreground: '#450a0a' },
  }[currentDiag?.severity] || { background: '#10b981', foreground: '#064e3b' };
  const activeFaultClassStyle = {
    normal: { background: '#15803d' },
    caution: { background: '#a16207' },
    warning: { background: '#c2410c' },
    critical: { background: '#b91c1c' },
  }[currentDiag?.severity] || { background: '#15803d' };
  const topGasIdx = snnResult ? snnResult.predicted_idx : 0;

  return (
    <div className="page-container">
      {(csvProgress || csvDialog) && (
        <div role="dialog" aria-modal="true" style={{ position: 'fixed', inset: 0, zIndex: 1100, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 20, backgroundColor: 'rgba(15, 23, 42, 0.65)', backdropFilter: 'blur(6px)' }}>
          <div style={{ width: 'min(640px, 100%)', maxHeight: '90vh', overflowY: 'auto', padding: 26, borderRadius: 20, backgroundColor: '#ffffff', color: '#0f172a', border: '1px solid #e2e8f0', boxShadow: '0 25px 60px -15px rgba(0, 0, 0, 0.35)' }}>

            {/* Header */}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', paddingBottom: 16, borderBottom: '1px solid #f1f5f9' }}>
              <div>
                <div style={{ color: '#0284c7', fontSize: 11, fontWeight: 700, letterSpacing: 1.5, textTransform: 'uppercase' }}>DGA Intelligent Analysis</div>
                <h3 style={{ margin: '4px 0 0', color: '#0f172a', fontSize: 20, fontWeight: 800 }}>{csvDialog?.title || 'CSV Prediction'}</h3>
              </div>
              <button
                type="button"
                onClick={() => { setCsvDialog(null); setCsvProgress(null); csvModeRef.current = false; }}
                aria-label="Close"
                style={{ color: '#64748b', background: '#f1f5f9', border: '1px solid #e2e8f0', borderRadius: 10, width: 34, height: 34, cursor: 'pointer', fontSize: 16, display: 'flex', alignItems: 'center', justifyContent: 'center', transition: 'all 0.2s' }}
              >✕</button>
            </div>

            {/* Progress Bar during calculation */}
            {csvProgress && csvProgress.progress < 100 && (
              <div style={{ marginTop: 20, padding: 16, borderRadius: 14, backgroundColor: '#f8fafc', border: '1px solid #e2e8f0' }}>
                <div style={{ color: '#475569', fontSize: 13, marginBottom: 8, fontWeight: 500 }}>{csvProgress.message}</div>
                <div style={{ height: 8, backgroundColor: '#e2e8f0', borderRadius: 99, overflow: 'hidden' }}>
                  <div style={{ width: `${csvProgress.progress}%`, height: '100%', background: 'linear-gradient(90deg, #0284c7, #10b981)', borderRadius: 99, transition: 'width 0.3s ease' }} />
                </div>
                <div style={{ color: '#0284c7', fontSize: 12, marginTop: 6, textAlign: 'right', fontWeight: 700 }}>{csvProgress.progress}%</div>
              </div>
            )}

            {/* Error Message if failed */}
            {csvDialog?.error && (
              <div style={{ marginTop: 18, padding: 16, borderRadius: 12, backgroundColor: '#fef2f2', border: '1px solid #fecaca', color: '#b91c1c', fontSize: 14 }}>
                <strong>Error:</strong> {csvDialog.error}
              </div>
            )}

            {/* Simple Text Fallback if message string provided */}
            {csvDialog?.message && (
              <pre style={{ whiteSpace: 'pre-wrap', lineHeight: 1.6, color: '#1e293b', backgroundColor: '#f8fafc', padding: 16, borderRadius: 12, border: '1px solid #e2e8f0', fontFamily: 'monospace', fontSize: 13, margin: '18px 0' }}>{csvDialog.message}</pre>
            )}

            {/* Rich Card Visual Result */}
            {csvDialog?.data && (() => {
              const res = csvDialog.data;
              const diag = res.iec_diagnosis || {};
              const fault = res.predicted_fault || diag.fault_code || 'Normal';
              const sevKey = (diag.severity || 'normal').toLowerCase();

              // Bold solid severity theme with crisp white text
              const SOLID_SEV = {
                normal:   { bg: '#10b981', color: '#ffffff', subColor: '#d1fae5', badgeBg: 'rgba(255, 255, 255, 0.25)', label: 'NORMAL', shadow: '0 4px 14px rgba(16, 185, 129, 0.4)' },
                caution:  { bg: '#eab308', color: '#ffffff', subColor: '#fef9c3', badgeBg: 'rgba(0, 0, 0, 0.18)', label: 'CAUTION', shadow: '0 4px 14px rgba(234, 179, 8, 0.4)' },
                warning:  { bg: '#f97316', color: '#ffffff', subColor: '#ffedd5', badgeBg: 'rgba(255, 255, 255, 0.25)', label: 'WARNING', shadow: '0 4px 14px rgba(249, 115, 22, 0.4)' },
                critical: { bg: '#ef4444', color: '#ffffff', subColor: '#fee2e2', badgeBg: 'rgba(255, 255, 255, 0.25)', label: 'CRITICAL', shadow: '0 4px 14px rgba(239, 68, 68, 0.4)' },
              };
              const sevInfo = SOLID_SEV[sevKey] || SOLID_SEV.normal;

              const gasConfs = res.gas_confidences || {};
              const dominantGas = res.predicted_gas || '—';
              const dominantPct = ((res.confidence || 0) * 100).toFixed(1);

              return (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 16, marginTop: 18 }}>

                  {/* 1. OIL FAULT CLASSIFICATION CARD (SOLID COLOR + WHITE TEXT) */}
                  <div style={{
                    padding: '18px 22px',
                    borderRadius: 16,
                    backgroundColor: sevInfo.bg,
                    boxShadow: sevInfo.shadow,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    color: '#ffffff',
                  }}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <span style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1.2, color: sevInfo.subColor, textTransform: 'uppercase' }}>
                        Oil Fault Classification
                      </span>
                      <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
                        <span style={{ fontSize: 30, fontWeight: 900, color: '#ffffff', letterSpacing: '-0.5px' }}>
                          {fault}
                        </span>
                        <span style={{ fontSize: 15, fontWeight: 700, color: '#ffffff', opacity: 0.95 }}>
                          ({diag.fault_type || 'No Fault'})
                        </span>
                      </div>
                      <span style={{ fontSize: 13, color: sevInfo.subColor, marginTop: 2, fontWeight: 500 }}>
                        {diag.description || 'Transformer oil condition is normal'}
                      </span>
                    </div>

                    <div style={{
                      padding: '6px 14px',
                      borderRadius: 20,
                      backgroundColor: sevInfo.badgeBg,
                      color: '#ffffff',
                      fontSize: 12,
                      fontWeight: 800,
                      letterSpacing: 1,
                      textTransform: 'uppercase',
                      border: '1px solid rgba(255, 255, 255, 0.35)',
                      backdropFilter: 'blur(4px)'
                    }}>
                      {sevInfo.label}
                    </div>
                  </div>

                  {/* 2. DISSOLVED GAS COMPOSITION (SNN OUTPUT) */}
                  <div style={{ padding: '16px 20px', borderRadius: 16, backgroundColor: '#f8fafc', border: '1px solid #e2e8f0' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 14 }}>
                      <span style={{ fontSize: 11, fontWeight: 700, letterSpacing: 1, color: '#64748b', textTransform: 'uppercase' }}>
                        Dissolved Gas Composition (SNN Output)
                      </span>
                      <span style={{ fontSize: 12, color: '#0284c7', fontWeight: 700 }}>
                        Dominant: <strong>{dominantGas} ({dominantPct}%)</strong>
                      </span>
                    </div>

                    <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                      {GAS_KEYS.map((key, idx) => {
                        const val = gasConfs[key] || 0;
                        const isTop = key === dominantGas;
                        const color = GAS_COLORS[idx];

                        return (
                          <div key={key} style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                            <div style={{ width: 68 }}>
                              <span style={{ fontSize: 13, fontWeight: isTop ? 800 : 600, color: isTop ? '#0f172a' : '#64748b' }}>
                                {GAS_NAMES[idx]}
                              </span>
                            </div>

                            <div style={{ flex: 1, height: 8, backgroundColor: '#e2e8f0', borderRadius: 99, overflow: 'hidden' }}>
                              <div style={{
                                width: `${Math.min(100, Math.max(0, val))}%`,
                                height: '100%',
                                backgroundColor: color,
                                borderRadius: 99,
                                transition: 'width 0.4s ease',
                              }} />
                            </div>

                            <span style={{ width: 45, textAlign: 'right', fontSize: 12, fontWeight: isTop ? 800 : 600, color: isTop ? '#0f172a' : '#64748b' }}>
                              {val}%
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  {/* 3 & 4. PERFORMANCE & POWER METRICS GRID */}
                  <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: 12 }}>

                    {/* Latency Metrics */}
                    <div style={{ padding: '14px 16px', borderRadius: 14, backgroundColor: '#f8fafc', border: '1px solid #e2e8f0' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginBottom: 8, color: '#0284c7' }}>
                        <Clock size={14} />
                        <span style={{ fontSize: 11, fontWeight: 700, letterSpacing: 0.8, textTransform: 'uppercase' }}>Latency Metrics</span>
                      </div>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, fontSize: 12, color: '#475569' }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                          <span>Feature Extraction:</span>
                          <strong style={{ color: '#0f172a' }}>{res.feature_latency_ms ?? '—'} ms</strong>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                          <span>SNN Inference:</span>
                          <strong style={{ color: '#0f172a' }}>{res.latency_ms ?? '—'} ms</strong>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                          <span>Execution Engine:</span>
                          <strong style={{ color: res.mode === 'hw' ? '#15803d' : '#b45309' }}>
                            {res.mode === 'hw' ? 'FPGA hardware' : `Software fallback (${res.mode || 'unknown'})`}
                          </strong>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between', borderTop: '1px solid #e2e8f0', paddingTop: 4, marginTop: 2 }}>
                          <span>Total Latency:</span>
                          <strong style={{ color: '#0284c7' }}>{res.total_latency_ms ?? '—'} ms</strong>
                        </div>
                      </div>
                    </div>

                    {/* Board Power Metrics */}
                    <div style={{ padding: '14px 16px', borderRadius: 14, backgroundColor: '#f8fafc', border: '1px solid #e2e8f0' }}>
                      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: '#b45309' }}>
                          <Zap size={14} />
                          <span style={{ fontSize: 11, fontWeight: 700, letterSpacing: 0.8, textTransform: 'uppercase' }}>Kria KV260 Power</span>
                        </div>
                        {res.power_sample_count > 0 && (
                          <span style={{ fontSize: 10, color: '#64748b', fontWeight: 600 }}>{res.power_sample_count} samples</span>
                        )}
                      </div>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: 4, fontSize: 12, color: '#475569' }}>
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                          <span>Avg Power ({res.total_latency_ms ?? 0}ms):</span>
                          <strong style={{ color: '#b45309' }}>{res.kria_power_w != null ? `${res.kria_power_w} W` : 'N/A'}</strong>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                          <span>Total Energy:</span>
                          <strong style={{ color: '#15803d' }}>{res.kria_energy_mj != null ? `${res.kria_energy_mj} mJ` : '—'}</strong>
                        </div>
                        <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                          <span>Peak / Voltage:</span>
                          <strong style={{ color: '#0f172a' }}>{res.kria_peak_power_w != null ? `${res.kria_peak_power_w} W` : 'N/A'} / {res.kria_vbus_v != null ? `${res.kria_vbus_v} V` : 'N/A'}</strong>
                        </div>
                        {res.power_status && res.power_status !== 'ok' && (
                          <div title={res.power_status} style={{ marginTop: 3, color: '#b45309', fontSize: 10 }}>
                            Power telemetry unavailable: {res.power_status}
                          </div>
                        )}
                      </div>
                    </div>

                  </div>

                </div>
              );
            })()}

            {/* Close Button */}
            {csvDialog && (
              <button
                type="button"
                onClick={() => { setCsvDialog(null); setCsvProgress(null); csvModeRef.current = false; }}
                style={{
                  color: '#fff',
                  background: 'linear-gradient(135deg, #0284c7, #2563eb)',
                  border: 0,
                  borderRadius: 12,
                  padding: '12px 24px',
                  fontWeight: 700,
                  fontSize: 14,
                  cursor: 'pointer',
                  width: '100%',
                  marginTop: 20,
                  transition: 'opacity 0.2s',
                  boxShadow: '0 4px 14px rgba(2, 132, 199, 0.25)'
                }}
              >
                Done
              </button>
            )}

          </div>
        </div>
      )}

      <div className="page-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 className="page-title">Real-Time DGA Classification</h2>
          <span className="app-subtitle" style={{ display: 'inline-block', marginLeft: '12px' }}>
            SNN FPGA Inference — IEC 60599 Diagnosis
          </span>
        </div>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          {/* Connection status badge */}
          <div style={{
            display: 'flex', gap: '8px', alignItems: 'center',
            backgroundColor: '#243F81', padding: '6px 14px', borderRadius: '24px',
            border: `1px solid ${wsConnected ? 'rgba(16, 185, 129, 0.4)' : 'rgba(239, 68, 68, 0.4)'}`,
          }}>
            {wsConnected ? <Wifi size={14} color="#10b981" /> : <WifiOff size={14} color="#ef4444" />}
            <span style={{ color: wsConnected ? '#10b981' : '#ef4444', fontSize: '12px', fontWeight: 600 }}>
              {wsConnected ? 'Backend Connected' : 'Disconnected'}
            </span>
          </div>
          {latency !== null && (
            <div style={{
              display: 'flex', gap: '8px', alignItems: 'center',
              backgroundColor: '#243F81', padding: '6px 14px', borderRadius: '24px',
              border: '1px solid rgba(117, 189, 224, 0.4)',
            }}>
              <Clock size={14} color="#75BDE0" />
              <span style={{ color: '#fff', fontSize: '12px' }}>
                Inference Latency: <strong style={{ color: '#75BDE0' }}>{latency} ms</strong>
              </span>
            </div>
          )}
          {snnResult?.mode && (
            <div style={{
              display: 'flex', gap: '8px', alignItems: 'center',
              backgroundColor: '#243F81', padding: '6px 14px', borderRadius: '24px',
              border: `1px solid ${snnResult.mode === 'hw' ? 'rgba(16, 185, 129, 0.4)' : 'rgba(234, 179, 8, 0.45)'}`,
            }}>
              <Cpu size={14} color={snnResult.mode === 'hw' ? '#10b981' : '#facc15'} />
              <span style={{ color: '#fff', fontSize: '12px' }}>
                Engine: <strong style={{ color: snnResult.mode === 'hw' ? '#10b981' : '#facc15' }}>
                  {snnResult.mode === 'hw' ? 'FPGA' : `Software fallback (${snnResult.mode})`}
                </strong>
              </span>
            </div>
          )}
          {isRunning && (
            <div style={{
              display: 'flex', gap: '8px', alignItems: 'center',
              backgroundColor: '#243F81', padding: '6px 14px', borderRadius: '24px',
              border: '1px solid rgba(117, 189, 224, 0.3)',
            }}>
              <Activity size={14} color="#75BDE0" />
              <span style={{ color: '#fff', fontSize: '12px' }}>
                 Inferences: <strong>{inferenceCount}</strong>
              </span>
            </div>
          )}
          {kriaPower && (
            <div style={{
              display: 'flex', gap: '8px', alignItems: 'center',
              backgroundColor: '#243F81', padding: '6px 14px', borderRadius: '24px',
              border: '1px solid rgba(16, 185, 129, 0.4)',
            }}>
              <Zap size={14} color="#10b981" />
              <span style={{ color: '#fff', fontSize: '12px' }}>
                Kria Power: <strong style={{ color: '#10b981' }}>{kriaPower.p} W</strong> ({kriaPower.v}V, {kriaPower.i}A)
              </span>
            </div>
          )}
        </div>
      </div>

      <div className="cls-layout">
        {/* LEFT PANEL — Controls */}
        <div className="cls-left">
          <div className="card" style={{ backgroundColor: 'var(--surface-color)' }}>
            <h3 className="card-title" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <Cpu size={18} /> FPGA SNN Accelerator
            </h3>

            {/* FPGA Hardware info */}
            <div style={{ marginTop: '16px', padding: '12px', backgroundColor: '#243F81', borderRadius: '8px', border: '1px solid rgba(35, 63, 124, 0.4)' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
                <span style={{ fontSize: '12px', color: '#94a3b8' }}>Platform</span>
                <span style={{ fontSize: '12px', color: '#fff', fontWeight: 600 }}>Kria KV260</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
                <span style={{ fontSize: '12px', color: '#94a3b8' }}>IP Core</span>
                <span style={{ fontSize: '12px', color: '#fff', fontWeight: 600 }}>SNN HLS @ 0xB0000000</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
                <span style={{ fontSize: '12px', color: '#94a3b8' }}>Model</span>
                <span style={{ fontSize: '12px', color: '#fff', fontWeight: 600 }}>Spiking Neural Network</span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ fontSize: '12px', color: '#94a3b8' }}>Status</span>
                <span style={{
                  fontSize: '12px', fontWeight: 600,
                  color: fpgaStatus === 'running' ? '#10b981' : fpgaStatus === 'error' ? '#ef4444' : '#94a3b8'
                }}>
                  {fpgaStatus === 'running' ? '● Operating' : fpgaStatus === 'error' ? '● Error' : '○ Idle'}
                </span>
              </div>
            </div>

            {errorMsg && (
              <div style={{ marginTop: '12px', padding: '10px', backgroundColor: 'rgba(239, 68, 68, 0.15)', border: '1px solid rgba(239, 68, 68, 0.3)', borderRadius: '8px', fontSize: '12px', color: '#fca5a5' }}>
                ⚠ {errorMsg}
              </div>
            )}

            {/* Start/Stop Button */}
            <button
              className="btn btn-primary"
              style={{
                width: '100%', marginTop: '20px', height: '48px', fontSize: '15px',
                background: isRunning
                  ? 'linear-gradient(135deg, #dc2626 0%, #ef4444 100%)'
                  : 'linear-gradient(135deg, #233F7C 0%, #2563eb 100%)',
                border: 'none',
              }}
              onClick={toggleInference}
              disabled={!wsConnected}
            >
              {isRunning ? <Square size={18} fill="currentColor" /> : <Play size={18} fill="currentColor" />}
              {isRunning ? ' Stop FPGA Inference' : ' Start FPGA Inference'}
            </button>

            {/* CSV Upload */}
            <div style={{ position: 'relative', marginTop: '12px' }}>
              <input
                ref={fileInputRef}
                type="file"
                accept=".csv"
                style={{ display: 'none' }}
                id="csv-upload"
                onChange={handleCsvUpload}
              />
              <button
                type="button"
                className="btn btn-outline"
                style={{
                  width: '100%',
                  height: '42px',
                  fontSize: '14px',
                  display: 'flex',
                  justifyContent: 'center',
                  alignItems: 'center',
                  gap: '8px',
                  cursor: 'pointer'
                }}
                onClick={() => {
                  if (!wsConnected) {
                    connectWs();
                  }
                  if (fileInputRef.current) {
                    fileInputRef.current.value = '';
                    fileInputRef.current.click();
                  } else {
                    document.getElementById('csv-upload')?.click();
                  }
                }}
              >
                <Upload size={16} /> Predict from Historical Data
              </button>
            </div>

          </div>

          {/* Inference History */}
          <div className="card" style={{ marginTop: '24px', flex: 1, display: 'flex', flexDirection: 'column' }}>
            <h3 className="card-title">Inference History</h3>
            {history.length === 0 ? (
              <div className="empty-box" style={{ height: '200px' }}>No predictions yet. Start FPGA inference.</div>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginTop: '12px' }}>
                {history.map((h, i) => {
                  const color = GAS_COLORS[h.predicted_idx] || '#94a3b8';
                  const faultCode = h.error ? 'ERROR' : (h.predicted_fault || h.iec_diagnosis?.fault_code || '—');
                  return (
                    <div key={i} style={{
                      display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                      padding: '10px 12px', backgroundColor: '#243F81', borderRadius: '8px',
                      border: `1px solid ${i === 0 ? color + '40' : 'rgba(35, 63, 124, 0.4)'}`,
                    }}>
                      <span style={{ fontSize: '11px', color: '#75BDE0', minWidth: '125px' }}>
                        {h.cycle ? `Cycle ${h.cycle}${h.cycle_sample_count ? ` · ${h.cycle_sample_count} samples` : ''}` : h.time}
                      </span>
                      <span style={{ fontWeight: 'bold', color: h.error ? '#fca5a5' : color, fontSize: '13px', flex: 1, textAlign: 'center' }}>
                        {h.error ? 'Inference failed' : h.predicted_gas}
                      </span>
                      <span style={{ fontSize: '11px', color: '#cbd5e1', minWidth: '55px', textAlign: 'center', fontWeight: 600 }}>
                        {faultCode}
                      </span>
                      <span style={{ fontSize: '11px', color: '#94a3b8', minWidth: '55px', textAlign: 'right' }}>
                        {h.error ? '—' : `${h.latency_ms} ms`}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        {/* RIGHT PANEL — Results */}
        <div className="cls-right">
          {/* Big Status Panel */}
          <div className="card status-panel" style={{
            display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
            gap: '18px', padding: '28px', minHeight: '0', backgroundColor: '#243F81',
            border: `2px solid ${snnResult ? activeFaultClassStyle.background : 'rgba(35, 63, 124, 0.4)'}`,
            transition: 'border-color 0.3s ease',
          }}>
            {snnResult ? (
              <>
                <div style={{
                  width: '100%', maxWidth: '680px', textAlign: 'center',
                }}>
                  <div style={{ fontSize: '11px', fontWeight: 800, letterSpacing: '1.4px', color: '#94a3b8', marginBottom: '7px' }}>
                    FAULT CLASS
                  </div>
                  <div style={{ fontSize: '36px', fontWeight: 900, lineHeight: 1.15, color: '#ffffff' }}>
                    {currentDiag?.fault_code || snnResult.predicted_fault || '—'}
                  </div>
                  {currentDiag?.fault_type && (
                    <div style={{ fontSize: '18px', fontWeight: 600, color: '#dbeafe', marginTop: '4px' }}>
                      {currentDiag.fault_type}
                    </div>
                  )}
                </div>
                <div style={{
                  width: '100%', maxWidth: '680px', display: 'flex', alignItems: 'center',
                  justifyContent: 'center', gap: '8px', flexWrap: 'wrap', textAlign: 'center',
                  paddingTop: '14px', borderTop: '1px solid rgba(255,255,255,0.12)',
                }}>
                  <span style={{ fontSize: '16px', fontWeight: 700, color: '#ffffff' }}>
                    Dominant Gas:
                  </span>
                  <span style={{ fontSize: '16px', fontWeight: 800, color: '#ffffff' }}>
                    {topGasIdx === 4 ? GAS_LABELS[topGasIdx] : `${GAS_NAMES[topGasIdx]} — ${GAS_LABELS[topGasIdx]}`}
                  </span>
                  <span aria-hidden="true" style={{ color: '#94a3b8' }}>·</span>
                  <span style={{ fontSize: '16px', fontWeight: 600, color: '#ffffff', whiteSpace: 'nowrap' }}>
                    Confidence: {(snnResult.confidence * 100).toFixed(1)}%
                  </span>
                </div>
              </>
            ) : (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '8px' }}>
                  <div className="status-indicator" style={{
                    width: '16px', height: '16px', backgroundColor: '#64748b',
                  }}></div>
                  <h1 className="status-text" style={{ fontSize: '28px', margin: 0, color: '#64748b' }}>
                    STANDBY
                  </h1>
                </div>
                <p className="status-conf" style={{ fontSize: '16px', color: '#cbd5e1', margin: 0, marginTop: '8px' }}>
                  {isRunning ? 'Inference aktif; hasil akan diperbarui setelah setiap cycle selesai.' : 'Inference nonaktif — tekan Start FPGA Inference untuk mengaktifkan inference per cycle.'}
                </p>
              </>
            )}
          </div>

          {/* DGA Class Probabilities */}
          <div className="card" style={{ marginTop: '24px', flex: 1, display: 'flex', flexDirection: 'column' }}>
            <h3 className="card-title">Gas Composition — SNN Output {snnResult?.cycle ? `(Cycle ${snnResult.cycle})` : ''}</h3>
            <div className="prob-list" style={{ marginTop: '20px' }}>
              {GAS_NAMES.map((gasName, idx) => {
                const raw = snnResult ? snnResult.outputs[idx] : 0;
                const val = Math.max(0, Math.min(raw * 100, 100));
                const color = GAS_COLORS[idx];
                const isTop = snnResult ? snnResult.predicted_idx === idx : false;

                return (
                  <div key={gasName} className="prob-item" style={{ marginBottom: '16px' }}>
                    <div className="prob-info" style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
                      <span style={{
                        fontWeight: isTop ? '700' : '500',
                        color: isTop ? color : 'var(--text-primary)',
                        fontSize: '14px',
                        display: 'flex', alignItems: 'center', gap: '6px',
                      }}>
                        {gasName}
                        <span style={{ fontSize: '11px', color: '#94a3b8', fontWeight: 400 }}>
                          ({GAS_LABELS[idx]})
                        </span>
                        {isTop && snnResult && (
                          <span style={{
                            fontSize: '10px', backgroundColor: color + '30', color: color,
                            padding: '2px 8px', borderRadius: '10px', fontWeight: 700,
                          }}>
                            DOMINANT
                          </span>
                        )}
                      </span>
                      <span style={{
                        fontWeight: 'bold',
                        color: val > 5 ? color : 'var(--text-muted)',
                        fontSize: '14px', fontVariantNumeric: 'tabular-nums',
                      }}>
                        {val.toFixed(1)}%
                      </span>
                    </div>
                    <div className="progress-container" style={{
                      backgroundColor: 'rgba(35, 63, 124, 0.3)', height: '12px', borderRadius: '6px',
                      overflow: 'hidden',
                    }}>
                      <div className="progress-bar" style={{
                        width: `${Math.min(val, 100)}%`,
                        backgroundColor: color,
                        transition: 'width 0.4s ease',
                        height: '100%', borderRadius: '6px',
                        boxShadow: isTop ? `0 0 8px ${color}40` : 'none',
                      }}></div>
                    </div>
                  </div>
                );
              })}
            </div>

          </div>

          {/* IEC diagnosis is a separate, high-contrast card below gas composition. */}
          {isRunning && currentDiag && (() => {
            return (
              <div className="card iec-fault-card" data-severity={currentDiag.severity} style={{
                marginTop: '16px', padding: '18px', borderRadius: '12px',
                backgroundColor: activeSeveritySolid.background,
                border: 'none', color: activeSeveritySolid.foreground,
              }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                  <span style={{ fontSize: '13px', fontWeight: 800, letterSpacing: '0.5px', color: activeSeveritySolid.foreground }}>
                    IEC 60599 FAULT CLASSIFICATION
                  </span>
                  <span style={{ fontSize: '20px', fontWeight: 800, color: activeSeveritySolid.foreground }}>
                    {currentDiag.fault_code}
                  </span>
                </div>
                <div style={{ fontSize: '13px', lineHeight: 1.7, color: activeSeveritySolid.foreground }}>
                  <strong>Type:</strong> {currentDiag.fault_type}<br />
                  <strong>Description:</strong> {currentDiag.description}<br />
                  <strong>Severity:</strong> {activeSeverity.label}
                </div>
              </div>
            );
          })()}
        </div>
      </div>
    </div>
  );
};

export default Classification;
