<!-- PROTOTYPE: editor chrome placement study. Throwaway route; compare with ?variant=A|B|C. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import {
    ArrowLeft,
    ArrowRight,
    BookOpen,
    Check,
    ChevronDown,
    Copy,
    Download,
    FileInput,
    FolderOpen,
    GitCompareArrows,
    ImageDown,
    Maximize,
    MessageCircle,
    MoreHorizontal,
    PanelLeft,
    Plus,
    Search,
    Share2,
    Sparkles,
    Upload,
    UserCircle,
    Wand2,
    ZoomIn,
    ZoomOut,
  } from 'lucide-svelte';

  type Variant = 'A' | 'B' | 'C';
  const variants: Array<{ id: Variant; name: string; summary: string }> = [
    { id: 'A', name: '分区工具栏', summary: '每个功能贴近它作用的对象' },
    { id: 'B', name: '共享操作带', summary: '编辑器与画布共用一条上下文工具带' },
    { id: 'C', name: '侧边工作区', summary: '文件与工作区操作集中在左侧' },
  ];

  const codeLines = [
    ['1', '{'],
    ['2', '  "object": { "bool": true, "float": 0.125,'],
    ['3', '    "int": 42'],
    ['4', '  },'],
    ['5', '  "preview": {'],
    ['6', '    "base64": "aHR0cHM6Ly90cmVhc2UuY29t...",'],
    ['7', '    "color": "#4f46e5",'],
    ['8', '    "uris": ['],
    ['9', '      "https://treease.com/path?redirect=true"'],
    ['10', '    ]'],
    ['11', '  }'],
    ['12', '}'],
  ];

  let variant: Variant = 'A';
  let activeTab = 'Untitled 2';
  let menu = '';
  let commandOpen = false;
  let zoom = 100;
  let selectedPath = 'object › preview › uris › [1]';
  let notice = '点击按钮感受它作用的区域';
  let copied = false;

  onMount(() => {
    const urlVariant = new URL(window.location.href).searchParams.get('variant');
    if (urlVariant === 'A' || urlVariant === 'B' || urlVariant === 'C') variant = urlVariant;
    const handlePopState = () => {
      const next = new URL(window.location.href).searchParams.get('variant');
      if (next === 'A' || next === 'B' || next === 'C') variant = next;
    };
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  });

  function setVariant(next: Variant) {
    variant = next;
    const url = new URL(window.location.href);
    url.searchParams.set('variant', next);
    window.history.replaceState({}, '', url);
    menu = '';
    notice = `${variants.find((item) => item.id === next)?.name}：${variants.find((item) => item.id === next)?.summary}`;
  }

  function cycle(delta: number) {
    const index = variants.findIndex((item) => item.id === variant);
    setVariant(variants[(index + delta + variants.length) % variants.length].id);
  }

  function action(label: string) {
    menu = '';
    notice = `${label}：这是原型中的交互反馈`;
  }

  function copyPath() {
    copied = true;
    notice = '当前节点路径已复制';
    setTimeout(() => (copied = false), 900);
  }

  function changeZoom(delta: number) {
    zoom = Math.max(50, Math.min(180, zoom + delta));
    notice = `画布缩放：${zoom}%`;
  }
</script>

<svelte:head>
  <title>Treease · Editor chrome prototype</title>
</svelte:head>

<div class="prototype-page" on:click={() => (menu = '')}>
  <div class="prototype-ribbon">
    <span class="prototype-dot"></span>
    <strong>EDITOR CHROME / 原型</strong>
    <span>这不是生产界面，仅用于比较位置和理解成本</span>
  </div>

  {#if variant === 'A'}
    <section class="app-frame variant-a" on:click|stopPropagation>
      <header class="topbar">
        <button class="brand file-trigger" on:click={() => (menu = menu === 'file' ? '' : 'file')}>
          <span class="brand-mark">↗</span><span>Treease</span><ChevronDown size={12} />
        </button>
        <div class="tabs">
          <button class="tab" on:click={() => action('Untitled 4')}>Untitled 4 <span>×</span></button>
          <button class="tab" on:click={() => action('Untitled 2')}>Untitled 2 <span>×</span></button>
          <button class="tab active">{activeTab} <span>×</span></button>
          <button class="new-tab" on:click={() => action('New tab')}><Plus size={13} /></button>
        </div>
        <button class="command-trigger" on:click={() => (commandOpen = !commandOpen)}><Search size={13} /> Commands <kbd>⌘K</kbd></button>
        <div class="top-actions">
          <button class="text-button primary" on:click={() => action('Share')}><Share2 size={13} /> Share</button>
          <button class="icon-button" aria-label="More" on:click={() => (menu = menu === 'more' ? '' : 'more')}><MoreHorizontal size={16} /></button>
          <button class="icon-button" aria-label="Account"><UserCircle size={16} /></button>
        </div>
        {#if menu === 'file'}
          <div class="popover file-menu">
            <button on:click={() => action('Open file')}><FolderOpen size={14} /> Open file <kbd>⌘O</kbd></button>
            <button on:click={() => action('Import')}><Upload size={14} /> Import format…</button>
            <button on:click={() => action('Export')}><Download size={14} /> Export / Save as… <kbd>⌘S</kbd></button>
          </div>
        {:else if menu === 'more'}
          <div class="popover more-menu">
            <button on:click={() => action('Tutorial')}><BookOpen size={14} /> Tutorial</button>
            <button on:click={() => action('Feedback')}><MessageCircle size={14} /> Feedback</button>
            <button on:click={() => action('Settings')}>Settings</button>
          </div>
        {/if}
        {#if commandOpen}
          <div class="command-popover">
            <div class="command-heading"><Search size={14} /> Search commands</div>
            <button on:click={() => action('Format document')}>Format document <kbd>⌥⇧F</kbd></button>
            <button on:click={() => action('Minify document')}>Minify document</button>
            <button on:click={() => action('Ask AI')}>Ask AI about this document</button>
          </div>
        {/if}
      </header>

      <div class="split-view">
        <section class="pane editor-pane">
          <div class="pane-toolbar">
            <button class="select-like" on:click={() => action('Language: JSON')}>JSON <ChevronDown size={12} /></button>
            <span class="toolbar-divider"></span>
            <button class="tool-button emphasized" on:click={() => action('Format')}><Wand2 size={13} /> Format</button>
            <button class="tool-button" on:click={() => action('Transform')}>Transform <ChevronDown size={12} /></button>
            <button class="tool-button ai" on:click={() => action('Ask AI')}><Sparkles size={13} /> Ask AI</button>
          </div>
          <div class="code-area">
            {#each codeLines as line, index}
              <div class:code-selected={index === 7} class="code-line"><span class="line-number">{line[0]}</span><span>{line[1]}</span></div>
            {/each}
          </div>
        </section>
        <section class="pane graph-pane">
          <div class="pane-toolbar graph-toolbar">
            <button class="tool-button search-button" on:click={() => action('Search nodes')}><Search size={13} /> Search nodes</button>
            <span class="toolbar-divider"></span>
            <button class="square-button" on:click={() => changeZoom(-10)} aria-label="Zoom out"><ZoomOut size={13} /></button>
            <button class="zoom-value" on:click={() => (zoom = 100)}>{zoom}%</button>
            <button class="square-button" on:click={() => changeZoom(10)} aria-label="Zoom in"><ZoomIn size={13} /></button>
            <button class="tool-button" on:click={() => action('Fit to view')}><Maximize size={13} /> Fit</button>
            <button class="tool-button" on:click={() => action('Export image')}><ImageDown size={13} /> Export</button>
          </div>
          <div class="path-bar"><span>Current node</span><strong>{selectedPath}</strong><button on:click={copyPath} aria-label="Copy path">{#if copied}<Check size={13} />{:else}<Copy size={13} />{/if}</button></div>
          {@render graphMarkup()}
        </section>
      </div>
      <footer class="statusbar"><span class="status-language">JSON <ChevronDown size={11} /></span><span class="status-message">{notice}</span><span class="status-selection">{selectedPath}</span><span>Ln 19, Col 42 <b>(15 selected)</b></span></footer>
    </section>
  {:else if variant === 'B'}
    <section class="app-frame variant-b" on:click|stopPropagation>
      <header class="topbar">
        <button class="brand"><span class="brand-mark">↗</span><span>Treease</span></button>
        <div class="file-actions"><button on:click={() => action('Open')}><FolderOpen size={13} /> Open</button><button on:click={() => action('Save')}><Download size={13} /> Save</button></div>
        <div class="tabs"><button class="tab">Untitled 4 <span>×</span></button><button class="tab">Untitled 2 <span>×</span></button><button class="tab active">{activeTab} <span>×</span></button><button class="new-tab" on:click={() => action('New tab')}><Plus size={13} /></button></div>
        <div class="top-actions"><button class="text-button primary" on:click={() => action('Share')}><Share2 size={13} /> Share</button><button class="icon-button"><MoreHorizontal size={16} /></button><button class="icon-button"><UserCircle size={16} /></button></div>
      </header>
      <div class="shared-rail">
        <div class="rail-group"><span class="rail-label">EDITOR</span><button class="select-like" on:click={() => action('Language: JSON')}>JSON <ChevronDown size={12} /></button><button class="tool-button emphasized" on:click={() => action('Format')}><Wand2 size={13} /> Format</button><button class="tool-button" on:click={() => action('Transform')}>Transform <ChevronDown size={12} /></button><button class="tool-button ai" on:click={() => action('Ask AI')}><Sparkles size={13} /> Ask AI</button></div>
        <div class="rail-separator"></div>
        <div class="rail-group"><span class="rail-label">GRAPH</span><button class="tool-button search-button" on:click={() => action('Search nodes')}><Search size={13} /> Search nodes</button><button class="square-button" on:click={() => changeZoom(-10)}><ZoomOut size={13} /></button><span class="zoom-value">{zoom}%</span><button class="square-button" on:click={() => changeZoom(10)}><ZoomIn size={13} /></button><button class="tool-button" on:click={() => action('Fit to view')}><Maximize size={13} /> Fit</button><button class="tool-button" on:click={() => action('Export image')}><ImageDown size={13} /> Export</button></div>
      </div>
      <div class="split-view">
        <section class="pane editor-pane"><div class="pane-caption">Source document <span>⌘1</span></div><div class="code-area">{#each codeLines as line, index}<div class:code-selected={index === 7} class="code-line"><span class="line-number">{line[0]}</span><span>{line[1]}</span></div>{/each}</div></section>
        <section class="pane graph-pane"><div class="graph-context-label">Graph preview <span>⌘2</span></div>{@render graphMarkup()}</section>
      </div>
      <footer class="statusbar"><span class="status-language">JSON</span><span class="status-message">{notice}</span><span class="status-selection">Path: {selectedPath}</span><span>Ln 19, Col 42 <b>(15 selected)</b></span></footer>
    </section>
  {:else}
    <section class="app-frame variant-c" on:click|stopPropagation>
      <header class="topbar compact-top"><button class="brand"><span class="brand-mark">↗</span><span>Treease</span></button><div class="tabs"><button class="tab">Untitled 4 <span>×</span></button><button class="tab">Untitled 2 <span>×</span></button><button class="tab active">{activeTab} <span>×</span></button><button class="new-tab" on:click={() => action('New tab')}><Plus size={13} /></button></div><button class="command-trigger" on:click={() => (commandOpen = !commandOpen)}><Search size={13} /> Commands <kbd>⌘K</kbd></button><div class="top-actions"><button class="text-button primary" on:click={() => action('Share')}><Share2 size={13} /> Share</button><button class="icon-button"><UserCircle size={16} /></button></div></header>
      <div class="sidebar-layout">
        <aside class="workspace-sidebar"><div class="sidebar-title"><PanelLeft size={14} /> WORKSPACE</div><button class="side-action active" on:click={() => action('Documents')}><FileInput size={14} /> Documents <span>3</span></button><button class="side-action" on:click={() => action('Open file')}><FolderOpen size={14} /> Open file <kbd>⌘O</kbd></button><button class="side-action" on:click={() => action('Import')}><Upload size={14} /> Import</button><button class="side-action" on:click={() => action('Export')}><Download size={14} /> Export</button><div class="sidebar-rule"></div><div class="sidebar-title">TOOLS</div><button class="side-action" on:click={() => action('Format')}><Wand2 size={14} /> Format</button><button class="side-action" on:click={() => action('Compare')}><GitCompareArrows size={14} /> Compare</button><button class="side-action ai-side" on:click={() => action('Ask AI')}><Sparkles size={14} /> Ask AI</button><div class="sidebar-hint">侧栏把“工作区级”操作<br />和“文本级”操作集中起来</div></aside>
        <section class="pane editor-pane"><div class="pane-caption">Source document <span>JSON · Untitled 2</span></div><div class="code-area">{#each codeLines as line, index}<div class:code-selected={index === 7} class="code-line"><span class="line-number">{line[0]}</span><span>{line[1]}</span></div>{/each}</div><div class="editor-local-status">JSON <span>•</span> Ln 19, Col 42</div></section>
        <section class="pane graph-pane"><div class="floating-canvas-tools"><button on:click={() => action('Search nodes')} aria-label="Search nodes"><Search size={14} /></button><div class="tool-stack"><button on:click={() => changeZoom(10)}><ZoomIn size={14} /></button><span>{zoom}%</span><button on:click={() => changeZoom(-10)}><ZoomOut size={14} /></button></div><button on:click={() => action('Fit to view')}><Maximize size={14} /></button><button on:click={() => action('Export image')}><ImageDown size={14} /></button></div><div class="floating-path"><span>{selectedPath}</span><button on:click={copyPath}>{#if copied}<Check size={13} />{:else}<Copy size={13} />{/if}</button></div>{@render graphMarkup()}</section>
      </div>
      <footer class="statusbar"><span class="status-message">{notice}</span><span class="status-selection">Selected: {selectedPath}</span><span>Ln 19, Col 42 <b>(15 selected)</b></span></footer>
    </section>
  {/if}

  <div class="prototype-switcher" on:click|stopPropagation>
    <button aria-label="Previous variant" on:click={() => cycle(-1)}><ArrowLeft size={15} /></button>
    <div class="switcher-copy"><strong>{variant} · {variants.find((item) => item.id === variant)?.name}</strong><span>{variants.find((item) => item.id === variant)?.summary}</span></div>
    <div class="variant-dots">{#each variants as item}<button class:current={item.id === variant} aria-label={`Variant ${item.id}`} on:click={() => setVariant(item.id)}></button>{/each}</div>
    <button aria-label="Next variant" on:click={() => cycle(1)}><ArrowRight size={15} /></button>
  </div>
</div>

{#snippet graphMarkup()}
  <div class="graph-surface">
    <div class="canvas-hint">Hold Space and drag to move the canvas</div>
    <div class="graph-card card-object"><span class="card-title">object</span><span class="card-count">{3}</span><div><b>bool</b><em>true</em></div><div><b>float</b><em>0.125</em></div><div><b>int</b><em>42</em></div></div>
    <div class="graph-card card-preview"><span class="card-title">preview</span><span class="card-count">{7}</span><div><b>base64</b><em>aHR0cHM6Ly90cmVhc2UuY29t...</em></div><div><b>color</b><em>#4f46e5</em></div><div><b>uris</b><em>[2]</em></div></div>
    <div class="graph-card card-table"><span class="card-title">table_with_header</span><div class="mini-table"><span></span><span>h1</span><span>h2</span><span>0</span><em>11</em><em>12</em><span>1</span><em>21</em><em>22</em></div></div>
    <svg class="graph-lines" viewBox="0 0 800 430" preserveAspectRatio="none" aria-hidden="true"><path d="M220 178 C280 178 270 123 340 123"/><path d="M220 197 C280 197 275 300 360 300"/><path d="M220 218 C275 218 280 384 350 384"/></svg>
  </div>
{/snippet}

<style>
  :global(*) { box-sizing: border-box; }
  :global(body) { background: #eaf0f7; }
  button { font: inherit; }
  .prototype-page { min-height: 100vh; padding: 26px 28px 88px; color: #18283d; background: radial-gradient(circle at 50% -10%, #f8fbff 0 25%, #eaf0f7 70%); }
  .prototype-ribbon { width: min(1500px, 100%); margin: 0 auto 12px; display: flex; align-items: center; gap: 10px; color: #60738b; font-size: 11px; letter-spacing: .02em; }
  .prototype-ribbon strong { color: #23466d; font-size: 10px; letter-spacing: .12em; }
  .prototype-dot { width: 7px; height: 7px; border-radius: 50%; background: #e78a38; box-shadow: 0 0 0 4px #f8dfc7; }
  .app-frame { position: relative; width: min(1500px, 100%); height: min(820px, calc(100vh - 90px)); min-height: 590px; margin: 0 auto; overflow: hidden; border: 1px solid #cbd7e5; border-radius: 12px; background: #fff; box-shadow: 0 22px 60px rgba(36, 63, 93, .13); }
  .topbar { position: relative; z-index: 5; height: 48px; display: grid; grid-template-columns: auto minmax(250px, 1fr) auto auto; align-items: center; gap: 12px; padding: 0 14px; border-bottom: 1px solid #d9e3ef; background: #fff; }
  .brand, .file-trigger { display: inline-flex; align-items: center; gap: 6px; height: 30px; border: 0; background: transparent; color: #183653; font-weight: 700; cursor: pointer; }
  .brand-mark { width: 23px; height: 23px; display: grid; place-items: center; border-radius: 7px; color: white; background: #193c5d; font-size: 15px; }
  .tabs { min-width: 0; display: flex; align-items: center; gap: 4px; overflow: hidden; }
  .tab, .new-tab { height: 29px; display: inline-flex; align-items: center; gap: 12px; flex: 0 0 auto; border: 1px solid transparent; border-radius: 6px; padding: 0 10px; color: #6b7e94; background: transparent; font-size: 12px; cursor: pointer; }
  .tab span { color: #9baabd; font-size: 14px; }
  .tab.active { border-color: #78aee4; color: #193b61; background: #eef6ff; box-shadow: inset 0 0 0 1px #d3e7fc; }
  .new-tab { width: 29px; justify-content: center; padding: 0; border-color: #cbd8e6; color: #3b668d; }
  .command-trigger, .text-button, .icon-button, .file-actions button { height: 30px; display: inline-flex; align-items: center; justify-content: center; gap: 7px; border: 1px solid #d5e0ec; border-radius: 6px; padding: 0 9px; color: #52708e; background: #fff; font-size: 11px; cursor: pointer; white-space: nowrap; }
  .command-trigger kbd, kbd { margin-left: auto; color: #8da0b5; font-family: inherit; font-size: 10px; }
  .top-actions { display: flex; align-items: center; gap: 5px; }
  .text-button.primary { color: #fff; border-color: #376d9e; background: #376d9e; }
  .icon-button { width: 30px; padding: 0; border-color: transparent; }
  .icon-button:hover, .tool-button:hover, .square-button:hover, .side-action:hover { background: #edf4fb; color: #21547e; }
  .popover, .command-popover { position: absolute; top: 42px; z-index: 20; width: 220px; padding: 6px; border: 1px solid #cbd9e7; border-radius: 8px; background: white; box-shadow: 0 14px 34px rgba(21, 48, 77, .15); }
  .file-menu { left: 9px; } .more-menu { right: 9px; }
  .popover button, .command-popover button { width: 100%; height: 30px; display: flex; align-items: center; gap: 9px; padding: 0 9px; border: 0; border-radius: 5px; color: #36536e; background: transparent; text-align: left; font-size: 11px; cursor: pointer; }
  .popover button:hover, .command-popover button:hover { background: #edf5fc; }
  .command-popover { right: 125px; width: 290px; padding: 8px; }
  .command-heading { display: flex; gap: 7px; align-items: center; padding: 6px 8px 8px; color: #7790a8; font-size: 10px; text-transform: uppercase; letter-spacing: .08em; }
  .split-view { height: calc(100% - 80px); display: grid; grid-template-columns: 39% 61%; min-height: 0; }
  .pane { min-width: 0; min-height: 0; position: relative; overflow: hidden; }
  .editor-pane { border-right: 1px solid #d9e3ef; background: #fbfcfe; }
  .graph-pane { background: #f7fafe; }
  .pane-toolbar, .shared-rail { height: 42px; display: flex; align-items: center; gap: 5px; padding: 0 12px; border-bottom: 1px solid #d9e3ef; background: rgba(255,255,255,.92); }
  .pane-toolbar { white-space: nowrap; }
  .graph-toolbar { background: #fbfdff; }
  .tool-button, .select-like, .square-button { height: 28px; display: inline-flex; align-items: center; gap: 6px; border: 1px solid transparent; border-radius: 5px; padding: 0 8px; color: #5b728a; background: transparent; font-size: 11px; cursor: pointer; white-space: nowrap; }
  .select-like { color: #244d73; border-color: #cad8e7; background: #f5f9fd; font-weight: 650; }
  .tool-button.emphasized { color: #275e8c; background: #e8f2fc; }
  .tool-button.ai { color: #956024; background: #fff5e6; }
  .square-button { width: 28px; justify-content: center; padding: 0; border-color: #d7e2ed; background: #fff; }
  .zoom-value { min-width: 38px; color: #57718c; font-size: 11px; text-align: center; }
  .toolbar-divider, .rail-separator { width: 1px; height: 19px; margin: 0 3px; background: #dce5ef; }
  .search-button { color: #285a87; border-color: #c7d9ea; background: #f3f8fd; }
  .code-area { height: calc(100% - 42px); overflow: auto; padding: 13px 0; color: #49627b; font: 12px/1.85 ui-monospace, SFMono-Regular, Menlo, monospace; }
  .code-line { display: grid; grid-template-columns: 42px 1fr; min-height: 22px; padding-right: 15px; white-space: pre; }
  .code-line.code-selected { background: #e8f2fc; }
  .line-number { padding-right: 12px; color: #9cabbc; text-align: right; user-select: none; }
  .path-bar { position: absolute; z-index: 2; top: 52px; left: 14px; right: 14px; height: 30px; display: flex; align-items: center; gap: 8px; padding: 0 8px; border: 1px solid #d2e0ed; border-radius: 5px; color: #8194a7; background: rgba(255,255,255,.9); font-size: 10px; }
  .path-bar strong { min-width: 0; overflow: hidden; color: #3d5f7d; font: 10px ui-monospace, monospace; text-overflow: ellipsis; white-space: nowrap; }
  .path-bar button, .floating-path button { margin-left: auto; display: grid; place-items: center; border: 0; color: #66829d; background: transparent; cursor: pointer; }
  .graph-surface { position: absolute; inset: 0; overflow: hidden; background-color: #f7fafe; background-image: linear-gradient(#e5edf5 1px, transparent 1px), linear-gradient(90deg, #e5edf5 1px, transparent 1px); background-size: 24px 24px; }
  .pane-toolbar + .graph-surface { top: 42px; }
  .path-bar ~ .graph-surface { top: 42px; }
  .canvas-hint { position: absolute; top: 14px; left: 50%; color: #8ca0b5; font-size: 10px; transform: translateX(-50%); white-space: nowrap; }
  .graph-card { position: absolute; z-index: 1; display: grid; grid-template-columns: 1fr auto; gap: 6px 18px; padding: 9px 12px; border: 1px solid #bdccd9; border-radius: 2px; color: #5b6f83; background: rgba(255,255,255,.94); box-shadow: 0 3px 9px rgba(57, 86, 112, .06); font: 11px ui-monospace, monospace; }
  .graph-card .card-title { grid-column: 1; color: #677d92; }
  .graph-card .card-count { grid-column: 2; color: #94a5b6; }
  .graph-card div { display: contents; }
  .graph-card b { color: #c3514b; font-weight: 500; }
  .graph-card em { color: #1476ba; font-style: normal; text-align: right; }
  .card-object { top: 112px; left: 7%; width: 180px; }
  .card-preview { top: 205px; left: 35%; width: 300px; }
  .card-table { top: 355px; left: 36%; width: 220px; }
  .mini-table { grid-column: 1 / -1; display: grid !important; grid-template-columns: repeat(3, 1fr); border: 1px solid #cbd7e2; text-align: center; }
  .mini-table span, .mini-table em { padding: 3px; border-right: 1px solid #d5dfe8; border-bottom: 1px solid #d5dfe8; }
  .mini-table em { color: #15856f; }
  .graph-lines { position: absolute; inset: 0; width: 100%; height: 100%; stroke: #a9c5e1; stroke-width: 1.2; fill: none; }
  .statusbar { position: absolute; right: 0; bottom: 0; left: 0; height: 32px; display: flex; align-items: center; gap: 15px; padding: 0 12px; border-top: 1px solid #d9e3ef; color: #758aa0; background: #fff; font-size: 10px; }
  .status-language { display: inline-flex; align-items: center; gap: 4px; color: #385d7f; font-weight: 650; }
  .status-message { color: #9aabba; }
  .status-selection { min-width: 0; flex: 1; overflow: hidden; color: #66809a; font-family: ui-monospace, monospace; text-overflow: ellipsis; white-space: nowrap; }
  .statusbar > span:last-child { white-space: nowrap; }
  .statusbar b { color: #a2afbd; font-weight: 400; }

  .variant-b .topbar { grid-template-columns: auto auto minmax(250px, 1fr) auto; }
  .file-actions { display: flex; gap: 3px; }
  .file-actions button { border-color: transparent; padding: 0 7px; }
  .shared-rail { height: 50px; justify-content: space-between; padding: 0 14px; background: #fdfefe; }
  .rail-group { display: flex; align-items: center; gap: 5px; }
  .rail-label { margin-right: 4px; color: #9aabbb; font-size: 9px; font-weight: 700; letter-spacing: .1em; }
  .shared-rail + .split-view { height: calc(100% - 82px); }
  .rail-separator { height: 27px; margin: 0 12px; }
  .pane-caption { height: 37px; display: flex; align-items: center; justify-content: space-between; padding: 0 14px; border-bottom: 1px solid #e3ebf3; color: #73889d; font-size: 10px; text-transform: uppercase; letter-spacing: .08em; }
  .pane-caption span, .graph-context-label span { color: #a2b1bf; font-size: 9px; text-transform: none; letter-spacing: 0; }
  .variant-b .code-area { height: calc(100% - 37px); }
  .graph-context-label { height: 37px; display: flex; align-items: center; justify-content: space-between; padding: 0 14px; border-bottom: 1px solid #e3ebf3; color: #73889d; font-size: 10px; text-transform: uppercase; letter-spacing: .08em; }
  .graph-context-label + .graph-surface { top: 37px; }

  .compact-top { grid-template-columns: auto minmax(250px, 1fr) auto auto; }
  .sidebar-layout { height: calc(100% - 80px); display: grid; grid-template-columns: 192px 32% 1fr; min-height: 0; }
  .workspace-sidebar { display: flex; flex-direction: column; gap: 3px; padding: 14px 10px; border-right: 1px solid #d9e3ef; background: #f8fbfe; }
  .sidebar-title { display: flex; align-items: center; gap: 7px; margin: 2px 7px 8px; color: #8b9caf; font-size: 9px; font-weight: 750; letter-spacing: .12em; }
  .side-action { height: 31px; display: flex; align-items: center; gap: 9px; border: 0; border-radius: 5px; padding: 0 8px; color: #5a728b; background: transparent; text-align: left; font-size: 11px; cursor: pointer; }
  .side-action span { margin-left: auto; color: #9aacbd; font-size: 10px; }
  .side-action kbd { margin-left: auto; }
  .side-action.active { color: #225581; background: #e6f1fb; font-weight: 650; }
  .ai-side { color: #996124; }
  .sidebar-rule { height: 1px; margin: 11px 6px; background: #dce6ef; }
  .sidebar-hint { margin: auto 7px 2px; color: #99aabd; font-size: 10px; line-height: 1.65; }
  .sidebar-layout .code-area { height: calc(100% - 67px); }
  .editor-local-status { position: absolute; right: 0; bottom: 0; left: 0; height: 28px; display: flex; gap: 8px; align-items: center; padding: 0 12px; border-top: 1px solid #e1eaf2; color: #718aa1; background: rgba(255,255,255,.9); font-size: 10px; }
  .editor-local-status span { color: #bac7d3; }
  .floating-canvas-tools { position: absolute; z-index: 4; top: 14px; right: 14px; display: flex; flex-direction: column; align-items: center; gap: 3px; padding: 4px; border: 1px solid #c7d7e6; border-radius: 7px; background: rgba(255,255,255,.93); box-shadow: 0 6px 16px rgba(40, 74, 107, .1); }
  .floating-canvas-tools button { width: 28px; height: 27px; display: grid; place-items: center; border: 0; border-radius: 4px; color: #55738f; background: transparent; cursor: pointer; }
  .floating-canvas-tools button:hover { color: #1f5988; background: #eaf4fc; }
  .tool-stack { display: flex; flex-direction: column; align-items: center; border-top: 1px solid #e0e8ef; border-bottom: 1px solid #e0e8ef; }
  .tool-stack span { color: #6b8298; font-size: 9px; }
  .floating-path { position: absolute; z-index: 3; right: 14px; bottom: 13px; left: 14px; height: 30px; display: flex; align-items: center; padding: 0 8px; border: 1px solid #cbdbe9; border-radius: 5px; color: #496b88; background: rgba(255,255,255,.88); font: 10px ui-monospace, monospace; }
  .floating-path button { height: 24px; width: 24px; }
  .variant-c .graph-pane .graph-surface { top: 0; }
  .variant-c .sidebar-layout { height: calc(100% - 80px); }

  .prototype-switcher { position: fixed; z-index: 50; bottom: 18px; left: 50%; display: flex; align-items: center; gap: 11px; min-width: 330px; padding: 7px 9px; border: 1px solid #24476a; border-radius: 10px; color: #eaf4fc; background: #1f3e5d; box-shadow: 0 12px 30px rgba(22, 48, 75, .25); transform: translateX(-50%); }
  .prototype-switcher > button { width: 28px; height: 28px; display: grid; place-items: center; border: 1px solid #557898; border-radius: 6px; color: #e6f2fc; background: transparent; cursor: pointer; }
  .prototype-switcher > button:hover { background: #345c81; }
  .switcher-copy { display: flex; flex-direction: column; min-width: 165px; gap: 2px; }
  .switcher-copy strong { font-size: 11px; }
  .switcher-copy span { color: #a9c2d8; font-size: 10px; }
  .variant-dots { display: flex; gap: 5px; margin-left: auto; }
  .variant-dots button { width: 6px; height: 6px; padding: 0; border: 0; border-radius: 50%; background: #7693ad; cursor: pointer; }
  .variant-dots button.current { background: #fff; box-shadow: 0 0 0 3px #4f789c; }
  @media (max-width: 960px) {
    .prototype-page { padding: 12px 10px 82px; }
    .prototype-ribbon span:not(.prototype-dot), .command-trigger { display: none; }
    .topbar { grid-template-columns: auto minmax(0, 1fr) auto; gap: 7px; }
    .top-actions .text-button { width: 30px; padding: 0; font-size: 0; }
    .top-actions .text-button svg { display: block; }
    .pane-toolbar .tool-button:not(.search-button), .shared-rail .tool-button:not(.search-button) { padding: 0 5px; font-size: 0; }
    .pane-toolbar .tool-button svg, .shared-rail .tool-button svg { display: block; }
    .sidebar-layout { grid-template-columns: 155px 38% 1fr; }
    .prototype-switcher { min-width: 300px; }
  }
</style>
