const fs = require('fs');
let content = fs.readFileSync('/home/petalinux/maulvin/DGA_ML/src/components/MLStudio.jsx', 'utf8');

content = content.replace(
  "const GAS_NAMES = ['Udara Bersih', 'Asetilena', 'Etilena', 'Hidrogen', 'Alcohol'];",
  "const GAS_NAMES = ['Udara Bersih', 'Asetilena', 'Etilena', 'Hidrogen', 'Metana', 'Alcohol'];"
);

content = content.replace(
  "{ name: 'Hidrogen (H2)', desc: 'Hydrogen standard gas', count: '150 cycles' },",
  "{ name: 'Hidrogen (H2)', desc: 'Hydrogen standard gas', count: '150 cycles' },\n                { name: 'Metana (CH4)', desc: 'Methane standard gas', count: '150 cycles' },"
);

content = content.replace(
  "Executing TSFRESH Feature Extraction for 5 gas classes",
  "Executing TSFRESH Feature Extraction for 6 gas classes"
);

content = content.replace(
  "LOMO & LOCO Analysis for 5 gas types",
  "LOMO & LOCO Analysis for 6 gas types"
);

content = content.replace(
  "if (sensor === 'MQ-3' && c === 'Alcohol') val = '0.95';",
  "if (sensor === 'MQ-3' && c === 'Alcohol') val = '0.95';\n                      if (sensor === 'MQ-4' && c === 'Metana') val = '0.90';"
);

content = content.replace(
  "gridTemplateColumns: '80px repeat(5, 1fr)'",
  "gridTemplateColumns: '80px repeat(6, 1fr)'"
);

fs.writeFileSync('/home/petalinux/maulvin/DGA_ML/src/components/MLStudio.jsx', content);
