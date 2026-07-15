import React, { useState } from 'react';
import { Play, Square } from 'lucide-react';
import './Classification.css';

const Classification = () => {
  const [isClassifying, setIsClassifying] = useState(false);

  const toggle = () => setIsClassifying(!isClassifying);

  return (
    <div className="page-container">
      <div className="page-header">
        <h2 className="page-title">Real-Time Classification</h2>
      </div>

      <div className="cls-layout">
        <div className="cls-left">
          <div className="card">
            <h3 className="card-title">Configuration</h3>
            <div className="form-group">
              <label>Model</label>
              <select className="select-input">
                <option>SVM - 2026-06-09</option>
              </select>
            </div>
            
            <button 
              className={`btn ${isClassifying ? 'btn-danger' : 'btn-primary'}`} 
              style={{ width: '100%', marginTop: '24px' }}
              onClick={toggle}
            >
              {isClassifying ? <Square size={16} /> : <Play size={16} />}
              {isClassifying ? ' Stop' : ' Start Classification'}
            </button>
          </div>
          
          <div className="card" style={{ marginTop: '24px' }}>
            <h3 className="card-title">History</h3>
            <div className="empty-box" style={{ height: '200px' }}>No predictions yet.</div>
          </div>
        </div>

        <div className="cls-right">
          <div className="card status-panel">
            <div className="status-indicator" style={{ backgroundColor: isClassifying ? 'var(--status-normal)' : 'var(--text-muted)' }}></div>
            <h1 className="status-text">{isClassifying ? 'NORMAL' : 'STANDBY'}</h1>
            <p className="status-conf">{isClassifying ? 'Confidence: 98.5%' : 'Waiting...'}</p>
          </div>

          <div className="card" style={{ marginTop: '24px' }}>
            <h3 className="card-title">Probabilities</h3>
            <div className="prob-list">
              {['Baseline', 'Normal', 'Overheating', 'Arcing'].map((label, idx) => {
                const val = isClassifying ? (idx === 1 ? 98.5 : idx === 0 ? 1.2 : 0) : 0;
                return (
                  <div key={label} className="prob-item">
                    <div className="prob-info">
                      <span>{label}</span>
                      <span>{val.toFixed(1)}%</span>
                    </div>
                    <div className="progress-container">
                      <div className="progress-bar" style={{ width: `${val}%` }}></div>
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
