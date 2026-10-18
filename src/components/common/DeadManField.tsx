import { createSignal, createEffect, Show } from 'solid-js';
import { confirm } from '../../lib/dialog';
import { mapTauriError } from '../../lib/errors';
import { useI18n } from '../../i18n';

export const MAX_DEAD_MAN_DAYS = 3650;

interface DeadManFieldProps {
  id: string;
  /** Label naming what gets erased (the journal or the vault). */
  label: string;
  /** Armed period in days, 0 = off. */
  days: number;
  /** Saves a new period (0 disarms). */
  onSave: (days: number) => Promise<void>;
}

/** Number field for a dead man's switch; arming asks for confirmation first. */
export default function DeadManField(props: DeadManFieldProps) {
  const t = useI18n();
  const [value, setValue] = createSignal(props.days);
  const [error, setError] = createSignal<string | null>(null);
  createEffect(() => setValue(props.days));

  const save = async (input: HTMLInputElement) => {
    setError(null);
    const n = Math.round(Number(input.value));
    const days = Number.isFinite(n) ? Math.min(MAX_DEAD_MAN_DAYS, Math.max(0, n)) : 0;
    input.value = String(days);
    if (days === value()) return;
    if (days > 0 && !(await confirm(t('deadMan.confirmArm', { days })))) {
      input.value = String(value());
      return;
    }
    try {
      await props.onSave(days);
      setValue(days);
    } catch (err) {
      input.value = String(value());
      setError(mapTauriError(err, t));
    }
  };

  return (
    <div class="flex flex-col gap-1">
      <div class="flex flex-wrap items-center gap-2">
        <label class="text-sm text-secondary" for={props.id}>
          {props.label}
        </label>
        <input
          id={props.id}
          type="number"
          min="0"
          max={MAX_DEAD_MAN_DAYS}
          step="1"
          value={value()}
          onChange={(e) => void save(e.currentTarget)}
          class="w-20 px-2 py-1 text-sm border border-primary rounded-md bg-primary text-primary focus:outline-none focus:ring-2 focus:ring-blue-500"
          data-testid={props.id}
        />
        <span class="text-xs text-tertiary">{t('deadMan.hint')}</span>
      </div>
      <Show when={error()}>
        <p role="alert" class="text-xs text-error">
          {error()}
        </p>
      </Show>
    </div>
  );
}
