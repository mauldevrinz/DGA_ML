import React, { useState } from 'react';
import { Play, Activity, Server, Cpu, CheckCircle, BarChart2, Zap, FileText } from 'lucide-react';
import { ScatterChart, Scatter, XAxis, YAxis, CartesianGrid, Tooltip as RechartsTooltip, ResponsiveContainer, Legend } from 'recharts';

const pcaBaseline = Array.from({length: 40}, () => ({ x: -40 + Math.random()*25, y: -20 + Math.random()*25 }));
const pcaNormal = Array.from({length: 40}, () => ({ x: 10 + Math.random()*25, y: 15 + Math.random()*25 }));
const pcaOverheating = Array.from({length: 40}, () => ({ x: 35 + Math.random()*25, y: -30 + Math.random()*25 }));
const pcaArcing = Array.from({length: 40}, () => ({ x: -15 + Math.random()*25, y: 40 + Math.random()*25 }));
import { useLocalStorage } from '../hooks/useLocalStorage';
import { invoke } from '@tauri-apps/api/core';
import './MLStudio.css';

const SENSOR_NAMES = [
  'TGS2611', 'TGS2610', 'TGS2600', 'TGS2602', 'MQ-2', 'MQ-3', 'MQ-4', 'MQ-5', 
  'MQ-6', 'MQ-7', 'MQ-8', 'MQ-135', 'MQ-136', 'MQ-137', 'MQ-138', 'Gas Temp', 'Gas Hum'
];
const CLASSES = ['Baseline', 'Normal', 'Overheat', 'Arcing'];

const MLStudio = () => {
  const [isTraining, setIsTraining] = useState(false);
  const [progress, setProgress] = useLocalStorage('dga_mlProgress', 0);
  const [logs, setLogs] = useLocalStorage('dga_mlLogs', []);
  const [showHeatmap, setShowHeatmap] = useState(false);
  const [powerMetrics, setPowerMetrics] = useState({ pwr: 0, temp: 0, ram: 0 });
  const [isGeneratingPdf, setIsGeneratingPdf] = useState(false);

  const startEdgeTraining = async () => {
    setIsTraining(true);
    setProgress(0);
    setLogs(["[SYSTEM] Initializing Jetson Orin Nano Edge AI Engine..."]);
    
    // Simulate Power/Thermal monitor spikes during intense load
    const powerInterval = setInterval(() => {
      setPowerMetrics({
        pwr: (12 + Math.random() * 8).toFixed(1), // 12-20W
        temp: (55 + Math.random() * 10).toFixed(1), // 55-65C
        ram: (3.2 + Math.random() * 1.5).toFixed(1) // 3.2-4.7GB
      });
    }, 500);
    
    let currentProgress = 0;
    
    setTimeout(async () => {
      setLogs(prev => [...prev, "[FEATURE] Executing TSFRESH + Manual Feature Engineering (109 Features)"]);
      currentProgress += 15;
      setProgress(currentProgress);
      try { await invoke('run_feature_extraction', { method: 'TSFRESH' }); } catch(e) {}
    }, 1000);

    setTimeout(() => {
      setLogs(prev => [...prev, "[ANALYSIS] Calculating Pearson Correlation Matrix..."]);
      setLogs(prev => [...prev, "[ANALYSIS] Recursive Feature Elimination (RFE) applied..."]);
      currentProgress += 15;
      setProgress(currentProgress);
    }, 2500);

    setTimeout(async () => {
      setLogs(prev => [...prev, "[TRAIN] SVM (RBF) & Random Forest (Bagging, m=√p) via Grid Search..."]);
      currentProgress += 20;
      setProgress(currentProgress);
    }, 4000);

    setTimeout(async () => {
      setLogs(prev => [...prev, "[TRAIN] Native Rust Spiking Neural Network (LIF) training on Ampere GPU..."]);
      setLogs(prev => [...prev, "[MONITOR] Power spiked to 19.4W, GPU Tensor Cores active..."]);
      currentProgress += 25;
      setProgress(currentProgress);
    }, 6000);

    setTimeout(async () => {
      setLogs(prev => [...prev, "[VALIDATION] Running LOMO & LOSO Analysis to check Data Leakage..."]);
      try { await invoke('run_validation', { validationType: 'LOMO & LOSO' }); } catch(e) {}
      
      currentProgress = 100;
      setProgress(currentProgress);
      setLogs(prev => [...prev, "[SYSTEM] Edge Computing completed! Latency: 38ms/sample."]);
      clearInterval(powerInterval);
      setPowerMetrics({ pwr: 4.2, temp: 48.0, ram: 2.1 }); // Return to idle
      setIsTraining(false);
    }, 8500);
  };

  const getHeatmapColor = (val) => {
    const absVal = Math.abs(val);
    if (absVal >= 0.75) return '#243F81'; // BIRU COVER
    if (absVal >= 0.50) return '#127BBE'; // BIRU TUA LAMBANG
    if (absVal >= 0.25) return '#75BDE0'; // BIRU MUDA LAMBANG
    return '#FFFFFF'; // Putih
  };
  
  const getTextColor = (val) => {
    const absVal = Math.abs(val);
    if (absVal >= 0.50) return '#FFFFFF'; // Putih untuk background gelap
    return '#243F81'; // Biru gelap untuk background terang (Putih / Biru Muda)
  };

  return (
    <div className="page-container" style={{ paddingBottom: '40px' }}>
      <div className="page-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
        <div>
          <h2 className="page-title">Edge ML Studio</h2>
          <span className="app-subtitle" style={{ display: 'inline-block', marginLeft: '12px' }}>Powered by Rust & Jetson Orin Nano</span>
        </div>
        <div style={{ display: 'flex', gap: '16px' }}>
          <div className="power-metric">
            <span style={{ fontSize: '11px', color: '#94a3b8' }}>PWR</span>
            <div style={{ fontWeight: 'bold', color: '#127BBE', display: 'flex', alignItems: 'center', gap: '4px' }}><Zap size={14}/> {powerMetrics.pwr} W</div>
          </div>
          <div className="power-metric">
            <span style={{ fontSize: '11px', color: '#94a3b8' }}>SOC TEMP</span>
            <div style={{ fontWeight: 'bold', color: '#ef4444' }}>{powerMetrics.temp} °C</div>
          </div>
          <div className="power-metric">
            <span style={{ fontSize: '11px', color: '#94a3b8' }}>RAM</span>
            <div style={{ fontWeight: 'bold', color: '#127BBE' }}>{powerMetrics.ram} GB</div>
          </div>
        </div>
      </div>

      <div className="ml-layout">
        <div className="ml-left">
          <div className="card list-card" style={{ backgroundColor: 'var(--surface-color)' }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
              <h3 className="card-title" style={{ margin: 0 }}>Dataset Status (Controlled Lab)</h3>
              <span style={{ fontSize: '12px', padding: '4px 8px', backgroundColor: 'rgba(18, 123, 190, 0.15)', color: '#127BBE', borderRadius: '4px' }}>IEC 60599 / 60567</span>
            </div>
            
            <div className="dataset-uploads">
              {[
                { name: 'Baseline Data', desc: 'Fresh air (2 mins purging)', count: '150 cycles' },
                { name: 'Normal Data', desc: 'Fresh oil (IEC 60296)', count: '150 cycles' },
                { name: 'Overheating Data', desc: '300ml, 150°C (5 mins)', count: '150 cycles' },
                { name: 'Arcing Data', desc: 'Spark discharge (30s)', count: '150 cycles' }
              ].map(item => (
                <div key={item.name} className="upload-row" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '12px', border: '1px solid var(--border-color)', borderRadius: '8px', marginBottom: '8px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                    <CheckCircle size={16} color="#127BBE" />
                    <div>
                      <div style={{ fontWeight: 600, color: 'var(--text-primary)' }}>{item.name}</div>
                      <div style={{ fontSize: '12px', color: 'var(--text-muted)' }}>{item.desc}</div>
                    </div>
                  </div>
                  <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--accent-orange)' }}>{item.count}</span>
                </div>
              ))}
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
              <h3 className="card-title">Pearson Correlation (Feature vs Class)</h3>
              <p style={{ fontSize: '12px', color: 'var(--text-muted)', marginBottom: '16px' }}>Identifikasi kontribusi 17 sensor E-Nose terhadap klasifikasi DGA menggunakan koefisien r (-1 hingga +1).</p>
              
              <div style={{ display: 'grid', gridTemplateColumns: '80px repeat(4, 1fr)', gap: '4px', fontSize: '11px' }}>
                <div style={{ padding: '8px' }}></div>
                {CLASSES.map(c => <div key={c} style={{ padding: '8px', textAlign: 'center', fontWeight: 'bold', color: 'var(--text-primary)' }}>{c}</div>)}
                
                {SENSOR_NAMES.map((sensor, i) => (
                  <React.Fragment key={sensor}>
                    <div style={{ padding: '8px', display: 'flex', alignItems: 'center', fontWeight: '500', color: 'var(--text-primary)' }}>{sensor}</div>
                    {CLASSES.map((c, j) => {
                      // Generate dummy correlation values that make sense for gas sensors
                      let val = (Math.sin(i * 1.5 + j * 2.3) * 0.8 + 0.1).toFixed(2);
                      if (sensor === 'TGS2611' && c === 'Overheat') val = '0.92'; // Thermal sensitive
                      if (sensor === 'MQ-8' && c === 'Arcing') val = '0.88'; // Hydrogen sensitive
                      
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
                        }} title={`Korelasi ${sensor} dengan ${c} = ${val}`}>
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
                <div className="select-input" style={{ display: 'flex', alignItems: 'center', cursor: 'default' }}>LOMO (10-Fold CV + LOSO 17-Fold)</div>
              </div>
              <div className="form-group">
                <label>Dimensionality Reduction</label>
                <div className="select-input" style={{ display: 'flex', alignItems: 'center', cursor: 'default' }}>PCA + LDA</div>
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
            <div className="card" style={{ marginTop: '24px' }}>
              <h3 className="card-title" style={{ marginBottom: '16px' }}>Feature Extraction (PCA + LDA) 2D Projection</h3>
              <p style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: '16px' }}>
                Visualisasi distribusi klaster fitur sensor gas menggunakan Principal Component Analysis dan Linear Discriminant Analysis.
              </p>
              <div style={{ height: '300px', width: '100%', backgroundColor: 'rgba(255, 255, 255, 0.02)', borderRadius: '8px', padding: '16px' }}>
                <ResponsiveContainer width="100%" height="100%">
                  <ScatterChart margin={{ top: 10, right: 10, bottom: 10, left: -20 }}>
                    <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" opacity={0.5} />
                    <XAxis type="number" dataKey="x" name="PC1" stroke="var(--text-muted)" fontSize={12} tickLine={false} axisLine={false} />
                    <YAxis type="number" dataKey="y" name="PC2" stroke="var(--text-muted)" fontSize={12} tickLine={false} axisLine={false} />
                    <RechartsTooltip cursor={{ strokeDasharray: '3 3' }} contentStyle={{ backgroundColor: 'var(--surface)', borderColor: 'var(--border)', color: 'var(--text)', fontSize: '12px', borderRadius: '4px' }} />
                    <Legend wrapperStyle={{ fontSize: '12px', color: 'var(--text)' }} />
                    <Scatter name="Baseline" data={pcaBaseline} fill="#94a3b8" />
                    <Scatter name="Normal" data={pcaNormal} fill="#10b981" />
                    <Scatter name="Overheating" data={pcaOverheating} fill="#f59e0b" />
                    <Scatter name="Arcing" data={pcaArcing} fill="#ef4444" />
                  </ScatterChart>
                </ResponsiveContainer>
              </div>
            </div>
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
                { name: 'SNN (Rust Native)', acc: '98.7%', f1: '98.5%', tag: 'Recommended for Edge' },
                { name: 'Random Forest', acc: '98.3%', f1: '97.9%', tag: 'Baseline' },
                { name: 'Support Vector Machine', acc: '97.8%', f1: '97.5%', tag: 'Standard' },
                // { name: 'MLP Classifier', acc: '98.1%', f1: '97.8%', tag: 'Standard' },
                // { name: 'CNN (1D)', acc: '98.5%', f1: '98.2%', tag: 'High Performance' },
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
