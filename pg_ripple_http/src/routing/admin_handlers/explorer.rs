//! Visual graph explorer handler (v0.62.0, split to sub-module v0.122.0).

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::common::{AppState, check_auth};

// ─── v0.62.0: Visual graph explorer ─────────────────────────────────────────

/// Serve the browser-based visual graph explorer at `/explorer`.
///
/// EXPLORER-AUTH-01 (v0.80.0): authentication is required. Unauthenticated
/// requests receive HTTP 401 so that the full RDF graph cannot be browsed
/// without credentials.
///
/// The explorer is a single-page application that accepts a SPARQL CONSTRUCT
/// query, renders the resulting triples as a force-directed graph using
/// sigma.js, and allows clicking any node to expand its neighbourhood.
pub(crate) async fn explorer_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    // EXPLORER-AUTH-01: require authentication before serving the explorer UI.
    if let Err(resp) = check_auth(&state, &headers) {
        return resp;
    }
    let html = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>pg_ripple Graph Explorer</title>
  <style>
    body { margin: 0; font-family: sans-serif; display: flex; flex-direction: column; height: 100vh; background: #1a1a2e; color: #eee; }
    #toolbar { padding: 10px; background: #16213e; display: flex; gap: 8px; align-items: center; border-bottom: 1px solid #0f3460; }
    #toolbar label { font-size: 13px; color: #a0aec0; }
    #query { flex: 1; padding: 6px 10px; border-radius: 4px; border: 1px solid #0f3460; background: #0f3460; color: #eee; font-family: monospace; font-size: 13px; }
    #run-btn { padding: 6px 16px; border-radius: 4px; border: none; background: #e94560; color: #fff; cursor: pointer; font-size: 13px; }
    #run-btn:hover { background: #c73652; }
    #status { font-size: 12px; color: #a0aec0; padding: 4px; }
    #canvas { flex: 1; background: #0d1117; }
    #info-panel { position: fixed; right: 10px; top: 60px; width: 300px; background: #16213e; border: 1px solid #0f3460; border-radius: 6px; padding: 12px; display: none; font-size: 12px; max-height: 80vh; overflow-y: auto; }
    .node-label { font-weight: bold; color: #e94560; margin-bottom: 6px; word-break: break-all; }
    .triple-row { margin: 4px 0; padding: 4px; background: #0f3460; border-radius: 3px; word-break: break-all; }
  </style>
</head>
<body>
  <div id="toolbar">
    <label>SPARQL CONSTRUCT:</label>
    <input id="query" type="text" value="CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o } LIMIT 100" placeholder="Enter SPARQL CONSTRUCT query..." />
    <button id="run-btn" onclick="runQuery()">Run</button>
    <span id="status"></span>
  </div>
  <canvas id="canvas"></canvas>
  <div id="info-panel">
    <div class="node-label" id="info-title"></div>
    <div id="info-triples"></div>
    <button onclick="expandNode()" style="margin-top:8px;padding:4px 10px;border-radius:3px;border:none;background:#e94560;color:#fff;cursor:pointer;font-size:12px;">Expand</button>
  </div>

  <script>
    const SPARQL_ENDPOINT = '/sparql';
    let graph = { nodes: {}, edges: [] };
    let canvas, ctx, selectedNode = null;
    let positions = {};
    let velocities = {};
    let animFrame = null;

    function init() {
      canvas = document.getElementById('canvas');
      ctx = canvas.getContext('2d');
      canvas.width = canvas.offsetWidth;
      canvas.height = canvas.offsetHeight;
      canvas.addEventListener('click', onCanvasClick);
      window.addEventListener('resize', () => { canvas.width = canvas.offsetWidth; canvas.height = canvas.offsetHeight; draw(); });
    }

    async function runQuery() {
      const q = document.getElementById('query').value.trim();
      if (!q) return;
      document.getElementById('status').textContent = 'Running...';
      try {
        const resp = await fetch('/sparql', {
          method: 'POST',
          headers: {'Content-Type': 'application/x-www-form-urlencoded', 'Accept': 'application/sparql-results+json'},
          body: 'query=' + encodeURIComponent(q)
        });
        if (!resp.ok) throw new Error(await resp.text());
        const data = await resp.json();
        buildGraph(data);
        document.getElementById('status').textContent = graph.edges.length + ' triples, ' + Object.keys(graph.nodes).length + ' nodes';
      } catch(e) {
        document.getElementById('status').textContent = 'Error: ' + e.message;
      }
    }

    function buildGraph(results) {
      graph = { nodes: {}, edges: [] };
      positions = {};
      velocities = {};
      const W = canvas.width, H = canvas.height;
      for (const row of results) {
        const s = row.s && row.s.value || row.s || null;
        const p = row.p && row.p.value || row.p || null;
        const o = row.o && row.o.value || row.o || null;
        if (!s || !p || !o) continue;
        if (!graph.nodes[s]) { graph.nodes[s] = { id: s, triples: [] }; positions[s] = { x: Math.random()*W, y: Math.random()*H }; velocities[s] = { x: 0, y: 0 }; }
        if (!graph.nodes[o]) { graph.nodes[o] = { id: o, triples: [] }; positions[o] = { x: Math.random()*W, y: Math.random()*H }; velocities[o] = { x: 0, y: 0 }; }
        graph.nodes[s].triples.push({ p, o });
        graph.edges.push({ s, p, o });
      }
      if (animFrame) cancelAnimationFrame(animFrame);
      simulate();
    }

    function simulate() {
      const nodes = Object.keys(graph.nodes);
      if (nodes.length === 0) return;
      for (let i = 0; i < 5; i++) forceStep(nodes);
      draw();
      animFrame = requestAnimationFrame(simulate);
    }

    function forceStep(nodes) {
      const k = 100, W = canvas.width, H = canvas.height;
      for (const a of nodes) {
        let fx = 0, fy = 0;
        for (const b of nodes) {
          if (a === b) continue;
          const dx = positions[a].x - positions[b].x, dy = positions[a].y - positions[b].y;
          const dist = Math.max(Math.sqrt(dx*dx+dy*dy), 1);
          fx += (k*k/dist) * (dx/dist);
          fy += (k*k/dist) * (dy/dist);
        }
        for (const e of graph.edges) {
          let other = null;
          if (e.s === a) other = e.o;
          else if (e.o === a) other = e.s;
          if (!other) continue;
          const dx = positions[a].x - positions[other].x, dy = positions[a].y - positions[other].y;
          const dist = Math.max(Math.sqrt(dx*dx+dy*dy), 1);
          fx -= (dist*dist/k) * (dx/dist);
          fy -= (dist*dist/k) * (dy/dist);
        }
        // Centre gravity
        fx += (W/2 - positions[a].x) * 0.01;
        fy += (H/2 - positions[a].y) * 0.01;
        velocities[a].x = (velocities[a].x + fx) * 0.85;
        velocities[a].y = (velocities[a].y + fy) * 0.85;
        positions[a].x = Math.max(20, Math.min(W-20, positions[a].x + velocities[a].x * 0.1));
        positions[a].y = Math.max(20, Math.min(H-20, positions[a].y + velocities[a].y * 0.1));
      }
    }

    function shortLabel(iri) {
      if (!iri) return '';
      const s = iri.replace(/^<|>$/g, '');
      const h = s.lastIndexOf('#'), sl = s.lastIndexOf('/');
      const cut = Math.max(h, sl);
      return cut >= 0 ? s.slice(cut+1) : s.slice(-20);
    }

    function draw() {
      if (!ctx) return;
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.strokeStyle = '#0f3460';
      ctx.lineWidth = 1;
      for (const e of graph.edges) {
        if (!positions[e.s] || !positions[e.o]) continue;
        ctx.beginPath();
        ctx.moveTo(positions[e.s].x, positions[e.s].y);
        ctx.lineTo(positions[e.o].x, positions[e.o].y);
        ctx.stroke();
      }
      for (const [id, node] of Object.entries(graph.nodes)) {
        const p = positions[id];
        if (!p) continue;
        ctx.beginPath();
        ctx.arc(p.x, p.y, 8, 0, Math.PI*2);
        ctx.fillStyle = id === selectedNode ? '#e94560' : '#4361ee';
        ctx.fill();
        ctx.fillStyle = '#eee';
        ctx.font = '11px sans-serif';
        ctx.fillText(shortLabel(id), p.x+10, p.y+4);
      }
    }

    function onCanvasClick(e) {
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left, my = e.clientY - rect.top;
      for (const [id] of Object.entries(graph.nodes)) {
        const p = positions[id];
        if (!p) continue;
        if ((mx-p.x)*(mx-p.x)+(my-p.y)*(my-p.y) < 100) {
          selectedNode = id;
          showInfo(id);
          draw();
          return;
        }
      }
      selectedNode = null;
      document.getElementById('info-panel').style.display = 'none';
      draw();
    }

    function showInfo(id) {
      const node = graph.nodes[id];
      const panel = document.getElementById('info-panel');
      document.getElementById('info-title').textContent = id.replace(/^<|>$/g,'');
      const tbody = document.getElementById('info-triples');
      tbody.innerHTML = (node.triples||[]).slice(0,20).map(t =>
        '<div class="triple-row"><b>' + shortLabel(t.p) + '</b> → ' + shortLabel(t.o) + '</div>'
      ).join('');
      panel.style.display = 'block';
    }

    async function expandNode() {
      if (!selectedNode) return;
      const iri = selectedNode.replace(/^<|>$/g, '');
      const q = 'CONSTRUCT { <' + iri + '> ?p ?o } WHERE { <' + iri + '> ?p ?o } LIMIT 50';
      document.getElementById('query').value = q;
      document.getElementById('status').textContent = 'Expanding...';
      try {
        const resp = await fetch('/sparql', {
          method: 'POST',
          headers: {'Content-Type': 'application/x-www-form-urlencoded', 'Accept': 'application/sparql-results+json'},
          body: 'query=' + encodeURIComponent(q)
        });
        if (!resp.ok) throw new Error(await resp.text());
        const data = await resp.json();
        for (const row of data) {
          const s = row.s && row.s.value || row.s || null;
          const p = row.p && row.p.value || row.p || null;
          const o = row.o && row.o.value || row.o || null;
          if (!s || !p || !o) continue;
          const W = canvas.width, H = canvas.height;
          if (!graph.nodes[s]) { graph.nodes[s] = { id: s, triples: [] }; positions[s] = { x: Math.random()*W, y: Math.random()*H }; velocities[s] = { x: 0, y: 0 }; }
          if (!graph.nodes[o]) { graph.nodes[o] = { id: o, triples: [] }; positions[o] = { x: Math.random()*W, y: Math.random()*H }; velocities[o] = { x: 0, y: 0 }; }
          graph.nodes[s].triples.push({ p, o });
          const exists = graph.edges.some(e => e.s === s && e.p === p && e.o === o);
          if (!exists) graph.edges.push({ s, p, o });
        }
        document.getElementById('status').textContent = graph.edges.length + ' triples, ' + Object.keys(graph.nodes).length + ' nodes';
      } catch(e) {
        document.getElementById('status').textContent = 'Error: ' + e.message;
      }
    }

    window.onload = init;
  </script>
</body>
</html>"#;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
