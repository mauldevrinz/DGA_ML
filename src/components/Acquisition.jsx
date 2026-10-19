import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Play, Square, Save } from 'lucide-react';
import PropTypes from 'prop-types';
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

const crc32 = (bytes) => {
  let crc = 0xFFFFFFFF;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ ((crc & 1) ? 0xEDB88320 : 0);
  }
  return (crc ^ 0xFFFFFFFF) >>> 0;
};

const makeZipBlob = (files) => {
  const encoder = new TextEncoder();
  const localParts = [];
  const centralParts = [];
  let localOffset = 0;
  files.forEach(({ name, content }) => {
    const filename = encoder.encode(name);
    const data = typeof content === 'string' ? encoder.encode(content) : content;
    const checksum = crc32(data);
    const local = new Uint8Array(30 + filename.length);
    const localView = new DataView(local.buffer);
    localView.setUint32(0, 0x04034B50, true);
    localView.setUint16(4, 20, true);
    localView.setUint32(14, checksum, true);
    localView.setUint32(18, data.length, true);
    localView.setUint32(22, data.length, true);
    localView.setUint16(26, filename.length, true);
    local.set(filename, 30);
    localParts.push(local, data);

    const central = new Uint8Array(46 + filename.length);
    const view = new DataView(central.buffer);
    view.setUint32(0, 0x02014B50, true);
    view.setUint16(4, 20, true);
    view.setUint16(6, 20, true);
    view.setUint32(16, checksum, true);
    view.setUint32(20, data.length, true);
    view.setUint32(24, data.length, true);
    view.setUint16(28, filename.length, true);
    view.setUint32(42, localOffset, true);
    central.set(filename, 46);
    centralParts.push(central);
    localOffset += local.length + data.length;
  });
  const centralSize = centralParts.reduce((sum, part) => sum + part.length, 0);
  const end = new Uint8Array(22);
  const endView = new DataView(end.buffer);
  endView.setUint32(0, 0x06054B50, true);
  endView.setUint16(8, files.length, true);
  endView.setUint16(10, files.length, true);
  endView.setUint32(12, centralSize, true);
  endView.setUint32(16, localOffset, true);
  return new Blob([...localParts, ...centralParts, end], { type: 'application/zip' });
};

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
      <div className="chart-area" style={{ minWidth: 0, minHeight: '80px' }}>
        {latestData ? (
          <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={80}>
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
MiniChart.displayName = 'MiniChart';
MiniChart.propTypes = {
  title: PropTypes.string.isRequired,
  dataKey: PropTypes.string.isRequired,
  color: PropTypes.string.isRequired,
  domain: PropTypes.array.isRequired,
  unit: PropTypes.string.isRequired,
  dataHistory: PropTypes.array.isRequired,
  latestData: PropTypes.object,
};

const Acquisition = ({ isConnected, setIsConnected }) => {
  const [dataHistory, setDataHistory] = useState([]);
  const [frameDiagnostics, setFrameDiagnostics] = useState({ missing: 0, malformed: 0, nonNumeric: 0, malformedLengths: {}, duplicates: 0, resets: 0 });
  const frameDiagnosticsRef = useRef({ lastSequence: null, missing: 0, malformed: 0, nonNumeric: 0, malformedLengths: {}, duplicates: 0, resets: 0 });
  const [safetyWarning, setSafetyWarning] = useState(null);
  const [viewMode, setViewMode] = useLocalStorage('dga_viewMode', 'grid');
  const [combinedChartMode, setCombinedChartMode] = useLocalStorage('dga_combinedChartMode', 'gas');
  
  // Serial Port states
  const [serialPort, setSerialPort] = useState(null);
  const serialPortRef = useRef(null);

  // Pending SAVE_GRAPH response callback (resolve/reject from existing WS connection)
  // Avoids opening a new WS that would immediately receive Teensy broadcast before SAVE_GRAPH response
  const saveGraphPendingRef = useRef(null);

  // Peltier setpoint control
  const [setpointInput, setSetpointInput] = useState('25.0');
  const [showChamberPopup, setShowChamberPopup] = useState(false);
  const [chamberKp, setChamberKp] = useState('5.0');
  const [chamberKi, setChamberKi] = useState('0.5');
  const [chamberKd, setChamberKd] = useState('1.0');
  
  // Session / Recording states
  const [sessionName, setSessionName] = useLocalStorage('dga_sessionName', '');
  const [faultLabel, setFaultLabel] = useLocalStorage('dga_faultLabel', 'Baseline');
  const [isRecording, setIsRecording] = useState(false);
  const [recordingStartTime, setRecordingStartTime] = useState(null);
  const [recordingElapsed, setRecordingElapsed] = useState(0);
  const [sessionData, setSessionData] = useState([]); // data captured during this recording session
  const sessionDataRef = useRef([]);
  const cycleDataRef = useRef([]);
  const currentCycleIndexRef = useRef(0);
  const [completedCycleCount, setCompletedCycleCount] = useState(0);
  const latestDataRef = useRef(null);
  const [baselines, setBaselines] = useState({}); // Stores R0 for normalization
  const [saveGraphStatus, setSaveGraphStatus] = useState(''); // Status message for save graph
  const recordingRef = useRef(false); // ref to avoid stale closure in the data capture effect

  const appendSessionSample = useCallback((sample) => {
    const point = { ...sample, time: sessionDataRef.current.length + 1 };
    sessionDataRef.current.push(point);
    setSessionData(prev => {
      const next = [...prev, point];
      return next.length > 600 ? next.slice(-600) : next;
    });
    const frameCycleIndex = Number(sample.cycle_number) - 1;
    let cycleRows = null;
    if (frameCycleIndex >= 0 && frameCycleIndex < cycleDataRef.current.length) {
      cycleRows = cycleDataRef.current[frameCycleIndex];
    } else if (currentCycleIndexRef.current >= 0 && currentCycleIndexRef.current < cycleDataRef.current.length) {
      cycleRows = cycleDataRef.current[currentCycleIndexRef.current];
    } else if (cycleDataRef.current.length > 0) {
      cycleRows = cycleDataRef.current[0];
    }
    if (cycleRows) cycleRows.push({ ...sample, time: cycleRows.length + 1 });
  }, []);
  

  // ===== SYSTEM PHASE SETTINGS POPUP =====
  const [showSystemPopup, setShowSystemPopup] = useState(false);
  const [idleDuration, setIdleDuration] = useLocalStorage('dga_idleDuration', 2);   // minutes
  const [injectDuration, setInjectDuration] = useLocalStorage('dga_injectDuration', 5); // minutes
  const [purgeDuration, setPurgeDuration] = useLocalStorage('dga_purgeDuration', 4);  // minutes
  const [pump1Pwm, setPump1Pwm] = useLocalStorage('dga_pump1Pwm', 50); // percentage 0-100
  const [cycleCount, setCycleCount] = useLocalStorage('dga_cycleCount', 1);

  // ===== SYSTEM RUN STATE =====
  const [systemRunning, setSystemRunning] = useState(false);
  const systemRunningRef = useRef(false);
  const [currentPhase, setCurrentPhase] = useState(''); // 'IDLE' | 'INJECT' | 'PURGE' | ''
  const [phaseReady, setPhaseReady] = useState(false);
  const [phaseTimeLeft, setPhaseTimeLeft] = useState(0); // seconds remaining in current phase
  const systemTimerRef = useRef(null);
  const phaseStartTimeRef = useRef(null);
  const phaseDurationRef = useRef(0);

  // ===== SAVE POPUP after system completes =====
  const [showSavePopup, setShowSavePopup] = useState(false);

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
        kria_v: 0.0, kria_i: 0.0, kria_p: 0.0,
        setpoint_c: undefined, peltier_mode: undefined,
        ndir_ratio: 0.0, ndir_abs: 0.0, ndir_resp: 0.0, ndir_baseline: 0.0,
        flow_inlet: 0.0,
      };
      let lastUpdateTime = 0;
      let sawSampleFrames = false;
      
      ws.onmessage = (event) => {
        const line = event.data;

        if (typeof line === 'string' && line.startsWith('SAMPLE,')) {
          const fields = line.trim().split(',');
          // New frames include cycle + sequence. Older firmware sends only sequence.
          // Both carry 16 ADC + 4 NDIR + 4 climate + flow + 6 INA + setpoint.
          // Firmware currently emits cycle + sequence + 32 values. Accept
          // trailing fields so an added firmware diagnostic won't drop samples.
          if (fields.length >= 34) {
            const hasCycleNumber = fields.length === 35 || (
              fields.length > 35 && Number(fields[1]) >= 0 && Number(fields[1]) <= 100 &&
              Number(fields[2]) > Number(fields[1])
            );
            const requiredValueCount = hasCycleNumber ? 34 : 33;
            const values = fields.slice(1, 1 + requiredValueCount).map(Number);
            if (values.length === requiredValueCount && values.every(Number.isFinite)) {
              // Keep the legacy ADC parser active until one complete SAMPLE
              // frame is actually accepted, so truncated frames cannot blank the UI.
              const cycleNumber = hasCycleNumber
                ? values[0]
                : Math.max(0, currentCycleIndexRef.current + 1);
              const sequence = hasCycleNumber ? values[1] : values[0];
              const v = values.slice(hasCycleNumber ? 2 : 1);
              sawSampleFrames = true;
              const diag = frameDiagnosticsRef.current;
              const seq = Math.trunc(sequence);
              if (diag.lastSequence !== null) {
                if (seq === diag.lastSequence) {
                  diag.duplicates += 1;
                  setFrameDiagnostics({ ...diag });
                  return;
                }
                if (seq > diag.lastSequence + 1) diag.missing += seq - diag.lastSequence - 1;
                if (seq < diag.lastSequence) diag.resets += 1;
              }
              diag.lastSequence = seq;
              setFrameDiagnostics({ ...diag });
              const sample = {
                // KRIA telemetry arrives as a separate backend message. Carry
                // its latest values forward when the next Teensy SAMPLE frame
                // becomes the new chart/session point.
                ...(latestDataRef.current || {}),
                cycle_number: cycleNumber,
                sample_sequence: sequence,
                time: sequence,
                ...Object.fromEntries(v.slice(0, 16).map((value, i) => [`mos${i}`, value * (4.096 / 32768.0) * 1000])),
                ndir_ratio: v[16], ndir_baseline: v[17], ndir_resp: v[18], ndir_abs: v[19],
                temp0: v[20], temp1: v[21], hum0: v[22], hum1: v[23], flow_inlet: v[24],
                ina1_v: v[25], ina1_i: v[26], ina1_p: v[27],
                ina2_v: v[28], ina2_i: v[29], ina2_p: v[30], setpoint_c: v[31],
                peltier_mode: fields.length > 35 ? fields[35] : undefined,
                ndir_valid: fields.length > 36 ? Number(fields[36]) === 1 : undefined,
                ndir_baseline_ready: fields.length > 37 ? Number(fields[37]) === 1 : undefined,
              };
              latestDataRef.current = sample;
              setDataHistory(prev => {
                const next = [...prev, sample];
                return next.length > 600 ? next.slice(-600) : next;
              });
              if (recordingRef.current) appendSessionSample(sample);
            } else {
              const diagnostics = frameDiagnosticsRef.current;
              diagnostics.malformed += 1;
              diagnostics.nonNumeric += 1;
              setFrameDiagnostics({ ...frameDiagnosticsRef.current });
              console.warn('Ignoring invalid Teensy SAMPLE frame: non-numeric field');
            }
          } else {
            const diagnostics = frameDiagnosticsRef.current;
            diagnostics.malformed += 1;
            diagnostics.malformedLengths[fields.length] = (diagnostics.malformedLengths[fields.length] || 0) + 1;
            setFrameDiagnostics({ ...diagnostics, malformedLengths: { ...diagnostics.malformedLengths } });
            console.warn(`Ignoring invalid Teensy SAMPLE frame: expected at least 34 fields, received ${fields.length}`);
          }
          // A bad SAMPLE line should not disable the older ADC-line fallback.
          return;
        }

        if (typeof line === 'string') {
          if (line.includes('SYSTEM_WATCHDOG=TRIPPED')) {
            systemRunningRef.current = false;
            setSystemRunning(false);
            setPhaseReady(false);
            setCurrentPhase('');
            setPhaseTimeLeft(0);
            setIsRecording(false);
            recordingRef.current = false;
            setRecordingStartTime(null);
            setCompletedCycleCount(cycleDataRef.current.filter(rows => rows.length > 0).length);
            localStorage.setItem('dga_acquisition_active', 'false');
            setSafetyWarning('The Teensy safety watchdog expired. The pump and valves have been turned off; check the Kria-to-Teensy connection before restarting.');
            setShowSavePopup(true);
          }
          const ack = line.match(/^ACK PHASE = /);
          if (ack) return;
        }

        if (typeof line === 'string' && line.trimStart().startsWith('{')) {
          try {
            const json = JSON.parse(line);
            if (json.type === 'SYSTEM_PHASE') {
              currentCycleIndexRef.current = Math.max(0, Number(json.cycle) - 1);
              phaseStartTimeRef.current = Date.now();
              phaseDurationRef.current = Number(json.duration_sec) || 0;
              setCurrentPhase(json.phase || '');
              setPhaseTimeLeft(phaseDurationRef.current);
              setPhaseReady(true);
              return;
            }
            if (json.type === 'SYSTEM_COMPLETE' || json.type === 'SYSTEM_STOPPED') {
              systemRunningRef.current = false;
              setSystemRunning(false);
              setPhaseReady(false);
              setCurrentPhase('');
              setPhaseTimeLeft(0);
              setIsRecording(false);
              recordingRef.current = false;
              setRecordingStartTime(null);
              setCompletedCycleCount(cycleDataRef.current.filter(rows => rows.length > 0).length);
              localStorage.setItem('dga_acquisition_active', 'false');
              setShowSavePopup(true);
              return;
            }
            if (json.type === 'SYSTEM_ERROR') {
              systemRunningRef.current = false;
              setSystemRunning(false);
              setPhaseReady(false);
              setCurrentPhase('');
              setPhaseTimeLeft(0);
              setIsRecording(false);
              recordingRef.current = false;
              setRecordingStartTime(null);
              setCompletedCycleCount(cycleDataRef.current.filter(rows => rows.length > 0).length);
              localStorage.setItem('dga_acquisition_active', 'false');
              setSafetyWarning(json.message || 'The backend could not continue the system plan.');
              setShowSavePopup(true);
              return;
            }
            if (saveGraphPendingRef.current && json.ok !== undefined) {
              saveGraphPendingRef.current.resolve(json);
              saveGraphPendingRef.current = null;
            }
          } catch (_) {
          }
          return;
        }

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

        const matchFlow = line.match(/FLOW_INLET\s*=\s*(-?\d+\.?\d*)/);
        if (matchFlow) {
          currentData.flow_inlet = Math.abs(parseFloat(matchFlow[1]));
          updated = true;
        }

        const matchPeltierMode = line.match(/PELTIER_MODE\s*=\s*(\w+)/);
        if (matchPeltierMode) {
          currentData.peltier_mode = matchPeltierMode[1];
          updated = true;
        }

        const matchNdirRatio = line.match(/NDIR_RATIO\s*=\s*(-?\d+\.\d+)/);
        if (matchNdirRatio) {
          currentData.ndir_ratio = parseFloat(matchNdirRatio[1]);
          updated = true;
        }

        const matchNdirBaseline = line.match(/NDIR_BASELINE\s*=\s*(-?\d+\.\d+)/);
        if (matchNdirBaseline) {
          currentData.ndir_baseline = parseFloat(matchNdirBaseline[1]);
          updated = true;
        }

        const matchNdirResp = line.match(/NDIR_RESPONSE\s*=\s*(-?\d+\.\d+)/);
        if (matchNdirResp) {
          currentData.ndir_resp = parseFloat(matchNdirResp[1]);
          updated = true;
        }

        const matchNdirAbs = line.match(/NDIR_ABSORBANCE\s*=\s*(-?\d+\.\d+)/);
        if (matchNdirAbs) {
          currentData.ndir_abs = parseFloat(matchNdirAbs[1]);
          updated = true;
        }

        const matchIna = line.match(/INA226_(\d+)\s+Vbus\s*=\s*(-?\d+\.?\d*)\s*V,\s*Vshunt\s*=\s*(-?\d+\.?\d*)\s*V,\s*I\s*=\s*(-?\d+\.?\d*)\s*A,\s*P\s*=\s*(-?\d+\.?\d*)\s*W/);
        if (matchIna) {
          const inaIndex = parseInt(matchIna[1]);
          const vbus = parseFloat(matchIna[2]);
          const currentMa = parseFloat(matchIna[4]) * 1000;
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
        
        const matchKria = line.match(/KRIA Vbus=(-?[0-9.]+)V I=(-?[0-9.]+)A P=(-?[0-9.]+)W/);
        if (matchKria) {
          const kriaUpdate = {
            kria_v: parseFloat(matchKria[1]),
            kria_i: parseFloat(matchKria[2]) * 1000,
            kria_p: parseFloat(matchKria[3]),
          };
          Object.assign(currentData, kriaUpdate);
          latestDataRef.current = { ...(latestDataRef.current || {}), ...kriaUpdate };
          setDataHistory(prev => {
            if (prev.length === 0) return prev;
            const next = [...prev];
            next[next.length - 1] = { ...next[next.length - 1], ...kriaUpdate };
            return next;
          });
          if (recordingRef.current) {
            const lastSessionIndex = sessionDataRef.current.length - 1;
            if (lastSessionIndex >= 0) {
              sessionDataRef.current[lastSessionIndex] = { ...sessionDataRef.current[lastSessionIndex], ...kriaUpdate };
              setSessionData(prev => {
                const next = [...prev];
                const visibleIndex = next.findIndex(row => row.time === sessionDataRef.current[lastSessionIndex].time);
                if (visibleIndex >= 0) next[visibleIndex] = { ...next[visibleIndex], ...kriaUpdate };
                return next;
              });
            }
            const cycleRows = cycleDataRef.current[currentCycleIndexRef.current];
            if (cycleRows?.length) {
              cycleRows[cycleRows.length - 1] = { ...cycleRows[cycleRows.length - 1], ...kriaUpdate };
            }
          }
          updated = true;
        }
        
        if (!sawSampleFrames && updated && currentData.mos15 !== undefined) {
           const now = Date.now();
           if (now - lastUpdateTime >= 1000) {
             lastUpdateTime = now;
             setDataHistory(prev => {
                currentData.time = prev.length > 0 ? prev[prev.length - 1].time + 1 : 1;
                const snapshot = { ...currentData };
                latestDataRef.current = snapshot;
                const next = [...prev, snapshot];
                return next.length > 600 ? next.slice(-600) : next;
             });
             if (recordingRef.current) {
               const legacyCycleNumber = Math.max(1, currentCycleIndexRef.current + 1);
               const legacySample = {
                 cycle_number: legacyCycleNumber,
                 sample_sequence: sessionDataRef.current.length + 1,
                 ...currentData,
               };
               appendSessionSample(legacySample);
             }
           } else {
             latestDataRef.current = { ...currentData };
           }
           currentData = { ...currentData, time: 0 };
        }
      };

      ws.onclose = () => {
        if (systemRunningRef.current) {
          systemRunningRef.current = false;
          setSystemRunning(false);
          setPhaseReady(false);
          setCurrentPhase('');
          setPhaseTimeLeft(0);
          setIsRecording(false);
          recordingRef.current = false;
          setRecordingStartTime(null);
          setCompletedCycleCount(cycleDataRef.current.filter(rows => rows.length > 0).length);
          localStorage.setItem('dga_acquisition_active', 'false');
          setSafetyWarning('The backend connection was lost while a cycle was running. The Teensy watchdog will turn off the actuators; session data already received can still be saved.');
          setShowSavePopup(true);
        }
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

  // --- Send chamber config to Kria (Fuzzy PID) and Teensy ---
  const sendChamberConfig = useCallback(() => {
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
      const payload = {
        type: 'CHAMBER_CTRL',
        setpoint: value,
        kp: parseFloat(chamberKp),
        ki: parseFloat(chamberKi),
        kd: parseFloat(chamberKd)
      };
      serialPort.send(JSON.stringify(payload));
      setShowChamberPopup(false);
    } catch (e) {
      console.error(e);
    }
  }, [serialPort, setpointInput, chamberKp, chamberKi, chamberKd]);

  // --- Elapsed timer ---
  useEffect(() => {
    if (!isRecording || !recordingStartTime) return;
    const interval = setInterval(() => {
      setRecordingElapsed(Math.floor((Date.now() - recordingStartTime) / 1000));
    }, 1000);
    return () => clearInterval(interval);
  }, [isRecording, recordingStartTime]);

  // Display the phase countdown; phase transitions are scheduled by the Kria backend.
  useEffect(() => {
    if (!systemRunning || !currentPhase || !phaseReady) return;

    const interval = setInterval(() => {
      const elapsed = (Date.now() - phaseStartTimeRef.current) / 1000;
      const remaining = Math.max(0, phaseDurationRef.current - elapsed);
      setPhaseTimeLeft(Math.ceil(remaining));
    }, 500);

    return () => clearInterval(interval);
  }, [systemRunning, currentPhase, phaseReady]);

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
      systemRunningRef.current = false;
      setPhaseReady(false);
      serialPortRef.current?.send(JSON.stringify({ type: 'STOP_SYSTEM' }));
      setCurrentPhase('');
      setPhaseTimeLeft(0);
      setSystemRunning(false);
      localStorage.setItem("dga_acquisition_active", "false");
      // Stop recording
      setIsRecording(false);
      recordingRef.current = false;
      setRecordingStartTime(null);
      setCompletedCycleCount(cycleDataRef.current.filter(rows => rows.length > 0).length);
      // Show save popup if we have data
      if (recordingRef.current === false) {
        // Use a timeout to allow sessionData state to settle
        setTimeout(() => setShowSavePopup(true), 100);
      }
      return;
    }

    // Show the settings popup
    setShowSystemPopup(true);
  }, [isConnected, sessionName, systemRunning]);

  // --- Actually start the system (called from popup "Start" button) ---
  const handleConfirmStart = useCallback(() => {
    setShowSystemPopup(false);

    // Build phase queue
    const queue = [];
    const requestedCycles = Math.max(1, Math.floor(Number(cycleCount) || 1));
    for (let cycleIndex = 0; cycleIndex < requestedCycles; cycleIndex += 1) {
      if (idleDuration > 0) {
        queue.push({ phase: 'IDLE', durationSec: idleDuration * 60, cycleIndex });
      }
      if (injectDuration > 0) {
        queue.push({ phase: 'INJECT', durationSec: injectDuration * 60, cycleIndex });
      }
      if (purgeDuration > 0) {
        queue.push({ phase: 'PURGE', durationSec: purgeDuration * 60, cycleIndex });
      }
    }

    if (queue.length === 0) {
      alert('All phase durations are 0. Please set at least one phase duration.');
      return;
    }

    // Capture baseline (R0) from the latest available data point
    const currentLatest = dataHistory.length > 0 ? dataHistory[dataHistory.length - 1] : null;
    const newBaselines = {};
    if (currentLatest) {
      GAS_SENSORS.slice(0, 14).forEach((_, i) => {
        newBaselines[`mos${i}`] = currentLatest[`mos${i}`] || 1; // Fallback to 1 to avoid div-by-zero
      });
    }
    setBaselines(newBaselines);

    // Start recording
    sessionDataRef.current = [];
    cycleDataRef.current = Array.from({ length: requestedCycles }, () => []);
    currentCycleIndexRef.current = 0;
    setCompletedCycleCount(0);
    frameDiagnosticsRef.current = { lastSequence: null, missing: 0, malformed: 0, nonNumeric: 0, malformedLengths: {}, duplicates: 0, resets: 0 };
    setFrameDiagnostics({ missing: 0, malformed: 0, nonNumeric: 0, malformedLengths: {}, duplicates: 0, resets: 0 });
    setSessionData([]);
    setRecordingStartTime(Date.now());
    setRecordingElapsed(0);
    setIsRecording(true);
    recordingRef.current = true;

    // The Kria backend schedules every phase and keeps the Teensy heartbeat alive.
    try {
      const socket = serialPortRef.current;
      if (!socket || socket.readyState !== WebSocket.OPEN) {
        throw new Error('The backend connection is not open. Reconnect the stream and try again.');
      }
      socket.send(JSON.stringify({
        type: 'START_SYSTEM_PLAN',
        pump_pwm: Number(pump1Pwm),
        phases: queue.map(({ phase, durationSec, cycleIndex }) => ({
          phase,
          duration_sec: durationSec,
          cycle: cycleIndex + 1,
        })),
      }));
      systemRunningRef.current = true;
      setSystemRunning(true);
      localStorage.setItem("dga_acquisition_active", "true");
    } catch (error) {
      systemRunningRef.current = false;
      setSystemRunning(false);
      setPhaseReady(false);
      setIsRecording(false);
      recordingRef.current = false;
      setRecordingStartTime(null);
      localStorage.setItem('dga_acquisition_active', 'false');
      setSafetyWarning(error.message || 'Could not start the system run.');
      setShowSavePopup(true);
    }
  }, [idleDuration, injectDuration, purgeDuration, pump1Pwm, cycleCount, dataHistory]);

  // --- Save Data as CSV ---
  const handleSaveData = useCallback(() => {
    const cycles = cycleDataRef.current
      .map((rows, index) => ({ rows, cycleNumber: index + 1 }))
      .filter(cycle => cycle.rows.length > 0);
    if (cycles.length === 0) {
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
      'kria_vbus_V', 'kria_current_mA', 'kria_power_W',
      'setpoint_C', 'peltier_mode',
      'ndir_ratio', 'ndir_baseline', 'ndir_response_pct', 'ndir_absorbance_au',
      'ndir_valid', 'ndir_baseline_ready',
      'flow_inlet'
    ];
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
    const safeName = sessionName.trim().replace(/[^a-zA-Z0-9_-]/g, '_') || 'session';
    const csvFiles = cycles.map(({ rows, cycleNumber }) => {
      const csvRows = [headers.join(',')];
      for (const point of rows) {
        csvRows.push([
          point.time ?? '',
          ...GAS_SENSORS.map((_, i) => (point[`mos${i}`] ?? '').toString()),
          (point.temp0 ?? '').toString(), (point.temp1 ?? '').toString(),
          (point.hum0 ?? '').toString(), (point.hum1 ?? '').toString(),
          (point.ina1_v ?? '').toString(), (point.ina1_i ?? '').toString(),
          (point.ina1_p ?? '').toString(), (point.ina2_v ?? '').toString(),
          (point.ina2_i ?? '').toString(), (point.ina2_p ?? '').toString(),
          (point.kria_v ?? '').toString(), (point.kria_i ?? '').toString(),
          (point.kria_p ?? '').toString(), (point.setpoint_c ?? '').toString(),
          (point.peltier_mode ?? '').toString(), (point.ndir_ratio ?? '').toString(),
          (point.ndir_baseline ?? '').toString(), (point.ndir_resp ?? '').toString(),
          (point.ndir_abs ?? '').toString(), (point.ndir_valid ?? '').toString(),
          (point.ndir_baseline_ready ?? '').toString(), (point.flow_inlet ?? '').toString(),
        ].join(','));
      }
      return {
        name: `DGA_${safeName}_${faultLabel}_cycle_${String(cycleNumber).padStart(2, '0')}_${timestamp}.csv`,
        content: csvRows.join('\n'),
      };
    });
    const blob = makeZipBlob(csvFiles);
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `DGA_${safeName}_${faultLabel}_${timestamp}.zip`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }, [sessionName, faultLabel]);

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

  const normalizedSessionData = React.useMemo(() => {
    if (Object.keys(baselines).length === 0 || sessionData.length === 0) return [];
    return sessionData.map(point => {
      const normPoint = { time: point.time };
      GAS_SENSORS.slice(0, 14).forEach((sensor, i) => {
        const val = point[`mos${i}`] || 0;
        const base = baselines[`mos${i}`] || 1;
        normPoint[`mos${i}`] = val / base;
      });
      return normPoint;
    });
  }, [sessionData, baselines]);

  const handleSaveGraph = useCallback(async () => {
    const cycles = cycleDataRef.current
      .map((rows, index) => ({ rows, cycleNumber: index + 1 }))
      .filter(cycle => cycle.rows.length > 0);
    if (cycles.length === 0 || Object.keys(baselines).length === 0) {
      setSaveGraphStatus('❌ No cycle data or baselines are available to generate graphs.');
      return;
    }
    const ws = serialPortRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      setSaveGraphStatus('❌ Not connected to the backend. Connect the stream first.');
      return;
    }

    setSaveGraphStatus(`⏳ Generating ${cycles.length} cycle graph(s)...`);
    try {
      const graphFiles = [];
      for (const [index, cycle] of cycles.entries()) {
        setSaveGraphStatus(`⏳ Generating graph ${index + 1}/${cycles.length}...`);
        const result = await new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            saveGraphPendingRef.current = null;
            reject(new Error(`Timeout generating graph for cycle ${cycle.cycleNumber}`));
          }, 30000);

          saveGraphPendingRef.current = {
            resolve: (value) => { clearTimeout(timeout); resolve(value); },
          };
          ws.send(JSON.stringify({
            type: 'SAVE_GRAPH',
            data: cycle.rows,
            baselines,
            sessionName: `${sessionName || 'session'}_cycle_${cycle.cycleNumber}`,
            sensorNames: GAS_SENSORS.slice(0, 14),
          }));
        });

        if (!result.ok || !result.b64_image) {
          throw new Error(result.error || `Failed to generate graph for cycle ${cycle.cycleNumber}`);
        }
        const binary = atob(result.b64_image);
        const imageBytes = Uint8Array.from(binary, char => char.charCodeAt(0));
        graphFiles.push({ name: result.filename || `cycle_${cycle.cycleNumber}.png`, content: imageBytes });
      }

      const blob = makeZipBlob(graphFiles);
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      const safeName = sessionName.trim().replace(/[^a-zA-Z0-9_-]/g, '_') || 'session';
      a.href = url;
      a.download = `DGA_${safeName}_cycle_graphs_${timestamp}.zip`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      setSaveGraphStatus(`✅ Downloaded ${graphFiles.length} cycle graph(s) as a ZIP file.`);
    } catch (e) {
      saveGraphPendingRef.current = null;
      setSaveGraphStatus(`❌ ${e.message}`);
    }
  }, [baselines, sessionName]);

  const handleResetData = () => {
    setDataHistory([]);
    latestDataRef.current = null;
    sessionDataRef.current = [];
    cycleDataRef.current = [];
    setCompletedCycleCount(0);
    setSessionData([]);
    setBaselines({});
  };

  // Safety check effect
  useEffect(() => {
    if (dataHistory.length > 0) {
      const latest = dataHistory[dataHistory.length - 1];
      if (latest.peltier_mode && latest.peltier_mode.startsWith('SAFETY_')) {
         if (latest.peltier_mode === 'SAFETY_COOL') setSafetyWarning("OVERHEAT DETECTED (≥ 45°C)! Peltier forced to COOL mode to protect sensors.");
         else if (latest.peltier_mode === 'SAFETY_HEAT') setSafetyWarning("FREEZING DETECTED (≤ 5°C)! Peltier forced to HEAT mode to protect sensors.");
         else if (latest.peltier_mode === 'SAFETY_OFF') setSafetyWarning("HIGH HUMIDITY (≥ 85%)! System forced OFF to protect sensors.");
      } else {
         setSafetyWarning(prev => prev && /^(OVERHEAT|FREEZING|HIGH HUMIDITY)/.test(prev) ? null : prev);
      }
    }
  }, [dataHistory]);

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
            <button className="btn btn-danger" onClick={disconnectSerial} disabled={systemRunning}>
              Stop Stream
            </button>
          )}

          <button className="btn btn-secondary" onClick={handleResetData} disabled={isRecording || systemRunning}>
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
              Cycle {currentCycleIndexRef.current + 1}/{cycleCount} · {currentPhase} — {formatPhaseTime(phaseTimeLeft)}
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
            <option value="normalized">Normalized Trend</option>
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
                  : latestData && latestData.peltier_mode === 'SAFETY_OFF' ? 'red'
                  : 'var(--text-muted)'
              }}>
                {latestData && latestData.peltier_mode ? latestData.peltier_mode : '--'}
              </strong>
            </span>
          </div>
          <button
            className="btn btn-primary"
            onClick={() => setShowChamberPopup(true)}
            disabled={!isConnected}
            title={!isConnected ? 'Connect to serial first' : 'Configure Chamber PID'}
          >
            Chamber Control
          </button>
        </div>
      </div>

      {viewMode === 'grid' && (
        <div className="charts-grid-4x5">
          {/* Render 14 MOX Gas Sensors (exclude raw IR12EM ACT/REF from UI) */}
          {GAS_SENSORS.slice(0, 14).map((name, i) => (
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
            title="TEMP SAMPLE CHAMBER"
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
            title="HUM SAMPLE CHAMBER"
            dataKey="hum1"
            color="var(--status-normal)"
            domain={[0, 100]}
            unit="%"
            dataHistory={dataHistory}
            latestData={latestData}
          />

          {/* INA226 Current Sensors: #1 measures actuator current & voltage,
              #2 measures sensor + microcontroller supply current & voltage */}
          <MiniChart
            title="ACTUATOR CURRENT"
            dataKey="ina1_i"
            color="var(--accent-orange)"
            domain={['auto', 'auto']}
            unit="mA"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="ACTUATOR VOLTAGE"
            dataKey="ina1_v"
            color="var(--accent-blue)"
            domain={['auto', 'auto']}
            unit="V"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="SENSOR HEATER CURRENT"
            dataKey="ina2_i"
            color="var(--accent-orange)"
            domain={['auto', 'auto']}
            unit="mA"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="SENSOR HEATER VOLTAGE"
            dataKey="ina2_v"
            color="var(--accent-blue)"
            domain={['auto', 'auto']}
            unit="V"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="KRIA SOM CURRENT"
            dataKey="kria_i"
            color="var(--accent-orange)"
            domain={['auto', 'auto']}
            unit="mA"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="KRIA SOM VOLTAGE"
            dataKey="kria_v"
            color="var(--accent-blue)"
            domain={['auto', 'auto']}
            unit="V"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="IR12EM Absorbance"
            dataKey="ndir_abs"
            color="#FF5733"
            domain={['auto', 'auto']}
            unit="AU"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="IR12EM Response"
            dataKey="ndir_resp"
            color="#33FF57"
            domain={['auto', 'auto']}
            unit="%"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="IR12EM Ratio"
            dataKey="ndir_ratio"
            color="#3357FF"
            domain={['auto', 'auto']}
            unit="x"
            dataHistory={dataHistory}
            latestData={latestData}
          />
          <MiniChart
            title="Flow Inlet"
            dataKey="flow_inlet"
            color="#2DD4BF"
            domain={['auto', 'auto']}
            unit="L/min"
            dataHistory={dataHistory}
            latestData={latestData}
          />
        </div>
      )}

      {viewMode === 'combined' && (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '20px', marginTop: '20px' }}>
          {/* Large Combined Chart */}
          <div className="mini-chart-card" style={{ flex: 1, minHeight: '450px', display: 'flex', flexDirection: 'column' }}>
            <div className="chart-header" style={{ marginBottom: '10px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '12px' }}>
              <span className="chart-title" style={{ fontSize: '18px', textTransform: 'none' }}>
                {combinedChartMode === 'gas' ? 'Gas Sensors Trend (mV)' : 'NDIR Trend'}
              </span>
              <select
                className="select-input"
                aria-label="Select a Combined Trend chart"
                style={{ width: '150px', minWidth: '150px' }}
                value={combinedChartMode}
                onChange={(e) => setCombinedChartMode(e.target.value)}
              >
                <option value="gas">Gas Sensors (mV)</option>
                <option value="ndir">NDIR Sensor</option>
              </select>
            </div>
            <div className="chart-area" style={{ flex: 1, minWidth: 0, minHeight: '300px', display: 'flex', flexDirection: 'column', gap: '14px' }}>
              {latestData ? (
                combinedChartMode === 'gas' ? (
                  <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={300}>
                    <LineChart data={dataHistory} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
                      <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgba(35, 63, 124, 0.3)" />
                      <XAxis dataKey="time" stroke="#75BDE0" />
                      <YAxis
                        yAxisId="left"
                        domain={[0, 5000]}
                        ticks={[0, 1000, 2000, 3000, 4000, 5000]}
                        allowDataOverflow={true}
                        stroke="#75BDE0"
                      />
                      <Tooltip
                        contentStyle={{ backgroundColor: '#243F81', border: 'none', borderRadius: '8px', color: '#ffffff' }}
                        itemStyle={{ color: '#ffffff' }}
                      />
                      <Legend wrapperStyle={{ paddingTop: '20px' }} />
                      {GAS_SENSORS.slice(0, 14).map((name, i) => (
                        <Line
                          yAxisId="left"
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
                  <div style={{ flex: 1, minHeight: '360px', display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(260px, 1fr))', gap: '14px' }}>
                    {[
                      { key: 'ndir_ratio', title: 'NDIR Ratio', color: '#3357FF', unit: 'x' },
                      { key: 'ndir_resp', title: 'NDIR Response', color: '#33A852', unit: '%' },
                      { key: 'ndir_abs', title: 'NDIR Absorbance', color: '#FF5733', unit: 'AU' },
                    ].map(({ key, title, color, unit }) => (
                      <div key={key} className="mini-chart-card" style={{ minWidth: 0, minHeight: '320px', display: 'flex', flexDirection: 'column' }}>
                        <div className="chart-header"><span className="chart-title">{title} ({unit})</span></div>
                        <div className="chart-area" style={{ flex: 1, minWidth: 0, minHeight: '260px' }}>
                          <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={260}>
                            <LineChart data={dataHistory} margin={{ top: 10, right: 12, left: 0, bottom: 0 }}>
                              <CartesianGrid strokeDasharray="2 2" vertical={false} stroke="#e2e8f0" />
                              <XAxis dataKey="time" hide />
                              <YAxis domain={['auto', 'auto']} width={48} tick={{ fontSize: 10 }} />
                              <Tooltip
                                contentStyle={{ backgroundColor: '#243F81', border: 'none', borderRadius: '8px', color: '#ffffff' }}
                                itemStyle={{ color: '#ffffff' }}
                                formatter={(value) => [`${Number(value).toFixed(4)} ${unit}`, title]}
                              />
                              <Line type="monotone" dataKey={key} name={title} stroke={color} strokeWidth={2} dot={false} isAnimationActive={false} connectNulls />
                            </LineChart>
                          </ResponsiveContainer>
                        </div>
                      </div>
                    ))}
                  </div>
                )
              ) : (
                <div className="chart-empty" style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  {!isConnected
                    ? 'Stream is not connected. Click Start Stream.'
                    : dataHistory.length > 0
                      ? 'Sensor stream is active. Click Start System to record data and generate the Normalized Trend.'
                      : 'Backend is connected but has not received sensor data from the Teensy.'}
                </div>
              )}
            </div>
          </div>

          {/* Text-only Temp and Hum Info */}
          <div style={{ display: 'flex', gap: '20px' }}>
            <div className="mini-chart-card" style={{ flex: 1 }}>
              <div className="chart-header">
                <span className="chart-title">Sensor Chamber Conditions</span>
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
                <span className="chart-title">Sample Chamber Conditions</span>
              </div>
              <div style={{ marginTop: '10px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>TEMP SAMPLE CHAMBER:</span>
                  <span style={{ color: 'var(--accent-blue)', fontWeight: 'bold' }}>
                    {latestData && latestData.temp1 !== undefined ? latestData.temp1.toFixed(2) : '--.--'} °C
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>HUM SAMPLE CHAMBER:</span>
                  <span style={{ color: 'var(--status-normal)', fontWeight: 'bold' }}>
                    {latestData && latestData.hum1 !== undefined ? latestData.hum1.toFixed(2) : '--.--'} %
                  </span>
                </div>
              </div>
            </div>

            <div className="mini-chart-card" style={{ flex: 1 }}>
              <div className="chart-header">
                <span className="chart-title">Current Sensors & Flow</span>
              </div>
              <div style={{ marginTop: '10px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>ACTUATOR (I/V/P):</span>
                  <span style={{ color: 'var(--accent-orange)', fontWeight: 'bold' }}>
                    {latestData && latestData.ina1_i !== undefined ? latestData.ina1_i.toFixed(0) : '--'} mA
                    {' / '}
                    {latestData && latestData.ina1_v !== undefined ? latestData.ina1_v.toFixed(2) : '--.--'} V
                    {' / '}
                    {latestData && latestData.ina1_p !== undefined ? latestData.ina1_p.toFixed(2) : '--.--'} W
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>SENSOR HEATER (I/V/P):</span>
                  <span style={{ color: 'var(--accent-orange)', fontWeight: 'bold' }}>
                    {latestData && latestData.ina2_i !== undefined ? latestData.ina2_i.toFixed(0) : '--'} mA
                    {' / '}
                    {latestData && latestData.ina2_v !== undefined ? latestData.ina2_v.toFixed(2) : '--.--'} V
                    {' / '}
                    {latestData && latestData.ina2_p !== undefined ? latestData.ina2_p.toFixed(2) : '--.--'} W
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>KRIA MAIN (I/V/P):</span>
                  <span style={{ color: 'var(--status-normal)', fontWeight: 'bold' }}>
                    {latestData && latestData.kria_i !== undefined ? latestData.kria_i.toFixed(0) : '--'} mA
                    {' / '}
                    {latestData && latestData.kria_v !== undefined ? latestData.kria_v.toFixed(2) : '--.--'} V
                    {' / '}
                    {latestData && latestData.kria_p !== undefined ? latestData.kria_p.toFixed(2) : '--.--'} W
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '16px' }}>
                  <span style={{ color: 'var(--text-muted)' }}>Flow Inlet:</span>
                  <span style={{ color: '#2DD4BF', fontWeight: 'bold' }}>
                    {latestData && latestData.flow_inlet !== undefined ? latestData.flow_inlet.toFixed(2) : '--.--'} L/min
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {viewMode === 'normalized' && (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: '20px', marginTop: '20px' }}>
          <div className="mini-chart-card" style={{ flex: 1, minHeight: '450px', display: 'flex', flexDirection: 'column' }}>
            <div className="chart-header" style={{ marginBottom: '10px' }}>
              <span className="chart-title" style={{ fontSize: '18px', textTransform: 'none' }}>Normalized Gas Sensors Response (Rs/R0)</span>
            </div>
            <div className="chart-area" style={{ flex: 1, minWidth: 0, minHeight: '300px' }}>
              {normalizedSessionData.length > 0 ? (
                <ResponsiveContainer width="100%" height="100%" minWidth={0} minHeight={300}>
                  <LineChart data={normalizedSessionData} margin={{ top: 10, right: 30, left: 0, bottom: 0 }}>
                    <CartesianGrid strokeDasharray="3 3" vertical={false} stroke="rgba(35, 63, 124, 0.3)" />
                    <XAxis dataKey="time" stroke="#75BDE0" />
                    <YAxis yAxisId="left" domain={['auto', 'auto']} stroke="#75BDE0" />
                    <Tooltip 
                      contentStyle={{ backgroundColor: '#243F81', border: 'none', borderRadius: '8px', color: '#ffffff' }}
                      itemStyle={{ color: '#ffffff' }}
                    />
                    <Legend wrapperStyle={{ paddingTop: '20px' }} />
                    {GAS_SENSORS.slice(0, 14).map((name, i) => (
                      <Line
                        yAxisId="left"
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
                  Press 'Start System' to capture baseline and view normalized response.
                </div>
              )}
            </div>
          </div>
        </div>
      )}

      {/* ===== SYSTEM SETTINGS POPUP ===== */}
      {showSystemPopup && (
        <div className="modal-overlay" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'rgba(0,0,0,0.7)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }}>
          <div className="modal-content" style={{ backgroundColor: '#ffffff', color: '#1e293b', padding: '28px', borderRadius: '12px', width: '560px', maxHeight: '90vh', overflowY: 'auto', border: '1px solid #e2e8f0', boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.25)' }}>
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

              {/* Number of complete phase cycles */}
              <div style={{ backgroundColor: '#ffffff', padding: '16px', borderRadius: '8px', border: '1px solid #BFDBFE' }}>
                <label htmlFor="acquisition-cycle-count" style={{ display: 'block', fontWeight: '700', color: '#1E40AF', fontSize: '15px', marginBottom: '8px' }}>
                  Number of Cycles
                </label>
                <input
                  id="acquisition-cycle-count"
                  type="number"
                  min="1"
                  max="100"
                  step="1"
                  value={cycleCount}
                  onChange={(e) => setCycleCount(Math.max(1, Math.min(100, parseInt(e.target.value, 10) || 1)))}
                  style={{ width: '100%', padding: '9px 10px', borderRadius: '6px', border: '1px solid #CBD5E1', color: '#1e293b' }}
                />
                <div style={{ fontSize: '12px', color: '#6B7280', marginTop: '6px' }}>
                  Each cycle runs IDLE → INJECT → PURGE and is saved as its own CSV.
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
                Total Duration: <strong>{((idleDuration + injectDuration + purgeDuration) * cycleCount).toFixed(1)} min</strong>
                {' '}({formatElapsed(Math.round((idleDuration + injectDuration + purgeDuration) * cycleCount * 60))})
                <div style={{ marginTop: '3px' }}>{cycleCount} cycle(s) · {((idleDuration + injectDuration + purgeDuration)).toFixed(1)} min each</div>
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

      {/* ===== CHAMBER CONTROL POPUP ===== */}
      {showChamberPopup && (
        <div className="modal-overlay" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'rgba(0,0,0,0.7)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000 }}>
          <div className="modal-content" style={{ backgroundColor: '#ffffff', color: '#1e293b', padding: '28px', borderRadius: '12px', width: '380px', border: '1px solid #e2e8f0', boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.25)' }}>
            <h3 style={{ margin: '0 0 16px 0', color: '#0f172a', fontSize: '20px', textAlign: 'center' }}>Chamber PID Control</h3>
            
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginBottom: '24px' }}>
              <div>
                <label style={{ display: 'block', marginBottom: '4px', fontSize: '14px', fontWeight: 600 }}>Setpoint (°C)</label>
                <input type="number" step="0.1" value={setpointInput} onChange={(e) => setSetpointInput(e.target.value)} style={{ width: '100%', padding: '8px', borderRadius: '6px', border: '1px solid #cbd5e1' }} />
              </div>
              <div style={{ display: 'flex', gap: '12px' }}>
                <div style={{ flex: 1 }}>
                  <label style={{ display: 'block', marginBottom: '4px', fontSize: '14px', fontWeight: 600 }}>Kp</label>
                  <input type="number" step="0.1" value={chamberKp} onChange={(e) => setChamberKp(e.target.value)} style={{ width: '100%', padding: '8px', borderRadius: '6px', border: '1px solid #cbd5e1' }} />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={{ display: 'block', marginBottom: '4px', fontSize: '14px', fontWeight: 600 }}>Ki</label>
                  <input type="number" step="0.1" value={chamberKi} onChange={(e) => setChamberKi(e.target.value)} style={{ width: '100%', padding: '8px', borderRadius: '6px', border: '1px solid #cbd5e1' }} />
                </div>
                <div style={{ flex: 1 }}>
                  <label style={{ display: 'block', marginBottom: '4px', fontSize: '14px', fontWeight: 600 }}>Kd</label>
                  <input type="number" step="0.1" value={chamberKd} onChange={(e) => setChamberKd(e.target.value)} style={{ width: '100%', padding: '8px', borderRadius: '6px', border: '1px solid #cbd5e1' }} />
                </div>
              </div>
            </div>

            <div style={{ display: 'flex', gap: '12px', justifyContent: 'flex-end' }}>
              <button 
                onClick={() => setShowChamberPopup(false)}
                style={{ padding: '8px 16px', backgroundColor: '#f1f5f9', color: '#475569', border: 'none', borderRadius: '6px', cursor: 'pointer', fontWeight: 600 }}
              >
                Cancel
              </button>
              <button 
                onClick={sendChamberConfig}
                style={{ padding: '8px 16px', backgroundColor: '#0ea5e9', color: 'white', border: 'none', borderRadius: '6px', cursor: 'pointer', fontWeight: 600 }}
              >
                Apply
              </button>
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
            {completedCycleCount} cycle(s) recorded. Each cycle contains the selected IDLE, INJECT, and PURGE phases.
          </p>
          <p style={{ color: '#64748b', fontSize: '14px', marginBottom: '20px' }}>
            <strong>{sessionDataRef.current.length}</strong> data points recorded for session &quot;<strong>{sessionName}</strong>&quot;.
          </p>
          {(frameDiagnostics.missing || frameDiagnostics.malformed || frameDiagnostics.duplicates || frameDiagnostics.resets) > 0 && (
            <p style={{ color: '#b45309', fontSize: '13px', marginBottom: '16px' }}>
              Stream quality: {frameDiagnostics.missing} samples missing, {frameDiagnostics.malformed} malformed frames ({frameDiagnostics.nonNumeric} non-numeric values; frame lengths: {Object.entries(frameDiagnostics.malformedLengths).map(([length, count]) => `${length} fields × ${count}`).join(', ') || '-'}), {frameDiagnostics.duplicates} duplicates, {frameDiagnostics.resets} sequence resets.
            </p>
          )}

            {saveGraphStatus && (
              <p style={{
                fontSize: '13px',
                marginBottom: '12px',
                color: saveGraphStatus.startsWith('✅') ? '#16a34a' : saveGraphStatus.startsWith('⏳') ? '#d97706' : '#dc2626',
              }}>
                {saveGraphStatus}
              </p>
            )}

            <div style={{ display: 'flex', gap: '12px', justifyContent: 'center', flexWrap: 'wrap' }}>
              <button
                className="btn btn-outline"
                onClick={() => { setShowSavePopup(false); setSaveGraphStatus(''); }}
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
                <Save size={16} /> Download cycle CSVs (.zip)
              </button>
              <button
                className="btn btn-primary"
                disabled={normalizedSessionData.length === 0 || saveGraphStatus.startsWith('⏳')}
                onClick={handleSaveGraph}
                style={{ display: 'flex', alignItems: 'center', gap: '6px', backgroundColor: '#7c3aed', borderColor: '#7c3aed' }}
              >
                📊 Download cycle graphs (.zip)
              </button>
            </div>
          </div>
        </div>
      )}



      {/* ===== SAFETY WARNING POPUP ===== */}
      {safetyWarning && (
        <div className="modal-overlay" style={{ position: 'fixed', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'rgba(255,0,0,0.3)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 9999 }}>
          <div className="modal-content" style={{ backgroundColor: '#fee2e2', color: '#991b1b', padding: '28px', borderRadius: '12px', width: '500px', border: '2px solid #ef4444', boxShadow: '0 25px 50px -12px rgba(239, 68, 68, 0.4)', textAlign: 'center' }}>
            <div style={{ fontSize: '48px', marginBottom: '10px' }}>⚠️</div>
            <h3 style={{ fontSize: '20px', fontWeight: 'bold', marginBottom: '12px', marginTop: 0 }}>SAFETY OVERRIDE ACTIVE</h3>
            <p style={{ fontSize: '16px', lineHeight: '1.5', margin: 0, fontWeight: '500' }}>
              {safetyWarning}
            </p>
            <button className="btn btn-danger" onClick={() => setSafetyWarning(null)} style={{ marginTop: '18px' }}>
              Dismiss Warning
            </button>
          </div>
        </div>
      )}

    </div>
  );
};

Acquisition.propTypes = {
  isConnected: PropTypes.bool.isRequired,
  setIsConnected: PropTypes.func.isRequired,
};

export default Acquisition;
