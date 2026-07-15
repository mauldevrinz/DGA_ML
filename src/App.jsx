import React, { useState } from 'react';
import { Activity, BrainCircuit, Zap, Info, Menu } from 'lucide-react';
import { useLocalStorage } from './hooks/useLocalStorage';
import Acquisition from './components/Acquisition';
import MLStudio from './components/MLStudio';
import Classification from './components/Classification';
import About from './components/About';

function App() {
  const [currentPage, setCurrentPage] = useLocalStorage('dga_currentPage', 'acquisition');
  const [isConnected, setIsConnected] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useLocalStorage('dga_isSidebarOpen', true);



  return (
    <div className="app-container">
      {/* Top Navbar */}
      <header className="top-navbar">
        <div className="nav-left">
          <button onClick={() => setIsSidebarOpen(!isSidebarOpen)} style={{ marginRight: '12px', display: 'flex', alignItems: 'center' }}>
            <Menu size={24} color="var(--text-primary)" />
          </button>
          <img src="/LOGO NAVBAR.png" alt="Navbar Logo" className="nav-logo" onError={(e) => e.target.style.display='none'} />
        </div>
        
        <div className="nav-text-container" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
          <h1 className="app-title">Dissolved Gas Analysis (DGA) Electronic Nose</h1>
          <span className="app-subtitle">Transformer Fault Diagnosis System</span>
        </div>
        
        <div className={`status-badge ${isConnected ? 'connected' : 'disconnected'}`}>
          <div style={{ width: 8, height: 8, borderRadius: '50%', backgroundColor: isConnected ? 'var(--status-normal)' : 'var(--text-muted)' }}></div>
          {isConnected ? 'Connected' : 'Disconnected'}
        </div>
      </header>

      <div className="main-wrapper">
        {/* Sidebar */}
        <aside className={`sidebar ${isSidebarOpen ? '' : 'collapsed'}`}>
          <div className="sidebar-label">Main Menu</div>
          
          <button 
            className={`nav-button ${currentPage === 'acquisition' ? 'active' : ''}`}
            onClick={() => setCurrentPage('acquisition')}
          >
            <Activity size={18} color={currentPage === 'acquisition' ? 'var(--accent-orange)' : 'currentColor'} /> Data Acquisition
          </button>
          
          <button 
            className={`nav-button ${currentPage === 'ml' ? 'active' : ''}`}
            onClick={() => setCurrentPage('ml')}
          >
            <BrainCircuit size={18} color={currentPage === 'ml' ? 'var(--accent-orange)' : 'currentColor'} /> ML Studio
          </button>
          
          <button 
            className={`nav-button ${currentPage === 'classification' ? 'active' : ''}`}
            onClick={() => setCurrentPage('classification')}
          >
            <Zap size={18} color={currentPage === 'classification' ? 'var(--accent-orange)' : 'currentColor'} /> Classification
          </button>
          
          <div className="spacer"></div>
          
          <button 
            className={`nav-button ${currentPage === 'about' ? 'active' : ''}`}
            onClick={() => setCurrentPage('about')}
          >
            <Info size={18} color={currentPage === 'about' ? 'var(--accent-orange)' : 'currentColor'} /> About
          </button>
        </aside>

        {/* Main Content Area */}
        <main className="content-area">
          <div style={{ display: currentPage === 'acquisition' ? 'block' : 'none', height: '100%' }}>
            <Acquisition isConnected={isConnected} setIsConnected={setIsConnected} />
          </div>
          <div style={{ display: currentPage === 'ml' ? 'block' : 'none', height: '100%' }}>
            <MLStudio />
          </div>
          <div style={{ display: currentPage === 'classification' ? 'block' : 'none', height: '100%' }}>
            <Classification />
          </div>
          <div style={{ display: currentPage === 'about' ? 'block' : 'none', height: '100%' }}>
            <About />
          </div>
        </main>
      </div>
    </div>
  );
}

export default App;
