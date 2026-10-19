import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';
import DeadManSettings, { splitDays, splitSecs, type DeadManRules } from './DeadManSettings';

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(() => Promise.resolve(true)),
}));

vi.mock('../../lib/dialog', () => ({ confirm: mocks.confirm }));

const OFF: DeadManRules = { days: 0, back_secs: 0 };

function setup(rules: DeadManRules = OFF, onSave = vi.fn(() => Promise.resolve())) {
  renderWithI18n(() => <DeadManSettings id="dm" title="Switch" rules={rules} onSave={onSave} />);
  const el = (suffix: string) => screen.getByTestId(`dm-${suffix}`) as HTMLInputElement;
  return { onSave, el };
}

function choose(el: HTMLInputElement, value: string) {
  fireEvent.change(el, { target: { value } });
}

describe('DeadManSettings', () => {
  beforeEach(() => vi.clearAllMocks());

  it('splits stored values into the largest even unit', () => {
    expect(splitDays(730)).toEqual({ amount: 2, unit: 'years' });
    expect(splitDays(90)).toEqual({ amount: 3, unit: 'months' });
    expect(splitDays(10)).toEqual({ amount: 10, unit: 'days' });
    expect(splitSecs(172800)).toEqual({ amount: 2, unit: 'days' });
    expect(splitSecs(7200)).toEqual({ amount: 2, unit: 'hours' });
    expect(splitSecs(1800)).toEqual({ amount: 30, unit: 'minutes' });
  });

  it('shows the saved rules', () => {
    const { el } = setup({ days: 60, back_secs: 7200 });
    expect(el('period-on').checked).toBe(true);
    expect(el('period-amount').value).toBe('2');
    expect(el('period-unit').value).toBe('months');
    expect(el('clock-on').checked).toBe(true);
    expect(el('clock-amount').value).toBe('2');
    expect(el('clock-unit').value).toBe('hours');
  });

  it('saves "not unlocked for 1 year" after confirming', async () => {
    const { el, onSave } = setup();
    expect(el('save').disabled).toBe(true);
    fireEvent.click(el('period-on'));
    choose(el('period-amount'), '1');
    choose(el('period-unit'), 'years');
    fireEvent.click(el('save'));
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledWith({ days: 365, back_secs: 0 }));
    expect(mocks.confirm).toHaveBeenCalledTimes(1);
  });

  it('saves "clock set back more than 30 minutes"', async () => {
    const { el, onSave } = setup();
    fireEvent.click(el('clock-on'));
    choose(el('clock-amount'), '30');
    choose(el('clock-unit'), 'minutes');
    fireEvent.click(el('save'));
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledWith({ days: 0, back_secs: 1800 }));
  });

  it('raises a clock threshold below 10 minutes to 10 minutes', async () => {
    const { el, onSave } = setup();
    fireEvent.click(el('clock-on'));
    choose(el('clock-amount'), '2');
    choose(el('clock-unit'), 'minutes');
    fireEvent.click(el('save'));
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledWith({ days: 0, back_secs: 600 }));
  });

  it('turns everything off without asking', async () => {
    const { el, onSave } = setup({ days: 30, back_secs: 3600 });
    fireEvent.click(el('period-on'));
    fireEvent.click(el('clock-on'));
    fireEvent.click(el('save'));
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledWith(OFF));
    expect(mocks.confirm).not.toHaveBeenCalled();
  });

  it('does not save when the confirmation is declined', async () => {
    mocks.confirm.mockResolvedValueOnce(false);
    const { el, onSave } = setup();
    fireEvent.click(el('period-on'));
    fireEvent.click(el('save'));
    await vi.waitFor(() => expect(mocks.confirm).toHaveBeenCalledTimes(1));
    expect(onSave).not.toHaveBeenCalled();
  });

  it('shows an error from saving', async () => {
    const { el } = setup(
      OFF,
      vi.fn(() => Promise.reject(new Error('boom'))),
    );
    fireEvent.click(el('period-on'));
    fireEvent.click(el('save'));
    await screen.findByRole('alert');
  });
});
