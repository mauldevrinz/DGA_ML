const fs = require('fs');
let content = fs.readFileSync('/home/petalinux/maulvin/DGA_ML/src/components/Classification.jsx', 'utf8');

content = content.replace(
  "const GAS_TYPES = ['Udara Bersih', 'Asetilena (C2H2)', 'Etilena (C2H4)', 'Hidrogen (H2)', 'Alcohol'];",
  "const GAS_TYPES = ['Udara Bersih', 'Asetilena (C2H2)', 'Etilena (C2H4)', 'Hidrogen (H2)', 'Metana (CH4)', 'Alcohol'];"
);

content = content.replace(
  "const diagnoseIEC60599 = (gasConf) => {\n  const { asetilena, etilena, hidrogen, udara } = gasConf;",
  "const diagnoseIEC60599 = (gasConf) => {\n  const { asetilena, etilena, hidrogen, metana, udara } = gasConf;"
);

content = content.replace(
  "const faultTotal = asetilena + etilena + hidrogen;",
  "const faultTotal = asetilena + etilena + hidrogen + (metana || 0);"
);

content = content.replace(
  "const [prob, setProb] = useState([0, 0, 0, 0, 0]);",
  "const [prob, setProb] = useState([0, 0, 0, 0, 0, 0]);"
);

content = content.replace(
  "let newProb = [0, 0, 0, 0, 0];",
  "let newProb = [0, 0, 0, 0, 0, 0];"
);

content = content.replace(
  "newProb = [85 + Math.random() * 10, noise(), noise(), noise(), noise()];",
  "newProb = [85 + Math.random() * 10, noise(), noise(), noise(), noise(), noise()];"
);

content = content.replace(
  "newProb = [noise(), noise(), 70 + Math.random() * 20, 30 + Math.random() * 20, noise()];",
  "newProb = [noise(), noise(), 70 + Math.random() * 20, 30 + Math.random() * 20, 40 + Math.random() * 20, noise()];"
);

content = content.replace(
  "newProb = [noise(), 75 + Math.random() * 15, noise(), 60 + Math.random() * 20, noise()];",
  "newProb = [noise(), 75 + Math.random() * 15, noise(), 60 + Math.random() * 20, noise(), noise()];"
);

content = content.replace(
  "const gasConf = {\n            udara: newProb[0],\n            asetilena: newProb[1],\n            etilena: newProb[2],\n            hidrogen: newProb[3]\n          };",
  "const gasConf = {\n            udara: newProb[0],\n            asetilena: newProb[1],\n            etilena: newProb[2],\n            hidrogen: newProb[3],\n            metana: newProb[4]\n          };"
);

content = content.replace(
  "const currentGasConf = {\n    udara: prob[0],\n    asetilena: prob[1],\n    etilena: prob[2],\n    hidrogen: prob[3]\n  };",
  "const currentGasConf = {\n    udara: prob[0],\n    asetilena: prob[1],\n    etilena: prob[2],\n    hidrogen: prob[3],\n    metana: prob[4]\n  };"
);

content = content.replace(
  "const colors = ['#75BDE0', '#ef4444', '#f97316', '#eab308', '#94a3b8'];",
  "const colors = ['#75BDE0', '#ef4444', '#f97316', '#eab308', '#8b5cf6', '#94a3b8'];"
);

content = content.replace(
  "const newProb = [Math.random()*20, Math.random()*80, Math.random()*80, Math.random()*80, noise()];\n                          const diag = diagnoseIEC60599({ udara: newProb[0], asetilena: newProb[1], etilena: newProb[2], hidrogen: newProb[3] });",
  "const newProb = [Math.random()*20, Math.random()*80, Math.random()*80, Math.random()*80, Math.random()*80, noise()];\n                          const diag = diagnoseIEC60599({ udara: newProb[0], asetilena: newProb[1], etilena: newProb[2], hidrogen: newProb[3], metana: newProb[4] });"
);

fs.writeFileSync('/home/petalinux/maulvin/DGA_ML/src/components/Classification.jsx', content);
