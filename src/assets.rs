pub const HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0, user-scalable=no">
<title>TypeBridge</title>
<style>
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

  :root {
    --bg:      #0c0c10;
    --surface: #13131a;
    --border:  #22222f;
    --accent:  #7c6af7;
    --accent2: #f76ab4;
    --text:    #dddaf0;
    --muted:   #4a4960;
    --success: #4ef7a4;
    --danger:  #f74e6a;
  }

  html, body {
    height: 100%;
    background: var(--bg);
    color: var(--text);
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', 'Consolas', 'Menlo', monospace;
  }

  body::before {
    content: '';
    position: fixed;
    inset: 0;
    background-image:
      linear-gradient(rgba(124,106,247,.03) 1px, transparent 1px),
      linear-gradient(90deg, rgba(124,106,247,.03) 1px, transparent 1px);
    background-size: 36px 36px;
    pointer-events: none;
  }

  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 100dvh;
    padding: 20px 16px 24px;
    gap: 14px;
    max-width: 520px;
    margin: 0 auto;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .logo {
    font-family: system-ui, -apple-system, sans-serif;
    font-size: 18px;
    background: linear-gradient(120deg, var(--accent), var(--accent2));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }
  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 11px;
    border-radius: 99px;
    border: 1px solid var(--border);
    font-size: 11px;
    color: var(--muted);
    transition: all .3s;
  }
  .pill .dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: var(--muted);
    transition: all .3s;
  }
  .pill.ok { color: var(--success); border-color: rgba(78,247,164,.2); }
  .pill.ok .dot { background: var(--success); box-shadow: 0 0 5px var(--success); }

  .box {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 16px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .box-label {
    padding: 9px 14px 7px;
    font-size: 10px;
    letter-spacing: .1em;
    text-transform: uppercase;
    color: var(--muted);
    border-bottom: 1px solid var(--border);
  }
  textarea {
    width: 100%;
    min-height: 160px;
    padding: 14px;
    background: transparent;
    border: none;
    outline: none;
    resize: none;
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', 'Consolas', 'Menlo', monospace;
    font-size: 16px;
    color: var(--text);
    line-height: 1.6;
    caret-color: var(--accent);
  }
  textarea::placeholder { color: var(--muted); font-style: italic; }

  #send-btn {
    width: 100%;
    padding: 16px;
    border-radius: 14px;
    border: none;
    background: linear-gradient(135deg, var(--accent), var(--accent2));
    color: #fff;
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', 'Consolas', 'Menlo', monospace;
    font-size: 15px;
    font-weight: 500;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    transition: opacity .2s, transform .15s;
    -webkit-tap-highlight-color: transparent;
  }
  #send-btn:active { transform: scale(.97); opacity: .85; }
  #send-btn:disabled { opacity: .4; cursor: default; }

  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .act {
    padding: 13px;
    border-radius: 13px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font-family: 'SF Mono', 'Fira Code', 'Cascadia Code', 'Consolas', 'Menlo', monospace;
    font-size: 13px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    transition: all .18s;
    -webkit-tap-highlight-color: transparent;
  }
  .act:active { transform: scale(.94); }
  .act.back  { color: var(--accent2); border-color: rgba(247,106,180,.2); }
  .act.back:active  { background: rgba(247,106,180,.08); }
  .act.clear { color: var(--danger);  border-color: rgba(247,78,106,.2); }
  .act.clear:active { background: rgba(247,78,106,.08); }
  .act.local { color: var(--text); border-color: var(--border); }
  .act.local:active { background: rgba(255,255,255,.04); }

  .act.enter {
    color: var(--accent);
    border-color: rgba(124,106,247,.2);
  }
  .act.enter:active { background: rgba(124,106,247,.08); }

  .history-box { }
  .history-list { max-height: 200px; overflow-y: auto; padding: 4px 0; }
  .history-item {
    padding: 10px 14px;
    cursor: pointer;
    font-size: 13px;
    border-bottom: 1px solid var(--border);
    text-overflow: ellipsis;
    overflow: hidden;
    white-space: nowrap;
  }
  .history-item:last-child { border-bottom: none; }
  .history-item:hover { background: rgba(255,255,255,.03); }

  .hidden { display: none !important; }

  .toast {
    position: fixed;
    bottom: 80px;
    left: 50%;
    transform: translateX(-50%) translateY(12px);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 99px;
    padding: 7px 16px;
    font-size: 12px;
    color: var(--muted);
    opacity: 0;
    transition: all .25s;
    pointer-events: none;
    white-space: nowrap;
    z-index: 99;
  }
  .toast.show { opacity: 1; transform: translateX(-50%) translateY(0); }
</style>
</head>
<body>
<div class="app">

  <header>
    <span class="logo">TypeBridge</span>
    <span class="pill" id="pill">
      <span class="dot"></span>
      <span id="pill-text">connecting</span>
    </span>
  </header>

  <div class="box">
    <div class="box-label">type here — use phone keyboard or voice</div>
    <textarea id="input" placeholder="Start typing…" autocomplete="off" autocorrect="off" spellcheck="false"></textarea>
  </div>

  <button id="send-btn">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
      <line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/>
    </svg>
    send to PC
  </button>

  <div class="box history-box hidden" id="history-box">
    <div class="box-label">recent history</div>
    <div class="history-list" id="history-list"></div>
  </div>

  <div class="actions">
    <button class="act back" id="back-btn">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 4H8l-7 8 7 8h13a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2z"/>
        <line x1="18" y1="9" x2="12" y2="15"/><line x1="12" y1="9" x2="18" y2="15"/>
      </svg>
      backspace
    </button>

    <button class="act local" id="clear-text-btn">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
      Clear text
    </button>

    <button class="act clear" id="clear-pc-btn">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="3 6 5 6 21 6"/>
        <path d="M19 6l-1 14H6L5 6"/>
        <path d="M10 11v6"/><path d="M14 11v6"/>
        <path d="M9 6V4h6v2"/>
      </svg>
      Clear PC field
    </button>

    <button class="act enter" id="enter-btn">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="9 10 4 15 9 20"/>
        <path d="M20 4v7a4 4 0 0 1-4 4H4"/>
      </svg>
      enter
    </button>
  </div>

</div>

<div class="toast" id="toast"></div>
<script>
document.addEventListener('DOMContentLoaded', function() {
  // ── DOM refs ──
  var textarea = document.getElementById('input');
  var sendBtn = document.getElementById('send-btn');
  var backBtn = document.getElementById('back-btn');
  var clearTextBtn = document.getElementById('clear-text-btn');
  var clearPcBtn = document.getElementById('clear-pc-btn');
  var enterBtn = document.getElementById('enter-btn');
  var pill = document.getElementById('pill');
  var pillText = document.getElementById('pill-text');
  var toast = document.getElementById('toast');
  var historyBox = document.getElementById('history-box');
  var historyList = document.getElementById('history-list');

  // ── Toast ──
  function showToast(msg) {
    toast.textContent = msg;
    toast.classList.add('show');
    setTimeout(function() { toast.classList.remove('show'); }, 2000);
  }

  // ── API helper (adds Bearer token and JSON content-type) ──
  function api(path, options) {
    options = options || {};
    var headers = options.headers || {};
    var t = sessionStorage.getItem('token');
    if (t) { headers['Authorization'] = 'Bearer ' + t; }
    if (options.body) { headers['Content-Type'] = 'application/json'; }
    return fetch(path, { method: options.method, headers: headers, body: options.body });
  }

  // ── Token extraction ──
  var params = new URLSearchParams(location.search);
  var urlToken = params.get('token');
  if (urlToken) {
    sessionStorage.setItem('token', urlToken);
    history.replaceState(null, '', '/');
  } else if (!sessionStorage.getItem('token')) {
    showToast('no auth token');
  }

  // ── Command queue (serial execution via Promise chain) ──
  var queue = Promise.resolve();

  function enqueue(body, successMsg) {
    queue = queue.then(function() {
      return api('/api/commands', {
        method: 'POST',
        body: JSON.stringify(body)
      }).then(function(res) {
        if (res.status === 202) {
          showToast(successMsg || 'sent');
          if (body.type === 'type_text') {
            textarea.value = '';
            loadHistory();
          }
          return res;
        }
        return res.json().then(function(data) {
          showToast(data.error || 'command failed');
        });
      }).catch(function() {
        showToast('connection error');
      });
    });
    return queue;
  }

  // ── Send to PC ──
  sendBtn.addEventListener('click', function() {
    var text = textarea.value.trim();
    if (!text) { showToast('no text to send'); return; }
    enqueue({type: 'type_text', text: text});
  });

  // ── Backspace ──
  backBtn.addEventListener('click', function() {
    enqueue({type: 'backspace'}, 'backspace sent');
  });

  // ── Clear text (local only, no HTTP) ──
  clearTextBtn.addEventListener('click', function() {
    textarea.value = '';
    textarea.focus();
  });

  // ── Clear PC field ──
  clearPcBtn.addEventListener('click', function() {
    enqueue({type: 'clear_pc_field'}, 'clear sent');
  });

  // ── Enter ──
  enterBtn.addEventListener('click', function() {
    enqueue({type: 'enter'}, 'enter sent');
  });

  // ── Keyboard: Enter = send, Shift+Enter = newline ──
  textarea.addEventListener('keydown', function(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      sendBtn.click();
    }
  });

  // ── Status polling (every 5s) ──
  function updateStatus() {
    api('/api/status').then(function(res) { return res.json(); }).then(function(data) {
      if (data.enabled) {
        pill.classList.add('ok');
        pillText.textContent = 'enabled';
      } else {
        pill.classList.remove('ok');
        pillText.textContent = 'paused';
      }
    }).catch(function() {
      pill.classList.remove('ok');
      pillText.textContent = 'offline';
    });
  }
  updateStatus();
  setInterval(updateStatus, 5000);

  // ── History (load on init and after each send) ──
  function loadHistory() {
    api('/api/history').then(function(res) { return res.json(); }).then(function(entries) {
      historyList.innerHTML = '';
      if (!entries || entries.length === 0) {
        historyBox.classList.add('hidden');
        return;
      }
      historyBox.classList.remove('hidden');
      entries.forEach(function(text) {
        var div = document.createElement('div');
        div.className = 'history-item';
        div.textContent = text;
        div.addEventListener('click', function() {
          textarea.value = text;
          textarea.focus();
        });
        historyList.appendChild(div);
      });
    }).catch(function() {});
  }
  loadHistory();
});
</script>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_has_doctype() {
        assert!(HTML.starts_with("<!DOCTYPE html>"));
    }

    #[test]
    fn test_html_contains_textarea() {
        assert!(HTML.contains("<textarea"));
    }

    #[test]
    fn test_html_has_no_socket_io_script() {
        assert!(!HTML.contains("socket.io"));
    }

    #[test]
    fn test_html_contains_send_button() {
        assert!(HTML.contains("send to PC"));
    }

    #[test]
    fn test_html_contains_backspace_button() {
        assert!(HTML.contains("backspace"));
    }

    #[test]
    fn test_html_contains_enter_button() {
        assert!(HTML.contains("id=\"enter-btn\""));
    }

    #[test]
    fn test_html_closes_properly() {
        assert!(HTML.ends_with("</html>"));
    }

    #[test]
    fn test_html_contains_script_tag() {
        assert!(HTML.contains("<script>"));
    }

    #[test]
    fn test_html_contains_session_storage() {
        assert!(HTML.contains("sessionStorage"));
    }

    #[test]
    fn test_html_contains_fetch() {
        assert!(HTML.contains("fetch("));
    }

    #[test]
    fn test_html_contains_replace_state() {
        assert!(HTML.contains("replaceState"));
    }

    #[test]
    fn test_html_contains_clear_pc_field() {
        assert!(HTML.contains("Clear PC field"));
    }

    #[test]
    fn test_html_contains_authorization_header() {
        assert!(HTML.contains("Authorization"));
        assert!(HTML.contains("Bearer"));
    }

    #[test]
    fn test_html_contains_promise_queue() {
        assert!(HTML.contains(".then("));
    }

    #[test]
    fn test_html_contains_setinterval() {
        assert!(HTML.contains("setInterval"));
    }

    #[test]
    fn test_html_contains_history_section() {
        assert!(HTML.contains("history-box"));
    }

    #[test]
    fn test_html_contains_clear_text_button() {
        assert!(HTML.contains("Clear text"));
    }

    #[test]
    fn test_html_contains_no_onclick() {
        assert!(!HTML.contains("onclick="));
    }
}
