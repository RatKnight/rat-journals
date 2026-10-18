import { createMemo, createSignal, For, Show } from 'solid-js';
import { ContextMenu } from '@kobalte/core/context-menu';
import {
  ArrowDown,
  ArrowUp,
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
  const [selectedId, setSelectedId] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [dragging, setDragging] = createSignal(false);
  const [prompt, setPrompt] = createSignal<Prompt | null>(null);
  const [promptError, setPromptError] = createSignal<string | null>(null);
  const [moving, setMoving] = createSignal<VaultFile | null>(null);
  const [previewing, setPreviewing] = createSignal<VaultFile | null>(null);
  const [settingsOpen, setSettingsOpen] = createSignal(false);

  const files = createMemo(() =>
    sortVaultFiles(filterVaultFiles(vaultListing().files, folder(), query()), sortKey(), sortDir()),
  );
  const selected = () => vaultListing().files.find((f) => f.id === selectedId()) ?? null;
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

  const deleteFile = async (file: VaultFile) => {
    const ok = await confirm(t('vault.deleteConfirm', { name: file.name }), {
      title: t('vault.delete'),
      kind: 'warning',
    });
    if (!ok) return;
    if (await act(() => tauri.vaultDeleteFile(file.id))) setSelectedId(null);
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
    const file = moving();
    if (!file) return;
    if (await act(() => tauri.vaultMoveFile(file.id, target))) setMoving(null);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const file = selected();
    if (!file || (e.target as HTMLElement).closest('input, textarea, select')) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      void openFile(file);
    } else if (e.key === 'Delete') {
      e.preventDefault();
      void deleteFile(file);
    } else if (e.key === 'F2') {
      e.preventDefault();
      setPrompt({ kind: 'rename-file', file });
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
      class="fixed inset-0 z-40 flex flex-col bg-secondary"
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
                  <col />
                  <col class="w-36" />
                  <col class="w-24" />
                  <col class="w-44" />
                </colgroup>
                <thead class="sticky top-0 bg-secondary">
                  <tr class="border-b border-primary">
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
                          class={`cursor-default border-b border-primary ${
                            selectedId() === file.id ? 'bg-active' : 'hover:bg-hover'
                          }`}
                          tabIndex={0}
                          aria-selected={selectedId() === file.id}
                          data-testid="vault-file-row"
                          onClick={() => setSelectedId(file.id)}
                          onFocus={() => setSelectedId(file.id)}
                          onDblClick={() => void openFile(file)}
                          onContextMenu={() => setSelectedId(file.id)}
                        >
                          <td class="truncate px-3 py-2 text-primary" title={file.name}>
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
                    <ContextMenu.Item class={menuItem} onSelect={() => setMoving(file())}>
                      <FolderInput size={15} />
                      {t('vault.moveToFolder')}
                    </ContextMenu.Item>
                    <ContextMenu.Item class={menuItem} onSelect={() => void deleteFile(file())}>
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
            <Show when={selected()}>
              {(file) => (
                <span class="flex items-center gap-1">
                  <button
                    type="button"
                    class="rounded px-2 py-0.5 hover:bg-hover"
                    onClick={() => void openFile(file())}
                  >
                    {vaultPreviewKind(file()) ? t('vault.preview') : t('vault.openInApp')}
                  </button>
                  <button
                    type="button"
                    class="rounded px-2 py-0.5 hover:bg-hover"
                    onClick={() => void saveCopy(file())}
                  >
                    {t('vault.saveCopy')}
                  </button>
                </span>
              )}
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
        current={moving()?.folder ?? null}
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
