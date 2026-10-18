import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';
import DeadManField from './DeadManField';

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(() => Promise.resolve(true)),
}));

vi.mock('../../lib/dialog', () => ({
  confirm: mocks.confirm,
}));

describe('DeadManField', () => {
  const defaultProps = {
    id: 'dm',
    label: 'Test Label',
    days: 0,
    onSave: vi.fn(() => Promise.resolve()),
  };

  beforeEach(() => {
    mocks.confirm.mockClear();
    defaultProps.onSave.mockClear();
  });

  it('arming (change input from 0 to 7) asks confirm and, when confirmed, calls onSave(7)', async () => {
    renderWithI18n(() => <DeadManField {...defaultProps} />);
    const input = screen.getByTestId('dm') as HTMLInputElement;

    mocks.confirm.mockResolvedValueOnce(true);
    fireEvent.change(input, { target: { value: '7' } });

    await vi.waitFor(() => expect(mocks.confirm).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(defaultProps.onSave).toHaveBeenCalledWith(7));
  });

  it('when confirm resolves false, onSave is not called and the input goes back to "0"', async () => {
    renderWithI18n(() => <DeadManField {...defaultProps} />);
    const input = screen.getByTestId('dm') as HTMLInputElement;

    mocks.confirm.mockResolvedValueOnce(false);
    fireEvent.change(input, { target: { value: '7' } });

    await vi.waitFor(() => expect(mocks.confirm).toHaveBeenCalledTimes(1));
    await vi.waitFor(() => expect(defaultProps.onSave).not.toHaveBeenCalled());
    await vi.waitFor(() => expect(input.value).toBe('0'));
  });

  it('disarming (days=5, change to 0) calls onSave(0) without calling confirm', async () => {
    renderWithI18n(() => <DeadManField {...defaultProps} days={5} />);
    const input = screen.getByTestId('dm') as HTMLInputElement;

    fireEvent.change(input, { target: { value: '0' } });

    await vi.waitFor(() => expect(mocks.confirm).not.toHaveBeenCalled());
    await vi.waitFor(() => expect(defaultProps.onSave).toHaveBeenCalledWith(0));
  });

  it('values above 3650 are clamped to 3650 before saving', async () => {
    renderWithI18n(() => <DeadManField {...defaultProps} />);
    const input = screen.getByTestId('dm') as HTMLInputElement;

    mocks.confirm.mockResolvedValueOnce(true);
    fireEvent.change(input, { target: { value: '4000' } });

    await vi.waitFor(() => expect(defaultProps.onSave).toHaveBeenCalledWith(3650));
  });

  it('when onSave rejects with an Error("boom"), an element with role alert appears and the input goes back to the previous value', async () => {
    renderWithI18n(() => <DeadManField {...defaultProps} days={5} />);
    const input = screen.getByTestId('dm') as HTMLInputElement;

    defaultProps.onSave.mockRejectedValueOnce(new Error('boom'));
    fireEvent.change(input, { target: { value: '7' } });

    await vi.waitFor(() => expect(defaultProps.onSave).toHaveBeenCalledWith(7));
    await vi.waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert).toBeInTheDocument();
    });
    await vi.waitFor(() => expect(input.value).toBe('5'));
  });
});
