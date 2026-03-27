//! MCP Server Web Dashboard — 內嵌 HTML/JS/CSS
//! 提供工具測試介面、即時事件監控、場景狀態概覽

pub const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="zh-TW">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Kolibri MCP Server</title>
<style>
  :root {
    --bg: #1a1b2e; --panel: #232438; --border: #2d2e4a;
    --text: #e0e0e8; --muted: #8888aa; --brand: #4c8bf5;
    --green: #3cba6c; --red: #e85454; --orange: #e8a234;
    --radius: 10px; --font: 'Segoe UI', system-ui, sans-serif;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: var(--font); background: var(--bg); color: var(--text); height: 100vh; display: flex; flex-direction: column; }

  /* Header */
  header { background: var(--panel); border-bottom: 1px solid var(--border); padding: 12px 24px; display: flex; align-items: center; gap: 16px; }
  header h1 { font-size: 18px; font-weight: 600; }
  header h1 span { color: var(--brand); }
  .status { margin-left: auto; display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--muted); }
  .status .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--green); }
  .status.offline .dot { background: var(--red); }

  /* Main layout */
  .main { flex: 1; display: flex; overflow: hidden; }

  /* Sidebar */
  .sidebar { width: 240px; background: var(--panel); border-right: 1px solid var(--border); display: flex; flex-direction: column; overflow-y: auto; }
  .sidebar section { padding: 16px; border-bottom: 1px solid var(--border); }
  .sidebar h3 { font-size: 11px; text-transform: uppercase; letter-spacing: 1px; color: var(--muted); margin-bottom: 10px; }
  .tool-btn { display: block; width: 100%; text-align: left; padding: 6px 10px; border: none; background: none; color: var(--text); font-size: 13px; cursor: pointer; border-radius: 6px; margin-bottom: 2px; }
  .tool-btn:hover { background: rgba(76,139,245,0.15); }
  .tool-btn.active { background: var(--brand); color: #fff; }
  .stat { display: flex; justify-content: space-between; font-size: 13px; margin-bottom: 4px; }
  .stat .val { color: var(--brand); font-weight: 600; }

  /* Content */
  .content { flex: 1; display: flex; flex-direction: column; padding: 20px; gap: 16px; overflow-y: auto; }
  .card { background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius); padding: 16px; }
  .card h2 { font-size: 14px; margin-bottom: 12px; color: var(--muted); text-transform: uppercase; letter-spacing: 0.5px; }

  /* Tool playground */
  .playground { display: flex; gap: 16px; flex: 1; }
  .playground .left, .playground .right { flex: 1; display: flex; flex-direction: column; gap: 12px; }
  label { font-size: 12px; color: var(--muted); display: block; margin-bottom: 4px; }
  select, textarea, input[type=text] {
    width: 100%; padding: 8px 12px; background: var(--bg); border: 1px solid var(--border);
    color: var(--text); border-radius: 6px; font-family: 'Consolas', monospace; font-size: 13px; resize: vertical;
  }
  select { cursor: pointer; }
  textarea { min-height: 120px; }
  .btn { padding: 8px 20px; border: none; border-radius: 6px; cursor: pointer; font-size: 13px; font-weight: 600; }
  .btn-primary { background: var(--brand); color: #fff; }
  .btn-primary:hover { background: #5a9aff; }
  .btn-danger { background: var(--red); color: #fff; }
  .btn-sm { padding: 4px 12px; font-size: 12px; }

  /* Result */
  .result-box { background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 12px; font-family: 'Consolas', monospace; font-size: 12px; white-space: pre-wrap; max-height: 300px; overflow-y: auto; color: var(--green); }
  .result-box.error { color: var(--red); }

  /* Event log */
  .events { max-height: 200px; overflow-y: auto; }
  .event-item { padding: 4px 8px; font-size: 12px; font-family: 'Consolas', monospace; border-bottom: 1px solid var(--border); display: flex; gap: 8px; }
  .event-item .time { color: var(--muted); min-width: 70px; }
  .event-item .tool { color: var(--brand); min-width: 120px; }

  /* Description */
  .tool-desc { font-size: 13px; color: var(--muted); padding: 8px 0; line-height: 1.5; }
  .schema-hint { font-size: 11px; color: var(--muted); margin-top: 4px; }
</style>
</head>
<body>

<header>
  <h1><span>K</span> Kolibri MCP Server</h1>
  <div class="status" id="status">
    <div class="dot"></div>
    <span id="statusText">Connecting...</span>
  </div>
</header>

<div class="main">
  <!-- Sidebar -->
  <div class="sidebar">
    <section>
      <h3>Server</h3>
      <div class="stat"><span>Objects</span><span class="val" id="objCount">-</span></div>
      <div class="stat"><span>Version</span><span class="val" id="sceneVer">-</span></div>
      <div class="stat"><span>Port</span><span class="val" id="portNum">-</span></div>
    </section>
    <section>
      <h3>Tools (17)</h3>
      <div id="toolList"></div>
    </section>
    <section>
      <h3>Events</h3>
      <div class="events" id="eventLog"></div>
    </section>
  </div>

  <!-- Content -->
  <div class="content">
    <div class="card">
      <h2>Tool Playground</h2>
      <div class="playground">
        <div class="left">
          <div>
            <label>Tool</label>
            <select id="toolSelect" onchange="onToolChange()"></select>
          </div>
          <div class="tool-desc" id="toolDesc"></div>
          <div>
            <label>Arguments (JSON)</label>
            <textarea id="argsInput">{}</textarea>
            <div class="schema-hint" id="schemaHint"></div>
          </div>
          <div style="display:flex;gap:8px;">
            <button class="btn btn-primary" onclick="executeTool()">▶ Execute</button>
            <button class="btn btn-danger btn-sm" onclick="clearScene()">🗑 Clear Scene</button>
          </div>
        </div>
        <div class="right">
          <div>
            <label>Result</label>
            <div class="result-box" id="resultBox">// 點擊 Execute 執行工具</div>
          </div>
          <div>
            <label>Scene Preview</label>
            <div id="scenePreview" style="background:var(--bg);border:1px solid var(--border);border-radius:6px;min-height:150px;text-align:center;overflow:hidden"></div>
            <label style="margin-top:8px">Scene Objects</label>
            <div id="objectList" style="background:var(--bg);border:1px solid var(--border);border-radius:6px;padding:8px;max-height:200px;overflow-y:auto;font-size:12px"></div>
            <label style="margin-top:8px">Scene State (JSON)</label>
            <div class="result-box" id="sceneBox" style="max-height:150px">// 點擊 Refresh 更新</div>
          </div>
          <button class="btn btn-sm btn-primary" onclick="refreshScene()">↻ Refresh Scene</button>
        </div>
      </div>
    </div>
  </div>
</div>

<script>
const BASE = window.location.origin;
let tools = [];
let eventSource = null;

// ─── MCP JSON-RPC call ──────────────────────────────────────────────
async function mcpCall(method, params) {
  const body = { jsonrpc: '2.0', id: Date.now(), method, params };
  const res = await fetch(BASE + '/mcp', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  return await res.json();
}

async function callTool(name, args) {
  return mcpCall('tools/call', { name, arguments: args });
}

// ─── Initialize ─────────────────────────────────────────────────────
async function init() {
  try {
    // Health check
    const health = await (await fetch(BASE + '/health')).json();
    document.getElementById('statusText').textContent = 'Connected';
    document.getElementById('objCount').textContent = health.object_count;
    document.getElementById('portNum').textContent = window.location.port;

    // Get tools list
    const res = await mcpCall('tools/list', {});
    if (res.result && res.result.tools) {
      tools = res.result.tools;
      renderToolList();
      renderToolSelect();
    }

    // Scene preview
    refreshPreview();
    // SSE events
    connectSSE();
  } catch (e) {
    document.getElementById('status').classList.add('offline');
    document.getElementById('statusText').textContent = 'Disconnected';
  }
}

function renderToolList() {
  const el = document.getElementById('toolList');
  el.innerHTML = tools.map((t, i) =>
    `<button class="tool-btn" onclick="selectTool(${i})">${t.name}</button>`
  ).join('');
}

function renderToolSelect() {
  const sel = document.getElementById('toolSelect');
  sel.innerHTML = tools.map(t => `<option value="${t.name}">${t.name}</option>`).join('');
  onToolChange();
}

function selectTool(idx) {
  document.getElementById('toolSelect').value = tools[idx].name;
  onToolChange();
  // Highlight sidebar
  document.querySelectorAll('.tool-btn').forEach((b,i) => b.classList.toggle('active', i===idx));
}

function onToolChange() {
  const name = document.getElementById('toolSelect').value;
  const tool = tools.find(t => t.name === name);
  if (!tool) return;
  document.getElementById('toolDesc').textContent = tool.description;

  // Generate example args from schema
  const schema = tool.inputSchema;
  const example = {};
  if (schema && schema.properties) {
    for (const [key, prop] of Object.entries(schema.properties)) {
      if (prop.default !== undefined) example[key] = prop.default;
      else if (prop.type === 'number') example[key] = 1000;
      else if (prop.type === 'string') example[key] = '';
      else if (prop.type === 'array') example[key] = [0, 0, 0];
    }
  }
  document.getElementById('argsInput').value = JSON.stringify(example, null, 2);

  const required = schema?.required?.join(', ') || 'none';
  document.getElementById('schemaHint').textContent = `Required: ${required}`;
}

async function executeTool() {
  const name = document.getElementById('toolSelect').value;
  let args;
  try {
    args = JSON.parse(document.getElementById('argsInput').value);
  } catch (e) {
    document.getElementById('resultBox').textContent = 'JSON parse error: ' + e.message;
    document.getElementById('resultBox').classList.add('error');
    return;
  }

  const resultBox = document.getElementById('resultBox');
  resultBox.classList.remove('error');
  resultBox.textContent = '// Executing...';

  try {
    const res = await callTool(name, args);
    if (res.result && res.result.content) {
      resultBox.textContent = res.result.content[0].text;
    } else if (res.error) {
      resultBox.textContent = res.error.message;
      resultBox.classList.add('error');
    }
  } catch (e) {
    resultBox.textContent = 'Error: ' + e.message;
    resultBox.classList.add('error');
  }

  refreshHealth(); refreshPreview(); refreshObjectList();
}

async function clearScene() {
  await callTool('clear_scene', {});
  document.getElementById('resultBox').textContent = '// Scene cleared';
  refreshScene();
  refreshHealth(); refreshPreview(); refreshObjectList();
}

async function refreshPreview() {
  try {
    const res = await fetch(BASE + '/scene_svg');
    document.getElementById('scenePreview').innerHTML = await res.text();
  } catch(_) {}
}

async function refreshObjectList() {
  try {
    const res = await callTool('get_scene_state', {});
    if (res.result && res.result.content) {
      const data = JSON.parse(res.result.content[0].text);
      const el = document.getElementById('objectList');
      if (data.objects && data.objects.length > 0) {
        el.innerHTML = data.objects.map(o =>
          `<div style="padding:2px 0;border-bottom:1px solid var(--border);display:flex;justify-content:space-between">
            <span style="color:var(--text)">${o.name || o.id}</span>
            <span style="color:var(--muted);font-size:11px">${o.material || ''}</span>
          </div>`
        ).join('');
      } else {
        el.innerHTML = '<span style="color:var(--muted)">場景為空</span>';
      }
    }
  } catch(_) {}
}

async function refreshScene() {
  try {
    const res = await callTool('get_scene_state', {});
    if (res.result && res.result.content) {
      document.getElementById('sceneBox').textContent = res.result.content[0].text;
    }
  } catch (e) {
    document.getElementById('sceneBox').textContent = 'Error: ' + e.message;
  }
}

async function refreshHealth() {
  try {
    const health = await (await fetch(BASE + '/health')).json();
    document.getElementById('objCount').textContent = health.object_count;
  } catch (_) {}
}

// ─── SSE ────────────────────────────────────────────────────────────
function connectSSE() {
  eventSource = new EventSource(BASE + '/sse');
  eventSource.onmessage = (e) => {
    try {
      const data = JSON.parse(e.data);
      addEvent(data.tool, JSON.stringify(data.result).substring(0, 80));
    } catch (_) {
      addEvent('event', e.data.substring(0, 80));
    }
  };
  eventSource.onerror = () => {
    document.getElementById('status').classList.add('offline');
    document.getElementById('statusText').textContent = 'Reconnecting...';
    setTimeout(() => {
      document.getElementById('status').classList.remove('offline');
      document.getElementById('statusText').textContent = 'Connected';
    }, 2000);
  };
}

function addEvent(tool, text) {
  const log = document.getElementById('eventLog');
  const time = new Date().toLocaleTimeString('en', {hour12:false});
  const div = document.createElement('div');
  div.className = 'event-item';
  div.innerHTML = `<span class="time">${time}</span><span class="tool">${tool}</span><span>${text}</span>`;
  log.prepend(div);
  // Keep max 50 events
  while (log.children.length > 50) log.removeChild(log.lastChild);
  refreshHealth(); refreshPreview(); refreshObjectList();
}

init();
</script>
</body>
</html>
"##;
