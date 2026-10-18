import { describe, it, expect, vi, beforeEach } from 'vitest';
import { screen, fireEvent } from '@solidjs/testing-library';
import { renderWithI18n } from '../../../test/i18n-test-utils';
import DuressSection from './DuressSection';

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(() => Promise.resolve(true)),
  duressStatus: vi.fn(),
  setDuress: vi.fn(() => Promise.resolve()),
  clearDuress: vi.fn(() => Promise.resolve()),
}));

vi.mock('../../../lib/dialog', () => ({ confirm: mocks.confirm }));
vi.mock('../../../lib/tauri', async () => ({
  ...(await vi.importActual<typeof import('../../../lib/tauri')>('../../../lib/tauri')),
  duressStatus: mocks.duressStatus,
  setDuress: mocks.setDuress,
  clearDuress: mocks.clearDuress,
}));
vi.mock('../../../state/journals', () => ({
  journals: () => [
    { id: 'a', name: 'Main' },
    { id: 'b', name: 'Decoy' },
  ],
  activeJournalId: () => 'a',
}));

function fill(password: string, repeat: string) {
  fireEvent.input(screen.getByTestId('duress-password'), { target: { value: password } });
  fireEvent.input(screen.getByTestId('duress-repeat'), { target: { value: repeat } });
}

describe('DuressSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.duressStatus.mockResolvedValue({ enabled: false, decoy_id: null, erase_vault: false });
  });

  it('sets the panic password with the decoy after confirming', async () => {
    renderWithI18n(() => <DuressSection />);
    await vi.waitFor(() => expect(mocks.duressStatus).toHaveBeenCalled());
    fill('pw', 'pw');
    fireEvent.click(screen.getByTestId('duress-save'));
    await vi.waitFor(() => expect(mocks.setDuress).toHaveBeenCalledWith('pw', 'b', false));
    expect(mocks.confirm).toHaveBeenCalledTimes(1);
  });

  it('passes the erase-vault choice', async () => {
    renderWithI18n(() => <DuressSection />);
    await vi.waitFor(() => expect(mocks.duressStatus).toHaveBeenCalled());
    fill('pw', 'pw');
    fireEvent.click(screen.getByTestId('duress-erase-vault'));
    fireEvent.click(screen.getByTestId('duress-save'));
    await vi.waitFor(() => expect(mocks.setDuress).toHaveBeenCalledWith('pw', 'b', true));
  });

  it('refuses mismatched passwords', async () => {
    renderWithI18n(() => <DuressSection />);
    await vi.waitFor(() => expect(mocks.duressStatus).toHaveBeenCalled());
    fill('pw', 'other');
    fireEvent.click(screen.getByTestId('duress-save'));
    await screen.findByRole('alert');
    expect(mocks.setDuress).not.toHaveBeenCalled();
  });

  it('removes an existing panic password', async () => {
    mocks.duressStatus.mockResolvedValue({ enabled: true, decoy_id: 'b', erase_vault: false });
    renderWithI18n(() => <DuressSection />);
    fireEvent.click(await screen.findByTestId('duress-remove'));
    await vi.waitFor(() => expect(mocks.clearDuress).toHaveBeenCalledTimes(1));
  });
});
