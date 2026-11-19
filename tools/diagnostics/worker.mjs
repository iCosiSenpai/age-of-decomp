const details = {
  launcher_crash: ['unexpected_exit'],
  game_crash: ['device_lost', 'out_of_memory', 'access_violation', 'unexpected_exit'],
  build_failure: ['preflight', 'exefs', 'resources', 'dlc', 'translate', 'compile', 'install', 'unknown'],
  game_session: ['completed'],
};

async function readReport(request) {
  if (!request.body) return null;
  const reader = request.body.getReader();
  const buffer = new Uint8Array(1024);
  let length = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (length + value.byteLength > buffer.length) {
        await reader.cancel();
        return null;
      }
      buffer.set(value, length);
      length += value.byteLength;
    }
    const report = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(buffer.subarray(0, length)));
    if (!report || typeof report !== 'object' || Object.keys(report).length !== 5) return null;
    const { version, os, event, detail, fps } = report;
    if (typeof version !== 'string' || !/^\d{1,6}\.\d{1,6}\.\d{1,6}$/.test(version)) return null;
    if (!['windows', 'linux'].includes(os)) return null;
    if (typeof event !== 'string' || !Object.hasOwn(details, event) || !details[event].includes(detail)) return null;
    if (event === 'game_session') {
      if (!Number.isInteger(fps) || fps < 5 || fps > 300 || fps % 5 !== 0) return null;
    } else if (fps !== null) return null;
    return { version, os, event, detail, fps: fps ?? 0 };
  } catch {
    return null;
  } finally {
    reader.releaseLock();
  }
}

export default {
  async fetch(request, env) {
    if (new URL(request.url).pathname !== '/v1/reports' || request.method !== 'POST') {
      return new Response(null, { status: 404 });
    }
    if (request.headers.get('Content-Type')?.split(';')[0].trim() !== 'application/json') {
      return new Response(null, { status: 415 });
    }
    const report = await readReport(request);
    if (!report) return new Response(null, { status: 400 });
    const { version, os, event, detail, fps } = report;
    try {
      await env.REPORTS.prepare(
        'INSERT INTO reports (day, version, os, event, detail, fps, total) VALUES (date(\'now\'), ?, ?, ?, ?, ?, 1) ' +
        'ON CONFLICT(day, version, os, event, detail, fps) DO UPDATE SET total = total + 1',
      ).bind(version, os, event, detail, fps).run();
      return new Response(null, { status: 204 });
    } catch {
      return new Response(null, { status: 503 });
    }
  },

  async scheduled(_event, env) {
    await env.REPORTS.prepare("DELETE FROM reports WHERE day < date('now', '-90 days')").run();
  },
};
