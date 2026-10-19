import { createSignal, Show } from 'solid-js';
import * as tauri from '../../lib/tauri';
import { confirm } from '../../lib/dialog';
import { mapTauriError } from '../../lib/errors';
import { useI18n } from '../../i18n';

type Phase = 'idle' | 'checking' | 'current' | 'available' | 'installing';

/** "Check for updates": contacts GitHub only when clicked. */
export default function UpdateCheck() {
  const t = useI18n();
  const [phase, setPhase] = createSignal<Phase>('idle');
  const [update, setUpdate] = createSignal<tauri.UpdateInfo | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  const check = async () => {
    setError(null);
    setPhase('checking');
    try {
      const found = await tauri.checkForUpdate();
      setUpdate(found);
      setPhase(found ? 'available' : 'current');
    } catch (err) {
      setError(mapTauriError(err, t));
      setPhase('idle');
    }
  };

  const install = async () => {
    const found = update();
    if (!found || !(await confirm(t('update.confirm', { version: found.version })))) return;
    setError(null);
    setPhase('installing');
    try {
      await tauri.installUpdate();
    } catch (err) {
      setError(mapTauriError(err, t));
      setPhase('available');
    }
  };

  const buttonClass =
    'rounded-md border border-primary px-3 py-1 text-sm text-secondary hover:bg-hover disabled:opacity-50';

  return (
    <div class="flex flex-col items-center gap-2 mb-6" data-testid="update-check">
      <Show when={phase() !== 'available' && phase() !== 'installing'}>
        <button
          type="button"
          class={buttonClass}
          disabled={phase() === 'checking'}
          onClick={() => void check()}
          data-testid="update-check-button"
        >
          {phase() === 'checking' ? t('update.checking') : t('update.check')}
        </button>
      </Show>
      <Show when={phase() === 'current'}>
        <p class="text-sm text-secondary" data-testid="update-current">
          {t('update.current')}
        </p>
      </Show>
      <Show when={(phase() === 'available' || phase() === 'installing') && update()}>
        {(found) => (
          <>
            <p class="text-sm text-primary" data-testid="update-available">
              {t('update.available', { version: found().version })}
            </p>
            <Show when={found().notes}>
              <p class="max-h-32 overflow-y-auto whitespace-pre-line text-xs text-secondary">
                {found().notes}
              </p>
            </Show>
            <button
              type="button"
              class={buttonClass}
              disabled={phase() === 'installing'}
              onClick={() => void install()}
              data-testid="update-install"
            >
              {phase() === 'installing' ? t('update.installing') : t('update.install')}
            </button>
          </>
        )}
      </Show>
      <Show when={error()}>
        <p role="alert" class="text-xs text-error text-center">
          {error()}
        </p>
      </Show>
      <p class="text-xs text-tertiary text-center">{t('update.privacy')}</p>
    </div>
  );
}
