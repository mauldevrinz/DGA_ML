const fs = require('fs');
let content = fs.readFileSync('/home/petalinux/maulvin/DGA_ML/src/components/MLStudio.jsx', 'utf8');

// Remove the UI elements for power metrics
const powerUI = `<div style={{ display: 'flex', gap: '16px' }}>
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
        </div>`;

content = content.replace(powerUI, "");

// Remove the state and interval logic to clean up
content = content.replace("const [powerMetrics, setPowerMetrics] = useState({ pwr: 0, temp: 0, ram: 0 });", "");

content = content.replace(`    // Simulate Power/Thermal monitor spikes during intense load
    const powerInterval = setInterval(() => {
      setPowerMetrics({
        pwr: (12 + Math.random() * 8).toFixed(1), // 12-20W
        temp: (55 + Math.random() * 10).toFixed(1), // 55-65C
        ram: (3.2 + Math.random() * 1.5).toFixed(1) // 3.2-4.7GB
      });
    }, 500);`, "");

content = content.replace("clearInterval(powerInterval);", "");
content = content.replace("setPowerMetrics({ pwr: 4.2, temp: 48.0, ram: 2.1 }); // Return to idle", "");

fs.writeFileSync('/home/petalinux/maulvin/DGA_ML/src/components/MLStudio.jsx', content);
