<!-- PROTOTYPE: requested placement revision based on screenshots 1 and 2. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    BookOpen, Bug, ChevronDown, Download, FileCode,
    ImageDown, Link2, Maximize,
    MessageCircle, PanelLeftClose, Plus, Search, Share2,
    Sparkles, Upload, UserCircle, Wand2, ZoomIn, ZoomOut,
  } from 'lucide-svelte';

  let commandOpen = false;
  let commandQuery = '';
  let zoom = 100;
  let notice = '选择一个功能查看反馈';

  const lines = [
    ['1', '{'], ['2', '  "object": { "bool": true, "float": 0.125,'], ['3', '    "int": 42'],
    ['4', '  },'], ['5', '  "preview": {'], ['6', '    "base64": "aHR0cHM6Ly90cmVhc2UuY29t...",'],
    ['7', '    "color": "#4f46e5",'], ['8', '    "uris": ['], ['9', '      "https://treease.com/path?redirect=true"'],
    ['10', '    ]'], ['11', '  }'], ['12', '}'],
  ];

  onMount(() => {
    const onKeydown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        commandOpen = true;
      }
      if (event.key === 'Escape') commandOpen = false;
    };
    window.addEventListener('keydown', onKeydown);
    return () => window.removeEventListener('keydown', onKeydown);
  });

  function action(label: string) {
    notice = `${label}：原型交互已触发`;
    commandOpen = false;
  }
  function changeZoom(delta: number) {
    zoom = Math.max(50, Math.min(180, zoom + delta));
    notice = `画布缩放至 ${zoom}%`;
  }
</script>

<svelte:head><title>Treease · Requested chrome prototype</title></svelte:head>

<div class="prototype-page" on:click={() => (commandOpen = false)}>
  <div class="prototype-note"><span></span><strong>PROTOTYPE / 按截图要求</strong><em>1：侧边栏　2：原 Import/Export　4：Command　5：编辑操作　6：全局操作　7：编辑器状态</em></div>

  <section class="app-frame" on:click|stopPropagation>
    <!-- 1：整条侧边栏替换为截图一同款，并吸收 2 / 5 / 6 -->
    <aside class="left-rail">
      <button class="rail-logo" title="Treease home">&#123; &#125;</button>
      <div class="rail-group file-group">
        <button title="Import" on:click={() => action('Import')}><Upload size={20} /></button>
        <button title="Export" on:click={() => action('Export')}><Download size={20} /></button>
      </div>
      <div class="rail-divider"></div>
      <!-- 5：AI / Format / Minify 等从 bottombar 移入侧边栏 -->
      <div class="rail-group editor-group">
        <button class="selected" title="Format" on:click={() => action('Format')}><FileCode size={20} /></button>
        <button title="Transform" on:click={() => action('Transform')}><Wand2 size={19} /></button>
        <button title="Compare" on:click={() => action('Compare')}><Link2 size={19} /></button>
      </div>
      <div class="rail-spacer"></div>
      <!-- 6：Tutorial / Feedback / Share / Account 从 topbar 移入侧边栏 -->
      <div class="rail-group global-group">
        <button title="Tutorial" on:click={() => action('Tutorial')}><BookOpen size={18} /></button>
        <button title="Feedback" on:click={() => action('Feedback')}><MessageCircle size={18} /></button>
        <button title="Report bug" on:click={() => action('Report bug')}><Bug size={18} /></button>
        <button title="Collapse sidebar" on:click={() => action('Collapse sidebar')}><PanelLeftClose size={18} /></button>
      </div>
    </aside>

    <div class="workspace">
      <div class="top-bars">
        <!-- topbar-left：保留 Tab，并把原 4 的功能移到这里；搜索命令改为短按钮 -->
        <header class="topbar topbar-left">
          <!-- 1：搜索命令保留为单个按钮，只显示放大镜 -->
          <button class="command-button" title="搜索命令（⌘K）" aria-label="搜索命令" on:click={() => (commandOpen = !commandOpen)}>
            <Search size={17} />
          </button>
          <div class="tabs">
            <button class="tab">Untitled 4 <b>×</b></button>
            <button class="tab">Untitled 2 <b>×</b></button>
            <button class="tab active">Untitled 2 <b>×</b></button>
            <button class="new-tab" on:click={() => action('New tab')}><Plus size={16} /></button>
          </div>
          {#if commandOpen}
            <div class="command-panel">
              <div class="command-input" role="combobox" aria-expanded="true" aria-haspopup="listbox"><Search size={16} /><input bind:value={commandQuery} placeholder="搜索命令" autofocus aria-label="搜索命令" /></div>
              <button on:click={() => action('Format document')}><Wand2 size={15} /> Format document <kbd>⌥⇧F</kbd></button>
              <button on:click={() => action('Minify document')}><FileCode size={15} /> Minify document</button>
              <button on:click={() => action('Ask AI')}><Sparkles size={15} /> Ask AI about this document</button>
            </div>
          {/if}
        </header>
        <!-- topbar-right：2 -> 1，把 Graph 工具从右侧竖栏移到 Graph topbar -->
        <header class="topbar topbar-right">
          <div class="graph-top-tools">
            <button title="Search nodes" on:click={() => action('Search nodes')}><Search size={17} /></button>
            <button title="Zoom in" on:click={() => changeZoom(10)}><ZoomIn size={17} /></button>
            <span>{zoom}%</span>
            <button title="Zoom out" on:click={() => changeZoom(-10)}><ZoomOut size={17} /></button>
            <button title="Fit to view" on:click={() => action('Fit to view')}><Maximize size={17} /></button>
            <button title="Export image" on:click={() => action('Export image')}><ImageDown size={17} /></button>
          </div>
          <!-- 4 -> 6：Share / Account 移到右侧 topbar 最右端 -->
          <div class="topbar-global">
            <button title="Share" on:click={() => action('Share')}><Share2 size={17} /></button>
            <button title="Account" on:click={() => action('Account')}><UserCircle size={18} /></button>
          </div>
        </header>
      </div>

      <main class="main-area">
        <section class="editor-pane">
          <div class="editor-content">
            <div class="editor-line selected"><span>1</span><b>&#123;</b></div>
            {#each lines.slice(1) as line, index}
              <div class:line-highlight={index === 7} class="editor-line"><span>{line[0]}</span><b>{line[1]}</b></div>
            {/each}
          </div>
        </section>
        <section class="graph-pane">
          <div class="canvas-hint">Hold Space and drag to move the canvas</div>
          <div class="graph-card object-card"><header>object <i>{3}</i></header><div><strong>bool</strong><em>true</em></div><div><strong>float</strong><em>0.125</em></div><div><strong>int</strong><em>42</em></div></div>
          <div class="graph-card preview-card"><header>preview <i>{7}</i></header><div><strong>base64</strong><em>aHR0cHM6Ly90cmVhc2UuY29t...</em></div><div><strong>color</strong><em>#4f46e5</em></div><div><strong>uris</strong><em>[2]</em></div></div>
          <div class="graph-card table-card"><header>table_with_header</header><div class="mini-table"><span></span><span>h1</span><span>h2</span><span>0</span><em>11</em><em>12</em><span>1</span><em>21</em><em>22</em></div></div>
          <svg class="connections" viewBox="0 0 900 700" preserveAspectRatio="none" aria-hidden="true"><path d="M220 270 C300 270 290 200 380 200" /><path d="M220 290 C315 290 310 400 410 400" /></svg>
        </section>
      </main>

      <!-- Bottombar 使用同一个父容器的两列网格，宽度与 editor / graph 对齐 -->
      <footer class="bottom-bars">
        <!-- left bottombar：7 移到最右侧 -->
        <!-- 3 -> 5：Ask AI 从侧边栏移到 left bottombar -->
        <div class="left-bottombar"><button class="language-button" on:click={() => action('Language')} >JSON <ChevronDown size={13} /></button><button class="bottombar-ai" title="Ask AI" on:click={() => action('Ask AI')}><Sparkles size={14} /> AI</button><span class="status-notice">{notice}</span><span class="cursor-status">Ln 19, Col 42 <small>(15 selected)</small></span></div>
        <!-- right bottombar：归属 graph，承载路径状态 -->
        <div class="right-bottombar"><span class="path-label">$</span><ChevronDown size={13} /><span class="path-segment">preview</span><ChevronDown size={13} /><span class="path-segment current">unicode</span></div>
      </footer>
    </div>
  </section>
</div>

<style>
  :global(*) { box-sizing: border-box; }
  :global(body) { margin: 0; background: #edf2f7; font-family: "SF Pro Text", "PingFang SC", "Segoe UI", sans-serif; color: #172334; }
  button { font: inherit; cursor: pointer; }
  .prototype-page { min-height: 100vh; padding: 20px; background: radial-gradient(circle at 50% -20%, #fff 0 26%, #edf2f7 72%); }
  .prototype-note { width: min(1600px, 100%); margin: 0 auto 10px; display: flex; align-items: center; gap: 9px; color: #70849a; font-size: 11px; }
  .prototype-note span { width: 7px; height: 7px; border-radius: 50%; background: #df8b3b; box-shadow: 0 0 0 4px #f5dec5; }
  .prototype-note strong { color: #315473; letter-spacing: .1em; font-size: 10px; }
  .prototype-note em { font-style: normal; color: #9aaaba; }
  .app-frame { width: min(1600px, 100%); height: min(900px, calc(100dvh - 62px)); min-height: 0; margin: 0 auto; display: grid; grid-template-columns: 68px 1fr; grid-template-rows: minmax(0, 1fr); overflow: hidden; border: 1px solid #ccd8e4; border-radius: 7px; background: white; box-shadow: 0 18px 50px rgba(40, 65, 90, .15); }
  .left-rail { display: flex; flex-direction: column; align-items: center; border-right: 1px solid #d9e2eb; background: #fff; }
  .rail-logo { width: 50px; height: 50px; margin: 7px 0 9px; border: 0; border-radius: 7px; color: #172334; background: #fff; font: 700 22px/1 ui-monospace, monospace; }
  .rail-group { display: flex; flex-direction: column; align-items: center; gap: 3px; width: 100%; }
  .rail-group button { width: 48px; height: 42px; display: grid; place-items: center; border: 0; border-radius: 6px; color: #556270; background: transparent; }
  .rail-group button:hover, .rail-group button.selected { color: #172334; background: #e8eef6; }
  .rail-divider { width: 43px; height: 1px; margin: 7px 0 9px; background: #dbe3eb; }
  .rail-spacer { flex: 1; }
  .global-group { padding-bottom: 10px; }
  .workspace { min-width: 0; min-height: 0; height: 100%; overflow: hidden; display: grid; grid-template-rows: 54px minmax(0, 1fr) 34px; }
  .top-bars { min-width: 0; display: grid; grid-template-columns: 35% 65%; border-bottom: 1px solid #d9e2eb; background: #fff; }
  .topbar { position: relative; z-index: 10; min-width: 0; display: flex; align-items: center; gap: 8px; padding: 0 10px; background: #fff; }
  .topbar-left { padding-left: 12px; border-right: 1px solid #d9e2eb; }
  .topbar-right { justify-content: space-between; padding: 0 12px 0 22px; }
  .command-button { width: 36px; height: 36px; display: inline-flex; flex: 0 0 auto; align-items: center; justify-content: center; border: 1px solid #d7e1eb; border-radius: 7px; padding: 0; color: #526b83; background: #fff; }
  .command-button:hover { border-color: #aebfd0; background: #fafcfe; }
  .command-button kbd { padding: 3px 4px; border: 1px solid #d9e3ee; border-radius: 4px; color: #5f7287; background: #f7fafd; font-size: 9px; }
  .tabs { display: flex; flex: 1 1 auto; align-items: center; gap: 3px; min-width: 0; overflow: hidden; }
  .tab, .new-tab { height: 32px; display: inline-flex; align-items: center; gap: 9px; flex: 0 0 auto; border: 1px solid transparent; border-radius: 6px; padding: 0 8px; color: #6d7d8f; background: transparent; font-size: 11px; }
  .tab b { color: #a1adba; font-weight: 400; font-size: 13px; }
  .tab.active { border-color: #93b5da; color: #1f3852; background: #f3f8fd; }
  .new-tab { width: 29px; justify-content: center; padding: 0; border-color: #d0dce8; }
  .view-mode { height: 34px; display: flex; align-items: center; gap: 1px; padding: 2px; border-radius: 8px; background: #f0f4f8; }
  .view-mode button { width: 25px; height: 29px; display: grid; place-items: center; border: 0; border-radius: 6px; color: #586676; background: transparent; }
  .view-mode button.active, .view-mode button:hover { color: #172334; background: #fff; box-shadow: 0 2px 5px rgba(45, 65, 83, .12); }
  .graph-top-tools { display: flex; align-items: center; gap: 3px; padding: 3px; border: 1px solid #d2dfe9; border-radius: 8px; background: rgba(255,255,255,.94); box-shadow: 0 3px 9px rgba(47, 74, 97, .07); }
  .graph-top-tools button { width: 32px; height: 32px; display: grid; place-items: center; border: 0; border-radius: 5px; color: #526a80; background: transparent; }
  .graph-top-tools button:hover { color: #205981; background: #edf5fb; }
  .graph-top-tools span { min-width: 35px; color: #778b9e; font-size: 10px; text-align: center; }
  .topbar-global { display: flex; align-items: center; gap: 4px; }
  .topbar-global button { width: 34px; height: 34px; display: grid; place-items: center; border: 0; border-radius: 6px; color: #526a80; background: transparent; }
  .topbar-global button:hover { color: #205981; background: #edf5fb; }
  .command-panel { position: absolute; top: 48px; left: 12px; z-index: 30; width: 300px; padding: 9px; border: 1px solid #cbd8e5; border-radius: 0 0 9px 9px; background: #fff; box-shadow: 0 15px 28px rgba(31, 55, 78, .14); }
  .command-input { height: 37px; display: flex; align-items: center; gap: 9px; margin-bottom: 6px; padding: 0 10px; border: 1px solid #bfcfdf; border-radius: 6px; color: #63788e; background: #f8fbfe; }
  .command-input input { min-width: 0; flex: 1; border: 0; outline: 0; color: #294967; background: transparent; font-size: 13px; }
  .command-panel button { width: 100%; height: 35px; display: flex; align-items: center; gap: 10px; border: 0; border-radius: 5px; padding: 0 9px; color: #4c647b; background: transparent; text-align: left; font-size: 12px; }
  .command-panel button:hover { background: #edf4fa; }
  .command-panel kbd { margin-left: auto; color: #94a6b7; font-size: 10px; }
  .main-area { min-width: 0; min-height: 0; display: grid; grid-template-columns: 35% 65%; }
  .editor-pane, .graph-pane { position: relative; min-width: 0; min-height: 0; overflow: hidden; }
  .editor-pane { border-right: 1px solid #d9e2eb; background: #fff; }
  .editor-content { height: 100%; overflow: hidden; padding: 9px 0; font: 15px/1.86 ui-monospace, SFMono-Regular, Menlo, monospace; }
  .editor-line { display: grid; grid-template-columns: 55px 1fr; min-height: 28px; padding-right: 20px; white-space: pre; }
  .editor-line span { padding-right: 15px; color: #5f7285; text-align: right; user-select: none; }
  .editor-line b { color: #1e486a; font-weight: 400; }
  .editor-line.line-highlight { background: #edf5fc; }
  .graph-pane { background-color: #fbfdff; background-image: radial-gradient(#aec1d2 1px, transparent 1px); background-size: 40px 40px; }
  .canvas-hint { position: absolute; top: 24px; left: 50%; color: #8ba0b2; font-size: 13px; transform: translateX(-50%); }
  .graph-card { position: absolute; z-index: 2; padding: 13px 17px; border: 1px solid #b9c9d7; color: #5a7085; background: rgba(255,255,255,.94); box-shadow: 0 3px 10px rgba(58, 85, 108, .05); font: 13px ui-monospace, monospace; }
  .graph-card header { display: flex; justify-content: space-between; margin-bottom: 10px; color: #60758a; }
  .graph-card header i { color: #98a8b7; font-style: normal; }
  .graph-card div { display: grid; grid-template-columns: 1fr auto; gap: 30px; margin-top: 7px; }
  .graph-card strong { color: #b54c48; font-weight: 500; }
  .graph-card em { color: #1475b8; font-style: normal; }
  .object-card { top: 120px; left: 10%; width: 220px; }
  .preview-card { top: 265px; left: 33%; width: 370px; }
  .table-card { top: 470px; left: 33%; width: 270px; }
  .mini-table { display: grid !important; grid-template-columns: repeat(3, 1fr); gap: 0 !important; border: 1px solid #cbd7e1; text-align: center; }
  .mini-table span, .mini-table em { padding: 5px; border-right: 1px solid #d5dfe7; border-bottom: 1px solid #d5dfe7; }
  .mini-table em { color: #148a74; }
  .connections { position: absolute; inset: 0; z-index: 1; width: 100%; height: 100%; fill: none; stroke: #b4cce2; stroke-width: 1.4; }
  .bottom-bars { min-width: 0; display: grid; grid-template-columns: 35% 65%; border-top: 1px solid #d9e2eb; background: #fff; color: #6d8194; font-size: 11px; }
  .left-bottombar, .right-bottombar { min-width: 0; display: flex; align-items: center; gap: 10px; padding: 0 12px; }
  .left-bottombar { border-right: 1px solid #d9e2eb; }
  .language-button { display: inline-flex; align-items: center; gap: 3px; border: 0; color: #3d5d79; background: transparent; font-size: 12px; }
  .status-notice { min-width: 0; overflow: hidden; color: #9aaaba; text-overflow: ellipsis; white-space: nowrap; }
  .cursor-status { margin-left: auto; flex: 0 0 auto; color: #506f8b; }
  .cursor-status small { color: #94a4b3; }
  .path-label { color: #778d9f; font-family: ui-monospace, monospace; }
  .path-segment { color: #70869a; font-family: ui-monospace, monospace; }
  .path-segment.current { padding: 4px 8px; border-radius: 4px; color: #28567d; background: #eaf3fb; }
  .bottombar-ai { display: inline-flex; align-items: center; gap: 4px; height: 25px; border: 0; border-radius: 5px; padding: 0 7px; color: #9a6426; background: #fff4e4; font-size: 11px; }
  .bottombar-ai:hover { background: #ffe9c9; }
  @media (max-width: 1100px) {
    .topbar-left { padding-left: 8px; gap: 4px; }
    .topbar-right { padding-left: 12px; }
    .command-button { width: 34px; }
    .tab { padding: 0 5px; gap: 5px; }
    .command-panel { left: 8px; width: 270px; }
    .editor-line { grid-template-columns: 38px 1fr; font-size: 12px; }
    .graph-card { font-size: 10px; }
  }
</style>
