import { execSync } from 'node:child_process';

try {
  if (process.platform === 'win32') {
    const out = execSync('netstat -ano -p tcp', { encoding: 'utf-8' });
    for (const line of out.split('\n')) {
      if (line.includes(':5173') && line.includes('LISTENING')) {
        const parts = line.trim().split(/\s+/);
        const pid = parts[parts.length - 1];
        if (pid && pid !== '0' && pid !== String(process.pid)) {
          try {
            execSync(`taskkill /F /PID ${pid}`, { stdio: 'ignore' });
          } catch {}
        }
      }
    }
  }
} catch {}
