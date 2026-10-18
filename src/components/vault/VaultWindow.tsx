import { createEffect, createMemo, createSignal, For, Show } from 'solid-js';
import { ContextMenu } from '@kobalte/core/context-menu';
import {
  ArrowDown,
  ArrowUp,
  ClipboardPaste,
  Download,
  Eye,
  ExternalLink,
  File,
  FileImage,
  FileText,
  Files,
  Folder,
  FolderInput,
  FolderPlus,
  Lock,
  Pencil,
  Plus,
  Settings,
  Trash2,
  X,
} from 'lucide-solid';
import * as tauri from '../../lib/tauri';
import type { VaultFile } from '../../lib/tauri';
import { open, save, confirm } from '../../lib/dialog';
import { mapTauriError } from '../../lib/errors';
import {
  filterVaultFiles,
  formatVaultAdded,
  formatVaultSize,
  sortVaultFiles,
  vaultPreviewKind,
  type VaultFolderFilter,
  type VaultSortDir,
  type VaultSortKey,
} from '../../lib/vault-files';
import { useI18n } from '../../i18n';
import { lockVault, refreshVault, vaultListing, vaultStatus } from '../../state/vault';
import { VaultMoveDialog, VaultTextPrompt } from './VaultDialogs';
import VaultPreview from './VaultPreview';
import VaultSettings from './VaultSettings';

type Prompt =
  | { kind: 'rename-file'; file: VaultFile }
  | { kind: 'new-folder' }
  | { kind: 'rename-folder'; folder: string };

/** The explorer-style vault screen. Shown over everything while a vault is unlocked. */
export default function VaultWindow() {
  const t = useI18n();
  const [folder, setFolder] = createSignal<VaultFolderFilter>(null);
  const [query, setQuery] = createSignal('');
  const [sortKey, setSortKey] = createSignal<VaultSortKey>('added');
  const [sortDir, setSortDir] = createSignal<VaultSortDir>('desc');
  /** Selected file ids (checkboxes, Ctrl/Shift-click, Ctrl+A). */
  const [selectedIds, setSelectedIds] = createSignal<ReadonlySet<string>>(new Set());
  /** The last row clicked without Shift: the start of a Shift-click range. */
  const [anchorId, setAnchorId] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [dragging, setDragging] = createSignal(false);
  const [prompt, setPrompt] = createSignal<Prompt | null>(null);
  const [promptError, setPromptError] = createSignal<string | null>(null);
  const [moving, setMoving] = createSignal<string[] | null>(null);
  const [previewing, setPreviewing] = createSignal<VaultFile | null>(null);
  const [settingsOpen, setSettingsOpen] = createSignal(false);

  const files = createMemo(() =>
    sortVaultFiles(filterVaultFiles(vaultListing().files, folder(), query()), sortKey(), sortDir()),
  );
  /** The selected files that are still in the vault, in list order. */
  const selectedFiles = () => files().filter((f) => selectedIds().has(f.id));
  /** The one selected file, when exactly one is selected. */
  const selected = () => {
    const sel = selectedFiles();
    return sel.length === 1 ? sel[0] : null;
  };
  const isSelected = (id: string) => selectedIds().has(id);
  const allSelected = () => files().length > 0 && files().every((f) => isSelected(f.id));
  const someSelected = () => selectedFiles().length > 0 && !allSelected();
  const selectOnly = (id: string) => {
    setSelectedIds(new Set([id]));
    setAnchorId(id);
  };
  const toggle = (id: string) => {
    const next = new Set(selectedIds());
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setSelectedIds(next);
    setAnchorId(id);
  };
  const selectRangeTo = (id: string) => {
    const list = files();
    const from = list.findIndex((f) => f.id === anchorId());
    const to = list.findIndex((f) => f.id === id);
    if (from < 0 || to < 0) return selectOnly(id);
    const [a, b] = from < to ? [from, to] : [to, from];
    setSelectedIds(new Set(list.slice(a, b + 1).map((f) => f.id)));
  };
  const selectAll = () => setSelectedIds(new Set(files().map((f) => f.id)));
  const clearSelection = () => setSelectedIds(new Set<string>());
  const onRowClick = (e: MouseEvent, id: string) => {
    if (e.shiftKey) selectRangeTo(id);
    else if (e.ctrlKey || e.metaKey) toggle(id);
    else selectOnly(id);
  };
  // Changing the folder or the search shows other files: start a fresh selection.
  createEffect(() => {
    folder();
    query();
    clearSelection();
  });
  const countIn = (name: string | null) =>
    name === null
      ? vaultListing().files.length
      : vaultListing().files.filter((f) => f.folder === name).length;

  /** Runs a vault action, refreshes the list, and shows any error. */
  const act = async (work: () => Promise<unknown>, done?: string) => {
    setError(null);
    setStatus(null);
    setBusy(true);
    try {
      await work();
      await refreshVault();
      if (done) setStatus(done);
      return true;
    } catch (err) {
      setError(mapTauriError(err, t));
      return false;
    } finally {
      setBusy(false);
    }
  };

  const sortBy = (key: VaultSortKey) => {
    if (sortKey() === key) setSortDir(sortDir() === 'asc' ? 'desc' : 'asc');
    else {
      setSortKey(key);
      setSortDir(key === 'added' || key === 'size' ? 'desc' : 'asc');
    }
  };

  const reportAdd = (added: number, failed: { name: string; error: string }[]) => {
    if (failed.length) {
      setError(failed.map((f) => `${f.name}: ${mapTauriError(f.error, t)}`).join('\n'));
    }
    if (added) setStatus(t('vault.addedCount', { count: added }));
  };

  const addFiles = async () => {
    const picked = await open({ multiple: true, directory: false, title: t('vault.addFiles') });
    if (!picked) return;
    const paths = Array.isArray(picked) ? picked : [picked];
    if (!paths.length) return;
    setStatus(t('vault.encrypting', { count: paths.length }));
    let result: tauri.VaultAddResult | null = null;
    await act(async () => {
      result = await tauri.vaultAddFiles(paths, folder());
    });
    if (result)
      reportAdd(
        (result as tauri.VaultAddResult).added.length,
        (result as tauri.VaultAddResult).failed,
      );
  };

  const dropFiles = async (list: FileList) => {
    const dropped = Array.from(list);
    if (!dropped.length) return;
    setStatus(t('vault.encrypting', { count: dropped.length }));
    const failed: { name: string; error: string }[] = [];
    let added = 0;
    await act(async () => {
      for (const f of dropped) {
        if (f.size > tauri.VAULT_MAX_DROP_BYTES) {
          failed.push({ name: f.name, error: t('vault.dropTooLarge') });
          continue;
        }
        try {
          await tauri.vaultAddBytes(f.name, folder(), await f.arrayBuffer());
          added += 1;
        } catch (err) {
          failed.push({ name: f.name, error: mapTauriError(err, t) });
        }
      }
    });
    reportAdd(added, failed);
  };

  const pasteFiles = async () => {
    let result: tauri.VaultAddResult | null = null;
    setStatus(t('vault.encrypting'));
    await act(async () => {
      result = await tauri.vaultPasteFiles(folder());
    });
    const r = result as tauri.VaultAddResult | null;
    if (r) reportAdd(r.added.length, r.failed);
    else setStatus(null);
  };

  const openFile = async (file: VaultFile) => {
    if (vaultPreviewKind(file)) {
      setPreviewing(file);
      return;
    }
    await openInApp(file);
  };

  const openInApp = async (file: VaultFile) => {
    const ok = await confirm(t('vault.openInAppConfirm', { name: file.name }), {
      title: t('vault.openInApp'),
    });
    if (!ok) return;
    await act(() => tauri.vaultOpenInApp(file.id));
  };

  const saveCopy = async (file: VaultFile) => {
    const dest = await save({ title: t('vault.saveCopy'), defaultPath: file.name });
    if (!dest) return;
    await act(() => tauri.vaultSaveCopy(file.id, dest), t('vault.saved', { name: file.name }));
  };

  /** Saves the selection: one file through a save dialog, several into a chosen folder. */
  const saveSelected = async () => {
    const sel = selectedFiles();
    if (sel.length === 1) return saveCopy(sel[0]);
    if (!sel.length) return;
    const dir = await open({ directory: true, multiple: false, title: t('vault.saveCopiesTitle') });
    if (typeof dir !== 'string') return;
    let saved = 0;
    if (
      await act(async () => {
        saved = await tauri.vaultSaveCopies(
          sel.map((f) => f.id),
          dir,
        );
      })
    )
      setStatus(t('vault.savedCount', { count: saved }));
  };

  const deleteSelected = async () => {
    const sel = selectedFiles();
    if (!sel.length) return;
    const message =
      sel.length === 1
        ? t('vault.deleteConfirm', { name: sel[0].name })
        : t('vault.deleteSelectedConfirm', { count: sel.length });
    const ok = await confirm(message, { title: t('vault.delete'), kind: 'warning' });
    if (!ok) return;
    if (await act(() => tauri.vaultDeleteFiles(sel.map((f) => f.id)))) clearSelection();
  };

  const moveSelected = () => {
    const sel = selectedFiles();
    if (sel.length) setMoving(sel.map((f) => f.id));
  };

  const deleteFolder = async (name: string) => {
    const ok = await confirm(t('vault.deleteFolderConfirm', { name }), {
      title: t('vault.deleteFolder'),
    });
    if (!ok) return;
    if (await act(() => tauri.vaultDeleteFolder(name))) setFolder(null);
  };

  const submitPrompt = async (value: string) => {
    const p = prompt();
    if (!p) return;
    setPromptError(null);
    try {
      if (p.kind === 'rename-file') await tauri.vaultRenameFile(p.file.id, value);
      else if (p.kind === 'new-folder') await tauri.vaultCreateFolder(value);
      else await tauri.vaultRenameFolder(p.folder, value);
      if (p.kind === 'rename-folder' && folder() === p.folder) setFolder(value);
      setPrompt(null);
      await refreshVault();
    } catch (err) {
      setPromptError(mapTauriError(err, t));
    }
  };

  const submitMove = async (target: string | null) => {
    const ids = moving();
    if (!ids) return;
    if (await act(() => tauri.vaultMoveFiles(ids, target))) setMoving(null);
  };
  /** The folder all files being moved share, to preselect it. */
  const movingFrom = () => {
    const ids = moving();
    const folders = new Set(
      vaultListing()
        .files.filter((f) => ids?.includes(f.id))
        .map((f) => f.folder),
    );
    return folders.size === 1 ? ([...folders][0] ?? null) : null;
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest('input:not([type=checkbox]), textarea, select, [role=dialog]')) return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      selectAll();
    } else if (mod && e.key.toLowerCase() === 'v') {
      e.preventDefault();
      void pasteFiles();
    } else if (e.key === 'Escape' && selectedIds().size) {
      e.preventDefault();
      clearSelection();
    } else if (e.key === 'Delete' && selectedFiles().length) {
      e.preventDefault();
      void deleteSelected();
    } else if (e.key === 'Enter' && selected()) {
      e.preventDefault();
      void openFile(selected()!);
    } else if (e.key === 'F2' && selected()) {
      e.preventDefault();
      setPrompt({ kind: 'rename-file', file: selected()! });
    }
  };

  const iconFor = (file: VaultFile) => {
    const kind = vaultPreviewKind(file);
    if (kind === 'image') return FileImage;
    if (kind === 'pdf' || kind === 'text') return FileText;
    return File;
  };

  const SortHeader = (p: { key: VaultSortKey; label: string; class?: string }) => (
    <th class={`px-3 py-2 text-left font-medium ${p.class ?? ''}`}>
      <button
        type="button"
        class="inline-flex items-center gap-1 text-secondary hover:text-primary"
        onClick={() => sortBy(p.key)}
        aria-sort={
          sortKey() === p.key ? (sortDir() === 'asc' ? 'ascending' : 'descending') : 'none'
        }
      >
        {p.label}
        <Show when={sortKey() === p.key}>
          {sortDir() === 'asc' ? <ArrowUp size={12} /> : <ArrowDown size={12} />}
        </Show>
      </button>
    </th>
  );

  const menuItem =
    'flex w-full cursor-pointer items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-primary outline-none hover:bg-hover data-[highlighted]:bg-hover';
  const toolButton =
    'inline-flex items-center gap-1.5 rounded-md border border-primary px-3 py-1.5 text-sm text-secondary hover:bg-hover disabled:opacity-50';

  return (
    <div
      class="absolute inset-0 z-40 flex flex-col bg-secondary"
      data-testid="vault-window"
      onKeyDown={onKeyDown}
      onDragOver={(e) => {
        if (e.dataTransfer?.types.includes('Files')) {
          e.preventDefault();
          setDragging(true);
        }
      }}
      onDragLeave={(e) => {
        if (e.currentTarget === e.target) setDragging(false);
      }}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        if (e.dataTransfer?.files) void dropFiles(e.dataTransfer.files);
      }}
    >
      {/* Toolbar */}
      <div class="flex items-center gap-2 border-b border-primary bg-primary px-4 py-2">
        <Lock size={18} class="text-tertiary" aria-hidden="true" />
        <div class="min-w-0">
          <p class="truncate text-sm font-semibold text-primary">{vaultStatus()?.name}</p>
          <p class="truncate text-xs text-tertiary" title={vaultStatus()?.path}>
            {vaultStatus()?.path}
          </p>
        </div>
        <span class="flex-1" />
        <input
          type="search"
          class="w-48 rounded-md border border-primary bg-primary px-3 py-1.5 text-sm text-primary focus:outline-none focus:ring-2 focus:ring-blue-500"
          placeholder={t('vault.searchPlaceholder')}
          aria-label={t('vault.searchPlaceholder')}
          value={query()}
          onInput={(e) => setQuery(e.currentTarget.value)}
          data-testid="vault-search"
        />
        <button
          type="button"
          class={toolButton}
          onClick={() => void pasteFiles()}
          disabled={busy()}
          title={t('vault.pasteHint')}
          data-testid="vault-paste"
        >
          <ClipboardPaste size={16} />
          {t('vault.paste')}
        </button>
        <button
          type="button"
          class="inline-flex items-center gap-1.5 rounded-md interactive-primary px-3 py-1.5 text-sm font-medium disabled:opacity-50"
          onClick={() => void addFiles()}
          disabled={busy()}
          data-testid="vault-add-files"
        >
          <Plus size={16} />
          {t('vault.addFiles')}
        </button>
        <button
          type="button"
          class="rounded p-2 text-tertiary hover:bg-hover"
          aria-label={t('vault.settings')}
          title={t('vault.settings')}
          onClick={() => setSettingsOpen(true)}
        >
          <Settings size={18} />
        </button>
        <button
          type="button"
          class={toolButton}
          onClick={() => void lockVault()}
          data-testid="vault-lock"
        >
          <Lock size={16} />
          {t('vault.lock')}
        </button>
      </div>

      <div class="flex min-h-0 flex-1">
        {/* Sidebar */}
        <nav
          class="w-52 shrink-0 overflow-y-auto border-r border-primary p-2"
          aria-label={t('vault.folders')}
        >
          <button
            type="button"
            class={`flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm ${
              folder() === null ? 'interactive-primary' : 'text-secondary hover:bg-hover'
            }`}
            aria-pressed={folder() === null}
            onClick={() => setFolder(null)}
            data-testid="vault-all-files"
          >
            <Files size={16} />
            <span class="flex-1">{t('vault.allFiles')}</span>
            <span class="text-xs opacity-70">{countIn(null)}</span>
          </button>
          <div class="mt-3 flex items-center justify-between px-3 py-1">
            <span class="text-xs font-medium text-tertiary">{t('vault.folders')}</span>
            <button
              type="button"
              class="rounded p-1 text-tertiary hover:bg-hover"
              aria-label={t('vault.newFolder')}
              title={t('vault.newFolder')}
              onClick={() => {
                setPromptError(null);
                setPrompt({ kind: 'new-folder' });
              }}
            >
              <FolderPlus size={14} />
            </button>
          </div>
          <For each={vaultListing().folders}>
            {(name) => (
              <div class="group flex items-center">
                <button
                  type="button"
                  class={`flex min-w-0 flex-1 items-center gap-2 rounded-md px-3 py-2 text-left text-sm ${
                    folder() === name ? 'interactive-primary' : 'text-secondary hover:bg-hover'
                  }`}
                  aria-pressed={folder() === name}
                  onClick={() => setFolder(name)}
                >
                  <Folder size={16} class="shrink-0" />
                  <span class="flex-1 truncate">{name}</span>
                  <span class="text-xs opacity-70">{countIn(name)}</span>
                </button>
                <Show when={folder() === name}>
                  <button
                    type="button"
                    class="rounded p-1 text-tertiary hover:bg-hover"
                    aria-label={t('vault.renameFolder')}
                    title={t('vault.renameFolder')}
                    onClick={() => {
                      setPromptError(null);
                      setPrompt({ kind: 'rename-folder', folder: name });
                    }}
                  >
                    <Pencil size={13} />
                  </button>
                  <button
                    type="button"
                    class="rounded p-1 text-tertiary hover:bg-hover"
                    aria-label={t('vault.deleteFolder')}
                    title={t('vault.deleteFolder')}
                    onClick={() => void deleteFolder(name)}
                  >
                    <Trash2 size={13} />
                  </button>
                </Show>
              </div>
            )}
          </For>
        </nav>

        {/* File list */}
        <div class="relative flex min-w-0 flex-1 flex-col">
          <ContextMenu>
            <ContextMenu.Trigger class="min-h-0 flex-1 overflow-auto">
              <table class="w-full table-fixed text-sm" data-testid="vault-file-table">
                <colgroup>
                  <col class="w-10" />
                  <col />
                  <col class="w-36" />
                  <col class="w-24" />
                  <col class="w-44" />
                </colgroup>
                <thead class="sticky top-0 bg-secondary">
                  <tr class="border-b border-primary">
                    <th class="px-3 py-2">
                      <input
                        type="checkbox"
                        class="h-4 w-4 align-middle"
                        aria-label={t('vault.selectAll')}
                        checked={allSelected()}
                        ref={(el) => createEffect(() => (el.indeterminate = someSelected()))}
                        onChange={() => (allSelected() ? clearSelection() : selectAll())}
                        disabled={files().length === 0}
                        data-testid="vault-select-all"
                      />
                    </th>
                    <SortHeader key="name" label={t('vault.colName')} />
                    <SortHeader key="folder" label={t('vault.colFolder')} />
                    <SortHeader key="size" label={t('vault.colSize')} />
                    <SortHeader key="added" label={t('vault.colAdded')} />
                  </tr>
                </thead>
                <tbody>
                  <For each={files()}>
                    {(file) => {
                      const Icon = iconFor(file);
                      return (
                        <tr
                          class={`cursor-default select-none border-b border-primary ${
                            isSelected(file.id) ? 'bg-active' : 'hover:bg-hover'
                          }`}
                          tabIndex={0}
                          aria-selected={isSelected(file.id)}
                          data-testid="vault-file-row"
                          onClick={(e) => onRowClick(e, file.id)}
                          onDblClick={() => void openFile(file)}
                          onContextMenu={() => {
                            if (!isSelected(file.id)) selectOnly(file.id);
                          }}
                        >
                          <td class="px-3 py-2" onClick={(e) => e.stopPropagation()}>
                            <input
                              type="checkbox"
                              class="h-4 w-4 align-middle"
                              aria-label={t('vault.selectFile', { name: file.name })}
                              checked={isSelected(file.id)}
                              onChange={() => toggle(file.id)}
                              data-testid="vault-file-checkbox"
                            />
                          </td>
                          <td
                            class="truncate px-3 py-2 text-primary"
                            title={file.name}
                            data-testid="vault-file-name"
                          >
                            <Icon size={15} class="mr-2 inline text-tertiary" aria-hidden="true" />
                            {file.name}
                          </td>
                          <td class="truncate px-3 py-2 text-secondary">{file.folder ?? ''}</td>
                          <td class="px-3 py-2 text-secondary">{formatVaultSize(file.size)}</td>
                          <td class="px-3 py-2 text-secondary" title={file.added_at}>
                            {formatVaultAdded(file.added_at)}
                          </td>
                        </tr>
                      );
                    }}
                  </For>
                </tbody>
              </table>
              <Show when={files().length === 0}>
                <div class="p-10 text-center text-sm text-secondary">
                  {vaultListing().files.length === 0 ? t('vault.emptyVault') : t('vault.noMatches')}
                </div>
              </Show>
            </ContextMenu.Trigger>
            <Show when={selectedFiles().length > 1}>
              <ContextMenu.Portal>
                <ContextMenu.Content
                  class="z-[55] min-w-48 rounded-md border border-primary bg-primary p-1"
                  style={{ 'box-shadow': 'var(--shadow-lg)' }}
                  data-testid="vault-context-menu"
                >
                  <ContextMenu.Item class={menuItem} onSelect={() => void saveSelected()}>
                    <Download size={15} />
                    {t('vault.saveCopies')}
                  </ContextMenu.Item>
                  <ContextMenu.Item class={menuItem} onSelect={moveSelected}>
                    <FolderInput size={15} />
                    {t('vault.moveToFolder')}
                  </ContextMenu.Item>
                  <ContextMenu.Item class={menuItem} onSelect={() => void deleteSelected()}>
                    <Trash2 size={15} />
                    {t('vault.delete')}
                  </ContextMenu.Item>
                </ContextMenu.Content>
              </ContextMenu.Portal>
            </Show>
            <Show when={selected()}>
              {(file) => (
                <ContextMenu.Portal>
                  <ContextMenu.Content
                    class="z-[55] min-w-48 rounded-md border border-primary bg-primary p-1"
                    style={{ 'box-shadow': 'var(--shadow-lg)' }}
                    data-testid="vault-context-menu"
                  >
                    <Show when={vaultPreviewKind(file())}>
                      <ContextMenu.Item class={menuItem} onSelect={() => setPreviewing(file())}>
                        <Eye size={15} />
                        {t('vault.preview')}
                      </ContextMenu.Item>
                    </Show>
                    <ContextMenu.Item class={menuItem} onSelect={() => void openInApp(file())}>
                      <ExternalLink size={15} />
                      {t('vault.openInApp')}
                    </ContextMenu.Item>
                    <ContextMenu.Item class={menuItem} onSelect={() => void saveCopy(file())}>
                      <Download size={15} />
                      {t('vault.saveCopy')}
                    </ContextMenu.Item>
                    <ContextMenu.Separator class="my-1 border-t border-primary" />
                    <ContextMenu.Item
                      class={menuItem}
                      onSelect={() => {
                        setPromptError(null);
                        setPrompt({ kind: 'rename-file', file: file() });
                      }}
                    >
                      <Pencil size={15} />
                      {t('vault.rename')}
                    </ContextMenu.Item>
                    <ContextMenu.Item class={menuItem} onSelect={() => setMoving([file().id])}>
                      <FolderInput size={15} />
                      {t('vault.moveToFolder')}
                    </ContextMenu.Item>
                    <ContextMenu.Item class={menuItem} onSelect={() => void deleteSelected()}>
                      <Trash2 size={15} />
                      {t('vault.delete')}
                    </ContextMenu.Item>
                  </ContextMenu.Content>
                </ContextMenu.Portal>
              )}
            </Show>
          </ContextMenu>

          <Show when={dragging()}>
            <div
              class="pointer-events-none absolute inset-2 flex items-center justify-center rounded-lg border-2 border-dashed border-primary bg-primary text-sm text-secondary"
              style={{ opacity: 0.92 }}
              aria-hidden="true"
            >
              {folder() ? t('vault.dropHereFolder', { folder: folder()! }) : t('vault.dropHere')}
            </div>
          </Show>

          {/* Status bar */}
          <div class="flex items-center gap-3 border-t border-primary bg-primary px-4 py-2 text-xs text-secondary">
            <span>{t('vault.fileCount', { count: files().length })}</span>
            <Show when={selectedFiles().length > 0}>
              <span class="flex items-center gap-1" data-testid="vault-selection-bar">
                <span class="font-medium text-primary">
                  {t('vault.selectedCount', { count: selectedFiles().length })}
                </span>
                <Show when={selected()}>
                  {(file) => (
                    <button
                      type="button"
                      class="rounded px-2 py-0.5 hover:bg-hover"
                      onClick={() => void openFile(file())}
                    >
                      {vaultPreviewKind(file()) ? t('vault.preview') : t('vault.openInApp')}
                    </button>
                  )}
                </Show>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 rounded px-2 py-0.5 hover:bg-hover"
                  onClick={() => void saveSelected()}
                  disabled={busy()}
                  data-testid="vault-save-selected"
                >
                  <Download size={13} />
                  {selectedFiles().length === 1 ? t('vault.saveCopy') : t('vault.saveCopies')}
                </button>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 rounded px-2 py-0.5 hover:bg-hover"
                  onClick={moveSelected}
                  disabled={busy()}
                  data-testid="vault-move-selected"
                >
                  <FolderInput size={13} />
                  {t('vault.moveToFolder')}
                </button>
                <button
                  type="button"
                  class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-error hover:bg-hover"
                  onClick={() => void deleteSelected()}
                  disabled={busy()}
                  data-testid="vault-delete-selected"
                >
                  <Trash2 size={13} />
                  {t('vault.delete')}
                </button>
                <button
                  type="button"
                  class="rounded p-1 hover:bg-hover"
                  aria-label={t('vault.clearSelection')}
                  title={t('vault.clearSelection')}
                  onClick={clearSelection}
                >
                  <X size={13} />
                </button>
              </span>
            </Show>
            <span class="flex-1" />
            <Show when={status()}>
              <span data-testid="vault-status">{status()}</span>
            </Show>
          </div>
          <Show when={error()}>
            <div
              role="alert"
              class="flex items-start gap-2 border-t border-primary bg-error px-4 py-2"
            >
              <p class="flex-1 whitespace-pre-line text-sm text-error">{error()}</p>
              <button
                type="button"
                class="text-sm text-error hover:opacity-75"
                aria-label={t('common.close')}
                onClick={() => setError(null)}
              >
                ×
              </button>
            </div>
          </Show>
        </div>
      </div>

      <VaultTextPrompt
        open={prompt() !== null}
        title={
          prompt()?.kind === 'rename-file'
            ? t('vault.rename')
            : prompt()?.kind === 'new-folder'
              ? t('vault.newFolder')
              : t('vault.renameFolder')
        }
        label={t('vault.nameLabel')}
        initial={(() => {
          const p = prompt();
          if (!p) return '';
          if (p.kind === 'rename-file') return p.file.name;
          if (p.kind === 'rename-folder') return p.folder;
          return '';
        })()}
        confirmLabel={prompt()?.kind === 'new-folder' ? t('vault.create') : t('vault.rename')}
        error={promptError()}
        onSubmit={(v) => void submitPrompt(v)}
        onClose={() => setPrompt(null)}
      />
      <VaultMoveDialog
        open={moving() !== null}
        folders={vaultListing().folders}
        current={movingFrom()}
        error={error()}
        onSubmit={(f) => void submitMove(f)}
        onClose={() => setMoving(null)}
      />
      <Show when={previewing()}>
        {(file) => (
          <VaultPreview
            file={file()}
            onClose={() => setPreviewing(null)}
            onOpenInApp={(f) => {
              setPreviewing(null);
              void openInApp(f);
            }}
          />
        )}
      </Show>
      <VaultSettings open={settingsOpen()} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
