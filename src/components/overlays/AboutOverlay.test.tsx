import { describe, it, expect } from 'vitest';
import { screen } from '@solidjs/testing-library';
import { renderWithI18n } from '../../test/i18n-test-utils';

import AboutOverlay from './AboutOverlay';

describe('AboutOverlay', () => {
  it('shows the app name and credits Mini Diarium as plain text', () => {
    renderWithI18n(() => <AboutOverlay isOpen={true} onClose={() => {}} />);

    expect(screen.getAllByText('Rat Journal').length).toBeGreaterThan(0);
    expect(screen.getByText('Based on Mini Diarium by Francisco J. Revoredo')).toBeInTheDocument();
    expect(screen.getByText('MIT License')).toBeInTheDocument();
  });

  it('has no links to external sites or support prompts', () => {
    renderWithI18n(() => <AboutOverlay isOpen={true} onClose={() => {}} />);

    expect(screen.queryByRole('button', { name: /github/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /documentation/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /support/i })).not.toBeInTheDocument();
  });
});
