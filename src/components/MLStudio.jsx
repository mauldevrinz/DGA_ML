import React, { useState } from 'react';
import { Play, UploadCloud, FileText, Activity } from 'lucide-react';
import { useLocalStorage } from '../hooks/useLocalStorage';
import './MLStudio.css';

const MLStudio = () => {
  const [isTraining, setIsTraining] = useState(false);
  const [progress, setProgress] = useLocalStorage('dga_mlProgress', 0);
  const [logs, setLogs] = useLocalStorage('dga_mlLogs', []);

  const startTraining = () => {
    setIsTraining(true);
    setProgress(0);
    setLogs(["[SYSTEM] Connecting to Google Colab..."]);
    
    let currentProgress = 0;
    const interval = setInterval(() => {
      currentProgress += 10;
      
      if (currentProgress === 20) {
        setLogs(prev => [...prev, "[UPLOAD] Transmitting CSV datasets (2.4MB)..."]);
      } else if (currentProgress === 40) {
        setLogs(prev => [...prev, "[COLAB] Data received. Training Support Vector Machine (SVM)..."]);
      } else if (currentProgress === 60) {
        setLogs(prev => [...prev, "[COLAB] SVM completed. Training Random Forest..."]);
      } else if (currentProgress === 80) {
        setLogs(prev => [...prev, "[COLAB] RF completed. Training Spiking Neural Network (SNN)..."]);
      } else if (currentProgress === 100) {
        setLogs(prev => [...prev, "[SYSTEM] All models trained successfully. Fetching weights..."]);
        clearInterval(interval);
        setTimeout(() => {
           setIsTraining(false);
           setLogs(prev => [...prev, "[SUCCESS] Models downloaded. Ready for inference."]);
        }, 600);
      }
      
      setProgress(currentProgress);
    }, 800);
  };

  return (
    <div className="page-container">
      <div className="page-header">
        <h2 className="page-title">Machine Learning Studio</h2>
      </div>

      <div className="card" style={{ marginBottom: '24px', backgroundColor: '#262364', borderLeft: '4px solid var(--accent-orange)' }}>
        <div style={{ display: 'flex', gap: '12px', alignItems: 'flex-start' }}>
          <div style={{ color: '#ffffff' }}>
            <Activity size={24} />
          </div>
          <div>
            <h3 style={{ margin: '0 0 8px 0', fontSize: '16px', color: '#ffffff' }}>Cloud Offloading with Google Colab</h3>
            <p style={{ margin: 0, fontSize: '14px', color: 'rgba(255, 255, 255, 0.85)', lineHeight: '1.5' }}>
              <strong>Workflow:</strong><br/>
              1️⃣ <strong>Data Transmission:</strong> When you click "Train All Models", local CSV datasets are sent to a connected Google Colab instance.<br/>
              2️⃣ <strong>Remote Training:</strong> Colab utilizes its cloud GPUs to train the SVM, Random Forest, and SNN models simultaneously.<br/>
              3️⃣ <strong>Model Deployment:</strong> Once finished, the trained model weights and metrics are sent back to this local interface for real-time inference.
            </p>
          </div>
        </div>
      </div>

      <div className="ml-layout">
        <div className="ml-left">
          <div className="card list-card">
            <h3 className="card-title">Upload Datasets (CSV)</h3>
            <div className="dataset-uploads">
              <div className="upload-row">
                <span>Baseline Data</span>
                <button className="btn btn-outline btn-sm"><UploadCloud size={14}/> Upload</button>
              </div>
              <div className="upload-row">
                <span>Normal Data</span>
                <button className="btn btn-outline btn-sm"><UploadCloud size={14}/> Upload</button>
              </div>
              <div className="upload-row">
                <span>Overheating Data</span>
                <button className="btn btn-outline btn-sm"><UploadCloud size={14}/> Upload</button>
              </div>
              <div className="upload-row">
                <span>Arcing Data</span>
                <button className="btn btn-outline btn-sm"><UploadCloud size={14}/> Upload</button>
              </div>
            </div>
          </div>
          <div className="card list-card" style={{ marginTop: '24px' }}>
            <h3 className="card-title">Trained Models</h3>
            <div className="empty-box">No models trained yet.</div>
          </div>
        </div>

        <div className="ml-right">
          <div className="card">
            <h3 className="card-title">Train Configuration</h3>
            <div className="form-group">
              <label>Normalization</label>
              <select className="select-input">
                <option>Z-Score Standardization</option>
                <option>Min-Max Scaling</option>
              </select>
            </div>
            
            <button 
              className="btn btn-primary" 
              style={{ marginTop: '24px', width: '100%' }}
              onClick={startTraining}
              disabled={isTraining}
            >
              <Play size={16} /> {isTraining ? 'Training All Models...' : 'Train All Models'}
            </button>
          </div>

          {(progress > 0 || isTraining || logs.length > 0) && (
            <div className="card" style={{ marginTop: '24px', backgroundColor: '#0f172a', border: '1px solid #334155' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                <h3 className="card-title" style={{ color: '#e2e8f0', margin: 0 }}>Terminal Output</h3>
                <span style={{ fontSize: '12px', color: '#10b981' }}>{progress}%</span>
              </div>
              
              <div style={{ backgroundColor: '#020617', padding: '12px', borderRadius: '6px', fontFamily: 'monospace', fontSize: '13px', color: '#10b981', minHeight: '150px', maxHeight: '200px', overflowY: 'auto' }}>
                {logs.map((log, i) => (
                  <div key={i} style={{ marginBottom: '6px' }}>{log}</div>
                ))}
                {isTraining && (
                  <div style={{ animation: 'blink 1s infinite' }}>_</div>
                )}
              </div>

              <div className="progress-container" style={{ marginTop: '16px', backgroundColor: '#334155' }}>
                <div className="progress-bar" style={{ width: `${progress}%`, backgroundColor: '#10b981', transition: 'width 0.8s ease' }}></div>
              </div>
            </div>
          )}

          {progress === 100 && !isTraining && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', marginTop: '24px' }}>
              {[
                { name: 'Support Vector Machine', acc: '98.5%', f1: '98.1%' },
                { name: 'Random Forest', acc: '99.2%', f1: '98.9%' },
                { name: 'Spiking Neural Network', acc: '97.8%', f1: '97.4%' }
              ].map(model => (
                <div key={model.name} className="card">
                  <h3 className="card-title">{model.name} Results</h3>
                  <div className="grid-2-col">
                    <div className="result-box">
                      <span className="result-label">Accuracy</span>
                      <span className="result-val">{model.acc}</span>
                    </div>
                    <div className="result-box">
                      <span className="result-label">F1-Score</span>
                      <span className="result-val">{model.f1}</span>
                    </div>
                  </div>
                  <div className="results-actions" style={{ marginTop: '16px', display: 'flex', gap: '12px' }}>
                    <button className="btn btn-secondary" style={{ flex: 1 }}><FileText size={16} /> Report</button>
                    <button className="btn btn-primary" style={{ flex: 1, backgroundColor: 'var(--status-normal)' }}><Activity size={16} /> Use Model</button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default MLStudio;
