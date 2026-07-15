import React from 'react';
import './About.css';
import { User, GraduationCap, Award } from 'lucide-react';

const About = () => {
  return (
    <div className="page-container">
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
            <span className="title-badge">PROPOSAL TUGAS AKHIR</span>
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
                <strong style={{ color: 'var(--text-primary)' }}>D-4 Teknologi Rekayasa Instrumentasi</strong><br/>
                Departemen Teknik Instrumentasi<br/>
                Fakultas Vokasi<br/>
                Institut Teknologi Sepuluh Nopember
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
