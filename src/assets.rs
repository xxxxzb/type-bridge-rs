pub const HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0, user-scalable=no">
<title>TypeBridge</title>
<script src="https://cdn.socket.io/4.7.5/socket.io.min.js"></script>
<style>
  @import url('https://fonts.googleapis.com/css2?family=DM+Mono:ital,wght@0,400;0,500;1,400&family=Syne:wght@800&display=swap');

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
    font-family: 'DM Mono', monospace;
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
    font-family: 'Syne', sans-serif;
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
    font-family: 'DM Mono', monospace;
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
    font-family: 'DM Mono', monospace;
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
    font-family: 'DM Mono', monospace;
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

  .act.enter {
    grid-column: span 2;
    color: var(--accent);
    border-color: rgba(124,106,247,.2);
  }
  .act.enter:active { background: rgba(124,106,247,.08); }

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

  <div class="actions">
    <button class="act back" id="back-btn">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 4H8l-7 8 7 8h13a2 2 0 0 0 2-2V6a2 2 0 0 0-2-2z"/>
        <line x1="18" y1="9" x2="12" y2="15"/><line x1="12" y1="9" x2="18" y2="15"/>
      </svg>
      backspace
    </button>

    <button class="act clear" id="clear-btn">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="3 6 5 6 21 6"/>
        <path d="M19 6l-1 14H6L5 6"/>
        <path d="M10 11v6"/><path d="M14 11v6"/>
        <path d="M9 6V4h6v2"/>
      </svg>
      clear
    </button>

    <button class="act enter" id="enter-btn">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <polyline points="9 10 4 15 9 20"/>
        <path d="M20 4v7a4 4 0 0 1-4 4H4"/>
      </svg>
      enter / new line on PC
    </button>
  </div>

</div>

<div class="toast" id="toast"></div>

<script>
const socket   = io();
const input    = document.getElementById('input');
const sendBtn  = document.getElementById('send-btn');
const backBtn  = document.getElementById('back-btn');
const clearBtn = document.getElementById('clear-btn');
const enterBtn = document.getElementById('enter-btn');
const pill     = document.getElementById('pill');
const pillText = document.getElementById('pill-text');
const toast    = document.getElementById('toast');

socket.on('connect', () => {
  pill.classList.add('ok');
  pillText.textContent = 'connected';
});
socket.on('disconnect', () => {
  pill.classList.remove('ok');
  pillText.textContent = 'disconnected';
});

let toastT;
function showToast(msg) {
  toast.textContent = msg;
  toast.classList.add('show');
  clearTimeout(toastT);
  toastT = setTimeout(() => toast.classList.remove('show'), 1600);
}

function sendText() {
  const text = input.value;
  if (!text.trim()) return;
  socket.emit('type_text', { text });
  input.value = '';
  showToast('sent!');
}

sendBtn.addEventListener('click', sendText);

input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    sendText();
  }
});

backBtn.addEventListener('click', () => {
  const v = input.value;
  if (v.length > 0) {
    input.value = v.slice(0, -1);
  }
  socket.emit('backspace', {});
  showToast('⌫');
});

clearBtn.addEventListener('click', () => {
  input.value = '';
  showToast('cleared');
});

enterBtn.addEventListener('click', () => {
  socket.emit('press_key', { key: 'enter' });
  showToast('↵ enter');
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
    fn test_html_contains_socket_io() {
        assert!(HTML.contains("socket.io"));
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
        assert!(HTML.contains("enter"));
    }

    #[test]
    fn test_html_contains_clear_button() {
        assert!(HTML.contains("clear"));
    }

    #[test]
    fn test_html_contains_type_text_event() {
        assert!(HTML.contains("type_text"));
    }

    #[test]
    fn test_html_closes_properly() {
        assert!(HTML.ends_with("</html>"));
    }
}
