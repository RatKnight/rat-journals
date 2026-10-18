import { createSignal, For, Show, onMount } from 'solid-js';
import * as tauri from '../../../lib/tauri';
import { confirm } from '../../../lib/dialog';
import { mapTauriError } from '../../../lib/errors';
import { journals, activeJournalId } from '../../../state/journals';
import { useI18n } from '../../../i18n';

/** Panic password settings for the open journal (see core::duress). */
export default function DuressSection() {
  const t = useI18n();
  const [status, setStatus] = createSignal<tauri.DuressStatus | null>(null);
  const [password, setPassword] = createSignal('');
  const [repeat, setRepeat] = createSignal('');
  const [decoyId, setDecoyId] = createSignal('');
  const [eraseVault, setEraseVault] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  const others = () => journals().filter((j) => j.id !== activeJournalId());

  const load = async () => {
    try {
      const s = await tauri.duressStatus();
      setStatus(s);
      setEraseVault(s.erase_vault);
      setDecoyId(s.decoy_id ?? others()[0]?.id ?? '');
    } catch (err) {
      setError(mapTauriError(err, t));
    }
  };
  onMount(() => void load());

  const save = async () => {
    setError(null);
    if (password() !== repeat()) {
      setError(t('duress.mismatch'));
      return;
    }
    if (!(await confirm(t('duress.confirm')))) return;
    setBusy(true);
    try {
      await tauri.setDuress(password(), decoyId(), eraseVault());
      setPassword('');
      setRepeat('');
      await load();
    } catch (err) {
      setError(mapTauriError(err, t));
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setError(null);
    setBusy(true);
    try {
      await tauri.clearDuress();
      await load();
    } catch (err) {
      setError(mapTauriError(err, t));
    } finally {
      setBusy(false);
    }
  };

  const inputClass =
    'w-full px-2 py-1 text-sm border border-primary rounded-md bg-primary text-primary focus:outline-none focus:ring-2 focus:ring-blue-500';

  return (
    <div class="flex flex-col gap-2" data-testid="duress-section">
      <h4 class="text-sm font-medium text-primary">{t('duress.title')}</h4>
      <p class="text-xs text-tertiary">{t('duress.explain')}</p>
      <Show when={status()?.enabled}>
        <div class="flex items-center gap-2">
          <span class="text-sm text-secondary">{t('duress.isOn')}</span>
          <button
            type="button"
            class="text-sm text-error hover:opacity-75 disabled:opacity-50"
            disabled={busy()}
            onClick={() => void remove()}
            data-testid="duress-remove"
          >
            {t('duress.remove')}
          </button>
        </div>
      </Show>
      <Show
        when={others().length > 0}
        fallback={<p class="text-xs text-secondary">{t('duress.needDecoy')}</p>}
      >
        <input
          type="password"
          class={inputClass}
          placeholder={t('duress.password')}
          value={password()}
          onInput={(e) => setPassword(e.currentTarget.value)}
          data-testid="duress-password"
        />
        <input
          type="password"
          class={inputClass}
          placeholder={t('duress.repeat')}
          value={repeat()}
          onInput={(e) => setRepeat(e.currentTarget.value)}
          data-testid="duress-repeat"
        />
        <label class="flex items-center gap-2 text-sm text-secondary">
          {t('duress.decoy')}
          <select
            class="px-2 py-1 text-sm border border-primary rounded-md bg-primary text-primary"
            value={decoyId()}
            onChange={(e) => setDecoyId(e.currentTarget.value)}
            data-testid="duress-decoy"
          >
            <For each={others()}>{(j) => <option value={j.id}>{j.name}</option>}</For>
          </select>
        </label>
        <label class="flex items-center gap-2 text-sm text-secondary">
          <input
            type="checkbox"
            checked={eraseVault()}
            onChange={(e) => setEraseVault(e.currentTarget.checked)}
            data-testid="duress-erase-vault"
          />
          {t('duress.eraseVault')}
        </label>
        <div>
          <button
            type="button"
            class="rounded-md border border-primary px-3 py-1 text-sm text-secondary hover:bg-hover disabled:opacity-50"
            disabled={busy() || !password() || !decoyId()}
            onClick={() => void save()}
            data-testid="duress-save"
          >
            {status()?.enabled ? t('duress.change') : t('duress.set')}
          </button>
        </div>
      </Show>
      <Show when={error()}>
        <p role="alert" class="text-xs text-error">
          {error()}
        </p>
      </Show>
    </div>
  );
}
