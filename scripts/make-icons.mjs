// Renders assets/icon/timetrek.svg to a 1024px PNG and lets `tauri icon`
// generate every platform icon (ico/icns/png) into src-tauri/icons.
import { Resvg } from '@resvg/resvg-js';
import { readFileSync, writeFileSync } from 'node:fs';
import { execSync } from 'node:child_process';

const svg = readFileSync('assets/icon/timetrek.svg');
const png = new Resvg(svg, { fitTo: { mode: 'width', value: 1024 } }).render().asPng();
writeFileSync('assets/icon/timetrek.png', png);
execSync('npx tauri icon assets/icon/timetrek.png', { stdio: 'inherit' });
