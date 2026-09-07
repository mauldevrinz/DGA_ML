import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Play, Square, Plug, Save, Settings } from 'lucide-react';
import { useLocalStorage } from '../hooks/useLocalStorage';
import { LineChart, Line, YAxis, XAxis, Tooltip, Legend, ResponsiveContainer, CartesianGrid } from 'recharts';
import './Acquisition.css';

const GAS_SENSORS = [
  "TGS2600", "TGS2611", "TGS2610", "TGS822", "TGS813",
  "MQ-2", "MQ-6", "MQ-8", "MQ-4", "MQ-3",
  "MQ-135", "MQ-9", "MQ-7", "MQ-5",
  "IR12EM ACT", "IR12EM REF"
];

// 16 distinct, high-contrast hues (evenly spaced around the color wheel,
// alternating lightness) so no two sensor lines ever share a color.
const GAS_COLORS = [
  '#C81C1C', '#E27636', '#C89D1C', '#CCE236',
  '#72C81C', '#4BE236', '#1CC847', '#36E2A1',
  '#1CC8C8', '#36A1E2', '#1C47C8', '#4B36E2',
  '#721CC8', '#CC36E2', '#C81C9D', '#E23676'
];

// Reusable small chart component
const MiniChart = React.memo(({ title, dataKey, color, domain, unit, dataHistory, latestData }) => {
  const latestVal = latestData && latestData[dataKey] !== undefined ? latestData[dataKey] : 0;
  const decimals = unit === 'mV' ? 4 : 1;
  const placeholder = unit === 'mV' ? '--.----' : '--.-';
  return (
    <div className="mini-chart-card">
      <div className="chart-header">
        <span className="chart-title">{title}</span>
        <span className="chart-val" style={{ color }}>
          {latestData && latestData[dataKey] !== undefined ? latestVal.toFixed(decimals) : placeholder} {unit}
        </span>
      </div>
      <div className="chart-area" style={{ minWidth: 0, minHeight: 0 }}>
        {latestData ? (
          <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={0}>
            <LineChart data={dataHistory}>
              <CartesianGrid strokeDasharray="2 2" vertical={false} stroke="#e2e8f0" />
              <XAxis dataKey="time" hide={true} />
              <YAxis domain={domain} width={30} tick={{ fontSize: 10 }} />
              <Line
                type="monotone"
                dataKey={dataKey}
                stroke={color}
                strokeWidth={2}
                dot={false}
                isAnimationActive={false}
                connectNulls={true}
              />
            </LineChart>
          </ResponsiveContainer>
        ) : (
          <div className="chart-empty">No Data</div>
        )}
      </div>
    </div>
  );
});

const Acquisition = ({ isConnected, setIsConnected }) => {
  const [ports, setPorts] = useState([]);
  const [selectedPort, setSelectedPort] = useState('');
  const [dataHistory, setDataHistory] = useState([]);
  const [viewMode, setViewMode] = useLocalStorage('dga_viewMode', 'grid');
  
  // Serial Port states
  const [serialPort, setSerialPort] = useState(null);
  const serialPortRef = useRef(null);
  const [abortController, setAbortController] = useState(null);

  // Peltier setpoint control
  const [setpointInput, setSetpointInput] = useState('25.0');
  
  // Session / Recording states
  const [sessionName, setSessionName] = useLocalStorage('dga_sessionName', '');
  const [faultLabel, setFaultLabel] = useLocalStorage('dga_faultLabel', 'Baseline');
  const [isRecording, setIsRecording] = useState(false);
  const [recordingStartTime, setRecordingStartTime] = useState(null);
  const [recordingElapsed, setRecordingElapsed] = useState(0);
  const [sessionData, setSessionData] = useState([]); // data captured during this recording session
  const recordingRef = useRef(false); // ref to avoid stale closure in the data capture effect
  const dataSnapshotIndexRef = useRef(0); // tracks how much of dataHistory we've already captured
  

  // ===== SYSTEM PHASE SETTINGS POPUP =====
  const [showSystemPopup, setShowSystemPopup] = useState(false);
  const [idleDuration, setIdleDuration] = useLocalStorage('dga_idleDuration', 2);   // minutes
  const [injectDuration, setInjectDuration] = useLocalStorage('dga_injectDuration', 5); // minutes
  const [purgeDuration, setPurgeDuration] = useLocalStorage('dga_purgeDuration', 4);  // minutes
  const [pump1Pwm, setPump1Pwm] = useLocalStorage('dga_pump1Pwm', 50); // percentage 0-100

  // ===== SYSTEM RUN STATE =====
  const [systemRunning, setSystemRunning] = useState(false);
  const [currentPhase, setCurrentPhase] = useState(''); // 'IDLE' | 'INJECT' | 'PURGE' | ''
  const [phaseTimeLeft, setPhaseTimeLeft] = useState(0); // seconds remaining in current phase
  const systemTimerRef = useRef(null);
  const phaseQueueRef = useRef([]); // queue of {phase, durationSec}
  const phaseStartTimeRef = useRef(null);
  const phaseDurationRef = useRef(0);

  // ===== SAVE POPUP after system completes =====
  const [showSavePopup, setShowSavePopup] = useState(false);

  useEffect(() => {
    setPorts([
      { name: 'Web Serial API', description: 'Teensy 4.1 USB Serial', is_teensy: true },
    ]);
    setSelectedPort('Web Serial API');
  }, []);

  const connectSerial = () => {
    try {
      const host = window.location.hostname || '127.0.0.1';
      const ws = new WebSocket(`ws://${host}:8080`);
      setSerialPort(ws);
      serialPortRef.current = ws;
      
      ws.onopen = () => {
        setIsConnected(true);
      };
      
      let currentData = {
        time: 0, temp0: 0.0, temp1: 0.0, hum0: 0.0, hum1: 0.0,
        ina1_v: 0.0, ina1_i: 0.0, ina1_p: 0.0,
        ina2_v: 0.0, ina2_i: 0.0, ina2_p: 0.0,
        setpoint_c: undefined, peltier_mode: undefined,
      };
      let lastUpdateTime = 0;
      
      ws.onmessage = (event) => {
        const line = event.data;
        let updated = false;

        const match = line.match(/ADC(\d+)\s*=\s*(-?\d+)/);
        if (match) {
          const adcIndex = parseInt(match[1]);
          const adcValue = parseInt(match[2]);
          currentData[`mos${adcIndex}`] = adcValue * (4.096 / 32768.0) * 1000;
          updated = true;
        }

        const matchTemp = line.match(/SHT30 Temp\s*=\s*(-?\d+\.\d+)/);
        if (matchTemp) {
          currentData.temp0 = parseFloat(matchTemp[1]);
          updated = true;
        }

        const matchHumi = line.match(/SHT30 Humi\s*=\s*(\d+\.\d+)/);
        if (matchHumi) {
          currentData.hum0 = parseFloat(matchHumi[1]);
          updated = true;
        }

        const matchTemp31 = line.match(/SHT31 Temp\s*=\s*(-?\d+\.\d+)/);
        if (matchTemp31) {
          currentData.temp1 = parseFloat(matchTemp31[1]);
          updated = true;
        }

        const matchHumi31 = line.match(/SHT31 Humi\s*=\s*(\d+\.\d+)/);
        if (matchHumi31) {
          currentData.hum1 = parseFloat(matchHumi31[1]);
          updated = true;
        }

        const matchSetpoint = line.match(/^(?:ACK )?SETPOINT\s*=\s*(-?\d+\.\d+)/);
        if (matchSetpoint) {
          currentData.setpoint_c = parseFloat(matchSetpoint[1]);
          updated = true;
        }

        const matchPeltierMode = line.match(/PELTIER_MODE\s*=\s*(\w+)/);
        if (matchPeltierMode) {
          currentData.peltier_mode = matchPeltierMode[1];
          updated = true;
        }

        // Format aktual dari firmware (lihat usb_println! di src/main.rs):
        // "INA226_1 Vbus = 11.702 V, Vshunt = 0.02616 V, I = 5.2315 A, P = 61.2375 W"
        const matchIna = line.match(/INA226_(\d+)\s+Vbus\s*=\s*(-?\d+\.?\d*)\s*V,\s*Vshunt\s*=\s*(-?\d+\.?\d*)\s*V,\s*I\s*=\s*(-?\d+\.?\d*)\s*A,\s*P\s*=\s*(-?\d+\.?\d*)\s*W/);
        if (matchIna) {
          const inaIndex = parseInt(matchIna[1]);
          const vbus = parseFloat(matchIna[2]);
          const currentMa = parseFloat(matchIna[4]) * 1000; // firmware kirim A, GUI pakai mA
          const power = parseFloat(matchIna[5]);
          if (inaIndex === 1) {
            currentData.ina1_v = vbus;
            currentData.ina1_i = currentMa;
            currentData.ina1_p = power;
          } else if (inaIndex === 2) {
            currentData.ina2_v = vbus;
            currentData.ina2_i = currentMa;
            currentData.ina2_p = power;
          }
          updated = true;
        }
        
        if (updated && currentData.mos15 !== undefined) {
           const now = Date.now();
           if (now - lastUpdateTime >= 1000) {
             lastUpdateTime = now;
             setDataHistory(prev => {
                currentData.time = prev.length > 0 ? prev[prev.length - 1].time + 1 : 1;
                return [...prev, { ...currentData }];
             });
           }
           // Carry every field forward (including mos0..mos15) so a sensor
           // that doesn't get a fresh reading this cycle keeps its last
           // known value instead of going undefined and breaking the line.
           currentData = { ...currentData, time: 0 };
        }
      };

      ws.onclose = () => {
        setIsConnected(false);
        setSerialPort(null);
        serialPortRef.current = null;
      };
      
      ws.onerror = (error) => {
        console.error("WebSocket error:", error);
        alert("Failed to connect to backend server on port 8080. Make sure the python script is running.");
      };

    } catch (e) {
      console.error(e);
    }
  };

  const disconnectSerial = () => {
    try {
      if (serialPort && serialPort.readyState === WebSocket.OPEN) {
        serialPort.close();
      }
    } catch (e) {
      console.error(e);
    } finally {
      setIsConnected(false);
      setSerialPort(null);
      serialPortRef.current = null;
    }
  };

  // --- Helper: send a serial command ---
  const sendCommand = useCallback((cmd) => {
    const ws = serialPortRef.current;
    if (ws && ws.readyState === WebSocket.OPEN) {
      try {
        ws.send(`${cmd}\n`);
      } catch (e) {
        console.error('sendCommand error:', e);
      }
    }
  }, []);

  // --- Send temperature setpoint to the Teensy (Peltier bang-bang controller) ---
  const sendSetpoint = useCallback(() => {
    if (!serialPort || serialPort.readyState !== WebSocket.OPEN) {
      alert('Please connect to the backend (Start Stream) first.');
      return;
    }
    const value = parseFloat(setpointInput);
    if (Number.isNaN(value)) {
      alert('Enter a valid setpoint temperature.');
      return;
    }

    try {
      serialPort.send(`SETPOINT=${value.toFixed(2)}\n`);
    } catch (e) {
      console.error(e);
    }
  }, [serialPort, setpointInput]);

  // --- Recording logic: capture new dataHistory entries into sessionData ---
  useEffect(() => {
    if (!recordingRef.current) return;
    // Append any new data points that arrived since last snapshot
    if (dataHistory.length > dataSnapshotIndexRef.current) {
      const newPoints = dataHistory.slice(dataSnapshotIndexRef.current);
      setSessionData(prev => [...prev, ...newPoints]);
      dataSnapshotIndexRef.current = dataHistory.length;
    }
  }, [dataHistory]);

  // --- Elapsed timer ---
  useEffect(() => {
    if (!isRecording || !recordingStartTime) return;
    const interval = setInterval(() => {
      setRecordingElapsed(Math.floor((Date.now() - recordingStartTime) / 1000));
    }, 1000);
    return () => clearInterval(interval);
  }, [isRecording, recordingStartTime]);

  // ===== SYSTEM PHASE EXECUTION ENGINE =====
  const startNextPhase = useCallback(() => {
    const queue = phaseQueueRef.current;
    if (queue.length === 0) {
      // All phases done — send OFF to teensy, stop recording, show save popup
      sendCommand('PHASE=OFF');
      setCurrentPhase('');
      setPhaseTimeLeft(0);
      setSystemRunning(false);
      // Stop recording
      setIsRecording(false);
      recordingRef.current = false;
      setRecordingStartTime(null);
      // Show save popup
      setShowSavePopup(true);
      return;
    }

    const next = queue.shift();
    setCurrentPhase(next.phase);
    phaseDurationRef.current = next.durationSec;
    phaseStartTimeRef.current = Date.now();
    setPhaseTimeLeft(next.durationSec);

    // Send phase command to teensy
    sendCommand(`PHASE=${next.phase}`);
    // Also send PWM for inject phase
    if (next.phase === 'INJECT') {
      sendCommand(`PWM=${pump1Pwm}`);
    }
  }, [sendCommand, pump1Pwm]);

  // Phase countdown timer
  useEffect(() => {
    if (!systemRunning || !currentPhase) return;

    const interval = setInterval(() => {
      const elapsed = (Date.now() - phaseStartTimeRef.current) / 1000;
      const remaining = Math.max(0, phaseDurationRef.current - elapsed);
      setPhaseTimeLeft(Math.ceil(remaining));

      if (remaining <= 0) {
        clearInterval(interval);
        startNextPhase();
      }
    }, 500);

    return () => clearInterval(interval);
  }, [systemRunning, currentPhase, startNextPhase]);

  // --- Handle "Start System" button click → show popup ---
  const handleStartSystemClick = useCallback(() => {
    if (!isConnected) {
      alert('Please connect to a serial port first before starting a session.');
      return;
    }
    if (!sessionName.trim()) {
      alert('Please enter a Session Name before starting.');
      return;
    }

    if (systemRunning) {
      // STOP the running system
      if (systemTimerRef.current) clearInterval(systemTimerRef.current);
      phaseQueueRef.current = [];
      sendCommand('PHASE=OFF');
      setCurrentPhase('');
      setPhaseTimeLeft(0);
      setSystemRunning(false);
      // Stop recording
      setIsRecording(false);
      recordingRef.current = false;
      setRecordingStartTime(null);
      // Show save popup if we have data
      if (recordingRef.current === false) {
        // Use a timeout to allow sessionData state to settle
        setTimeout(() => setShowSavePopup(true), 100);
      }
      return;
    }

    // Show the settings popup
    setShowSystemPopup(true);
  }, [isConnected, sessionName, systemRunning, sendCommand]);

  // --- Actually start the system (called from popup "Start" button) ---
  const handleConfirmStart = useCallback(() => {
    setShowSystemPopup(false);

    // Build phase queue
    const queue = [];
    if (idleDuration > 0) {
      queue.push({ phase: 'IDLE', durationSec: idleDuration * 60 });
    }
    if (injectDuration > 0) {
      queue.push({ phase: 'INJECT', durationSec: injectDuration * 60 });
    }
    if (purgeDuration > 0) {
      queue.push({ phase: 'PURGE', durationSec: purgeDuration * 60 });
    }

    if (queue.length === 0) {
      alert('All phase durations are 0. Please set at least one phase duration.');
      return;
    }

    phaseQueueRef.current = queue;

    // Send PWM setting to Teensy
    sendCommand(`PWM=${pump1Pwm}`);

    // Start recording
    setSessionData([]);
    dataSnapshotIndexRef.current = dataHistory.length;
    setRecordingStartTime(Date.now());
    setRecordingElapsed(0);
    setIsRecording(true);
    recordingRef.current = true;

    // Start system
    setSystemRunning(true);
    startNextPhase();
  }, [idleDuration, injectDuration, purgeDuration, pump1Pwm, sendCommand, dataHistory.length, startNextPhase]);

  // --- Save Data as CSV ---
  const handleSaveData = useCallback(() => {
    if (sessionData.length === 0) {
      alert('No data to save. Start a recording session and collect some data first.');
      return;
    }

    // Build CSV
    const sensorHeaders = GAS_SENSORS.map((_, i) => `mos${i}_mV`);
    const headers = [
      'time', ...sensorHeaders,
      'temp_chamber_C', 'temp_oil_C', 'humidity_chamber_pct', 'humidity_oil_pct',
      'ina226_1_vbus_V', 'ina226_1_current_mA', 'ina226_1_power_W',
      'ina226_2_vbus_V', 'ina226_2_current_mA', 'ina226_2_power_W',
    ];
    const csvRows = [headers.join(',')];

    for (const point of sessionData) {
      const row = [
        point.time ?? '',
        ...GAS_SENSORS.map((_, i) => (point[`mos${i}`] ?? '').toString()),
        (point.temp0 ?? '').toString(),
        (point.temp1 ?? '').toString(),
        (point.hum0 ?? '').toString(),
        (point.hum1 ?? '').toString(),
        (point.ina1_v ?? '').toString(),
        (point.ina1_i ?? '').toString(),
        (point.ina1_p ?? '').toString(),
        (point.ina2_v ?? '').toString(),
        (point.ina2_i ?? '').toString(),
        (point.ina2_p ?? '').toString(),
      ];
      csvRows.push(row.join(','));
    }

    const csvString = csvRows.join('\n');
    const blob = new Blob([csvString], { type: 'text/csv;charset=utf-8;' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
    const safeName = sessionName.trim().replace(/[^a-zA-Z0-9_-]/g, '_') || 'session';
    a.href = url;
    a.download = `DGA_${safeName}_${faultLabel}_${timestamp}.csv`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [sessionData, sessionName, faultLabel]);

  // --- Format elapsed time ---
  const formatElapsed = (secs) => {
    const m = Math.floor(secs / 60).toString().padStart(2, '0');
    const s = (secs % 60).toString().padStart(2, '0');
    return `${m}:${s}`;
  };

  // --- Format phase time left ---
  const formatPhaseTime = (secs) => {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  };

  const latestData = dataHistory.length > 0 ? dataHistory[dataHistory.length - 1] : null;

  // Compute total system duration for display
  const totalSystemDuration = (idleDuration + injectDuration + purgeDuration) * 60;

  return (
    <div className="page-container">
      <div className="acq-header">
        <h2 className="page-title">Data Acquisition</h2>
        <div className="top-controls-row">
          {!isConnected ? (
            <button className="btn btn-primary" onClick={connectSerial}>
              Start Stream
            </button>
          ) : (
            <button className="btn btn-danger" onClick={disconnectSerial}>
              Stop Stream
            </button>
          )}

          <button className="btn btn-secondary" onClick={() => setDataHistory([])}>
            Reset Data
          </button>

          <div className="divider-vert"></div>

          <input 
            type="text" 
            className="select-input" 
            placeholder="Session Name" 
            style={{ width: '200px', borderColor: isRecording ? 'var(--accent-orange)' : undefined }} 
            value={sessionName}
            onChange={(e) => setSessionName(e.target.value)}
            disabled={isRecording || systemRunning}
          />
          <select 
            className="select-input" 
            style={{ width: '150px' }}
            value={faultLabel}
            onChange={(e) => setFaultLabel(e.target.value)}
            disabled={isRecording || systemRunning}
          >
            <option value="Baseline">Baseline</option>
            <option value="Normal">Normal</option>
            <option value="Overheating">Overheating</option>
            <option value="Arcing">Arcing</option>
          </select>

          {!systemRunning ? (
            <button className="btn btn-primary" onClick={handleStartSystemClick} title="Start system with phase settings">
              <Play size={16} /> Start System
            </button>
          ) : (
            <button className="btn btn-danger" onClick={handleStartSystemClick} title="Stop running system">
              <Square size={16} /> Stop System ({formatElapsed(recordingElapsed)})
            </button>
          )}

          {/* Phase indicator during system run */}
          {systemRunning && currentPhase && (
            <span style={{
              padding: '4px 12px',
              borderRadius: '6px',
              fontSize: '13px',
              fontWeight: '700',
              letterSpacing: '0.5px',
              backgroundColor: currentPhase === 'IDLE' ? '#3B82F6' : currentPhase === 'INJECT' ? '#F59E0B' : '#10B981',
              color: '#fff',
              animation: 'pulse 2s infinite',
            }}>
              {currentPhase} — {formatPhaseTime(phaseTimeLeft)}
            </span>
          )}

          <select
            className="select-input"
            style={{ width: '160px', backgroundColor: 'var(--bg-secondary)', color: 'var(--accent-orange)' }}
            value={viewMode}
            onChange={(e) => setViewMode(e.target.value)}
          >
            <option value="grid">Grid View</option>
            <option value="combined">Combined Trend</option>
          </select>

          <div className="divider-vert"></div>

          {/* Peltier Control (Chamber) */}
          <div style={{ display: 'flex', flexDirection: 'column', justifyContent: 'center', gap: '2px', lineHeight: 1.3 }}>
            <span style={{ fontSize: '11px', color: 'var(--text-muted)', whiteSpace: 'nowrap' }}>
              Setpoint: <strong style={{ color: 'var(--accent-blue)' }}>
                {latestData && latestData.setpoint_c !== undefined ? latestData.setpoint_c.toFixed(2) : '--.--'} °C
              </strong>
            </span>
            <span style={{ fontSize: '11px', color: 'var(--text-muted)', whiteSpace: 'nowrap' }}>
              Mode: <strong style={{
                color: latestData && latestData.peltier_mode === 'COOL' ? 'var(--accent-blue)'
                  : latestData && latestData.peltier_mode === 'HEAT' ? 'var(--status-error)'
                  : 'var(--text-muted)'
              }}>
                {latestData && latestData.peltier_mode ? latestData.peltier_mode : '--'}
              </strong>
            </span>
          </div>
          <input
            type="number"
            step="0.1"
            className="select-input"
            style={{ width: '90px' }}
            value={setpointInput}
            onChange={(e) => setSetpointInput(e.target.value)}
            placeholder="°C"
          />
          <button
            className="btn btn-primary"
            onClick={sendSetpoint}
            disabled={!isConnected}
            title={!isConnected ? 'Connect to serial first' : 'Send setpoint to Teensy'}
          >
            Set
          </button>
        </div>
      </div>

      {viewMode === 'grid' ? (
        <div className="charts-grid-4x5">
          {/* Render 16 Gas Sensors */}
          {GAS_SENSORS.map((name, i) => (
            <MiniChart
              key={`mos${i}`}
              title={name}
              dataKey={`mos${i}`}
              color={GAS_COLORS[i % GAS_COLORS.length]}
              domain={['auto', 'auto']} // Dynamic Y-Axis, auto-scale to baseline & max
              unit="mV"
              dataHistory={dataHistory}
              latestData={latestData}
            />
          ))}
          
          {/* Render 2 Temperature Sensors */}
          <MiniChart 
            title="TEMP SENSOR CHAMBER"
            dataKey="temp0"
            color="var(--accent-blue)"
            domain={[0, 100]}
            unit="°C"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart 
            title="TEMP OIL VESSEL"
            dataKey="temp1"
            color="var(--accent-blue)"
            domain={[0, 100]}
            unit="°C"
            dataHistory={dataHistory}
            latestData={latestData}
          />

          {/* Render 2 Humidity Sensors */}
          <MiniChart 
            title="HUM SENSOR CHAMBER"
            dataKey="hum0"
            color="var(--status-normal)"
            domain={[0, 100]}
            unit="%"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="HUM OIL VESSEL"
            dataKey="hum1"
            color="var(--status-normal)"
            domain={[0, 100]}
            unit="%"
            dataHistory={dataHistory}
            latestData={latestData}
          />

          {/* INA226 Current Sensors: #1 mengukur arus & tegangan aktuator,
              #2 mengukur arus & tegangan suplai sensor + mikrokontroller */}
          <MiniChart
            title="INA226 #1 CURRENT (ACTUATOR)"
            dataKey="ina1_i"
            color="var(--accent-orange)"
            domain={['auto', 'auto']}
            unit="mA"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="INA226 #1 VOLTAGE (ACTUATOR)"
            dataKey="ina1_v"
            color="var(--accent-blue)"
            domain={['auto', 'auto']}
            unit="V"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="INA226 #2 CURRENT (SENSOR+MCU)"
            dataKey="ina2_i"
            color="var(--accent-orange)"
            domain={['auto', 'auto']}
            unit="mA"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="INA226 #2 VOLTAGE (SENSOR+MCU)"
            dataKey="ina2_v"
            color="var(--accent-blue)"
            domain={['auto', 'auto']}
            unit="V"
            dataHistory={dataHistory}
            latestData={latestData}
          />
        </div>
      ) : (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '20px', marginTop: '20px' }}>
          {/* Large Combined Chart */}
          <div className="mini-chart-card" style={{ flex: 1, minHeight: '450px', display: 'flex', flexDirection: 'column' }}>
            <div className="chart-header" style={{ marginBottom: '10px' }}>
              <span className="chart-title" style={{ fontSize: '18px' }}>Combined Gas Sensors Trend (mV)</span>
            </div>
            <div className="chart-area" style={{ flex: 1, minWidth: 0, minHeight: 0 }}>
              {latestData ? (
                <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={0}>
                  <LineChart data={dataHistory} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
                    <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgba(35, 63, 124, 0.3)" />
                    <XAxis dataKey="time" stroke="#75BDE0" />
                    <YAxis domain={[0, 5000]} stroke="#75BDE0" />
                    <Tooltip 
                      contentStyle={{ backgroundColor: '#243F81', border: 'none', borderRadius: '8px', color: '#ffffff' }}
                      itemStyle={{ color: '#ffffff' }}
                    />
                    <Legend wrapperStyle={{ paddingTop: '20px' }} />
                    {GAS_SENSORS.map((name, i) => (
                      <Line
                        key={name}
                        type="monotone"
                        dataKey={`mos${i}`}
                        name={name}
                        stroke={GAS_COLORS[i % GAS_COLORS.length]}
                        strokeWidth={2}
                        dot={false}
                        isAnimationActive={false}
                        connectNulls={true}
                      />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              ) : (
                <div className="chart-empty" style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  No Data Connected
                </div>
              )}
            </div>
          </div>

          {/* Text-only Temp and Hum Info */}
          <div style={{ display: 'flex', gap: '20px' }}>
            <div className="mini-chart-card" style={{ flex: 1 }}>
              <div className="chart-header">
                <span className="chart-title">Chamber Conditions</span>
              </div>
              <div style={{ marginTop: '10px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>TEMP SENSOR CHAMBER:</span>
                  <span style={{ color: 'var(--accent-blue)', fontWeight: 'bold' }}>
                    {latestData && latestData.temp0 !== undefined ? latestData.temp0.toFixed(2) : '--.--'} °C
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>HUM SENSOR CHAMBER:</span>
                  <span style={{ color: 'var(--status-normal)', fontWeight: 'bold' }}>
                    {latestData && latestData.hum0 !== undefined ? latestData.hum0.toFixed(2) : '--.--'} %
                  </span>
                </div>
              </div>
            </div>

            <div className="mini-chart-card" style={{ flex: 1 }}>
              <div className="chart-header">
                <span className="chart-title">Oil Vessel Conditions</span>
              </div>
              <div style={{ marginTop: '10px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>TEMP OIL VESSEL:</span>
                  <span style={{ color: 'var(--accent-blue)', fontWeight: 'bold' }}>
                    {latestData && latestData.temp1 !== undefined ? latestData.temp1.toFixed(2) : '--.--'} °C
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>HUM OIL VESSEL:</span>
                  <span style={{ color: 'var(--status-normal)', fontWeight: 'bold' }}>
                    {latestData && latestData.hum1 !== undefined ? latestData.hum1.toFixed(2) : '--.--'} %
                  </span>
                </div>
              </div>
            </div>

            <div className="mini-chart-card" style={{ flex: 1 }}>
              <div className="chart-header">
                <span className="chart-title">Current Sensors (INA226)</span>
              </div>
              <div style={{ marginTop: '10px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>INA226 #1 - Actuator (I / V):</span>
                  <span style={{ color: 'var(--accent-orange)', fontWeight: 'bold' }}>
                    {latestData && latestData.ina1_i !== undefined ? latestData.ina1_i.toFixed(2) : '--.--'} mA
                    {' / '}
                    {latestData && latestData.ina1_v !== undefined ? latestData.ina1_v.toFixed(3) : '--.---'} V
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>INA226 #2 - Sensor + MCU (I / V):</span>
                  <span style={{ color: 'var(--accent-orange)', fontWeight: 'bold' }}>
                    {latestData && latestData.ina2_i !== undefined ? latestData.ina2_i.toFixed(2) : '--.--'} mA
                    {' / '}
                    {latestData && latestData.ina2_v !== undefined ? latestData.ina2_v.toFixed(3) : '--.---'} V
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ===== SYSTEM SETTINGS POPUP ===== */}
      {showSystemPopup && (
        <div className="modal-overlay" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'rgba(0,0,0,0.7)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }}>
          <div className="modal-content" style={{ backgroundColor: '#ffffff', color: '#1e293b', padding: '28px', borderRadius: '12px', width: '560px', border: '1px solid #e2e8f0', boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.25)' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '24px' }}>
              <h3 style={{ margin: 0, color: '#0f172a', fontSize: '20px' }}>⚙️ System Phase Settings</h3>
              <button onClick={() => setShowSystemPopup(false)} style={{ background: 'none', border: 'none', color: '#64748b', cursor: 'pointer', fontSize: '20px' }}>✕</button>
            </div>

            {/* Phase Durations */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
              
              {/* IDLE Phase */}
              <div style={{ backgroundColor: '#ffffff', padding: '16px', borderRadius: '8px', border: '1px solid #BFDBFE' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                  <div>
                    <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '15px' }}>IDLE Phase</span>
                  </div>
                  <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '18px', minWidth: '60px', textAlign: 'right' }}>{idleDuration} min</span>
                </div>
                <input
                  type="range"
                  min="0" max="5" step="0.5"
                  value={idleDuration}
                  onChange={(e) => setIdleDuration(parseFloat(e.target.value))}
                  style={{ width: '100%', accentColor: '#3B82F6' }}
                />
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', color: '#9CA3AF' }}>
                  <span>0 min</span><span>5 min</span>
                </div>
              </div>

              {/* INJECT Phase */}
              <div style={{ backgroundColor: '#ffffff', padding: '16px', borderRadius: '8px', border: '1px solid #BFDBFE' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                  <div>
                    <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '15px' }}>INJECTING Phase</span>
                  </div>
                  <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '18px', minWidth: '60px', textAlign: 'right' }}>{injectDuration} min</span>
                </div>
                <input
                  type="range"
                  min="0" max="5" step="0.5"
                  value={injectDuration}
                  onChange={(e) => setInjectDuration(parseFloat(e.target.value))}
                  style={{ width: '100%', accentColor: '#3B82F6' }}
                />
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', color: '#9CA3AF' }}>
                  <span>0 min</span><span>5 min</span>
                </div>
              </div>

              {/* PURGE Phase */}
              <div style={{ backgroundColor: '#ffffff', padding: '16px', borderRadius: '8px', border: '1px solid #BFDBFE' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                  <div>
                    <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '15px' }}>PURGING Phase</span>
                  </div>
                  <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '18px', minWidth: '60px', textAlign: 'right' }}>{purgeDuration} min</span>
                </div>
                <input
                  type="range"
                  min="0" max="5" step="0.5"
                  value={purgeDuration}
                  onChange={(e) => setPurgeDuration(parseFloat(e.target.value))}
                  style={{ width: '100%', accentColor: '#3B82F6' }}
                />
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', color: '#9CA3AF' }}>
                  <span>0 min</span><span>5 min</span>
                </div>
              </div>

              {/* PUMP1 PWM Slider */}
              <div style={{ backgroundColor: '#ffffff', padding: '16px', borderRadius: '8px', border: '1px solid #BFDBFE' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                  <div>
                    <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '15px' }}>Pump 1 PWM (Inject Phase)</span>
                    <div style={{ fontSize: '12px', color: '#6B7280', marginTop: '2px' }}>BTS7960 pump speed during the Injecting phase</div>
                  </div>
                  <span style={{ fontWeight: '700', color: '#1E40AF', fontSize: '18px', minWidth: '60px', textAlign: 'right' }}>{pump1Pwm}%</span>
                </div>
                <input
                  type="range"
                  min="0" max="100" step="5"
                  value={pump1Pwm}
                  onChange={(e) => setPump1Pwm(parseInt(e.target.value))}
                  style={{ width: '100%', accentColor: '#3B82F6' }}
                />
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', color: '#9CA3AF' }}>
                  <span>0%</span><span>50%</span><span>100%</span>
                </div>
              </div>
            </div>

            {/* Summary + Buttons */}
            <div style={{ marginTop: '20px', padding: '12px 16px', backgroundColor: '#EFF6FF', borderRadius: '8px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ fontSize: '13px', color: '#475569' }}>
                Total Duration: <strong>{(idleDuration + injectDuration + purgeDuration).toFixed(1)} min</strong>
                {' '}({formatElapsed(Math.round((idleDuration + injectDuration + purgeDuration) * 60))})
              </div>
              <div style={{ display: 'flex', gap: '10px' }}>
                <button 
                  className="btn btn-outline"
                  onClick={() => setShowSystemPopup(false)}
                  style={{ color: '#64748b', borderColor: '#CBD5E1' }}
                >
                  Cancel
                </button>
                <button 
                  className="btn btn-primary"
                  onClick={handleConfirmStart}
                  style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
                >
                  <Play size={16} /> Start System
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ===== SAVE DATA POPUP (after system completes) ===== */}
      {showSavePopup && (
        <div className="modal-overlay" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'rgba(0,0,0,0.7)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }}>
          <div className="modal-content" style={{ backgroundColor: '#ffffff', color: '#1e293b', padding: '28px', borderRadius: '12px', width: '460px', border: '1px solid #e2e8f0', boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.25)', textAlign: 'center' }}>
            <div style={{ fontSize: '48px', marginBottom: '12px' }}>✅</div>
            <h3 style={{ margin: '0 0 8px 0', color: '#0f172a', fontSize: '20px' }}>System Phases Completed!</h3>
            <p style={{ color: '#475569', marginBottom: '6px' }}>
              All phases (Idle → Injecting → Purging) have finished.
            </p>
            <p style={{ color: '#64748b', fontSize: '14px', marginBottom: '20px' }}>
              <strong>{sessionData.length}</strong> data points recorded during session "<strong>{sessionName}</strong>".
            </p>

            <div style={{ display: 'flex', gap: '12px', justifyContent: 'center' }}>
              <button
                className="btn btn-outline"
                onClick={() => setShowSavePopup(false)}
                style={{ color: '#64748b', borderColor: '#CBD5E1' }}
              >
                Close
              </button>
              <button
                className="btn btn-primary"
                onClick={() => {
                  handleSaveData();
                  setShowSavePopup(false);
                }}
                style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
              >
                <Save size={16} /> Save as CSV
              </button>
            </div>
          </div>
        </div>
      )}



    </div>
  );
};

export default Acquisition;
