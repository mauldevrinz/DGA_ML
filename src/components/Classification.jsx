import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Play, Square, Activity, Cpu, Upload, Wifi, WifiOff, Zap, ThermometerSun, AlertTriangle, CheckCircle2, Clock, Wind, Flame, Droplets } from 'lucide-react';
import './Classification.css';

// Gas composition labels — matches training code (colab/DGA_ML_GasComposition_Training.py)
const GAS_NAMES   = ['AIR', 'C₂H₂', 'C₂H₄', 'H₂', 'CH₄'];
const GAS_KEYS    = ['AIR', 'C2H2', 'C2H4', 'H2', 'CH4'];   // keys from backend JSON
const GAS_LABELS  = ['Udara Bersih', 'Asetilena', 'Etilena', 'Hidrogen', 'Metana'];
const GAS_COLORS  = ['#10b981', '#ef4444', '#f97316', '#8b5cf6', '#eab308'];
const GAS_ICONS   = [Wind, Zap, Flame, Droplets, Activity];

// IEC severity → color mapping (diagnosis comes from backend now)
const SEVERITY_STYLE = {
  normal:   { bg: '#10b98130', color: '#6ee7b7', label: 'NORMAL' },
  caution:  { bg: '#eab30830', color: '#fde047', label: 'CAUTION' },
  warning:  { bg: '#f9731630', color: '#fdba74', label: 'WARNING' },
  critical: { bg: '#ef444430', color: '#fca5a5', label: 'CRITICAL' },
};

const Classification = () => {
  const [isRunning, setIsRunning] = useState(false);
  const [acquisitionActive, setAcquisitionActive] = useState(() => localStorage.getItem("dga_acquisition_active") === "true");
  const [wsConnected, setWsConnected] = useState(false);
  const wsRef = useRef(null);
  const csvModeRef = useRef(false);
  const intervalRef = useRef(null);

  // Latest sensor data received from Acquisition stream
  const [sensorData, setSensorData] = useState(null);
  const sensorDataRef = useRef(null);

  // SNN inference results
  const [snnResult, setSnnResult] = useState(null);
  const [latency, setLatency] = useState(null);
  const [inferenceCount, setInferenceCount] = useState(0);
  const [history, setHistory] = useState([]);
  const [fpgaStatus, setFpgaStatus] = useState('idle'); // 'idle' | 'running' | 'error'
  const [errorMsg, setErrorMsg] = useState('');
  const [csvProgress, setCsvProgress] = useState(null);
  const [csvDialog, setCsvDialog] = useState(null);

  const showDialog = useCallback((message, title = 'CSV Prediction') => {
    setCsvDialog({ title, message });
  }, []);

  // Connect to WebSocket
  const connectWs = useCallback(() => {
    if (wsRef.current && wsRef.current.readyState === WebSocket.OPEN) return;

    const host = window.location.hostname || '127.0.0.1';
    const ws = new WebSocket(`ws://${host}:8080`);
    wsRef.current = ws;

    ws.onopen = () => {
      setWsConnected(true);
      setErrorMsg('');
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
          if (json.type === 'SNN_RESULT') {
            if (csvModeRef.current) {
              csvModeRef.current = false;
              if (json.ok) {
                setCsvProgress({ progress: 100, message: 'CSV prediction selesai' });
                const gas = Object.entries(json.gas_confidences || {});
                const gasText = gas.map(([name, value]) => `${name}: ${value}%`).join('\n');
                const diagnosis = json.iec_diagnosis || {};
                showDialog(`CSV Prediction

Fault class: ${diagnosis.fault_code || ' '} - ${diagnosis.fault_type || ' '}
Severity: ${diagnosis.severity || ' '}

Gas composition:
${gasText}

Dominant: ${json.predicted_gas || ' '} (${((json.confidence || 0) * 100).toFixed(1)}%)

Feature extraction: ${json.feature_latency_ms ?? ' '} ms
FPGA inference: ${json.latency_ms ?? ' '} ms
Total latency: ${json.total_latency_ms ?? ' '} ms`);
              } else {
                showDialog(`CSV Prediction failed

${json.error || 'Unknown FPGA error'}`);
              }
              return;
            }
            if (json.ok) {
              setSnnResult(json);
              setLatency(json.latency_ms);
              setFpgaStatus('running');
              setInferenceCount(prev => prev + 1);
              setHistory(prev => [{
                time: new Date().toLocaleTimeString(),
                predicted_gas: json.predicted_gas,
                predicted_idx: json.predicted_idx,
                confidence: json.confidence,
                latency_ms: json.latency_ms,
                outputs: json.outputs,
                gas_confidences: json.gas_confidences,
                iec_diagnosis: json.iec_diagnosis,
              }, ...prev].slice(0, 10));
            } else {
              setFpgaStatus('error');
              setErrorMsg(json.error || 'Unknown FPGA error');
            }
          }
        } catch (_) { /* ignore non-JSON */ }
        return;
      }

      // Parse sensor data from Teensy stream (same format as Acquisition)
      if (line.match(/ADC\d+\s*=\s*-?\d+/)) {
        const match = line.match(/ADC(\d+)\s*=\s*(-?\d+)/);
        if (match) {
          const adcIndex = parseInt(match[1]);
          const adcValue = parseInt(match[2]);
          const mV = adcValue * (4.096 / 32768.0) * 1000;
          setSensorData(prev => {
            const next = { ...(prev || {}) };
            next[`mos${adcIndex}`] = mV;
            sensorDataRef.current = next;
            return next;
          });
        }
      }
    };

    ws.onclose = () => {
      setWsConnected(false);
      wsRef.current = null;
    };

    ws.onerror = () => {
      setErrorMsg('WebSocket connection failed. Is serial_ws.py running?');
    };
  }, []);

  // Disconnect
  const disconnectWs = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    setWsConnected(false);
  }, []);

  // Send SNN inference request
  const sendInference = useCallback(() => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    const latestSensorData = sensorDataRef.current;
    if (!latestSensorData) return;

    // Collect the latest 16 sensor values on every interval tick
    const sensors = [];
    for (let i = 0; i < 16; i++) {
      sensors.push(latestSensorData[`mos${i}`] || 0);
    }

    csvModeRef.current = true;
          ws.send(JSON.stringify({ type: 'SNN_INFER', sensors }));
  }, []);

  useEffect(() => {
    const syncAcquisition = () => setAcquisitionActive(localStorage.getItem("dga_acquisition_active") === "true");
    window.addEventListener("storage", syncAcquisition);
    const timer = setInterval(syncAcquisition, 1000);
    return () => { window.removeEventListener("storage", syncAcquisition); clearInterval(timer); };
  }, []);

  // Start/stop live inference loop
  const toggleInference = useCallback(() => {
    if (isRunning) {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      setIsRunning(false);
      setFpgaStatus('idle');
    } else {
      if (!wsConnected) connectWs();
      setIsRunning(true);
      setFpgaStatus('running');
      sendInference();
      intervalRef.current = setInterval(sendInference, 1000);
    }
  }, [isRunning, wsConnected, acquisitionActive, connectWs, sendInference]);

  // Auto-connect on mount
  useEffect(() => {
    connectWs();
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
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
      const rows = dataLines.slice(0, 288).map(line => {
        const cols = line.split(',');
        return Object.fromEntries(header.map((name, i) => [name, parseFloat(cols[i]) || 0]));
      });
      const sendCsv = (attempt = 0) => {
        const csvWs = wsRef.current;
        if (csvWs && csvWs.readyState === WebSocket.OPEN) {
          csvModeRef.current = true;
          csvWs.send(JSON.stringify({ type: 'SNN_CSV_INFER', rows }));
          showDialog(`CSV dikirim untuk inference: ${rows.length}/288 baris.`);
          return;
        }
        if (attempt < 10) {
          if (!csvWs || csvWs.readyState === WebSocket.CLOSED) connectWs();
          setTimeout(() => sendCsv(attempt + 1), 500);
        } else {
          showDialog('Backend WebSocket belum terhubung setelah 5 detik. Periksa port 8080.');
        }
      };
      sendCsv();
      return;

      let rowIdx = 0;
      const processRow = () => {
        if (rowIdx >= dataLines.length) return;
        const cols = dataLines[rowIdx].split(',');
        const sensors = [];
        for (let i = 0; i < 16; i++) {
          const colIdx = header.indexOf(`mos${i}_mV`);
          if (colIdx >= 0 && colIdx < cols.length) {
            sensors.push(parseFloat(cols[colIdx]) || 0);
          } else {
            sensors.push(parseFloat(cols[i + 1]) || 0);
          }
        }
        const ws = wsRef.current;
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'SNN_INFER', sensors }));
        }
        rowIdx++;
        if (rowIdx < dataLines.length) {
          setTimeout(processRow, 500);
        }
      };
      processRow();
      showDialog(`CSV loaded: ${dataLines.length} rows will be predicted sequentially.`);
    };
    reader.readAsText(file);
    e.target.value = '';
  };

  // Current diagnosis from backend IEC 60599 analysis
  const currentDiag = snnResult?.iec_diagnosis || null;
  const topGasIdx = snnResult ? snnResult.predicted_idx : 0;
  const StatusIcon = snnResult ? GAS_ICONS[topGasIdx] : Activity;

  return (
    <div className="page-container">
      {(csvProgress || csvDialog) && (<div role="dialog" aria-modal="true" style={{ position: 'fixed', inset: 0, zIndex: 1100, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 20, background: 'rgba(2, 6, 23, 0.72)' }}>
        <div style={{ width: 'min(560px, 100%)', maxHeight: '80vh', overflowY: 'auto', padding: 22, borderRadius: 14, background: '#172554', color: '#fff' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', paddingBottom: 14, borderBottom: '1px solid rgba(255,255,255,.14)' }}><div><div style={{ color: '#7dd3fc', fontSize: 11, fontWeight: 700, letterSpacing: 1.2, textTransform: 'uppercase' }}>Inference result</div><h3 style={{ margin: '4px 0 0', color: '#fff', fontSize: 20 }}>{csvDialog?.title || 'CSV Prediction'}</h3></div><button type="button" onClick={() => { setCsvDialog(null); setCsvProgress(null); csvModeRef.current = false; }} aria-label="Close" style={{ color: '#fff', background: 'rgba(255,255,255,.12)', border: '1px solid rgba(255,255,255,.22)', borderRadius: 8, width: 32, height: 32, cursor: 'pointer', fontSize: 18 }}>x</button></div>
          {csvProgress && csvProgress.progress < 100 && <div style={{ marginTop: 18, padding: 14, borderRadius: 10, background: 'rgba(15,23,42,.55)' }}><div style={{ color: '#fff', fontSize: 13, marginBottom: 10 }}>{csvProgress.message}</div><div style={{ height: 9, background: '#334155', borderRadius: 99, overflow: 'hidden' }}><div style={{ width: '100%', height: '100%', background: 'linear-gradient(90deg,#38bdf8,#10b981)', borderRadius: 99 }} /></div><div style={{ color: '#cbd5e1', fontSize: 11, marginTop: 7, textAlign: 'right' }}>{csvProgress.progress}%</div></div>}
          {csvDialog?.message && <pre style={{ whiteSpace: 'pre-wrap', lineHeight: 1.6, color: '#fff', fontFamily: 'inherit', fontSize: 13, margin: '18px 0' }}>{csvDialog.message}</pre>}
          {csvDialog && <button type="button" onClick={() => { setCsvDialog(null); setCsvProgress(null); csvModeRef.current = false; }} style={{ color: '#fff', background: '#0ea5e9', border: 0, borderRadius: 8, padding: '10px 22px', fontWeight: 700, cursor: 'pointer', width: '100%' }}>Close</button>}
        </div>
      </div>)}

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
          {isRunning && latency !== null && (
            <div style={{
              display: 'flex', gap: '8px', alignItems: 'center',
              backgroundColor: '#243F81', padding: '6px 14px', borderRadius: '24px',
              border: '1px solid rgba(117, 189, 224, 0.4)',
            }}>
              <Clock size={14} color="#75BDE0" />
              <span style={{ color: '#fff', fontSize: '12px' }}>
                FPGA Latency: <strong style={{ color: '#75BDE0' }}>{latency} ms</strong>
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
              <input type="file" accept=".csv" style={{ display: 'none' }} id="csv-upload" onChange={handleCsvUpload} />
              <button
                className="btn btn-outline"
                style={{ width: '100%', height: '42px', fontSize: '14px', display: 'flex', justifyContent: 'center', alignItems: 'center', gap: '8px' }}
                onClick={() => document.getElementById('csv-upload').click()}
                disabled={!wsConnected}
              >
                <Upload size={16} /> Predict from CSV File
              </button>
            </div>

            {/* Live sensor input summary */}
            {sensorData && isRunning && (
              <div style={{ marginTop: '16px', padding: '12px', backgroundColor: '#243F81', borderRadius: '8px', border: '1px solid rgba(35, 63, 124, 0.4)' }}>
                <span style={{ fontSize: '11px', color: '#94a3b8', fontWeight: 600, letterSpacing: '0.5px', textTransform: 'uppercase' }}>Live Sensor Input</span>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: '4px', marginTop: '8px' }}>
                  {Array.from({ length: 16 }, (_, i) => (
                    <div key={i} style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px' }}>
                      <span style={{ color: '#94a3b8' }}>mos{i}</span>
                      <span style={{ color: '#fff', fontVariantNumeric: 'tabular-nums' }}>
                        {(sensorData[`mos${i}`] || 0).toFixed(1)}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}
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
                  const iecCode = h.iec_diagnosis?.fault_code || '—';
                  return (
                    <div key={i} style={{
                      display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                      padding: '10px 12px', backgroundColor: '#243F81', borderRadius: '8px',
                      border: `1px solid ${i === 0 ? color + '40' : 'rgba(35, 63, 124, 0.4)'}`,
                    }}>
                      <span style={{ fontSize: '11px', color: '#75BDE0', minWidth: '70px' }}>{h.time}</span>
                      <span style={{ fontWeight: 'bold', color, fontSize: '13px', flex: 1, textAlign: 'center' }}>
                        {h.predicted_gas}
                      </span>
                      <span style={{ fontSize: '11px', color: '#cbd5e1', minWidth: '40px', textAlign: 'center' }}>
                        {iecCode}
                      </span>
                      <span style={{ fontSize: '11px', color: '#94a3b8', minWidth: '55px', textAlign: 'right' }}>
                        {h.latency_ms} ms
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
            minHeight: '180px', backgroundColor: '#243F81',
            border: `2px solid ${snnResult ? (GAS_COLORS[topGasIdx] || '#94a3b8') : 'rgba(35, 63, 124, 0.4)'}`,
            transition: 'border-color 0.3s ease',
          }}>
            {snnResult ? (
              <>
                <div style={{ display: 'flex', alignItems: 'center', gap: '16px', marginBottom: '8px' }}>
                  <div className="status-indicator" style={{
                    width: '20px', height: '20px',
                    backgroundColor: GAS_COLORS[topGasIdx],
                    boxShadow: `0 0 20px ${GAS_COLORS[topGasIdx]}`,
                  }}></div>
                  <StatusIcon size={32} color={GAS_COLORS[topGasIdx]} />
                  <h1 className="status-text" style={{
                    fontSize: '32px', margin: 0,
                    color: GAS_COLORS[topGasIdx],
                  }}>
                    {GAS_NAMES[topGasIdx]} — {GAS_LABELS[topGasIdx]}
                  </h1>
                </div>
                <div style={{ display: 'flex', gap: '8px', marginTop: '8px', flexWrap: 'wrap', justifyContent: 'center' }}>
                  <span style={{
                    backgroundColor: 'rgba(255,255,255,0.1)', padding: '4px 14px',
                    borderRadius: '16px', fontSize: '14px', color: '#75BDE0',
                  }}>
                    Dominan: {(snnResult.confidence * 100).toFixed(1)}%
                  </span>
                  {currentDiag && (
                    <>
                      <span style={{
                        backgroundColor: 'rgba(255,255,255,0.1)', padding: '4px 14px',
                        borderRadius: '16px', fontSize: '14px', color: '#fff', fontWeight: 'bold',
                      }}>
                        IEC 60599: {currentDiag.fault_code}
                      </span>
                      <span style={{
                        backgroundColor: (SEVERITY_STYLE[currentDiag.severity] || SEVERITY_STYLE.normal).bg,
                        padding: '4px 14px', borderRadius: '16px', fontSize: '14px',
                        color: (SEVERITY_STYLE[currentDiag.severity] || SEVERITY_STYLE.normal).color,
                        fontWeight: 700,
                      }}>
                        {currentDiag.fault_type}
                      </span>
                    </>
                  )}
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
                  Awaiting FPGA inference — connect sensors and press Start
                </p>
              </>
            )}
          </div>

          {/* DGA Class Probabilities */}
          <div className="card" style={{ marginTop: '24px', flex: 1, display: 'flex', flexDirection: 'column' }}>
            <h3 className="card-title">Gas Composition — SNN Output</h3>
            <div className="prob-list" style={{ marginTop: '20px' }}>
              {GAS_NAMES.map((gasName, idx) => {
                const raw = (isRunning && snnResult) ? snnResult.outputs[idx] : 0;
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
                        {React.createElement(GAS_ICONS[idx], { size: 14, color: isTop ? color : '#94a3b8' })}
                        {gasName}
                        <span style={{ fontSize: '11px', color: '#94a3b8', fontWeight: 400 }}>
                          ({GAS_LABELS[idx]})
                        </span>
                        {isTop && isRunning && (
                          <span style={{
                            fontSize: '10px', backgroundColor: color + '30', color: color,
                            padding: '2px 8px', borderRadius: '10px', fontWeight: 700,
                          }}>
                            DOMINAN
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

            {/* IEC 60599 Fault Classification (text-based, from backend) */}
            {isRunning && currentDiag && (() => {
              const sev = SEVERITY_STYLE[currentDiag.severity] || SEVERITY_STYLE.normal;
              return (
                <div style={{
                  marginTop: '16px', padding: '16px', borderRadius: '12px',
                  backgroundColor: sev.bg,
                  border: `1px solid ${sev.color}40`,
                }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                    <span style={{ fontSize: '13px', fontWeight: 700, color: '#fff', letterSpacing: '0.5px' }}>
                      IEC 60599 FAULT CLASSIFICATION
                    </span>
                    <span style={{
                      fontSize: '20px', fontWeight: 800,
                      color: sev.color,
                    }}>
                      {currentDiag.fault_code}
                    </span>
                  </div>
                  <div style={{ fontSize: '13px', color: '#cbd5e1' }}>
                    <strong>Type:</strong> {currentDiag.fault_type}<br />
                    <strong>Description:</strong> {currentDiag.description}<br />
                    <strong>Severity:</strong>{' '}
                    <span style={{
                      padding: '2px 8px', borderRadius: '10px', fontSize: '11px', fontWeight: 700,
                      backgroundColor: sev.bg, color: sev.color,
                    }}>
                      {sev.label}
                    </span>
                  </div>
                </div>
              );
            })()}
          </div>
        </div>
      </div>
    </div>
  );
};

export default Classification;
