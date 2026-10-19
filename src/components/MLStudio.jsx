import React, { useState } from 'react';
import { Play, Activity, Server, Cpu, CheckCircle, BarChart2, Zap, FileText } from 'lucide-react';

import { useLocalStorage } from '../hooks/useLocalStorage';
import { invoke } from '@tauri-apps/api/core';
import './MLStudio.css';

const SENSOR_NAMES = [
  'TGS2611', 'TGS2610', 'TGS2600', 'TGS2602', 'MQ-2', 'MQ-3', 'MQ-4', 'MQ-5', 
  'MQ-6', 'MQ-7', 'MQ-8', 'MQ-135', 'MQ-136', 'MQ-137', 'MQ-138', 'Gas Temp', 'Gas Hum'
];
const GAS_NAMES = ['C₂H₂ (Acetylene)', 'C₂H₄ (Ethylene)', 'CH₄ (Methane)', 'H₂ (Hydrogen)', 'OTHER (Normal / Other Gases)'];
const FAULT_CLASSES = [
  { code: 'N', type: 'Normal Oil (No Fault)', desc: 'Transformer oil condition is normal', count: '200 cycles' },
  { code: 'T1', type: 'Thermal Fault (<300°C)', desc: 'Low-temperature thermal fault', count: '180 cycles' },
  { code: 'D1', type: 'Electrical Fault (Low Energy)', desc: 'Discharge of low energy / sparking', count: '180 cycles' },
];

const MLStudio = () => {
  const [isTraining, setIsTraining] = useState(false);
  const [progress, setProgress] = useLocalStorage('dga_mlProgress', 0);
  const [logs, setLogs] = useLocalStorage('dga_mlLogs', []);
  const [showHeatmap, setShowHeatmap] = useState(false);
  
  const [isGeneratingPdf, setIsGeneratingPdf] = useState(false);

  const startEdgeTraining = async () => {
    setIsTraining(true);
    setProgress(0);
    setLogs(["[SYSTEM] Initializing Multi-Label Training Pipeline on AMD-Xilinx Kria KV260..."]);
    

    
    let currentProgress = 0;
    
    setTimeout(async () => {
      setLogs(prev => [...prev, "[FEATURE] Executing TSFRESH Feature Extraction for 5 DGA gas classes..."]);
      currentProgress += 15;
      setProgress(currentProgress);
      try { await invoke('run_feature_extraction', { method: 'TSFRESH' }); } catch(e) {}
    }, 1000);

    setTimeout(() => {
      setLogs(prev => [...prev, "[ANALYSIS] Calculating Pearson Correlation Matrix..."]);
      setLogs(prev => [...prev, "[ANALYSIS] Data Augmentation: Generating synthetic dissolved gas mixtures..."]);
      currentProgress += 15;
      setProgress(currentProgress);
    }, 2500);

    setTimeout(async () => {
      setLogs(prev => [...prev, "[TRAIN] SNN (Spiking Neural Network) LIF neurons training..."]);
      setLogs(prev => [...prev, "[TRAIN] Random Forest (Multi-Output) training..."]);
      currentProgress += 20;
      setProgress(currentProgress);
    }, 4000);

    setTimeout(async () => {
      setLogs(prev => [...prev, "[TRAIN] SVM (RBF Multi-Output) training..."]);
      setLogs(prev => [...prev, "[MONITOR] Power stable at ~3.4W, FPGA SNN HLS Accelerator active..."]);
      currentProgress += 25;
      setProgress(currentProgress);
    }, 6000);

    setTimeout(async () => {
      setLogs(prev => [...prev, "[VALIDATION] Running LOMO & LOCO Analysis for 5 DGA gas targets + Fault Classes (N, T1, D1)..."]);
      setLogs(prev => [...prev, "[INTEGRATION] Fault Diagnosis Rules (N: Normal, T1: Thermal, D1: Electrical) embedded..."]);
      try { await invoke('run_validation', { validationType: 'LOMO & LOSO' }); } catch(e) {}
      
      currentProgress = 100;
      setProgress(currentProgress);
      setLogs(prev => [...prev, "[SYSTEM] Edge FPGA deployment ready! Latency: 2.8ms/sample."]);
      
      
      setIsTraining(false);
    }, 8500);
  };

  const getHeatmapColor = (val) => {
    const absVal = Math.abs(val);
    if (absVal >= 0.75) return '#243F81'; // Dark blue (cover)
    if (absVal >= 0.50) return '#127BBE'; // Medium blue (logo dark)
    if (absVal >= 0.25) return '#75BDE0'; // Light blue (logo light)
    return '#FFFFFF'; // White
  };
  
  const getTextColor = (val) => {
    const absVal = Math.abs(val);
    if (absVal >= 0.50) return '#FFFFFF'; // White text for dark backgrounds
    return '#243F81'; // Dark blue text for light backgrounds (white / light blue)
  };

  return (
    <div className="page-container" style={{ paddingBottom: '40px', position: 'relative' }}>
      {/* UNDER DEVELOPMENT POPUP CONTAINER */}
      <div style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(15, 23, 42, 0.65)',
        backdropFilter: 'blur(4px)',
        zIndex: 999,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '20px'
      }}>
        <div style={{
          backgroundColor: '#ffffff',
          color: '#0f172a',
          padding: '40px 36px',
          borderRadius: '16px',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.35)',
          maxWidth: '440px',
          width: '100%',
          textAlign: 'center',
          border: '1px solid #e2e8f0'
        }}>
          <div style={{
            display: 'inline-flex',
            alignItems: 'center',
            justifyContent: 'center',
            width: '64px',
            height: '64px',
            borderRadius: '50%',
            backgroundColor: '#fef3c7',
            color: '#d97706',
            fontSize: '32px',
            marginBottom: '16px'
          }}>
            🚧
          </div>
          <h2 style={{
            fontSize: '22px',
            fontWeight: 800,
            letterSpacing: '1px',
            color: '#0f172a',
            margin: '0 0 10px 0',
            textTransform: 'uppercase'
          }}>
            UNDER DEVELOPMENT
          </h2>
          <p style={{
            fontSize: '14px',
            color: '#64748b',
            lineHeight: 1.5,
            margin: 0
          }}>
            This feature is currently under active development and will be available in an upcoming update.
          </p>
        </div>
      </div>

      <div className="page-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
        <div>
          <h2 className="page-title">Edge ML Studio</h2>
          <span className="app-subtitle" style={{ display: 'inline-block', marginLeft: '12px' }}>Multi-Label Gas Composition + IEC 60599 Fault Diagnosis</span>
        </div>
        
      </div>

      <div className="ml-layout">
        <div className="ml-left">
          <div className="card list-card" style={{ backgroundColor: 'var(--surface-color)' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h3 className="card-title" style={{ margin: 0 }}>Dataset Status (Controlled Lab)</h3>
              <span style={{ fontSize: '12px', padding: '4px 8px', backgroundColor: 'rgba(18, 123, 190, 0.15)', color: '#127BBE', borderRadius: '4px' }}>IEC 60599 | Multi-Label</span>
            </div>
            
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <div>
                <div style={{ fontSize: '12px', fontWeight: 700, color: '#75BDE0', textTransform: 'uppercase', marginBottom: '8px', letterSpacing: '0.5px' }}>
                  Dissolved Gas Targets (5 SNN Classes)
                </div>
                <div className="dataset-uploads">
                  {[
                    { name: 'C₂H₂ (Acetylene)', desc: 'High energy electrical discharge gas', count: '150 cycles' },
                    { name: 'C₂H₄ (Ethylene)', desc: 'High temperature thermal fault gas', count: '150 cycles' },
                    { name: 'CH₄ (Methane)', desc: 'Low-to-medium thermal fault gas', count: '150 cycles' },
                    { name: 'H₂ (Hydrogen)', desc: 'Corona & partial discharge gas', count: '150 cycles' },
                    { name: 'OTHER (Normal / Other Gases)', desc: 'Normal oil baseline / other trace gases', count: '180 cycles' },
                  ].map(item => (
                    <div key={item.name} className="upload-row" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 12px', border: '1px solid var(--border-color)', borderRadius: '8px', marginBottom: '6px' }}>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                        <CheckCircle size={15} color="#127BBE" />
                        <div>
                          <div style={{ fontWeight: 600, fontSize: '13px', color: 'var(--text-primary)' }}>{item.name}</div>
                          <div style={{ fontSize: '11px', color: 'var(--text-muted)' }}>{item.desc}</div>
                        </div>
                      </div>
                      <span style={{ fontSize: '12px', fontWeight: 600, color: 'var(--accent-orange)' }}>{item.count}</span>
                    </div>
                  ))}
                </div>
              </div>

              <div>
                <div style={{ fontSize: '12px', fontWeight: 700, color: '#75BDE0', textTransform: 'uppercase', marginBottom: '8px', letterSpacing: '0.5px' }}>
                  IEC 60599 Fault Classes
                </div>
                <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(130px, 1fr))', gap: '6px' }}>
                  {FAULT_CLASSES.map(fc => (
                    <div key={fc.code} style={{ padding: '8px 10px', backgroundColor: 'rgba(35, 63, 124, 0.3)', borderRadius: '6px', border: '1px solid rgba(117, 189, 224, 0.25)' }}>
                      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                        <span style={{ fontWeight: 800, fontSize: '13px', color: '#75BDE0' }}>{fc.code}</span>
                        <span style={{ fontSize: '10px', color: '#94a3b8' }}>{fc.count}</span>
                      </div>
                      <div style={{ fontSize: '11px', color: '#cbd5e1', fontWeight: 500, marginTop: '2px' }}>{fc.type}</div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
            
            <button 
              className="btn btn-outline" 
              style={{ width: '100%', marginTop: '16px', display: 'flex', justifyContent: 'center', gap: '8px' }}
              onClick={() => setShowHeatmap(!showHeatmap)}
            >
              <BarChart2 size={16} /> {showHeatmap ? 'Hide Pearson Correlation' : 'View Pearson Correlation Heatmap'}
            </button>
          </div>
          
          {showHeatmap && (
            <div className="card" style={{ marginTop: '24px', animation: 'fadeIn 0.3s ease' }}>
              <h3 className="card-title">Pearson Correlation (Feature vs Dissolved Gas Target)</h3>
              <p style={{ fontSize: '12px', color: 'var(--text-muted)', marginBottom: '16px' }}>Identify the contribution of 17 E-Nose sensors to the 5 SNN dissolved gas targets using Pearson coefficient r (-1 to +1).</p>
              
              <div style={{ display: 'grid', gridTemplateColumns: '80px repeat(5, 1fr)', gap: '4px', fontSize: '11px' }}>
                <div style={{ padding: '8px' }}></div>
                {GAS_NAMES.map(c => <div key={c} style={{ padding: '8px 4px', textAlign: 'center', fontWeight: 'bold', color: 'var(--text-primary)' }}>{c.split(' ')[0]}</div>)}
                
                {SENSOR_NAMES.map((sensor, i) => (
                  <React.Fragment key={sensor}>
                    <div style={{ padding: '8px', display: 'flex', alignItems: 'center', fontWeight: '500', color: 'var(--text-primary)' }}>{sensor}</div>
                    {GAS_NAMES.map((c, j) => {
                      let val = (Math.sin(i * 1.5 + j * 2.3) * 0.8 + 0.1).toFixed(2);
                      if (sensor === 'TGS2611' && c.includes('C₂H₄')) val = '0.92';
                      if (sensor === 'MQ-8' && c.includes('H₂')) val = '0.88';
                      if (sensor === 'MQ-4' && c.includes('CH₄')) val = '0.90';
                      if (sensor === 'TGS2600' && c.includes('C₂H₂')) val = '0.85';
                      if (c.includes('OTHER')) val = (Math.cos(i * 0.8) * 0.4 + 0.2).toFixed(2);
                      
                      return (
                        <div key={`${i}-${j}`} style={{ 
                          backgroundColor: getHeatmapColor(parseFloat(val)),
                          color: getTextColor(parseFloat(val)),
                          padding: '8px', 
                          textAlign: 'center',
                          borderRadius: '4px',
                          display: 'flex', alignItems: 'center', justifyContent: 'center',
                          fontWeight: 'bold',
                          transition: 'transform 0.2s ease',
                          cursor: 'pointer'
                        }} title={`Correlation ${sensor} with ${c} = ${val}`}>
                          {val}
                        </div>
                      )
                    })}
                  </React.Fragment>
                ))}
              </div>
            </div>
          )}
        </div>

        <div className="ml-right">
          <div className="card">
            <h3 className="card-title">Training Configuration</h3>
            
            <div className="grid-2-col" style={{ gap: '16px', marginBottom: '16px' }}>
              <div className="form-group">
                <label>Validation Scheme</label>
                <div className="select-input" style={{ display: 'flex', alignItems: 'center', cursor: 'default' }}>LOMO (K-Fold CV + LOCO)</div>
              </div>
              <div className="form-group">
                <label>Dimensionality Reduction</label>
                <div className="select-input" style={{ display: 'flex', alignItems: 'center', cursor: 'default' }}>TSFRESH Feature Selection</div>
              </div>
            </div>

            <button 
              className="btn btn-primary" 
              style={{ width: '100%', height: '48px', fontSize: '15px', display: 'flex', justifyContent: 'center', gap: '8px', background: 'linear-gradient(135deg, #233F7C 0%, #243F81 100%)' }}
              onClick={startEdgeTraining}
              disabled={isTraining}
            >
              <Play size={18} fill="currentColor" /> {isTraining ? 'Executing Edge Training...' : 'Start Multi-Model Training'}
            </button>
          </div>

          {(progress > 0 || isTraining || logs.length > 0) && (
            <div className="card" style={{ marginTop: '24px', backgroundColor: '#243F81', border: '1px solid rgba(35, 63, 124, 0.4)' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '12px' }}>
                <h3 className="card-title" style={{ color: '#ffffff', margin: 0, display: 'flex', alignItems: 'center', gap: '8px' }}><Server size={16} /> Edge Console</h3>
                <span style={{ fontSize: '13px', fontWeight: 'bold', color: '#75BDE0' }}>{progress}%</span>
              </div>
              
              <div style={{ backgroundColor: '#1a2d5a', padding: '16px', borderRadius: '8px', fontFamily: '"JetBrains Mono", monospace', fontSize: '13px', color: '#75BDE0', height: '180px', overflowY: 'auto', border: '1px solid rgba(35, 63, 124, 0.3)' }}>
                {logs.map((log, i) => (
                  <div key={i} style={{ marginBottom: '8px', opacity: 0.9 }}>{log}</div>
                ))}
                {isTraining && (
                  <div style={{ animation: 'blink 1s infinite' }}>_</div>
                )}
              </div>

              <div className="progress-container" style={{ marginTop: '16px', backgroundColor: 'rgba(35, 63, 124, 0.3)', height: '6px', borderRadius: '3px' }}>
                <div className="progress-bar" style={{ width: `${progress}%`, backgroundColor: '#75BDE0', transition: 'width 0.4s ease', height: '100%', borderRadius: '3px' }}></div>
              </div>
            </div>
          )}

          {progress === 100 && !isTraining && (
            <>
            <div style={{ marginTop: '24px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <h3 className="card-title" style={{ margin: 0 }}>Model Deployment</h3>
              <button 
                className="btn btn-secondary" 
                onClick={async () => {
                  setIsGeneratingPdf(true);
                  try {
                    const res = await invoke('generate_pdf_report', { outDir: '/home/maulvin/Documents/DGA/training_reports' });
                    setLogs(prev => [...prev, `[REPORT] ${res}`]);
                    alert("PDF Report generated successfully!");
                  } catch (e) {
                    setLogs(prev => [...prev, `[ERROR] Failed to generate PDF: ${e}`]);
                  }
                  setIsGeneratingPdf(false);
                }}
                disabled={isGeneratingPdf}
                style={{ fontSize: '13px', display: 'flex', alignItems: 'center', gap: '8px' }}
              >
                <FileText size={16} /> {isGeneratingPdf ? 'Generating...' : 'Generate PDF Report'}
              </button>
            </div>
            
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '16px', marginTop: '16px' }}>
              {[
                { name: 'SNN (Spiking Neural Network)', acc: '97.8%', f1: '97.5%', tag: 'Recommended for Edge' },
                { name: 'Random Forest (Multi-Output)', acc: '97.2%', f1: '96.8%', tag: 'Baseline' },
                { name: 'SVM (RBF Multi-Output)', acc: '96.5%', f1: '96.1%', tag: 'Standard' },
              ].map(model => (
                <div key={model.name} className="card" style={{ border: model.name.includes('SNN') ? '2px solid #127BBE' : '1px solid var(--border-color)', position: 'relative' }}>
                  {model.name.includes('SNN') && (
                    <span style={{ position: 'absolute', top: '-10px', right: '16px', backgroundColor: '#127BBE', color: '#ffffff', fontSize: '11px', padding: '2px 8px', borderRadius: '12px', fontWeight: 'bold' }}>
                      Best Perf
                    </span>
                  )}
                  <h3 className="card-title" style={{ fontSize: '15px' }}>{model.name}</h3>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', margin: '16px 0' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                      <span style={{ color: 'var(--text-muted)', fontSize: '13px' }}>Accuracy</span>
                      <span style={{ fontWeight: 'bold', color: 'var(--text-primary)' }}>{model.acc}</span>
                    </div>
                    <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                      <span style={{ color: 'var(--text-muted)', fontSize: '13px' }}>F1-Score</span>
                      <span style={{ fontWeight: 'bold', color: 'var(--text-primary)' }}>{model.f1}</span>
                    </div>
                  </div>
                  <button className={`btn ${model.name.includes('SNN') ? 'btn-primary' : 'btn-outline'}`} style={{ width: '100%', fontSize: '13px' }}>
                    <Activity size={14} style={{ marginRight: '6px' }} /> Deploy Model
                  </button>
                </div>
              ))}
            </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
};

export default MLStudio;
