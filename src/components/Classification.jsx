import React, { useState, useEffect, useRef } from 'react';
import { Play, Square, Activity, Cpu, CheckCircle, Upload } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import './Classification.css';

const CLASSES = ['Baseline', 'Normal', 'Overheating', 'Arcing'];

const Classification = () => {
  const [isClassifying, setIsClassifying] = useState(false);
  const [selectedModel, setSelectedModel] = useState('SNN (Rust Native Edge)');
  const [inferenceTime, setInferenceTime] = useState(0);
  const [prob, setProb] = useState([0, 0, 0, 0]);
  const [history, setHistory] = useState([]);

  const toggle = () => {
    if (isClassifying) {
      setIsClassifying(false);
    } else {
      setIsClassifying(true);
      // Simulate real-time inference loop
      let count = 0;
      const interval = setInterval(() => {
        count++;
        // Dummy live inference data simulating Overheating detection
        const noise = Math.random() * 5;
        let newProb = [0, 0, 0, 0];
        if (count < 5) {
           newProb = [95 + noise, Math.random()*2, Math.random()*2, Math.random()*1];
        } else {
           // Spike to Overheating
           newProb = [Math.random()*5, Math.random()*2, 92 + noise, Math.random()*3];
        }
        
        setProb(newProb);
        setInferenceTime((35 + Math.random() * 10).toFixed(1)); // 35-45ms latency
        
        if (count % 3 === 0) {
          const maxIdx = newProb.indexOf(Math.max(...newProb));
          setHistory(prev => [{ time: new Date().toLocaleTimeString(), class: CLASSES[maxIdx], conf: newProb[maxIdx].toFixed(1) }, ...prev].slice(0, 6));
        }
      }, 1000);
      
      // Cleanup on stop
      return () => clearInterval(interval);
    }
  };

  const getStatusColor = (probArray) => {
    const maxIdx = probArray.indexOf(Math.max(...probArray));
    if (maxIdx === 0) return '#FFFFFF'; // Baseline (White)
    if (maxIdx === 1) return '#75BDE0'; // Normal (Light Blue)
    if (maxIdx === 2) return '#93C5FD'; // Overheating (Cyan/Light Blue)
    if (maxIdx === 3) return '#ef4444'; // Arcing (Red - danger)
    return 'var(--text-muted)';
  };

  const maxProbIdx = prob.indexOf(Math.max(...prob));

  return (
    <div className="page-container">
      <div className="page-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 className="page-title">Real-Time Classification</h2>
          <span className="app-subtitle" style={{ display: 'inline-block', marginLeft: '12px' }}>Live Inference via Jetson Orin Nano</span>
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
                <option>SNN (Rust Native Edge)</option>
                <option>Random Forest (Bagging)</option>
                <option>SVM (RBF Kernel)</option>
              </select>
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
                    // Simulate CSV batch prediction
                    const reader = new FileReader();
                    reader.onload = () => {
                      const lines = reader.result.split('\n').filter(l => l.trim());
                      const dataRows = lines.length - 1; // minus header
                      for (let i = 0; i < Math.min(dataRows, 6); i++) {
                        const noise = Math.random() * 5;
                        const classIdx = Math.floor(Math.random() * 4);
                        const conf = (85 + noise).toFixed(1);
                        setTimeout(() => {
                          setHistory(prev => [{ time: `Row ${i+1}`, class: CLASSES[classIdx], conf }, ...prev].slice(0, 6));
                          setProb(CLASSES.map((_, idx) => idx === classIdx ? 85 + noise : Math.random() * 5));
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
                    <span style={{ fontSize: '12px', color: '#75BDE0' }}>{h.time}</span>
                    <span style={{ fontWeight: 'bold', color: '#ffffff', fontSize: '14px' }}>{h.class}</span>
                    <span style={{ fontSize: '12px', color: '#ffffff', backgroundColor: 'rgba(255, 255, 255, 0.2)', padding: '4px 8px', borderRadius: '12px' }}>{h.conf}%</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>

        <div className="cls-right">
          <div className="card status-panel" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', minHeight: '200px', backgroundColor: '#243F81', border: `2px solid ${isClassifying ? getStatusColor(prob) : 'rgba(35, 63, 124, 0.4)'}`, transition: 'border-color 0.3s ease' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '16px' }}>
              <div className="status-indicator" style={{ width: '16px', height: '16px', backgroundColor: isClassifying ? getStatusColor(prob) : 'var(--text-muted)', boxShadow: isClassifying ? `0 0 15px ${getStatusColor(prob)}` : 'none' }}></div>
              <h1 className="status-text" style={{ fontSize: '32px', margin: 0, color: isClassifying ? getStatusColor(prob) : '#64748b' }}>
                {isClassifying ? CLASSES[maxProbIdx].toUpperCase() : 'STANDBY'}
              </h1>
            </div>
            <p className="status-conf" style={{ fontSize: '18px', color: '#cbd5e1', margin: 0 }}>
              {isClassifying ? `Confidence: ${Math.max(...prob).toFixed(1)}%` : 'Awaiting sensor data streams...'}
            </p>
          </div>

          <div className="card" style={{ marginTop: '24px', flex: 1, display: 'flex', flexDirection: 'column' }}>
            <h3 className="card-title">Class Probabilities</h3>
            <div className="prob-list" style={{ marginTop: '20px' }}>
              {CLASSES.map((label, idx) => {
                const val = isClassifying ? prob[idx] : 0;
                const color = getStatusColor([0,0,0,0].map((_, i) => i === idx ? 1 : 0)); // Hack to get base color for each class
                
                return (
                  <div key={label} className="prob-item" style={{ marginBottom: '20px' }}>
                    <div className="prob-info" style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '8px' }}>
                      <span style={{ fontWeight: '500', color: 'var(--text-primary)' }}>{label}</span>
                      <span style={{ fontWeight: 'bold', color: val > 50 ? color : 'var(--text-muted)' }}>{val.toFixed(1)}%</span>
                    </div>
                    <div className="progress-container" style={{ backgroundColor: 'rgba(35, 63, 124, 0.3)', height: '8px', borderRadius: '4px' }}>
                      <div className="progress-bar" style={{ width: `${val}%`, backgroundColor: color, transition: 'width 0.3s ease', height: '100%', borderRadius: '4px' }}></div>
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
