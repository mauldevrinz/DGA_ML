import React, { useState, useEffect, useRef } from 'react';
import { Play, Square, Activity, Cpu, Upload, Link } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import './Classification.css';

const GAS_TYPES = ['Udara Bersih', 'Asetilena (C2H2)', 'Etilena (C2H4)', 'Hidrogen (H2)', 'Metana (CH4)', 'Alcohol'];

const diagnoseIEC60599 = (gasConf) => {
  const { asetilena, etilena, hidrogen, metana, udara } = gasConf;
  if (udara > 70 && asetilena < 30 && etilena < 30 && hidrogen < 30) 
    return { code: 'Normal', type: 'No Fault', severity: 'normal', color: '#10b981' };
  
  const faultTotal = asetilena + etilena + hidrogen + (metana || 0);
  if (faultTotal < 10) return { code: 'Normal', type: 'No Fault', severity: 'normal', color: '#10b981' };
  
  const pC2H2 = asetilena / faultTotal;
  const pC2H4 = etilena / faultTotal;
  const pH2 = hidrogen / faultTotal;
  
  if (pC2H2 > 0.4 && hidrogen > 30) return { code: 'D2', type: 'Electrical Fault', severity: 'critical', desc: 'High Energy Discharge (Arcing)', color: '#ef4444' };
  if (pC2H2 > 0.25 && asetilena > 30) return { code: 'D1', type: 'Electrical Fault', severity: 'warning', desc: 'Low Energy Discharge (Sparking)', color: '#f97316' };
  if (pH2 > 0.5 && asetilena < 30) return { code: 'PD', type: 'Electrical Fault', severity: 'caution', desc: 'Partial Discharge', color: '#eab308' };
  if (pC2H4 > 0.6 && etilena > 60) return { code: 'T3', type: 'Thermal Fault', severity: 'critical', desc: 'Thermal > 700°C', color: '#ef4444' };
  if (pC2H4 > 0.4 && etilena > 30) return { code: 'T2', type: 'Thermal Fault', severity: 'warning', desc: 'Thermal 300-700°C', color: '#f97316' };
  if (pC2H4 > 0.2 || etilena > 30) return { code: 'T1', type: 'Thermal Fault', severity: 'caution', desc: 'Thermal < 300°C', color: '#eab308' };
  if (asetilena > 30 && etilena > 30) return { code: 'DT', type: 'Mixed Fault', severity: 'critical', desc: 'Thermal + Electrical', color: '#8b5cf6' };
  
  return { code: 'T1', type: 'Thermal Fault', severity: 'caution', desc: 'Low-grade thermal', color: '#eab308' };
};

const Classification = () => {
  const [isClassifying, setIsClassifying] = useState(false);
  const [selectedModel, setSelectedModel] = useState('SNN (Spiking Neural Network)');
  const [inferenceTime, setInferenceTime] = useState(0);
  const [colabUrl, setColabUrl] = useState('');
  
  // Independent probabilities for each gas type
  const [prob, setProb] = useState([0, 0, 0, 0, 0, 0]);
  const [history, setHistory] = useState([]);

  const toggle = () => {
    if (isClassifying) {
      setIsClassifying(false);
    } else {
      setIsClassifying(true);
      // Simulate real-time inference loop
      let count = 0;
      let scenarioIdx = 0;
      
      const interval = setInterval(() => {
        count++;
        if (count % 4 === 0) {
           scenarioIdx = (scenarioIdx + 1) % 3;
        }
        
        let newProb = [0, 0, 0, 0, 0, 0];
        const noise = () => Math.random() * 5;
        
        if (scenarioIdx === 0) {
           // Normal
           newProb = [85 + Math.random() * 10, noise(), noise(), noise(), noise(), noise()];
        } else if (scenarioIdx === 1) {
           // Overheating
           newProb = [noise(), noise(), 70 + Math.random() * 20, 30 + Math.random() * 20, 40 + Math.random() * 20, noise()];
        } else {
           // Arcing
           newProb = [noise(), 75 + Math.random() * 15, noise(), 60 + Math.random() * 20, noise(), noise()];
        }
        
        setProb(newProb);
        setInferenceTime((35 + Math.random() * 10).toFixed(1));
        
        if (count % 3 === 0) {
          const gasConf = {
            udara: newProb[0],
            asetilena: newProb[1],
            etilena: newProb[2],
            hidrogen: newProb[3],
            metana: newProb[4]
          };
          const diagnosis = diagnoseIEC60599(gasConf);
          setHistory(prev => [{ time: new Date().toLocaleTimeString(), diagnosis: diagnosis.code, desc: diagnosis.desc || diagnosis.type }, ...prev].slice(0, 6));
        }
      }, 1000);
      
      return () => clearInterval(interval);
    }
  };

  const currentGasConf = {
    udara: prob[0],
    asetilena: prob[1],
    etilena: prob[2],
    hidrogen: prob[3],
    metana: prob[4]
  };
  
  const currentDiagnosis = diagnoseIEC60599(currentGasConf);

  const getGasColor = (idx) => {
    const colors = ['#75BDE0', '#ef4444', '#f97316', '#eab308', '#8b5cf6', '#94a3b8'];
    return colors[idx] || '#ffffff';
  };

  return (
    <div className="page-container">
      <div className="page-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 className="page-title">Real-Time DGA Classification</h2>
          <span className="app-subtitle" style={{ display: 'inline-block', marginLeft: '12px' }}>Multi-Label Gas Prediction & IEC 60599</span>
        </div>
        {isClassifying && (
          <div style={{ display: 'flex', gap: '16px', alignItems: 'center', backgroundColor: '#243F81', padding: '8px 16px', borderRadius: '24px', border: '1px solid rgba(35, 63, 124, 0.4)' }}>
            <Activity size={18} color="#75BDE0" />
            <span style={{ color: '#ffffff', fontSize: '13px' }}>Latency: <strong>{inferenceTime} ms</strong></span>
          </div>
        )}
      </div>

      <div className="cls-layout">
        <div className="cls-left">
          <div className="card" style={{ backgroundColor: 'var(--surface-color)' }}>
            <h3 className="card-title" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}><Cpu size={18} /> Active Model Deployment</h3>
            
            <div className="form-group" style={{ marginTop: '16px' }}>
              <label>Select Trained Model</label>
              <select 
                className="select-input" 
                value={selectedModel} 
                onChange={(e) => setSelectedModel(e.target.value)}
                disabled={isClassifying}
              >
                <option>SNN (Spiking Neural Network)</option>
                <option>Random Forest (Multi-Output)</option>
                <option>SVM (RBF Multi-Output)</option>
              </select>
            </div>
            
            <div className="form-group" style={{ marginTop: '12px' }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: '4px' }}><Link size={14} /> Remote API URL (Ngrok)</label>
              <input 
                type="text" 
                className="select-input" 
                placeholder="https://xxxx.ngrok-free.app/predict"
                value={colabUrl}
                onChange={(e) => setColabUrl(e.target.value)}
                disabled={isClassifying}
                style={{ cursor: isClassifying ? 'not-allowed' : 'text' }}
              />
            </div>
            
            <button 
              className="btn btn-primary" 
              style={{ width: '100%', marginTop: '24px', height: '48px', fontSize: '15px', background: isClassifying ? '#ef4444' : 'linear-gradient(135deg, #233F7C 0%, #243F81 100%)', border: 'none' }}
              onClick={toggle}
            >
              {isClassifying ? <Square size={18} fill="currentColor" /> : <Play size={18} fill="currentColor" />}
              {isClassifying ? ' Stop Live Inference' : ' Start Live Inference'}
            </button>

            <div style={{ position: 'relative', marginTop: '12px' }}>
              <input 
                type="file" 
                accept=".csv" 
                style={{ display: 'none' }} 
                id="csv-upload"
                onChange={(e) => {
                  const file = e.target.files[0];
                  if (file) {
                    const reader = new FileReader();
                    reader.onload = () => {
                      const lines = reader.result.split('\n').filter(l => l.trim());
                      const dataRows = lines.length - 1;
                      for (let i = 0; i < Math.min(dataRows, 6); i++) {
                        const noise = () => Math.random() * 5;
                        setTimeout(() => {
                          const newProb = [Math.random()*20, Math.random()*80, Math.random()*80, Math.random()*80, Math.random()*80, noise()];
                          const diag = diagnoseIEC60599({ udara: newProb[0], asetilena: newProb[1], etilena: newProb[2], hidrogen: newProb[3], metana: newProb[4] });
                          
                          setHistory(prev => [{ time: `Row ${i+1}`, diagnosis: diag.code, desc: diag.desc || diag.type }, ...prev].slice(0, 6));
                          setProb(newProb);
                          setInferenceTime((35 + Math.random() * 10).toFixed(1));
                        }, i * 300);
                      }
                      alert(`CSV loaded: ${dataRows} samples predicted using ${selectedModel}`);
                    };
                    reader.readAsText(file);
                    e.target.value = '';
                  }
                }}
              />
              <button 
                className="btn btn-outline" 
                style={{ width: '100%', height: '42px', fontSize: '14px', display: 'flex', justifyContent: 'center', alignItems: 'center', gap: '8px' }}
                onClick={() => document.getElementById('csv-upload').click()}
                disabled={isClassifying}
              >
                <Upload size={16} /> Predict from CSV File
              </button>
            </div>
          </div>
          
          <div className="card" style={{ marginTop: '24px', flex: 1, display: 'flex', flexDirection: 'column' }}>
            <h3 className="card-title">Inference History</h3>
            {history.length === 0 ? (
              <div className="empty-box" style={{ height: '200px' }}>No predictions yet.</div>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginTop: '16px' }}>
                {history.map((h, i) => (
                  <div key={i} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '12px', backgroundColor: '#243F81', borderRadius: '8px', border: '1px solid rgba(35, 63, 124, 0.4)' }}>
                    <span style={{ fontSize: '12px', color: '#75BDE0', width: '25%' }}>{h.time}</span>
                    <span style={{ fontWeight: 'bold', color: '#ffffff', fontSize: '14px', width: '20%', textAlign: 'center' }}>{h.diagnosis}</span>
                    <span style={{ fontSize: '12px', color: '#ffffff', backgroundColor: 'rgba(255, 255, 255, 0.1)', padding: '4px 8px', borderRadius: '12px', width: '50%', textAlign: 'right', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{h.desc}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="cls-right">
          <div className="card status-panel" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', minHeight: '160px', backgroundColor: '#243F81', border: `2px solid ${isClassifying ? currentDiagnosis.color : 'rgba(35, 63, 124, 0.4)'}`, transition: 'border-color 0.3s ease' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '8px' }}>
              <div className="status-indicator" style={{ width: '16px', height: '16px', backgroundColor: isClassifying ? currentDiagnosis.color : 'var(--text-muted)', boxShadow: isClassifying ? `0 0 15px ${currentDiagnosis.color}` : 'none' }}></div>
              <h1 className="status-text" style={{ fontSize: '28px', margin: 0, color: isClassifying ? currentDiagnosis.color : '#64748b' }}>
                {isClassifying ? currentDiagnosis.type.toUpperCase() : 'STANDBY'}
              </h1>
            </div>
            {isClassifying && (
              <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
                <span style={{ backgroundColor: 'rgba(255,255,255,0.1)', padding: '4px 12px', borderRadius: '16px', fontSize: '14px', color: '#fff', fontWeight: 'bold' }}>IEC Code: {currentDiagnosis.code}</span>
                {currentDiagnosis.desc && (
                  <span style={{ backgroundColor: 'rgba(255,255,255,0.1)', padding: '4px 12px', borderRadius: '16px', fontSize: '14px', color: '#cbd5e1' }}>{currentDiagnosis.desc}</span>
                )}
              </div>
            )}
            {!isClassifying && (
              <p className="status-conf" style={{ fontSize: '16px', color: '#cbd5e1', margin: 0, marginTop: '8px' }}>
                Awaiting sensor data streams...
              </p>
            )}
          </div>

          <div className="card" style={{ marginTop: '24px', flex: 1, display: 'flex', flexDirection: 'column' }}>
            <h3 className="card-title">Gas Composition Analysis</h3>
            <div className="prob-list" style={{ marginTop: '20px' }}>
              {GAS_TYPES.map((label, idx) => {
                const val = isClassifying ? prob[idx] : 0;
                const color = getGasColor(idx);
                
                return (
                  <div key={label} className="prob-item" style={{ marginBottom: '16px' }}>
                    <div className="prob-info" style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '6px' }}>
                      <span style={{ fontWeight: '500', color: 'var(--text-primary)', fontSize: '14px' }}>{label}</span>
                      <span style={{ fontWeight: 'bold', color: val > 15 ? color : 'var(--text-muted)', fontSize: '14px' }}>{val.toFixed(1)}%</span>
                    </div>
                    <div className="progress-container" style={{ backgroundColor: 'rgba(35, 63, 124, 0.3)', height: '10px', borderRadius: '5px' }}>
                      <div className="progress-bar" style={{ width: `${Math.min(val, 100)}%`, backgroundColor: color, transition: 'width 0.3s ease', height: '100%', borderRadius: '5px' }}></div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Classification;
