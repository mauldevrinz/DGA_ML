import React from 'react';
import './About.css';
import { User, GraduationCap, Award } from 'lucide-react';

const About = () => {
  return (
    <div className="page-container" style={{ position: 'relative', minHeight: 'calc(100vh - 160px)' }}>
      {/* UNDER DEVELOPMENT OVERLAY (Scoped to page content, sidebar remains fully accessible) */}
      <div style={{
        position: 'absolute',
        inset: 0,
        backgroundColor: 'rgba(15, 23, 42, 0.45)',
        backdropFilter: 'blur(3px)',
        zIndex: 20,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: '24px',
        borderRadius: '16px'
      }}>
        <div style={{
          backgroundColor: '#ffffff',
          color: '#0f172a',
          padding: '44px 36px',
          borderRadius: '16px',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.35)',
          maxWidth: '460px',
          width: '100%',
          textAlign: 'center',
          border: '1px solid #e2e8f0'
        }}>
          <div style={{
            display: 'inline-flex',
            alignItems: 'center',
            justifyContent: 'center',
            width: '68px',
            height: '68px',
            borderRadius: '50%',
            backgroundColor: '#fef3c7',
            color: '#d97706',
            fontSize: '34px',
            marginBottom: '18px'
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
            lineHeight: 1.6,
            margin: 0
          }}>
            This page is currently under active development. You can navigate to other menus via the sidebar.
          </p>
        </div>
      </div>

      <div className="page-header">
        <h2 className="page-title">About Project</h2>
      </div>

      <div className="about-wrapper">
        <div className="card about-header-card">
          <div className="logos-container">
            <img src="/LOGO ITS.png" alt="ITS" className="inst-logo" onError={(e) => e.target.style.display='none'} />
            <img src="/ELKA LOGO.jpg" alt="ELKA" className="inst-logo" onError={(e) => e.target.style.display='none'} />
          </div>
          <div className="title-container">
            <span className="title-badge">FINAL PROJECT THESIS</span>
            <h1 className="project-title">DGA Electronic Nose</h1>
            <p className="project-subtitle">Transformer Oil Fault Diagnosis System</p>
          </div>
        </div>

        <div className="card about-profile-card">
          <div className="profile-img-container">
            <img src="/Profile.jpeg" alt="Akhmad Maulvin" className="profile-photo" onError={(e) => e.target.style.display='none'} />
            <div className="profile-backdrop"></div>
          </div>
          
          <div className="profile-details">
            <div className="detail-section">
              <span className="section-label"><User size={14} /> RESEARCHER</span>
              <h2 className="student-name">Akhmad Maulvin Nazir Zakaria</h2>
              <span className="student-id">NRP. 2042231028</span>
            </div>

            <div className="detail-separator"></div>

            <div className="detail-section">
              <span className="section-label"><GraduationCap size={14} /> DEPARTMENT</span>
              <p className="dept-text">
                <strong style={{ color: 'var(--text-primary)' }}>D-4 Instrumentation Engineering Technology</strong><br/>
                Department of Instrumentation Engineering<br/>
                Faculty of Vocational Studies<br/>
                Institut Teknologi Sepuluh Nopember (ITS)
              </p>
            </div>

            <div className="detail-separator"></div>

            <div className="detail-section">
              <span className="section-label"><Award size={14} /> SUPERVISOR</span>
              <h3 className="supervisor-name">Ahmad Radhy, S.Si., M.Si</h3>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default About;
