import fs from 'fs';
import path from 'path';

// Read version and product name from tauri.conf.json
const tauriConfigPath = path.resolve('src-tauri/tauri.conf.json');
const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf-8'));
const version = tauriConfig.version;
const productName = tauriConfig.productName || 'app';

// Find sig file in target/release/bundle/
const bundleDir = path.resolve('src-tauri/target/release/bundle');

function findFiles(dir, ext) {
  let results = [];
  if (!fs.existsSync(dir)) return results;
  const list = fs.readdirSync(dir);
  for (const file of list) {
    const filePath = path.join(dir, file);
    const stat = fs.statSync(filePath);
    if (stat && stat.isDirectory()) {
      results = results.concat(findFiles(filePath, ext));
    } else if (file.endsWith(ext)) {
      results.push(filePath);
    }
  }
  return results;
}

const sigFiles = findFiles(bundleDir, '.sig');
console.log('Found signature files:', sigFiles);

if (sigFiles.length === 0) {
  console.error('No .sig files found in bundle directory:', bundleDir);
  process.exit(1);
}

const platforms = {};

for (const sigPath of sigFiles) {
  const signature = fs.readFileSync(sigPath, 'utf-8').trim();
  const zipPath = sigPath.slice(0, -4); // Remove .sig
  const zipName = path.basename(zipPath);
  
  // Determine updater target platform based on filename
  let platformKey = '';
  if (zipName.includes('x64') || zipName.includes('x86_64')) {
    platformKey = 'windows-x86_64';
  } else if (zipName.includes('x86') || zipName.includes('i686')) {
    platformKey = 'windows-i686';
  } else if (zipName.includes('arm64')) {
    platformKey = 'windows-arm64';
  } else {
    // Default fallback
    platformKey = 'windows-x86_64';
  }
  
  platforms[platformKey] = {
    signature: signature,
    url: `https://github.com/slawk0/Jarvis/releases/download/v${version}/${zipName}`
  };
}

const updaterJson = {
  version: version,
  notes: `Automated release for version ${version}`,
  pub_date: new Date().toISOString(),
  platforms: platforms
};

fs.writeFileSync('updater.json', JSON.stringify(updaterJson, null, 2), 'utf-8');
console.log('Successfully generated updater.json at root:', updaterJson);
