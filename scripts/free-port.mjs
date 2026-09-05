import { execSync } from 'node:child_process';
import net from 'node:net';

function killPortWindows(port) {
  try {
    const out = execSync('netstat -ano', { encoding: 'utf-8', stdio: ['ignore', 'pipe', 'ignore'] });
    const pids = new Set();
    for (const line of out.split('\n')) {
      if (line.includes(`:${port}`) && line.includes('LISTENING')) {
        const parts = line.trim().split(/\s+/);
        const pid = parts[parts.length - 1];
        if (pid && pid !== '0' && pid !== String(process.pid)) {
          pids.add(pid);
        }
      }
    }
    for (const pid of pids) {
      try {
        execSync(`taskkill /F /T /PID ${pid}`, { stdio: 'ignore' });
      } catch {}
    }
  } catch {}
}

function checkPortFree(port) {
  return new Promise((resolve) => {
    const tester = net.createServer()
      .once('error', () => resolve(false))
      .once('listening', () => {
        tester.close(() => resolve(true));
      })
      .listen(port);
  });
}

async function main() {
  const port = 5173;
  if (process.platform === 'win32') {
    killPortWindows(port);
  }

  const start = Date.now();
  while (Date.now() - start < 1500) {
    if (await checkPortFree(port)) {
      break;
    }
    if (process.platform === 'win32') {
      killPortWindows(port);
    }
    await new Promise((r) => setTimeout(r, 100));
  }
}

main();
