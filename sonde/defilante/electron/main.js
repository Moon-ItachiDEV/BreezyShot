// Cible Electron de la sonde défilante : une fenêtre sur la page témoin.
// SONDE_URL : l'adresse file:///… de la page ; SONDE_RECT : « x,y,l,h » en pixels.
'use strict';
const { app, BrowserWindow } = require('electron');

const [x, y, l, h] = String(process.env.SONDE_RECT || '0,0,700,720').split(',').map(Number);
app.whenReady().then(() => {
  const f = new BrowserWindow({ x, y, width: l, height: h, autoHideMenuBar: true, show: true });
  f.loadURL(String(process.env.SONDE_URL || 'about:blank'));
});
app.on('window-all-closed', () => app.quit());
