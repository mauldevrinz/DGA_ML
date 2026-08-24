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

const GAS_COLORS = [
  '#233F7C', '#243F81', '#127BBE', '#2B6CB0', '#3182CE', '#4299E1', 
  '#75BDE0', '#90CDF4', '#FDBB16', '#F6EC1D', '#D4A017', '#B8860B', 
  '#233F7C', '#127BBE', '#75BDE0', '#243F81'
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
  const [abortController, setAbortController] = useState(null);
  
  // Session / Recording states
  const [sessionName, setSessionName] = useLocalStorage('dga_sessionName', '');
  const [faultLabel, setFaultLabel] = useLocalStorage('dga_faultLabel', 'Baseline');
  const [isRecording, setIsRecording] = useState(false);
  const [recordingStartTime, setRecordingStartTime] = useState(null);
  const [recordingElapsed, setRecordingElapsed] = useState(0);
  const [sessionData, setSessionData] = useState([]); // data captured during this recording session
  const recordingRef = useRef(false); // ref to avoid stale closure in the data capture effect
  const dataSnapshotIndexRef = useRef(0); // tracks how much of dataHistory we've already captured
  
  // Actuator Modal States
  const [isActuatorModalOpen, setIsActuatorModalOpen] = useState(false);
  const [actuatorMode, setActuatorMode] = useLocalStorage('dga_actuatorMode', 'auto'); // 'auto' | 'manual'
  const [autoPhase, setAutoPhase] = useState('stopped'); // 'stopped' | 'idle' | 'injecting' | 'purging'
  const [manualFreshAir, setManualFreshAir] = useLocalStorage('dga_manualFreshAir', false);
  const [manualGasInlet, setManualGasInlet] = useLocalStorage('dga_manualGasInlet', false);
  const [manualGasOutlet, setManualGasOutlet] = useLocalStorage('dga_manualGasOutlet', false);
  
  useEffect(() => {
    setPorts([
      { name: 'Web Serial API', description: 'Teensy 4.1 USB Serial', is_teensy: true },
    ]);
    setSelectedPort('Web Serial API');
  }, []);

  const connectSerial = async () => {
    try {
      if (!('serial' in navigator)) {
        alert("Web Serial API is not supported in this browser. Please use Chrome or Edge.");
        return;
      }
      
      const port = await navigator.serial.requestPort();
      await port.open({ baudRate: 115200 });
      setSerialPort(port);
      setIsConnected(true);
      
      const ac = new AbortController();
      setAbortController(ac);
      
      // Use pipeTo with AbortController signal so we can cleanly abort
      const textDecoder = new TextDecoderStream();
      const pipeDone = port.readable.pipeTo(textDecoder.writable, { signal: ac.signal }).catch(() => {});
      const reader = textDecoder.readable.getReader();
      
      let buffer = "";
      let currentData = { time: 0, temp0: 0.0, temp1: 0.0, hum0: 0.0, hum1: 0.0 };
      let lastUpdateTime = 0;
      
      try {
        while (true) {
          const { value, done } = await reader.read();
          if (done) break;
          
          buffer += value;
          let lines = buffer.split('\n');
          buffer = lines.pop(); // Keep incomplete line
          
          let updated = false;
          for (let line of lines) {
            // Match pattern like "ADC0 = 1234" or "ADC0 = -1234"
            const match = line.match(/ADC(\d+)\s*=\s*(-?\d+)/);
            if (match) {
              const adcIndex = parseInt(match[1]);
              const adcValue = parseInt(match[2]);
              currentData[`mos${adcIndex}`] = adcValue * (4.096 / 32768.0) * 1000;
              updated = true;
            }

            // Match SHT30 Temperature
            const matchTemp = line.match(/SHT30 Temp\s*=\s*(-?\d+\.\d+)/);
            if (matchTemp) {
              currentData.temp0 = parseFloat(matchTemp[1]);
              updated = true;
            }

            // Match SHT30 Humidity
            const matchHumi = line.match(/SHT30 Humi\s*=\s*(\d+\.\d+)/);
            if (matchHumi) {
              currentData.hum0 = parseFloat(matchHumi[1]);
              updated = true;
            }
          }
          
          // Only push data to chart when we get the last ADC value (mos15)
          if (updated && currentData.mos15 !== undefined) {
             const now = Date.now();
             // Throttle: update UI only every 1 second
             if (now - lastUpdateTime >= 1000) {
               lastUpdateTime = now;
               setDataHistory(prev => {
                  currentData.time = prev.length > 0 ? prev[prev.length - 1].time + 1 : 1;
                  const next = [...prev, { ...currentData }];
                  return next;
               });
             }
             // Reset for next cycle
             currentData = { time: 0, temp0: currentData.temp0, temp1: 0.0, hum0: currentData.hum0, hum1: 0.0 };
          }
        }
      } catch (error) {
        // Expected when abort is called
      } finally {
        reader.releaseLock();
      }
      
      await pipeDone;
    } catch (e) {
      console.error(e);
    }
  };

  const disconnectSerial = async () => {
    try {
      // Abort the pipe first - this cleanly releases the readable stream
      if (abortController) {
        abortController.abort();
        setAbortController(null);
      }
      // Small delay to let the pipe fully release
      await new Promise(r => setTimeout(r, 100));
      if (serialPort) {
        try { await serialPort.close(); } catch (e) { /* already closed */ }
        setSerialPort(null);
      }
    } catch (e) {
      console.error(e);
    } finally {
      setIsConnected(false);
    }
  };

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

  // --- Start / Stop System ---
  const handleStartSystem = useCallback(() => {
    if (!isConnected) {
      alert('Please connect to a serial port first before starting a session.');
      return;
    }
    if (!sessionName.trim()) {
      alert('Please enter a Session Name before starting.');
      return;
    }

    if (!isRecording) {
      // START recording
      setSessionData([]);
      dataSnapshotIndexRef.current = dataHistory.length; // start from current position
      setRecordingStartTime(Date.now());
      setRecordingElapsed(0);
      setIsRecording(true);
      recordingRef.current = true;
    } else {
      // STOP recording
      setIsRecording(false);
      recordingRef.current = false;
      setRecordingStartTime(null);
    }
  }, [isConnected, isRecording, sessionName, dataHistory.length]);

  // --- Save Data as CSV ---
  const handleSaveData = useCallback(() => {
    if (sessionData.length === 0) {
      alert('No data to save. Start a recording session and collect some data first.');
      return;
    }

    // Build CSV
    const sensorHeaders = GAS_SENSORS.map((_, i) => `mos${i}_mV`);
    const headers = ['time', ...sensorHeaders, 'temp_chamber_C', 'temp_oil_C', 'humidity_chamber_pct', 'humidity_oil_pct'];
    const csvRows = [headers.join(',')];

    for (const point of sessionData) {
      const row = [
        point.time ?? '',
        ...GAS_SENSORS.map((_, i) => (point[`mos${i}`] ?? '').toString()),
        (point.temp0 ?? '').toString(),
        (point.temp1 ?? '').toString(),
        (point.hum0 ?? '').toString(),
        (point.hum1 ?? '').toString(),
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

  const latestData = dataHistory.length > 0 ? dataHistory[dataHistory.length - 1] : null;

  return (
    <div className="page-container">
      <div className="acq-header">
        <h2 className="page-title">Data Acquisition</h2>
        <div className="top-controls-row">
          <select 
            className="select-input" 
            style={{ width: '250px' }}
            value={selectedPort}
            onChange={(e) => setSelectedPort(e.target.value)}
            disabled={isConnected}
          >
            {ports.map(p => (
              <option key={p.name} value={p.name}>
                {p.name} {p.is_teensy ? '(Teensy)' : ''}
              </option>
            ))}
          </select>
          
          {!isConnected ? (
            <button className="btn btn-primary" onClick={connectSerial}>
              Connect
            </button>
          ) : (
            <button className="btn btn-danger" onClick={disconnectSerial}>
              Disconnect
            </button>
          )}

          <div className="divider-vert"></div>

          <input 
            type="text" 
            className="select-input" 
            placeholder="Session Name" 
            style={{ width: '200px', borderColor: isRecording ? 'var(--accent-orange)' : undefined }} 
            value={sessionName}
            onChange={(e) => setSessionName(e.target.value)}
            disabled={isRecording}
          />
          <select 
            className="select-input" 
            style={{ width: '150px' }}
            value={faultLabel}
            onChange={(e) => setFaultLabel(e.target.value)}
            disabled={isRecording}
          >
            <option value="Baseline">Baseline</option>
            <option value="Normal">Normal</option>
            <option value="Overheating">Overheating</option>
            <option value="Arcing">Arcing</option>
          </select>

          {!isRecording ? (
            <button className="btn btn-primary" onClick={handleStartSystem} title="Start recording session">
              <Play size={16} /> Start System
            </button>
          ) : (
            <button className="btn btn-danger" onClick={handleStartSystem} title="Stop recording session">
              <Square size={16} /> Stop ({formatElapsed(recordingElapsed)}) — {sessionData.length} pts
            </button>
          )}
          <button 
            className="btn btn-primary" 
            onClick={handleSaveData} 
            disabled={sessionData.length === 0}
            title={sessionData.length === 0 ? 'No data recorded yet' : `Save ${sessionData.length} data points as CSV`}
            style={{ opacity: sessionData.length === 0 ? 0.5 : 1 }}
          >
            <Save size={16} /> Save Data{sessionData.length > 0 ? ` (${sessionData.length})` : ''}
          </button>
          <button className="btn btn-secondary" onClick={() => setIsActuatorModalOpen(true)}>
            <Settings size={16} /> Actuator Control
          </button>
          
          <div className="divider-vert"></div>

          <select 
            className="select-input" 
            style={{ width: '160px', backgroundColor: 'var(--bg-secondary)', color: 'var(--accent-orange)' }}
            value={viewMode}
            onChange={(e) => setViewMode(e.target.value)}
          >
            <option value="grid">Grid View</option>
            <option value="combined">Combined Trend</option>
          </select>
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
              color="var(--accent-orange)"
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
          </div>
        </div>
      )}

      {/* Actuator Modal */}
      {isActuatorModalOpen && (
        <div className="modal-overlay" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'rgba(0,0,0,0.7)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }}>
          <div className="modal-content" style={{ backgroundColor: '#ffffff', color: '#1e293b', padding: '24px', borderRadius: '12px', width: '500px', border: '1px solid #e2e8f0', boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.25)' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '20px' }}>
              <h3 style={{ margin: 0, color: '#0f172a' }}>Actuator Control</h3>
              <button onClick={() => setIsActuatorModalOpen(false)} style={{ background: 'none', border: 'none', color: '#64748b', cursor: 'pointer', fontSize: '20px' }}>✕</button>
            </div>
            
            <div style={{ display: 'flex', gap: '12px', marginBottom: '20px' }}>
              <button 
                className={`btn ${actuatorMode === 'auto' ? 'btn-primary' : 'btn-outline'}`}
                style={{ flex: 1 }}
                onClick={() => setActuatorMode('auto')}
              >
                Automatic Mode
              </button>
              <button 
                className={`btn ${actuatorMode === 'manual' ? 'btn-primary' : 'btn-outline'}`}
                style={{ flex: 1 }}
                onClick={() => setActuatorMode('manual')}
              >
                Manual Mode
              </button>
            </div>

            {actuatorMode === 'auto' && (
              <div style={{ backgroundColor: '#f8fafc', padding: '16px', borderRadius: '8px', border: '1px solid #e2e8f0' }}>
                <h4 style={{ marginTop: 0, marginBottom: '12px', color: '#0f172a' }}>Automatic Sequence</h4>
                <ul style={{ color: '#475569', fontSize: '14px', paddingLeft: '20px', marginBottom: '16px', lineHeight: '1.6' }}>
                  <li><strong>Idle (2 mins):</strong> Fresh Air Pump ON. All Valves OPEN (except Gas Inlet Valve CLOSED).</li>
                  <li><strong>Injecting (5 mins):</strong> Gas Inlet Pump ON. Gas Inlet Valve OPEN. Fresh Air & Outlet Valves CLOSED.</li>
                  <li><strong>Purging (4 mins):</strong> Gas Outlet Pump ON. Gas Outlet Valve OPEN. Others OFF/CLOSED.</li>
                </ul>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div>
                    <span style={{ display: 'block', fontSize: '12px', color: '#64748b' }}>Current Phase:</span>
                    <strong style={{ color: autoPhase === 'stopped' ? '#94a3b8' : '#FDBB16' }}>
                      {autoPhase.toUpperCase()}
                    </strong>
                  </div>
                  <button 
                    className={`btn ${autoPhase === 'stopped' ? 'btn-primary' : 'btn-danger'}`}
                    onClick={() => setAutoPhase(autoPhase === 'stopped' ? 'idle' : 'stopped')}
                  >
                    {autoPhase === 'stopped' ? 'Start Sequence' : 'Stop Sequence'}
                  </button>
                </div>
              </div>
            )}

            {actuatorMode === 'manual' && (
              <div style={{ backgroundColor: '#f8fafc', padding: '16px', borderRadius: '8px', border: '1px solid #e2e8f0', display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ color: '#0f172a', fontWeight: '500' }}>Fresh Air (Pump & Valve)</span>
                  <button 
                    className={`btn ${manualFreshAir ? 'btn-primary' : 'btn-outline'}`}
                    onClick={() => setManualFreshAir(!manualFreshAir)}
                    style={{ minWidth: '80px', color: manualFreshAir ? '#fff' : '#0f172a', borderColor: manualFreshAir ? 'transparent' : '#cbd5e1' }}
                  >
                    {manualFreshAir ? 'ON' : 'OFF'}
                  </button>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ color: '#0f172a', fontWeight: '500' }}>Gas Inlet (Pump & Valve)</span>
                  <button 
                    className={`btn ${manualGasInlet ? 'btn-primary' : 'btn-outline'}`}
                    onClick={() => setManualGasInlet(!manualGasInlet)}
                    style={{ minWidth: '80px', color: manualGasInlet ? '#fff' : '#0f172a', borderColor: manualGasInlet ? 'transparent' : '#cbd5e1' }}
                  >
                    {manualGasInlet ? 'ON' : 'OFF'}
                  </button>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ color: '#0f172a', fontWeight: '500' }}>Gas Outlet (Pump & Valve)</span>
                  <button 
                    className={`btn ${manualGasOutlet ? 'btn-primary' : 'btn-outline'}`}
                    onClick={() => setManualGasOutlet(!manualGasOutlet)}
                    style={{ minWidth: '80px', color: manualGasOutlet ? '#fff' : '#0f172a', borderColor: manualGasOutlet ? 'transparent' : '#cbd5e1' }}
                  >
                    {manualGasOutlet ? 'ON' : 'OFF'}
                  </button>
                </div>
              </div>
            )}
            
            <div style={{ marginTop: '20px', fontSize: '12px', color: '#64748b', textAlign: 'center' }}>
              Commands will be sent directly to the embedded Teensy module.
            </div>
          </div>
        </div>
      )}

    </div>
  );
};

export default Acquisition;
