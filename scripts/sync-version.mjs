import fs from 'fs';
import path from 'path';

const pkgPath = path.resolve('package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));

const tauriConfPath = path.resolve('src-tauri/tauri.conf.json');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf-8'));

tauriConf.version = pkg.version;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2), 'utf-8');
console.log(`Synced version ${pkg.version} to tauri.conf.json`);
