import { createSignal, createEffect, Show } from 'solid-js';
import { confirm } from '../../lib/dialog';
import { mapTauriError } from '../../lib/errors';
import { useI18n } from '../../i18n';

/** A dead man's switch's rules; 0 turns a rule off (see core::deadman). */
export interface DeadManRules {
  days: number;
  back_secs: number;
}

export const MAX_DEAD_MAN_DAYS = 3650;
export const MIN_BACK_SECS = 600;
export const MAX_BACK_SECS = 365 * 86400;

type PeriodUnit = 'days' | 'months' | 'years';
type ClockUnit = 'minutes' | 'hours' | 'days';

const PERIOD_DAYS: Record<PeriodUnit, number> = { days: 1, months: 30, years: 365 };
const CLOCK_SECS: Record<ClockUnit, number> = { minutes: 60, hours: 3600, days: 86400 };

/** Shows a stored period in the largest unit that divides it evenly. */
export function splitDays(days: number): { amount: number; unit: PeriodUnit } {
  if (days > 0 && days % 365 === 0) return { amount: days / 365, unit: 'years' };
  if (days > 0 && days % 30 === 0) return { amount: days / 30, unit: 'months' };
  return { amount: days, unit: 'days' };
}

export function splitSecs(secs: number): { amount: number; unit: ClockUnit } {
  if (secs > 0 && secs % 86400 === 0) return { amount: secs / 86400, unit: 'days' };
  if (secs > 0 && secs % 3600 === 0) return { amount: secs / 3600, unit: 'hours' };
  return { amount: Math.round(secs / 60), unit: 'minutes' };
}

const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

interface DeadManSettingsProps {
  id: string;
  /** Heading naming what gets erased. */
  title: string;
  rules: DeadManRules;
  onSave: (rules: DeadManRules) => Promise<void>;
}

/** Dead man's switch rules: "not unlocked for" and "clock set back". */
export default function DeadManSettings(props: DeadManSettingsProps) {
  const t = useI18n();
  const [periodOn, setPeriodOn] = createSignal(false);
  const [periodAmount, setPeriodAmount] = createSignal(30);
  const [periodUnit, setPeriodUnit] = createSignal<PeriodUnit>('days');
  const [clockOn, setClockOn] = createSignal(false);
  const [clockAmount, setClockAmount] = createSignal(1);
  const [clockUnit, setClockUnit] = createSignal<ClockUnit>('hours');
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  const load = (rules: DeadManRules) => {
    setPeriodOn(rules.days > 0);
    if (rules.days > 0) {
      const p = splitDays(rules.days);
      setPeriodAmount(p.amount);
      setPeriodUnit(p.unit);
    }
    setClockOn(rules.back_secs > 0);
    if (rules.back_secs > 0) {
      const c = splitSecs(rules.back_secs);
      setClockAmount(c.amount);
      setClockUnit(c.unit);
    }
  };
  createEffect(() => load(props.rules));

  const wanted = (): DeadManRules => ({
    days: periodOn()
      ? clamp(Math.round(periodAmount() * PERIOD_DAYS[periodUnit()]), 1, MAX_DEAD_MAN_DAYS)
      : 0,
    back_secs: clockOn()
      ? clamp(Math.round(clockAmount() * CLOCK_SECS[clockUnit()]), MIN_BACK_SECS, MAX_BACK_SECS)
      : 0,
  });
  const changed = () =>
    wanted().days !== props.rules.days || wanted().back_secs !== props.rules.back_secs;

  const save = async () => {
    setError(null);
    const rules = wanted();
    if ((rules.days > 0 || rules.back_secs > 0) && !(await confirm(t('deadMan.confirmArm')))) {
      return;
    }
    setBusy(true);
    try {
      await props.onSave(rules);
    } catch (err) {
      setError(mapTauriError(err, t));
    } finally {
      setBusy(false);
    }
  };

  const numberClass =
    'w-20 px-2 py-1 text-sm border border-primary rounded-md bg-primary text-primary focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50';
  const selectClass =
    'px-2 py-1 text-sm border border-primary rounded-md bg-primary text-primary disabled:opacity-50';
  const amount = (value: string) => Math.max(1, Math.round(Number(value)) || 1);

  return (
    <div class="flex flex-col gap-2" data-testid={props.id}>
      <h4 class="text-sm font-medium text-primary">{props.title}</h4>
      <p class="text-xs text-tertiary">{t('deadMan.explain')}</p>

      <div class="flex flex-wrap items-center gap-2">
        <label class="flex items-center gap-2 text-sm text-secondary">
          <input
            type="checkbox"
            checked={periodOn()}
            onChange={(e) => setPeriodOn(e.currentTarget.checked)}
            data-testid={`${props.id}-period-on`}
          />
          {t('deadMan.periodLabel')}
        </label>
        <input
          type="number"
          min="1"
          class={numberClass}
          disabled={!periodOn()}
          value={periodAmount()}
          onChange={(e) => setPeriodAmount(amount(e.currentTarget.value))}
          data-testid={`${props.id}-period-amount`}
        />
        <select
          class={selectClass}
          disabled={!periodOn()}
          value={periodUnit()}
          onChange={(e) => setPeriodUnit(e.currentTarget.value as PeriodUnit)}
          data-testid={`${props.id}-period-unit`}
        >
          <option value="days">{t('deadMan.unitDays')}</option>
          <option value="months">{t('deadMan.unitMonths')}</option>
          <option value="years">{t('deadMan.unitYears')}</option>
        </select>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <label class="flex items-center gap-2 text-sm text-secondary">
          <input
            type="checkbox"
            checked={clockOn()}
            onChange={(e) => setClockOn(e.currentTarget.checked)}
            data-testid={`${props.id}-clock-on`}
          />
          {t('deadMan.clockLabel')}
        </label>
        <input
          type="number"
          min="1"
          class={numberClass}
          disabled={!clockOn()}
          value={clockAmount()}
          onChange={(e) => setClockAmount(amount(e.currentTarget.value))}
          data-testid={`${props.id}-clock-amount`}
        />
        <select
          class={selectClass}
          disabled={!clockOn()}
          value={clockUnit()}
          onChange={(e) => setClockUnit(e.currentTarget.value as ClockUnit)}
          data-testid={`${props.id}-clock-unit`}
        >
          <option value="minutes">{t('deadMan.unitMinutes')}</option>
          <option value="hours">{t('deadMan.unitHours')}</option>
          <option value="days">{t('deadMan.unitDays')}</option>
        </select>
      </div>
      <p class="text-xs text-tertiary">{t('deadMan.clockHint')}</p>

      <div>
        <button
          type="button"
          class="rounded-md border border-primary px-3 py-1 text-sm text-secondary hover:bg-hover disabled:opacity-50"
          disabled={busy() || !changed()}
          onClick={() => void save()}
          data-testid={`${props.id}-save`}
        >
          {t('deadMan.save')}
        </button>
      </div>
      <Show when={error()}>
        <p role="alert" class="text-xs text-error">
          {error()}
        </p>
      </Show>
    </div>
  );
}
