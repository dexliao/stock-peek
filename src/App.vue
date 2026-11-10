<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface WatchItem {
  code: string;
  name: string;
  costPrice: number;
  shares: number;
}

interface SearchHit {
  code: string;
  name: string;
}

interface QuoteRow extends WatchItem {
  price: number | null;
  changePct: number | null;
  pnl: number | null;
  date: string | null;
  error: string | null;
}

interface QuoteBatch {
  rows: QuoteRow[];
  fetchedAt: number;
}

interface WindowFlags {
  boss: boolean;
  clickThrough: boolean;
}

interface AppSettings {
  opacity: number;
  refreshSeconds: number;
}

interface Todo {
  id: number;
  text: string;
  done: boolean;
}

type Sheet = "none" | "manage" | "add" | "edit" | "settings";

const DEFAULT_REFRESH_SECONDS = 10;
const DEFAULT_SHARES = 100;
const MIN_REFRESH_SECONDS = 5;
const MAX_REFRESH_SECONDS = 60;

const rows = ref<QuoteRow[]>([]);
const loading = ref(true);
const refreshing = ref(false);
const lastUpdated = ref(0);
const refreshError = ref("");

const bossMode = ref(false);
const clickThrough = ref(false);
const settings = ref<AppSettings>({
  opacity: 0.75,
  refreshSeconds: DEFAULT_REFRESH_SECONDS,
});
const settingsError = ref("");

const sheet = ref<Sheet>("none");
const manageError = ref("");

const searchQuery = ref("");
const searchResults = ref<SearchHit[]>([]);
const searching = ref(false);
const searchError = ref("");
const selected = ref<SearchHit | null>(null);
const newCost = ref("");
const newShares = ref(String(DEFAULT_SHARES));
const addError = ref("");

const editing = ref<QuoteRow | null>(null);
const editCost = ref("");
const editShares = ref("");
const editError = ref("");
const confirmDelete = ref(false);

const searchInput = ref<HTMLInputElement | null>(null);
const costInput = ref<HTMLInputElement | null>(null);

const todos = ref<Todo[]>([
  { id: 1, text: "整理本周会议纪要", done: true },
  { id: 2, text: "回复客户邮件", done: false },
  { id: 3, text: "更新项目进度", done: false },
  { id: 4, text: "购买咖啡豆", done: false },
  { id: 5, text: "预约牙医", done: false },
]);

let refreshTimer: number | undefined;
let searchTimer: number | undefined;
let refreshInFlight = false;
let searchSeq = 0;
let unlistenMode: UnlistenFn | undefined;

const hasRows = computed(() => rows.value.length > 0);
const errorCount = computed(() => rows.value.filter((row) => row.error).length);
const remainingTodos = computed(
  () => todos.value.filter((todo) => !todo.done).length,
);
const opacityPercent = computed(() => Math.round(settings.value.opacity * 100));
const refreshSeconds = computed(() => settings.value.refreshSeconds);
const widgetStyle = computed(() => ({
  "--window-opacity": String(settings.value.opacity),
}));
const updatedLabel = computed(() =>
  lastUpdated.value ? formatTime(lastUpdated.value) : "--:--:--",
);
const manageStatus = computed(() => {
  if (refreshError.value) return "行情延迟";
  if (refreshing.value) return "更新中";
  if (errorCount.value > 0) return `${errorCount.value} 只延迟`;
  return lastUpdated.value ? `更新 ${updatedLabel.value}` : "等待行情";
});
const hoverSummary = computed(() => {
  if (!rows.value.length) return "";
  const parts = rows.value.slice(0, 3).map((row) => {
    const name = row.name.length > 4 ? `${row.name.slice(0, 4)}…` : row.name;
    const price = row.price == null ? "--" : row.price.toFixed(2);
    const pct =
      row.changePct == null
        ? "--"
        : `${row.changePct > 0 ? "+" : ""}${row.changePct.toFixed(1)}%`;
    return `${name} ${price} ${pct}`;
  });
  const extra = rows.value.length > 3 ? ` · +${rows.value.length - 3}` : "";
  return parts.join(" · ") + extra;
});

function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString("zh-CN", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function formatPrice(value: number | null | undefined): string {
  return value == null || !Number.isFinite(value) ? "--" : value.toFixed(2);
}

function formatSigned(value: number | null | undefined, suffix = ""): string {
  if (value == null || !Number.isFinite(value)) return "--";
  const normalized = Math.abs(value) < 0.005 ? 0 : value;
  const sign = normalized > 0 ? "+" : "";
  return `${sign}${normalized.toFixed(2)}${suffix}`;
}

function formatPct(value: number | null | undefined): string {
  return formatSigned(value, "%");
}

function formatPnl(value: number | null | undefined): string {
  return formatSigned(value);
}

function toggleTodo(todo: Todo) {
  todo.done = !todo.done;
}

function applyIncomingRows(incoming: QuoteRow[]) {
  const byCode = new Map(incoming.map((row) => [row.code, row]));
  rows.value = rows.value.map((row) => {
    const next = byCode.get(row.code);
    if (!next) return row;
    return {
      ...row,
      costPrice: next.costPrice,
      shares: next.shares,
      price: next.price ?? row.price,
      changePct: next.changePct ?? row.changePct,
      pnl: next.pnl ?? row.pnl,
      date: next.date ?? row.date,
      error: next.error ?? null,
    };
  });
}

async function refreshQuotes() {
  if (refreshInFlight) return;
  refreshInFlight = true;
  refreshing.value = true;

  try {
    const batch = await invoke<QuoteBatch>("refresh_quotes");
    applyIncomingRows(batch.rows);

    if (batch.rows.some((row) => row.price != null)) {
      lastUpdated.value = batch.fetchedAt;
    }
    refreshError.value =
      batch.rows.length > 0 && batch.rows.every((row) => row.price == null)
        ? "行情暂不可用"
        : "";
  } catch (error) {
    refreshError.value = String(error);
  } finally {
    refreshInFlight = false;
    refreshing.value = false;
  }
}

async function reload() {
  try {
    const items = await invoke<WatchItem[]>("get_watchlist");
    const previous = new Map(rows.value.map((row) => [row.code, row]));
    rows.value = items.map((item) => {
      const old = previous.get(item.code);
      return {
        ...item,
        price: old?.price ?? null,
        changePct: old?.changePct ?? null,
        pnl: old?.pnl ?? null,
        date: old?.date ?? null,
        error: old?.error ?? null,
      };
    });
    await refreshQuotes();
  } catch (error) {
    refreshError.value = String(error);
  } finally {
    loading.value = false;
  }
}

async function loadSettings() {
  try {
    settings.value = await invoke<AppSettings>("get_settings");
  } catch {
    // Keep the defaults if settings cannot be loaded.
  }
}

function scheduleRefresh() {
  if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
  const seconds = Math.min(
    MAX_REFRESH_SECONDS,
    Math.max(MIN_REFRESH_SECONDS, settings.value.refreshSeconds),
  );
  refreshTimer = window.setInterval(() => {
    void refreshQuotes();
  }, seconds * 1000);
}

async function persistSettings() {
  settingsError.value = "";
  scheduleRefresh();
  try {
    const saved = await invoke<AppSettings>("update_settings", {
      next: {
        opacity: settings.value.opacity,
        refreshSeconds: settings.value.refreshSeconds,
      },
    });
    settings.value = saved;
    scheduleRefresh();
  } catch (error) {
    settingsError.value = String(error);
  }
}

function handleWindowFlags(flags: WindowFlags) {
  const wasClickThrough = clickThrough.value;
  bossMode.value = flags.boss;
  clickThrough.value = flags.clickThrough;

  if (flags.boss || (wasClickThrough && !flags.clickThrough)) {
    closeSheet();
  }
}

async function refreshWindowFlags() {
  try {
    handleWindowFlags(await invoke<WindowFlags>("get_window_flags"));
  } catch {
    // Keep the local state if the command is unavailable during startup.
  }
}

function syncBossFromViewport() {
  const narrow = window.innerWidth <= 48 && window.innerHeight <= 48;
  if (narrow === bossMode.value) return;
  bossMode.value = narrow;
  if (narrow) closeSheet();
}

function openManage() {
  manageError.value = "";
  sheet.value = "manage";
}

function openSettings() {
  settingsError.value = "";
  sheet.value = "settings";
}

function closeSheet() {
  sheet.value = "none";
  selected.value = null;
  editing.value = null;
  confirmDelete.value = false;
  addError.value = "";
  editError.value = "";
  searchError.value = "";
  manageError.value = "";
  settingsError.value = "";
  if (searchTimer !== undefined) {
    window.clearTimeout(searchTimer);
    searchTimer = undefined;
  }
}

function backToManage() {
  selected.value = null;
  editing.value = null;
  confirmDelete.value = false;
  addError.value = "";
  editError.value = "";
  searchError.value = "";
  manageError.value = "";
  settingsError.value = "";
  if (searchTimer !== undefined) {
    window.clearTimeout(searchTimer);
    searchTimer = undefined;
  }
  sheet.value = "manage";
}

function resetAdd() {
  searchSeq += 1;
  if (searchTimer !== undefined) {
    window.clearTimeout(searchTimer);
    searchTimer = undefined;
  }
  searchQuery.value = "";
  searchResults.value = [];
  searching.value = false;
  selected.value = null;
  newCost.value = "";
  newShares.value = String(DEFAULT_SHARES);
  addError.value = "";
  searchError.value = "";
}

function openAdd() {
  resetAdd();
  sheet.value = "add";
  nextTick(() => searchInput.value?.focus());
}

function onSearchInput() {
  searchSeq += 1;
  selected.value = null;
  addError.value = "";
  searchError.value = "";

  const query = searchQuery.value.trim();
  if (!query) {
    searchResults.value = [];
    searching.value = false;
    return;
  }

  const seq = searchSeq;
  if (searchTimer !== undefined) window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => {
    void runSearch(query, seq);
  }, 260);
}

async function runSearch(query: string, seq: number) {
  if (seq !== searchSeq) return;
  searching.value = true;

  try {
    const hits = await invoke<SearchHit[]>("search_stocks", { query });
    if (seq !== searchSeq || searchQuery.value.trim() !== query) return;
    searchResults.value = hits;
  } catch (error) {
    if (seq === searchSeq) searchError.value = String(error);
  } finally {
    if (seq === searchSeq) searching.value = false;
  }
}

function selectHit(hit: SearchHit) {
  selected.value = hit;
  newCost.value = "";
  newShares.value = String(DEFAULT_SHARES);
  addError.value = "";
  nextTick(() => costInput.value?.focus());
}

function resetSelection() {
  selected.value = null;
  addError.value = "";
  nextTick(() => searchInput.value?.focus());
}

async function addSelected() {
  const hit = selected.value;
  if (!hit) return;

  const cost = Number(newCost.value);
  const shares = Number(newShares.value);
  if (!Number.isFinite(cost) || cost <= 0) {
    addError.value = "请输入有效的成本价";
    return;
  }
  if (!Number.isInteger(shares) || shares <= 0) {
    addError.value = "股数需为正整数";
    return;
  }

  addError.value = "";
  try {
    await invoke("add_watch_item", {
      item: {
        code: hit.code,
        name: hit.name,
        costPrice: cost,
        shares,
      },
    });
    backToManage();
    await reload();
  } catch (error) {
    addError.value = String(error);
  }
}

function openEdit(row: QuoteRow) {
  editing.value = row;
  editCost.value = row.costPrice.toFixed(2);
  editShares.value = String(row.shares);
  editError.value = "";
  confirmDelete.value = false;
  sheet.value = "edit";
}

async function saveEdit() {
  const row = editing.value;
  if (!row) return;

  const cost = Number(editCost.value);
  const shares = Number(editShares.value);
  if (!Number.isFinite(cost) || cost <= 0) {
    editError.value = "请输入有效的成本价";
    return;
  }
  if (!Number.isInteger(shares) || shares <= 0) {
    editError.value = "股数需为正整数";
    return;
  }

  editError.value = "";
  try {
    await invoke("update_watch_item", {
      patch: {
        code: row.code,
        costPrice: cost,
        shares,
      },
    });
    backToManage();
    await reload();
  } catch (error) {
    editError.value = String(error);
  }
}

async function removeEditing() {
  const row = editing.value;
  if (!row) return;
  if (!confirmDelete.value) {
    confirmDelete.value = true;
    return;
  }

  try {
    await invoke("remove_watch_item", { code: row.code });
    backToManage();
    await reload();
  } catch (error) {
    editError.value = String(error);
  }
}

async function toggleClickThrough(event: Event) {
  const input = event.target as HTMLInputElement;
  const enabled = input.checked;
  manageError.value = "";

  try {
    const flags = await invoke<WindowFlags>("set_click_through", { enabled });
    handleWindowFlags(flags);
    if (flags.clickThrough) closeSheet();
  } catch (error) {
    manageError.value = String(error);
    input.checked = clickThrough.value;
  }
}

function handleWindowFocus() {
  void refreshWindowFlags();
}

onMounted(async () => {
  try {
    unlistenMode = await listen<WindowFlags>(
      "window-mode-changed",
      (event) => handleWindowFlags(event.payload),
    );
  } catch {
    // The viewport guard below still keeps the 30x30 boss dot usable.
  }

  window.addEventListener("resize", syncBossFromViewport);
  window.addEventListener("focus", handleWindowFocus);
  syncBossFromViewport();
  await refreshWindowFlags();
  await loadSettings();
  await reload();
  scheduleRefresh();
});

onUnmounted(() => {
  if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
  if (searchTimer !== undefined) window.clearTimeout(searchTimer);
  unlistenMode?.();
  window.removeEventListener("focus", handleWindowFocus);
  window.removeEventListener("resize", syncBossFromViewport);
});
</script>

<template>
  <div v-if="bossMode" class="boss-dot"></div>

  <div
    v-else
    class="widget"
    :class="{ 'is-click-through': clickThrough }"
    :style="widgetStyle"
  >
    <header class="titlebar">
      <div class="drag-zone" data-tauri-drag-region>
        <span class="note-icon" aria-hidden="true"></span>
        <span class="note-title">todo.txt</span>
        <span class="note-status">已保存</span>
      </div>
      <button
        class="icon-button menu-button"
        type="button"
        title="菜单"
        aria-label="打开菜单"
        @click="openManage"
      >
        ⋯
      </button>
    </header>

    <main class="content">
      <div class="note-heading">
        <span>今天</span>
        <span>{{ remainingTodos }} 项待办</span>
      </div>

      <ul class="todo-list">
        <li
          v-for="todo in todos"
          :key="todo.id"
          class="todo"
          :class="{ done: todo.done }"
          @click="toggleTodo(todo)"
        >
          <span class="todo-box" :class="{ checked: todo.done }"></span>
          <span class="todo-text">{{ todo.text }}</span>
        </li>
      </ul>

      <div
        v-if="sheet === 'none' && hoverSummary"
        class="peek-line"
        aria-hidden="true"
      >
        {{ hoverSummary }}
      </div>
    </main>

    <footer class="statusbar">
      <span>纯文本</span>
      <span :class="{ 'mode-tag': clickThrough }">
        {{ clickThrough ? "穿透模式" : "自动保存" }}
      </span>
    </footer>

    <section v-if="sheet === 'manage'" class="sheet" @keydown.esc="closeSheet">
      <div class="sheet-head">
        <span class="sheet-title">自选股</span>
        <div class="sheet-head-actions">
          <button class="text-button" type="button" @click="openAdd">
            + 添加
          </button>
          <button
            class="icon-button"
            type="button"
            title="关闭"
            aria-label="关闭"
            @click="closeSheet"
          >
            ×
          </button>
        </div>
      </div>

      <div v-if="loading" class="manage-empty">加载中…</div>
      <div v-else-if="!hasRows" class="manage-empty">还没有自选股</div>
      <div v-else class="manage-list">
        <button
          v-for="row in rows"
          :key="row.code"
          class="manage-row"
          type="button"
          @click="openEdit(row)"
        >
          <span class="manage-top">
            <span class="manage-name" :title="row.name">{{ row.name }}</span>
            <span class="manage-code">{{ row.code }}</span>
            <span class="manage-price">{{ formatPrice(row.price) }}</span>
            <span class="manage-change">{{ formatPct(row.changePct) }}</span>
          </span>
          <span class="manage-sub">
            <span>成本价 {{ formatPrice(row.costPrice) }}</span>
            <span>{{ row.shares }} 股</span>
            <span class="manage-pnl">盈亏 {{ formatPnl(row.pnl) }}</span>
          </span>
        </button>
      </div>

      <p v-if="manageError" class="sheet-note error">{{ manageError }}</p>

      <div class="manage-bottom">
        <span class="manage-hint">{{ manageStatus }}</span>
        <button class="text-button" type="button" @click="openSettings">
          设置
        </button>
      </div>
    </section>

    <section
      v-if="sheet === 'add'"
      class="sheet"
      @keydown.esc="backToManage"
    >
      <div class="sheet-head">
        <span class="sheet-title">添加自选</span>
        <button
          class="icon-button"
          type="button"
          title="返回"
          aria-label="返回"
          @click="backToManage"
        >
          ×
        </button>
      </div>

      <template v-if="!selected">
        <input
          ref="searchInput"
          v-model="searchQuery"
          class="text-input"
          type="text"
          placeholder="输入代码或名称，如 600519 / 茅台"
          autocomplete="off"
          spellcheck="false"
          @input="onSearchInput"
        />

        <div class="search-results">
          <button
            v-for="hit in searchResults"
            :key="hit.code"
            class="search-hit"
            type="button"
            @click="selectHit(hit)"
          >
            <span class="hit-name">{{ hit.name }}</span>
            <span class="hit-code">{{ hit.code }}</span>
          </button>
          <p v-if="searching" class="sheet-note">搜索中…</p>
          <p v-else-if="searchError" class="sheet-note error">{{ searchError }}</p>
          <p
            v-else-if="searchQuery.trim() && searchResults.length === 0"
            class="sheet-note"
          >
            没有匹配的 A 股
          </p>
        </div>
      </template>

      <template v-else>
        <div class="picked">
          <div class="picked-name">
            <strong>{{ selected.name }}</strong>
            <span>{{ selected.code }}</span>
          </div>
          <button class="text-button" type="button" @click="resetSelection">
            重新搜索
          </button>
        </div>

        <label class="field">
          <span>成本价</span>
          <input
            ref="costInput"
            v-model="newCost"
            type="number"
            min="0.01"
            step="0.01"
            inputmode="decimal"
            placeholder="0.00"
          />
        </label>

        <label class="field">
          <span>股数</span>
          <input
            v-model="newShares"
            type="number"
            min="1"
            step="100"
            inputmode="numeric"
          />
        </label>

        <p v-if="addError" class="sheet-note error">{{ addError }}</p>

        <div class="sheet-actions">
          <button class="primary-button" type="button" @click="addSelected">
            加入自选
          </button>
        </div>
      </template>
    </section>

    <section
      v-if="sheet === 'edit'"
      class="sheet"
      @keydown.esc="backToManage"
    >
      <div class="sheet-head">
        <span class="sheet-title">持仓设置</span>
        <button
          class="icon-button"
          type="button"
          title="返回"
          aria-label="返回"
          @click="backToManage"
        >
          ×
        </button>
      </div>

      <div v-if="editing" class="picked">
        <div class="picked-name">
          <strong>{{ editing.name }}</strong>
          <span>{{ editing.code }}</span>
        </div>
        <span class="picked-price">{{ formatPrice(editing.price) }}</span>
      </div>

      <label class="field">
        <span>成本价</span>
        <input
          v-model="editCost"
          type="number"
          min="0.01"
          step="0.01"
          inputmode="decimal"
        />
      </label>

      <label class="field">
        <span>股数</span>
        <input
          v-model="editShares"
          type="number"
          min="1"
          step="100"
          inputmode="numeric"
        />
      </label>

      <p v-if="editError" class="sheet-note error">{{ editError }}</p>

      <div class="sheet-actions">
        <button class="danger-button" type="button" @click="removeEditing">
          {{ confirmDelete ? "确认删除" : "删除" }}
        </button>
        <button class="primary-button" type="button" @click="saveEdit">
          保存
        </button>
      </div>
    </section>

    <section
      v-if="sheet === 'settings'"
      class="sheet"
      @keydown.esc="backToManage"
    >
      <div class="sheet-head">
        <span class="sheet-title">设置</span>
        <button
          class="icon-button"
          type="button"
          title="返回"
          aria-label="返回"
          @click="backToManage"
        >
          ×
        </button>
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span>窗口透明度</span>
          <span class="setting-value">{{ opacityPercent }}%</span>
        </div>
        <input
          v-model.number="settings.opacity"
          class="range"
          type="range"
          min="0.3"
          max="0.9"
          step="0.05"
          @change="persistSettings"
        />
      </div>

      <div class="setting-row">
        <div class="setting-label">
          <span>刷新间隔</span>
          <span class="setting-value">{{ refreshSeconds }}s</span>
        </div>
        <input
          v-model.number="settings.refreshSeconds"
          class="range"
          type="range"
          min="5"
          max="60"
          step="1"
          @change="persistSettings"
        />
      </div>

      <div class="setting-row setting-switch">
        <div class="setting-label">
          <span>鼠标穿透</span>
          <small>开启后只能按 Ctrl+Shift+H 恢复</small>
        </div>
        <label class="switch">
          <input
            type="checkbox"
            :checked="clickThrough"
            @change="toggleClickThrough"
          />
          <span class="switch-track"><span class="switch-knob"></span></span>
        </label>
      </div>

      <p v-if="settingsError" class="sheet-note error">{{ settingsError }}</p>

      <div class="sheet-actions">
        <button class="primary-button" type="button" @click="backToManage">
          返回自选
        </button>
      </div>
    </section>
  </div>
</template>

<style>
:root {
  font-family:
    "Segoe UI Variable", "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei",
    system-ui, sans-serif;
  color: #d9d9d9;
  background: transparent;
  color-scheme: dark;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

* {
  box-sizing: border-box;
}

html,
body,
#app {
  width: 100%;
  height: 100%;
  margin: 0;
  padding: 0;
  overflow: hidden;
  background: transparent;
}

body {
  user-select: none;
  -webkit-user-select: none;
  cursor: default;
}

button,
input {
  font: inherit;
}

button {
  cursor: pointer;
  outline: none;
}

button:focus-visible {
  outline: 1px solid rgba(255, 255, 255, 0.32);
  outline-offset: 1px;
}

::selection {
  color: #f2f2f2;
  background: rgba(255, 255, 255, 0.2);
}

.widget {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
  color: #d9d9d9;
  background: rgba(20, 20, 20, var(--window-opacity, 0.75));
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 14px;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
  backdrop-filter: blur(18px) saturate(0.75);
  -webkit-backdrop-filter: blur(18px) saturate(0.75);
}

.titlebar {
  display: flex;
  flex: 0 0 32px;
  align-items: center;
  padding: 0 7px 0 11px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
}

.drag-zone {
  display: flex;
  flex: 1;
  gap: 7px;
  align-items: center;
  align-self: stretch;
  min-width: 0;
  cursor: grab;
}

.drag-zone:active {
  cursor: grabbing;
}

.drag-zone > * {
  pointer-events: none;
}

.note-icon {
  position: relative;
  flex: 0 0 9px;
  width: 9px;
  height: 11px;
  border: 1px solid #898989;
  border-radius: 1px;
}

.note-icon::after {
  position: absolute;
  top: -1px;
  right: -1px;
  width: 4px;
  height: 4px;
  content: "";
  background: #252525;
  border-bottom: 1px solid #898989;
  border-left: 1px solid #898989;
}

.note-title {
  overflow: hidden;
  font-size: 10px;
  line-height: 1;
  color: #d3d3d3;
  letter-spacing: 0.03em;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.note-status {
  margin-left: auto;
  font-size: 7px;
  line-height: 1;
  color: #6f6f6f;
  letter-spacing: 0.04em;
}

.icon-button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  font-size: 14px;
  line-height: 1;
  color: #8b8b8b;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 4px;
}

.icon-button:hover {
  color: #e0e0e0;
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.08);
}

.menu-button {
  flex: 0 0 20px;
  margin-left: 3px;
}

.content {
  position: relative;
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  padding: 8px 12px 0;
}

.note-heading {
  display: flex;
  flex: 0 0 12px;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
  font-size: 8px;
  line-height: 1;
  color: #6f6f6f;
  letter-spacing: 0.08em;
}

.todo-list {
  margin: 0;
  padding: 0;
  list-style: none;
}

.todo {
  display: flex;
  gap: 9px;
  align-items: center;
  height: 38px;
  overflow: hidden;
  font-size: 11px;
  line-height: 1.2;
  color: #c8c8c8;
  border-bottom: 1px solid rgba(255, 255, 255, 0.045);
  cursor: pointer;
}

.todo:last-child {
  border-bottom: 0;
}

.todo:hover {
  color: #e4e4e4;
  background: rgba(255, 255, 255, 0.018);
}

.todo-box {
  position: relative;
  flex: 0 0 9px;
  width: 9px;
  height: 9px;
  border: 1px solid #777;
  border-radius: 2px;
}

.todo.done .todo-box {
  background: #999;
  border-color: #999;
}

.todo.done .todo-box::after {
  position: absolute;
  top: 1px;
  left: 2px;
  width: 3px;
  height: 5px;
  content: "";
  border-right: 1px solid #222;
  border-bottom: 1px solid #222;
  transform: rotate(45deg);
}

.todo-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.todo.done .todo-text {
  color: #737373;
  text-decoration: line-through;
  text-decoration-color: #5b5b5b;
}

.peek-line {
  position: absolute;
  right: 12px;
  bottom: 1px;
  left: 12px;
  z-index: 2;
  overflow: hidden;
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 7px;
  line-height: 1.3;
  color: rgba(214, 214, 214, 0.42);
  letter-spacing: 0.01em;
  text-overflow: ellipsis;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.5);
  white-space: nowrap;
  pointer-events: none;
  opacity: 0;
  transition: opacity 160ms ease;
}

.widget:hover .peek-line {
  opacity: 1;
}

.widget.is-click-through .peek-line {
  opacity: 0 !important;
}

.statusbar {
  display: flex;
  flex: 0 0 20px;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  font-size: 8px;
  line-height: 1;
  color: #6b6b6b;
  letter-spacing: 0.04em;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.mode-tag {
  color: #9a9a9a;
}

.boss-dot {
  width: 100%;
  height: 100%;
  background: rgba(148, 148, 148, 0.78);
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 50%;
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.18),
    0 1px 5px rgba(0, 0, 0, 0.3);
}

.sheet {
  position: absolute;
  inset: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 7px 10px;
  overflow: hidden;
  background: rgba(18, 18, 18, 0.98);
  border-radius: 14px;
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
}

.sheet-head {
  display: flex;
  flex: 0 0 20px;
  align-items: center;
  justify-content: space-between;
}

.sheet-title {
  font-size: 9px;
  font-weight: 600;
  color: #cfcfcf;
  letter-spacing: 0.12em;
}

.sheet-head-actions {
  display: flex;
  gap: 4px;
  align-items: center;
}

.text-button {
  padding: 0 2px;
  font-size: 9px;
  color: #9d9d9d;
  background: transparent;
  border: 0;
}

.text-button:hover {
  color: #e2e2e2;
}

.manage-list {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  scrollbar-color: rgba(255, 255, 255, 0.16) transparent;
  scrollbar-width: thin;
}

.manage-row {
  display: flex;
  flex-direction: column;
  gap: 1px;
  justify-content: center;
  width: 100%;
  height: 40px;
  padding: 3px 2px;
  color: inherit;
  text-align: left;
  background: transparent;
  border: 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.manage-row:hover {
  background: rgba(255, 255, 255, 0.035);
}

.manage-top,
.manage-sub {
  display: flex;
  align-items: baseline;
  width: 100%;
  min-width: 0;
}

.manage-top {
  gap: 5px;
}

.manage-name {
  overflow: hidden;
  font-size: 10px;
  font-weight: 600;
  color: #e2e2e2;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.manage-code {
  flex: 0 0 auto;
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 8px;
  color: #747474;
}

.manage-price {
  margin-left: auto;
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 12px;
  color: #f0f0f0;
  font-variant-numeric: tabular-nums;
}

.manage-change {
  flex: 0 0 44px;
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 9px;
  color: #9d9d9d;
  text-align: right;
  font-variant-numeric: tabular-nums;
}

.manage-sub {
  gap: 7px;
  font-size: 8px;
  color: #777;
  font-variant-numeric: tabular-nums;
}

.manage-pnl {
  margin-left: auto;
  font-family: "Cascadia Mono", "Consolas", monospace;
  color: #a8a8a8;
}

.manage-empty {
  display: grid;
  flex: 1;
  place-items: center;
  font-size: 9px;
  color: #777;
}

.manage-bottom {
  display: flex;
  flex: 0 0 24px;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.switch {
  position: relative;
  display: inline-flex;
  flex: 0 0 24px;
  align-items: center;
  width: 24px;
  height: 12px;
  cursor: pointer;
}

.switch input {
  position: absolute;
  width: 0;
  height: 0;
  margin: 0;
  opacity: 0;
}

.switch-track {
  position: relative;
  display: block;
  width: 24px;
  height: 12px;
  background: rgba(255, 255, 255, 0.12);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 999px;
  transition: background 150ms ease;
}

.switch-knob {
  position: absolute;
  top: 1px;
  left: 1px;
  width: 8px;
  height: 8px;
  background: #8f8f8f;
  border-radius: 50%;
  transition:
    transform 150ms ease,
    background 150ms ease;
}

.switch input:checked + .switch-track {
  background: rgba(255, 255, 255, 0.3);
}

.switch input:checked + .switch-track .switch-knob {
  background: #e0e0e0;
  transform: translateX(12px);
}

.switch input:focus-visible + .switch-track {
  outline: 1px solid rgba(255, 255, 255, 0.32);
  outline-offset: 1px;
}

.setting-row {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 4px 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
}

.setting-label {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
  font-size: 9px;
  color: #a0a0a0;
}

.setting-label small {
  display: block;
  font-size: 7px;
  color: #6f6f6f;
}

.setting-value {
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 9px;
  color: #bdbdbd;
  font-variant-numeric: tabular-nums;
}

.setting-switch {
  flex-direction: row;
  align-items: center;
  justify-content: space-between;
  border-bottom: 0;
}

.setting-switch .setting-label {
  flex: 1;
  flex-direction: column;
  gap: 1px;
}

.range {
  width: 100%;
  height: 14px;
  margin: 0;
  background: transparent;
  outline: none;
  -webkit-appearance: none;
  appearance: none;
}

.range::-webkit-slider-runnable-track {
  height: 2px;
  background: rgba(255, 255, 255, 0.16);
  border-radius: 2px;
}

.range::-webkit-slider-thumb {
  width: 10px;
  height: 10px;
  margin-top: -4px;
  background: #cfcfcf;
  border: 1px solid rgba(0, 0, 0, 0.35);
  border-radius: 50%;
  -webkit-appearance: none;
  appearance: none;
}

.range:focus-visible::-webkit-slider-thumb {
  box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.2);
}

.manage-hint {
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 7px;
  color: #777;
  letter-spacing: 0.01em;
}

.text-input {
  width: 100%;
  height: 28px;
  padding: 0 8px;
  font-size: 10px;
  color: #e3e3e3;
  caret-color: #d9d9d9;
  background: rgba(255, 255, 255, 0.045);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 5px;
  outline: none;
  user-select: text;
  -webkit-user-select: text;
}

.text-input::placeholder,
.field input::placeholder {
  color: #666;
}

.text-input:focus {
  border-color: rgba(255, 255, 255, 0.28);
}

.search-results {
  flex: 1;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  border-top: 1px solid rgba(255, 255, 255, 0.06);
  scrollbar-color: rgba(255, 255, 255, 0.16) transparent;
  scrollbar-width: thin;
}

.search-hit {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  width: 100%;
  padding: 7px 3px;
  color: #c6c6c6;
  text-align: left;
  background: transparent;
  border: 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.search-hit:hover {
  background: rgba(255, 255, 255, 0.045);
}

.hit-name {
  overflow: hidden;
  font-size: 10px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.hit-code {
  flex: 0 0 auto;
  margin-left: 10px;
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 9px;
  color: #7c7c7c;
}

.picked {
  display: flex;
  flex: 0 0 26px;
  align-items: center;
  justify-content: space-between;
  padding: 0 2px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.07);
}

.picked-name {
  display: flex;
  gap: 7px;
  align-items: baseline;
  min-width: 0;
}

.picked-name strong {
  overflow: hidden;
  font-size: 11px;
  font-weight: 600;
  color: #ececec;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picked-name span,
.picked-price {
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 9px;
  color: #858585;
}

.field {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 9px;
  color: #8c8c8c;
}

.field input {
  width: 112px;
  height: 26px;
  padding: 0 7px;
  font-family: "Cascadia Mono", "Consolas", monospace;
  font-size: 11px;
  color: #e0e0e0;
  text-align: right;
  caret-color: #d9d9d9;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 4px;
  outline: none;
  user-select: text;
  -webkit-user-select: text;
}

.field input:focus {
  border-color: rgba(255, 255, 255, 0.27);
}

.sheet-note {
  margin: 0;
  font-size: 8px;
  line-height: 1.4;
  color: #858585;
}

.sheet-note.error {
  color: #c4c4c4;
}

.sheet-actions {
  display: flex;
  gap: 6px;
  margin-top: auto;
}

.primary-button,
.danger-button {
  flex: 1;
  height: 28px;
  font-size: 10px;
  color: #e2e2e2;
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 5px;
}

.primary-button:hover {
  background: rgba(255, 255, 255, 0.16);
}

.danger-button {
  color: #a5a5a5;
  background: rgba(255, 255, 255, 0.025);
  border-color: rgba(255, 255, 255, 0.1);
}

.danger-button:hover {
  color: #e0e0e0;
  background: rgba(255, 255, 255, 0.08);
}

input[type="number"]::-webkit-inner-spin-button,
input[type="number"]::-webkit-outer-spin-button {
  margin: 0;
  -webkit-appearance: none;
}

.manage-list::-webkit-scrollbar,
.search-results::-webkit-scrollbar {
  width: 3px;
}

.manage-list::-webkit-scrollbar-track,
.search-results::-webkit-scrollbar-track {
  background: transparent;
}

.manage-list::-webkit-scrollbar-thumb,
.search-results::-webkit-scrollbar-thumb {
  background: rgba(255, 255, 255, 0.16);
  border-radius: 2px;
}
</style>
